//! Pure game logic: button input + wall-clock time in, framebuffer out.

pub mod data;
pub mod decor;
pub(crate) mod evolve;
pub mod family;
pub mod home;
pub mod hunt;
pub mod mini;
pub mod motion;
pub mod pet;
pub mod progress;
pub mod save;
pub mod screen;
pub mod shop;
pub mod social;
pub mod sprite;
pub mod world;
mod ui;

use data::{Data, PART_BODY, PART_DIRT, PART_EYES, PART_MOUTH};
use motion::{Draw, Look, Node, KEEP};
use pet::{status, Pet};
pub use ui::Level;
use ui::Scene;
pub use screen::{HEIGHT, WIDTH};
use screen::Screen;
use sprite::Sprite;

/// Sprite with the 16 pet body colour palettes; set n = colour n (game-spec §8).
const COLOR_PALETTES: u16 = 206;
/// HOME anchors (character's bottom centre) for field, pet and cell zoom.
/// World point the camera centres on in the pet layer, so the field pet lands where the
/// close-up pet stands.
const PET_CAM: (i32, i32) = (64, 78);
const ANCHOR: [(i32, i32); 4] = [(64, 96), (64, 100), (64, 100), (64, 96)];

/// Frame step, 20 fps like the original (game-spec §2). Pet simulation runs on wall-clock seconds.
pub const TICK_MS: u32 = 50;

/// Input bitmask, same bits as the firmware (game-spec §2). Dial bits are one-frame pulses.
pub mod input {
    pub const A: u8 = 0x01;
    pub const B: u8 = 0x02;
    pub const C: u8 = 0x04;
    pub const PUSH: u8 = 0x08;
    pub const DOWN: u8 = 0x40;
    pub const UP: u8 = 0x80;
}

pub struct Game {
    pub data: Data,
    pub pet: Pet,
    screen: Screen,
    /// Frames since start (20 fps), drives animation.
    pub frame_count: u32,
    held: u8,
    /// HOME behaviour: state (07-animation §2.5), whether it was forced by the pet's condition,
    /// and the looping motion node playing it.
    home_state: Option<(u8, bool)>,
    home: Node,
    home_seq: usize,
    home_level: Option<Level>,
    rng: u32,
    draws: Vec<Draw>,
    scene: Scene,
    pub level: Level,
    /// String table index (`data::LANGS`), 1 = English.
    pub lang: u8,
    /// Sounds started since the last `take_sounds`: (melody id, volume 0–100).
    sounds: Vec<(u16, u8)>,
    /// Minute of the last calendar-event check.
    event_minute: u64,
    /// Frames the dial button has been held (long press opens the lab menu).
    push_frames: u16,
    /// Field camera: focused object (0 = pet, then placed toys) and current centre in world space.
    focus: usize,
    cam: (i32, i32),
    /// Outside toys on the field: (slot, looping show animation), rebuilt when the layout changes.
    toy_nodes: Vec<(usize, Node)>,
    toy_key: Option<(u8, bool, [u16; 6])>,
    /// Characters standing in for script roles 1–3 in the current scene (ChrID), see `motion::ROLE`.
    pub(crate) roles: [Option<u16>; 4],
    /// Planet ranks shown for planet roles 0 and 1 (rank-up: old, new).
    pub(crate) planet_ranks: Option<[u8; 2]>,
    /// Outfit-set visitor on the field (chr, looping motion 0x2f1) and the day it was rolled.
    visitor: Option<(u16, Node)>,
    visitor_day: Option<u64>,
    /// A request for the platform's connection panel ("playdate", "receive", "gift:<code>").
    connect: Option<String>,
    /// HOME zoom value z (scale 4^(z/16)); eases towards the current level's stop each frame.
    zoom: f32,
}

impl Game {
    /// Start a new game with an egg in the device's home area.
    pub fn new(data: Data, seed: u32) -> Self {
        let mut pet = Pet::new(data.character(1000).expect("egg 1000 in data"), seed);
        pet.field = data::attr_to_field(data.rom_type).unwrap_or(0);
        // Starter food (`inv_give_starter` 0x1001e558, table 0x10093d20).
        for id in [5068, 5076, 5058] {
            pet.give(id, 3);
        }
        Game::with_pet(data, pet)
    }

    fn with_pet(data: Data, pet: Pet) -> Self {
        let rng = pet.rng_seed() | 1;
        let mut g = Game {
            data,
            pet,
            screen: Screen::default(),
            frame_count: 0,
            held: 0,
            home_state: None,
            home: Node::new(0),
            home_seq: 0,
            home_level: None,
            rng,
            draws: Vec::new(),
            scene: Scene::Home,
            level: Level::Pet,
            lang: 1,
            sounds: Vec::new(),
            event_minute: 0,
            push_frames: 0,
            focus: 0,
            cam: ui::PET_FOCUS,
            toy_nodes: Vec::new(),
            toy_key: None,
            zoom: ui::zoom_stop(Level::Pet),
            roles: [None; 4],
            planet_ranks: None,
            connect: None,
            visitor: None,
            visitor_day: None,
        };
        g.render();
        g
    }

    /// One 20 fps frame: `now` = local wall-clock seconds, `held` = `input` bits held this frame.
    pub fn frame(&mut self, now: u64, held: u8) {
        // Buttons act on the press edge; dial bits are already one-frame pulses.
        let dial = input::UP | input::DOWN;
        let pressed = (held & !self.held & !dial) | (held & dial);
        self.held = held;
        self.frame_count = self.frame_count.wrapping_add(1);
        self.pet.advance_to(now);
        if self.pet.weather_day != now / 86_400 {
            self.pet.weather_day = now / 86_400;
            self.pet.roll_weather(&self.data, now);
        }
        if matches!(self.scene, Scene::Home) {
            if self.pet.is_pending(pet::req::EVOLUTION) || self.pet.is_pending(pet::req::METAMORPHO) {
                self.pet.evolve(&self.data);
            }
        }
        self.push_frames = if held & input::PUSH != 0 { self.push_frames.saturating_add(1) } else { 0 };
        // Holding the dial button for more than 13 frames opens the lab menu (08-care-ui §2.2.3).
        if self.push_frames == 14 && matches!(self.scene, Scene::Home) && self.pet.def.stage > 1 {
            self.sfx(1, 100);
            self.scene = self.open(ui::Action::Lab);
        }
        self.update_scene(pressed);
        // HOME zoom (`zoom_update` 0x1000ed48): close a quarter of the gap per frame.
        let target = ui::zoom_stop(self.level);
        self.zoom += (target - self.zoom) / 4.0;
        if (target - self.zoom).abs() <= 0.125 {
            self.zoom = target;
        }
        if matches!(self.scene, Scene::Home) && now / 60 != self.event_minute {
            self.event_minute = now / 60;
            let r = self.rand();
            self.check_events(now, r);
        }
        if matches!(self.scene, Scene::Home) {
            self.roles = [None; 4];
            self.planet_ranks = None;
            self.check_notices();
        }
        if matches!(self.scene, Scene::Home) && self.level == Level::Field {
            self.update_toys();
        }
        if matches!(self.scene, Scene::Home) && self.level != Level::Space {
            self.update_home(now);
            self.home.tick(&self.data.motions);
            self.home.take_sounds(&mut self.sounds);
        }
        self.render();
    }

    pub(crate) fn rand(&mut self) -> u32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        x
    }

    /// Choose the HOME behaviour; restart its motion when it changes.
    fn update_home(&mut self, now: u64) {
        let forced = home::forced_state(&self.pet, now);
        let level_changed = self.home_level != Some(self.level);
        self.home_level = Some(self.level);
        let next = match (forced, self.home_state) {
            (_, Some(cur)) if level_changed && forced.map_or(!cur.1, |f| f == cur.0) => cur,
            (Some(s), Some((cur, true))) if s == cur => return,
            (Some(s), _) => (s, true),
            // A random behaviour keeps playing until the pet's condition forces another.
            (None, Some((_, false))) => return,
            (None, _) => {
                let r = self.rand();
                (home::random_state(&self.pet, now, &mut self.home_seq, r), false)
            }
        };
        self.home_state = Some(next);
        let slot = self.level as usize;
        let script = self.data.home_motions.get(next.0 as usize).map(|m| if m[slot] != 0 { m[slot] } else { m[1] });
        self.home = Node::new(script.unwrap_or(0));
    }

    /// Advance the pet to local wall-clock seconds `now` and redraw.
    /// A pending evolution completes here, as the original does when the pet is on screen.
    pub fn advance_to(&mut self, now: u64) {
        self.pet.advance_to(now);
        if self.pet.is_pending(pet::req::EVOLUTION) || self.pet.is_pending(pet::req::METAMORPHO) {
            self.pet.evolve(&self.data);
        }
        self.render();
    }

    pub fn framebuffer(&self) -> &[u8] {
        &self.screen.fb
    }

    /// A connection request from the game's CONNECTION menu, for the platform to show its
    /// friend-code panel: "playdate", "receive", or "gift:<code>" with a code to hand over.
    pub fn take_connect(&mut self) -> Option<String> {
        self.connect.take()
    }

    /// This player's friend code for the current pet.
    pub fn friend_code(&self) -> String {
        self.pet.card(self.data.rom_type, None).encode()
    }

    /// Playdate with a friend's code; shows the result (PLAYED / FOUGHT / BRED) on screen.
    /// `marry` = both players agreed to marry if it is possible.
    pub fn playdate(&mut self, code: &str, marry: bool) -> Result<(), &'static str> {
        let card = social::Card::decode(code).ok_or("bad friend code")?;
        let outcome = self.pet.playdate(&self.data, &card, marry).ok_or("your tama cannot play now")?;
        let (text, how) = match outcome {
            social::Outcome::Play(n) => (0x73, n as usize),
            social::Outcome::Fight => (0x74, 4),
            social::Outcome::Married => (0x77, 3),
        };
        // The friend's tama plays role 1 in the playdate animation.
        self.roles[1] = self.data.character(card.tama).map(|c| c.chr_id);
        let then = Scene::Message { text, then: Box::new(Scene::Home) };
        let anim = ui::PLAYDATE_ANIM[how.min(6)];
        self.scene = self.play(&[anim], (0, ui::OnPress::None), None, then);
        Ok(())
    }

    /// Gift one of `item` as a code for a friend.
    pub fn gift(&mut self, item: u16) -> Option<String> {
        self.pet.gift(item)
    }

    /// Receive a friend's gift code.
    pub fn receive_gift(&mut self, code: &str) -> Option<u16> {
        let item = self.pet.receive_gift(&self.data, code)?;
        self.scene = Scene::Message { text: 0xcd, then: Box::new(Scene::Home) };
        Some(item)
    }

    /// Inventory as (item id, count, name string id), for platform UIs.
    pub fn inventory(&self) -> Vec<(u16, u8, String)> {
        self.pet
            .inventory
            .iter()
            .map(|&(id, n)| {
                let name = self.data.item(id).map(|i| self.plain_text(i.name)).unwrap_or_default();
                (id, n, name)
            })
            .collect()
    }

    /// A string as plain ASCII (Latin glyphs only), for platform UIs.
    pub fn plain_text(&self, id: u16) -> String {
        let glyphs = self.data.strings.get(1).and_then(|t| t.get(id as usize)).cloned().unwrap_or_default();
        glyphs
            .iter()
            .map(|&c| match c {
                318..=343 => (b'A' + (c - 318) as u8) as char,
                306..=315 => (b'0' + (c - 306) as u8) as char,
                screen::NEWLINE => ' ',
                _ => ' ',
            })
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Sounds to play since the last call, oldest first. The buzzer is single-voice, so a
    /// platform may play only the last one.
    pub fn take_sounds(&mut self) -> Vec<(u16, u8)> {
        std::mem::take(&mut self.sounds)
    }

    /// Sprite indexes the screen tried to draw that have not been added yet.
    pub fn missing_sprites(&self) -> Vec<u16> {
        self.screen.wanted.iter().copied().filter(|id| !self.screen.sprites.contains_key(id)).collect()
    }

    /// Script character id → (ChrID to draw, is the player's pet, face shown).
    pub(crate) fn resolve_chr(&self, chr: u16) -> (u16, bool, bool) {
        match chr {
            motion::PET if self.roles[0].is_some() => (self.roles[0].unwrap_or(0), false, true),
            // 0 = the pet, 1 = the pet inside the cell (medicine), 0xbd–0xdc = pet slots.
            motion::PET | motion::PET_IN_CELL | 0xbd..=0xdc => (self.pet.def.chr_id, true, true),
            motion::PET_NO_FACE => (self.pet.def.chr_id, true, false),
            motion::PLANET => (self.planet_chr_at(self.planet_ranks.map_or(self.pet.rank, |r| r[0])), false, true),
            c if (motion::PLANET_ROLE..motion::ROLE).contains(&c) => {
                let rank = self.planet_ranks.map_or(self.pet.rank, |r| r[((c - motion::PLANET_ROLE) as usize).min(1)]);
                (self.planet_chr_at(rank), false, true)
            }
            c if c > motion::ROLE => match self.roles.get((c - motion::ROLE) as usize).copied().flatten() {
                Some(chr) => (chr, false, true),
                None => (self.pet.def.chr_id, true, true),
            },
            c => (c, false, true),
        }
    }

    /// Draw one character pose (`chara_draw_pose` 0x100075c4).
    pub(crate) fn draw_chr(&mut self, d: &Draw, chr: u16, anim: u16, eyes: i16, mouth: i16) -> bool {
        let (chr_id, is_pet, face) = self.resolve_chr(chr);
        let Some(parts) = self.data.model(chr_id).and_then(|m| m.anims.get(anim as usize)).cloned() else { return false };
        let p = &self.pet;
        let color = p.body_color();
        let pal = (is_pet && p.def.stage > 1 && color < 16).then_some((COLOR_PALETTES, color as usize));
        let dirty = is_pet && p.status & status::DIRTY != 0;
        let mut drawn = false;
        for part in parts {
            let frame = match part.kind {
                PART_BODY => part.frame,
                PART_EYES if face => if eyes == KEEP { part.frame } else { eyes as u16 },
                PART_MOUTH if face => if mouth == KEEP { part.frame } else { mouth as u16 },
                PART_DIRT if dirty => part.frame,
                // Planet outfits: the worn item's sprite (item +4) replaces the placeholder.
                4 | 6 | 8 if chr == motion::PLANET || (motion::PLANET_ROLE..motion::ROLE).contains(&chr) => {
                    let worn = self.pet.equip[(part.kind / 2 - 2) as usize];
                    let Some(item) = self.data.item(worn) else { continue };
                    let x = if d.flip { -part.x } else { part.x } as i32;
                    drawn |= self.screen.blit(item.eat_sprite, part.frame, d.x + x, d.y + part.y as i32, d.alpha, part.flip ^ d.flip, None);
                    continue;
                }
                // 0x10 is never drawn.
                _ => continue,
            };
            let x = if d.flip { -part.x } else { part.x } as i32;
            let alpha = (part.alpha as u32 * d.alpha as u32 / 64) as u8;
            let pal = if part.kind == PART_BODY { pal } else { None };
            drawn |= self.screen.blit(part.sprite, frame, d.x + x, d.y + part.y as i32, alpha, part.flip ^ d.flip, pal);
        }
        drawn
    }

    /// Add a sprite in `sprite::Sprite` byte format. Returns false if the bytes are invalid.
    pub fn add_sprite(&mut self, id: u16, bytes: &[u8]) -> bool {
        let Some(s) = Sprite::from_bytes(bytes) else { return false };
        self.screen.sprites.insert(id, s);
        self.render();
        true
    }

    fn render(&mut self) {
        self.screen.wanted.clear();
        self.screen.rect(0, 0, WIDTH as i32, HEIGHT as i32, [0, 0, 0]);
        if self.render_scene() {
            return;
        }
        // Mid-zoom, draw the layer nearest the zoom value, scaled by the remaining distance.
        let real = self.level;
        let shown = [Level::Space, Level::Field, Level::Pet, Level::Cell]
            .into_iter()
            .min_by(|a, b| (ui::zoom_stop(*a) - self.zoom).abs().total_cmp(&(ui::zoom_stop(*b) - self.zoom).abs()))
            .unwrap_or(real);
        self.level = shown;
        self.screen.zoom = 4f32.powf((self.zoom - ui::zoom_stop(shown)) / 16.0);
        self.render_home();
        self.level = real;
        self.screen.zoom = 1.0;
    }

    fn render_home(&mut self) {
        match self.level {
            Level::Space => return self.draw_space(),
            Level::Cell => self.screen.rect(0, 0, WIDTH as i32, HEIGHT as i32, [250, 220, 230]),
            _ => {
                // The world layer scales with the real zoom (1× field, 2× pet); the camera moves
                // from the field focus to the pet as it zooms in (`home_camera_update` 0x100793fc).
                let t = (self.zoom / 8.0).clamp(0.0, 1.0);
                let lerp = |a: i32, b: i32| a + ((b - a) as f32 * t).round() as i32;
                let c = (lerp(self.cam.0, PET_CAM.0), lerp(self.cam.1, PET_CAM.1));
                self.screen.off = (64 - c.0, 64 - c.1);
                self.screen.zoom = 4f32.powf(self.zoom / 16.0);
                self.draw_field_bg();
                if self.level == Level::Field {
                    self.draw_toys();
                    self.draw_grazing();
                    self.draw_visitor();
                    self.draw_poop();
                }
                // The pet keeps its size; only the world around it zooms.
                self.screen.zoom = 1.0;
                if self.level == Level::Pet {
                    self.screen.off = (0, 0);
                }
            }
        }
        let (ax, ay) = ANCHOR[self.level as usize];
        let mut draws = Vec::new();
        self.home.draws(ax, ay, &mut draws);
        self.draw_all(&draws);
        self.screen.off = (0, 0);
        self.draws = draws;
    }

    /// Draw motion output: characters and sprites.
    pub(crate) fn draw_all(&mut self, draws: &[Draw]) {
        for d in draws {
            match d.look {
                Look::Chr { chr, anim, eyes, mouth } => {
                    self.draw_chr(d, chr, anim, eyes, mouth);
                }
                Look::Sprite { id, frame } => {
                    self.screen.blit(id, frame, d.x, d.y, d.alpha, d.flip, (d.palette > 0).then_some((id, d.palette as usize)));
                }
                Look::None => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic() {
        let run = || {
            let mut g = Game::new(evolve::tests::data(), 3);
            for h in 0..72 {
                g.advance_to(86_400 + h * 3600);
                g.frame(86_400 + h * 3600, input::A);
                g.frame(86_400 + h * 3600, 0);
            }
            (g.framebuffer().to_vec(), g.pet.def.id, g.pet.onaka, g.pet.osewamiss, g.pet.cold)
        };
        assert_eq!(run(), run());
    }
}
