//! Pure game logic: button input + wall-clock time in, framebuffer out.

pub mod data;
pub mod decor;
pub mod ease;
pub(crate) mod evolve;
pub mod family;
pub mod hant;
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
mod ui;
pub mod world;

use data::{Data, PART_BODY, PART_DIRT, PART_EYES, PART_MOUTH};
use motion::{Draw, Node, KEEP};
use pet::{status, Pet};
use screen::Screen;
pub use screen::{HEIGHT, WIDTH};
use sprite::Sprite;

pub use ui::Level;
use ui::Scene;

/// Sprite with the 16 pet body colour palettes; set n = colour n (game-spec §8).
const COLOR_PALETTES: u16 = 206;
/// The egg character (chr placeholder 0x1f).
const EGG_ID: u16 = 1000;
/// HOME anchors (character's bottom centre) for field, pet and cell zoom.
/// World point the camera centres on in the pet layer, so the field pet lands where the
/// close-up pet stands.
const PET_CAM: (i32, i32) = (64, 78);
/// Past scale 3.0 (z = 16·log4 3) the pet layer plays the mouth-open column.
const MOUTH_OPEN_ZOOM: f32 = 12.68;
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
    /// Sounds started since the last `take_sounds`: (melody id, volume 0–100).
    sounds: Vec<(u16, u8)>,
    /// Minute of the last calendar-event check.
    event_minute: u64,
    /// Frames the dial button has been held (long press opens the lab menu).
    push_frames: u16,
    /// Field camera: focused object (0 = pet, then placed toys) and current centre in world space.
    focus: usize,
    /// Ring menu wheel position in items, for the menu (item count, first item, back) it belongs to.
    menu_pos: Option<(usize, u16, Option<ui::Action>, f32)>,
    /// The field camera is still more than 4 px from its focus (C waits).
    cam_moving: bool,
    cam: (i32, i32),
    /// Outside toys on the field: (slot, looping show animation), rebuilt when the layout changes.
    toy_nodes: Vec<(usize, Node)>,
    toy_key: Option<(u8, bool, [u16; 6])>,
    /// Characters standing in for script roles 1–3 in the current scene (ChrID), see `motion::ROLE`.
    pub(crate) roles: [Option<u16>; 4],
    /// The pet's look before an evolution (chr 0xdb, EVENT_EVOLUTION), drawn with its palette.
    pub(crate) old_chr: Option<u16>,
    /// Frames without input (the planet level's idle clock).
    idle: u16,
    /// Screen fade to black, 0 (none) – 32 (black), easing back to 0 (`fade_set` 0x1007b40c).
    pub(crate) fade: u8,
    /// LABOMENU cursor, kept for coming back from a lab screen.
    pub(crate) lab_idx: u8,
    /// LABOMENU opened while the pet sleeps sick (`0x18001c6b` bit3): only SETTINGS.
    pub(crate) lab_settings_only: bool,
    /// The frame the C button was last let go (double-tap C cancels in the number editors).
    pub(crate) c_released: u32,
    /// Fade colour (black; pink or white in layer changes).
    pub(crate) fade_colour: [u8; 3],
    /// A layer change in progress (kind, frame, zoom at the start) and the layer it draws.
    pub(crate) change: Option<(ui::Change, u16, f32)>,
    pub(crate) shown: Option<Level>,
    /// Where the pet's mouth was last drawn on the pet layer (centre of the zoom into the cell).
    pet_mouth: (f32, f32),
    /// Warp overlay of a layer change, the indicator's frame count, the cell layer's scene.
    pub(crate) warp: Option<Node>,
    pub(crate) indicator: Option<u16>,
    pub(crate) cell_scene: Node,
    /// The layer the zoom last settled on (field ↔ pet arrival sound; none before the first).
    settled: Option<Level>,
    /// Planet ranks shown for planet roles 0 and 1 (rank-up: old, new).
    pub(crate) planet_ranks: Option<[u8; 2]>,
    /// Outfit-set visitor on the field (chr, looping motion 0x2f1) and the day it was rolled.
    visitor: Option<(u16, Node)>,
    visitor_day: Option<u64>,
    /// A request for the platform's connection panel ("playdate", "receive", "gift:<code>").
    connect: Option<String>,
    /// The build shown in GAME INFO (set by the platform: the published commit).
    build: String,
    /// Calls already announced with the call melody (bit per request id).
    calls_heard: u64,
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
        motion::set_motions(&data.motions);
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
            sounds: Vec::new(),
            event_minute: 0,
            push_frames: 0,
            focus: 0,
            cam_moving: false,
            menu_pos: None,
            cam: ui::PET_FOCUS,
            toy_nodes: Vec::new(),
            toy_key: None,
            zoom: ui::zoom_stop(Level::Pet),
            roles: [None; 4],
            old_chr: None,
            idle: 0,
            fade: 0,
            lab_idx: 0,
            lab_settings_only: false,
            // Long ago, so a first C press is never a double tap.
            c_released: u32::MAX / 2,
            fade_colour: [0, 0, 0],
            change: None,
            shown: None,
            pet_mouth: (64.0, 88.0),
            warp: None,
            indicator: None,
            cell_scene: Game::new_cell_scene(),
            settled: None,
            calls_heard: 0,
            planet_ranks: None,
            connect: None,
            build: "DEV".into(),
            visitor: None,
            visitor_day: None,
        };
        g.render();
        g
    }

    /// One 20 fps frame: `now` = local wall-clock seconds, `held` = `input` bits held this frame.
    pub fn frame(&mut self, now: u64, held: u8) {
        // DATE/TIME shifts the platform's clock.
        let now = now.saturating_add_signed(self.pet.clock_offset);
        // Buttons act on the press edge; dial bits are already one-frame pulses.
        let dial = input::UP | input::DOWN;
        let pressed = (held & !self.held & !dial) | (held & dial);
        if self.held & input::C != 0 && held & input::C == 0 {
            self.c_released = self.frame_count.wrapping_add(1);
        }
        self.held = held;
        self.idle = if held != 0 { 0 } else { self.idle.saturating_add(1) };
        self.frame_count = self.frame_count.wrapping_add(1);
        self.pet.advance_to(now);
        self.call_alarm();
        if self.pet.weather_day != now / 86_400 {
            self.pet.weather_day = now / 86_400;
            self.pet.roll_weather(&self.data, now);
        }
        if matches!(self.scene, Scene::Home) {
            // HOME's request check (`FUN_10019b48`): METAMORPHO changes the child at once,
            // EVOLUTION goes through its event scenes.
            if self.pet.is_pending(pet::req::METAMORPHO) {
                self.pet.evolve(&self.data);
            } else if self.pet.is_pending(pet::req::EVOLUTION) {
                self.scene = self.begin_evolution();
            } else if self.pet.is_pending(pet::req::UNCHI) && self.level != Level::Space {
                // EVENT_UNCHI (`FUN_1003e824`): the pet poops in front of the player.
                self.pet.clear(pet::req::UNCHI);
                self.scene = self.play(&[ui::script::POOP], (0, ui::OnPress::None), None, Scene::Home);
            }
        } else {
            // Opening HOME again settles a poop made elsewhere without the scene (`FUN_1001abd0`).
            self.pet.clear(pet::req::UNCHI);
        }
        self.push_frames = if held & input::PUSH != 0 { self.push_frames.saturating_add(1) } else { 0 };
        // Holding the dial button for more than 13 frames opens the lab menu (08-care-ui §2.2.3).
        if self.push_frames == 14 && matches!(self.scene, Scene::Home) && self.pet.def.stage > 1 {
            // Sick, hurt or dying, the lab only opens while the pet sleeps with a cold or a wound,
            // and then only for SETTINGS (`home_cell_state` / `FUN_1006d188`, c6b bit3).
            let asleep = self.pet.last.is_some_and(|t| self.pet.is_sleeping_at(t));
            let sick = matches!(self.pet.cond, pet::Cond::Cold | pet::Cond::Injury | pet::Cond::Dying);
            let settings_only = asleep && matches!(self.pet.cond, pet::Cond::Cold | pet::Cond::Injury);
            if !sick || settings_only {
                self.sfx(1, 100);
                self.scene = self.enter_lab(settings_only);
            }
        }
        // The dial and buttons wait while a layer change plays.
        let pressed = if self.change.is_some() { 0 } else { pressed };
        self.update_scene(pressed);
        if self.change.is_some() {
            self.step_change();
        } else {
            // HOME zoom (`zoom_update` 0x1000ed48): close a quarter of the gap per frame.
            let target = ui::zoom_stop(self.level);
            self.zoom += (target - self.zoom) / 4.0;
            if (target - self.zoom).abs() <= 0.125 {
                self.zoom = target;
                if self.settled != Some(self.level) {
                    let between = |l: Level| matches!(l, Level::Field | Level::Pet);
                    if self.settled.is_some_and(between) && between(self.level) && matches!(self.scene, Scene::Home) {
                        self.arrive();
                    }
                    self.settled = Some(self.level);
                }
            }
        }
        self.tick_overlays();
        if matches!(self.scene, Scene::Home) {
            self.route_home();
        }
        if matches!(self.scene, Scene::Home) && now / 60 != self.event_minute {
            self.event_minute = now / 60;
            let r = self.rand();
            self.check_events(now, r);
        }
        if matches!(self.scene, Scene::Home) {
            self.roles = [None; 4];
            self.old_chr = None;
            self.planet_ranks = None;
            self.check_notices();
        }
        if matches!(self.scene, Scene::Home) && self.level == Level::Field {
            self.update_toys();
        } else {
            // Off the field (zoomed in, or another layer) the camera goes back to the pet.
            self.focus = 0;
        }
        if matches!(self.scene, Scene::Home) && self.level != Level::Space {
            self.update_home(now);
            self.update_home_column();
            self.home.tick(&self.data.motions);
            self.home.take_sounds(&mut self.sounds);
        }
        if self.change.is_none() {
            self.fade = self.fade.saturating_sub(ui::FADE_STEP);
            if self.fade == 0 {
                self.fade_colour = [0, 0, 0];
            }
        }
        self.render();
    }

    /// The call melody when the pet starts calling (`FUN_1001abd0`, flag set by `FUN_10019b48` for
    /// HOMETRG, ONAKA, GOKIGEN, COLD, EVOL_SUPPORT and DODODO).
    fn call_alarm(&mut self) {
        if self.new_call().is_some() {
            self.sfx(ui::SND_CALL, 100);
        }
    }

    /// The first call that started since the last check (request id), if any.
    fn new_call(&mut self) -> Option<u8> {
        use pet::req;
        let calls = [req::HOMETRG, req::ONAKA, req::GOKIGEN, req::COLD, req::EVOL_SUPPORT, req::DODODO];
        let pending = calls.iter().filter(|&&r| self.pet.is_pending(r)).fold(0u64, |m, &r| m | 1 << r);
        let new = pending & !self.calls_heard;
        self.calls_heard = pending;
        // The specific need first; HOMETRG ("come and see me") last.
        calls.into_iter().cycle().skip(1).take(calls.len()).find(|&r| new & 1 << r != 0)
    }

    /// While the page is hidden: advance the pet only (no scenes, no drawing) and report a call
    /// that just started, for the platform to notify the player instead of lighting the screen.
    pub fn poll_call(&mut self, now: u64) -> Option<u8> {
        let now = now.saturating_add_signed(self.pet.clock_offset);
        self.pet.advance_to(now);
        self.new_call()
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
        let layer = self.shown.unwrap_or(self.level);
        // The planet has no pet behaviour (its first half of a change to the field draws it).
        if layer == Level::Space {
            return;
        }
        let level_changed = self.home_level != Some(layer);
        self.home_level = Some(layer);
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
        self.home = Node::new(self.home_script(next.0, layer));
    }

    /// The HOME motion for a state on a layer (`FUN_1008467c`, table 0x1009dbd8): field, close-up,
    /// or past 3× zoom on the pet layer the third column, the pet with its mouth open (the way
    /// into the cell, `home_zoom_dispatch` 0x100687dc). The cell has its own motions.
    fn home_script(&self, state: u8, layer: Level) -> u16 {
        if layer == Level::Cell {
            return self.cell_motion();
        }
        let slot = if layer == Level::Pet && self.zoom > MOUTH_OPEN_ZOOM { 2 } else { layer as usize };
        self.data.home_motions.get(state as usize).map_or(0, |m| if m[slot] != 0 { m[slot] } else { m[1] })
    }

    /// Restart the HOME motion when its column changes with the zoom (the firmware replays only
    /// when the script id changes).
    fn update_home_column(&mut self) {
        let Some((state, _)) = self.home_state else { return };
        let layer = self.shown.unwrap_or(self.level);
        if layer == Level::Space || layer == Level::Cell {
            return;
        }
        let script = self.home_script(state, layer);
        if script != self.home.script() {
            self.home = Node::new(script);
        }
    }

    /// Advance the pet to local wall-clock seconds `now` and redraw.
    /// A pending evolution completes here, as the original does when the pet is on screen.
    pub fn advance_to(&mut self, now: u64) {
        let now = now.saturating_add_signed(self.pet.clock_offset);
        self.pet.advance_to(now);
        if self.pet.is_pending(pet::req::EVOLUTION) || self.pet.is_pending(pet::req::METAMORPHO) {
            self.pet.evolve(&self.data);
        }
        self.pet.clear(pet::req::UNCHI);
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
            social::Outcome::Ate => (0x75, 5),
            social::Outcome::Eaten => (0x76, 6),
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
    /// Volumes are scaled by the SOUND level; level 0 plays nothing.
    pub fn take_sounds(&mut self) -> Vec<(u16, u8)> {
        let master = ui::MASTER_VOLUME[self.pet.volume.min(3) as usize];
        let sounds = std::mem::take(&mut self.sounds);
        if master == 0 {
            return Vec::new();
        }
        sounds.into_iter().map(|(id, v)| (id, (v as u32 * master / 100) as u8)).collect()
    }

    /// DELETE: what the original shows on a cold start with a save (CONTINUE / RESET ALL).
    pub fn begin_delete(&mut self) {
        self.scene = self.open(ui::Action::Delete);
    }

    /// Whether care calls should notify (SETTINGS, remake only).
    pub fn notify(&self) -> bool {
        self.pet.notify
    }

    /// The page turns notifications back off when the browser refuses them.
    pub fn set_notify(&mut self, on: bool) {
        self.pet.notify = on;
    }

    /// Whether the platform's touch controls on the screen are on (SETTINGS, remake only).
    pub fn touch(&self) -> bool {
        self.pet.touch
    }

    /// Sprite indexes the screen tried to draw that have not been added yet.
    pub fn missing_sprites(&self) -> Vec<u16> {
        self.screen.wanted.iter().copied().filter(|id| !self.screen.sprites.contains_key(id)).collect()
    }

    /// Script character id → (ChrID to draw, is the player's pet, face shown).
    pub(crate) fn resolve_chr(&self, chr: u16) -> (u16, bool, bool) {
        match chr {
            motion::PET if self.old_chr.is_some() => (self.old_chr.unwrap_or(0), true, true),
            motion::PET if self.roles[0].is_some() => (self.roles[0].unwrap_or(0), false, true),
            // 0 = the pet, 1 = the pet inside the cell (medicine), 0xbd–0xdc = pet slots.
            // 1 = the pet as a cell (HOME_CELL, medicine), its own model.
            motion::PET_IN_CELL => (1, false, true),
            motion::PET | 0xbd..=0xdc => (self.pet.def.chr_id, true, true),
            motion::EGG => (self.data.character(EGG_ID).map_or(self.pet.def.chr_id, |c| c.chr_id), false, true),
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
        let Some(mut parts) = self.data.model(chr_id).and_then(|m| m.anims.get(anim as usize)).cloned() else {
            return false;
        };
        // The cell (chr 1) wears the pet's own eyes from the same pose (`FUN_100075c4` case 1).
        if chr == motion::PET_IN_CELL {
            let eye =
                |m: &data::Model| m.anims.get(anim as usize)?.iter().find(|p| p.kind == PART_EYES).map(|p| p.sprite);
            if let Some(sprite) = self.data.model(self.pet.def.chr_id).and_then(eye) {
                for part in parts.iter_mut().filter(|p| p.kind == PART_EYES) {
                    part.sprite = sprite;
                }
            }
        }
        let p = &self.pet;
        let color = p.body_color();
        let pal = (is_pet && p.def.stage > 1 && color < 16).then_some((COLOR_PALETTES, color as usize));
        let dirty = is_pet && p.status & status::DIRTY != 0;
        let mut drawn = false;
        for part in parts {
            let frame = match part.kind {
                PART_BODY => part.frame,
                PART_EYES if face => {
                    if eyes == KEEP {
                        part.frame
                    } else {
                        eyes as u16
                    }
                }
                PART_MOUTH if face => {
                    if mouth == KEEP {
                        part.frame
                    } else {
                        mouth as u16
                    }
                }
                PART_DIRT if dirty => part.frame,
                // Planet outfits: the worn item's sprite (item +4) replaces the placeholder.
                4 | 6 | 8 if chr == motion::PLANET || (motion::PLANET_ROLE..motion::ROLE).contains(&chr) => {
                    let worn = self.pet.equip[(part.kind / 2 - 2) as usize];
                    let Some(item) = self.data.item(worn) else { continue };
                    let x = if d.flip { -part.x } else { part.x } as i32;
                    drawn |= self.screen.blit(
                        item.eat_sprite,
                        part.frame,
                        d.x + x,
                        d.y + part.y as i32,
                        d.alpha,
                        part.flip ^ d.flip,
                        None,
                    );
                    continue;
                }
                // 0x10 is never drawn.
                _ => continue,
            };
            let x = if d.flip { -part.x } else { part.x } as i32;
            if is_pet && part.kind == PART_MOUTH && self.level == Level::Pet && matches!(self.scene, Scene::Home) {
                self.pet_mouth = self.screen.project(d.x + x, d.y + part.y as i32);
            }
            let alpha = (part.alpha as u32 * d.alpha as u32 / 64) as u8;
            let pal = if part.kind == PART_BODY { pal } else { None };
            drawn |= self.screen.blit(part.sprite, frame, d.x + x, d.y + part.y as i32, alpha, part.flip ^ d.flip, pal);
        }
        drawn
    }

    /// The build id shown in GAME INFO.
    pub fn set_build(&mut self, build: &str) {
        self.build = build.chars().take(12).collect();
    }

    /// Add a sprite in `sprite::Sprite` byte format. Returns false if the bytes are invalid.
    pub fn add_sprite(&mut self, id: u16, bytes: &[u8]) -> bool {
        let Some(mut s) = Sprite::from_bytes(bytes) else { return false };
        if self.data.fonts.get(2).is_some_and(|f| f.sprite == id) {
            crate::hant::extend_font(&mut s);
        }
        self.screen.sprites.insert(id, s);
        self.render();
        true
    }

    fn render(&mut self) {
        self.screen.wanted.clear();
        self.screen.rect(0, 0, WIDTH as i32, HEIGHT as i32, [0, 0, 0]);
        if !self.render_scene() {
            self.render_layers();
            self.draw_warp();
            self.draw_indicator();
        }
        // The fade covers the screen with its colour by `fade` / 32 (a mini-game's own if deeper).
        let mini = match &self.scene {
            Scene::Mini { session, .. } => session.screen_fade(),
            _ => None,
        };
        let (fade, colour) = match mini {
            Some(f) if f.level() > self.fade => (f.level(), f.colour()),
            _ => (self.fade, self.fade_colour),
        };
        if fade > 0 {
            let (f, full) = (fade.min(ui::FADE_FULL) as u32, ui::FADE_FULL as u32);
            for px in self.screen.fb.as_chunks_mut::<4>().0 {
                for (c, &to) in px[..3].iter_mut().zip(&colour) {
                    *c = ((*c as u32 * (full - f) + to as u32 * f) / full) as u8;
                }
            }
        }
    }

    fn render_layers(&mut self) {
        // Field ↔ pet is one continuous zoom drawn as the level the dial is on (its motions play);
        // layer changes draw the layer of the half in progress. Either way the layer is drawn at
        // its own stop and scaled by the remaining distance about the zoom's centre.
        let real = self.level;
        let shown = self.shown.unwrap_or(real);
        self.level = shown;
        let rel = 4f32.powf((self.zoom - ui::zoom_stop(shown)) / 16.0);
        if shown == Level::Cell {
            self.screen.zoom = rel;
        } else {
            self.screen.rel = rel;
            self.screen.pivot = self.zoom_pivot(shown);
        }
        self.render_home();
        self.level = real;
        self.screen.zoom = 1.0;
        self.screen.rel = 1.0;
        self.screen.pivot = (64.0, 64.0);
    }

    /// Centre of the HOME zoom on screen: the planet to and from space, the pet's mouth to and
    /// from the cell, else the pet.
    fn zoom_pivot(&self, shown: Level) -> (f32, f32) {
        let planet = (ui::PLANET_POS.0 as f32, ui::PLANET_POS.1 as f32);
        match (shown, self.change.map(|c| c.0)) {
            (Level::Space, _) | (_, Some(ui::Change::ToSpace | ui::Change::FromSpace)) => planet,
            (_, Some(ui::Change::ToCell | ui::Change::FromCell)) => self.pet_mouth,
            // The field doubled about this point lines up with the pet layer (and the pet layer
            // halved with the field): it sits on the pet while the camera is on it.
            _ => ((64 + 2 * (64 - self.cam.0)) as f32, (92 + 2 * (64 - self.cam.1)) as f32),
        }
    }

    fn render_home(&mut self) {
        match self.level {
            Level::Space => {
                self.draw_space();
                if self.idle > ui::CLOCK_IDLE && matches!(self.scene, Scene::Home) {
                    self.draw_idle_clock();
                }
                return;
            }
            Level::Cell => self.draw_cell(),
            _ => {
                // The world: 1× about the field camera, or 2× about the pet (`home_camera_update`
                // 0x100793fc).
                let c = if self.level == Level::Pet { PET_CAM } else { self.cam };
                self.screen.off = (64 - c.0, 64 - c.1);
                self.screen.zoom = 4f32.powf(ui::zoom_stop(self.level) / 16.0);
                self.draw_field_bg();
                if self.level == Level::Field {
                    self.draw_toys();
                    self.draw_grazing();
                    self.draw_visitor();
                    self.draw_poop();
                }
                self.screen.zoom = 1.0;
                if self.level == Level::Pet {
                    self.screen.off = (0, 0);
                }
            }
        }
        // The cell's motions are placed at the screen centre.
        let (ax, ay) = if self.level == Level::Cell { (64, 64) } else { ANCHOR[self.level as usize] };
        let mut draws = Vec::new();
        self.home.draws(ax, ay, &mut draws);
        self.draw_all(&draws);
        self.screen.off = (0, 0);
        self.draws = draws;
    }

    /// Draw motion output: characters and sprites.
    pub(crate) fn draw_all(&mut self, draws: &[Draw]) {
        self.draw_list_of(draws, None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hidden_page_hears_each_call_once() {
        let mut g = Game::new(crate::evolve::tests::data(), 1);
        g.pet.def = g.data.character(2113).unwrap();
        let t = 86_400 * 3 + 12 * 3600;
        g.frame(t, 0);
        g.pet.onaka = 0;
        g.pet.raise(pet::req::ONAKA);
        assert_eq!(g.poll_call(t + 1), Some(pet::req::ONAKA));
        assert_eq!(g.poll_call(t + 2), None, "already told");
        g.frame(t + 3, 0);
        assert!(!g.take_sounds().iter().any(|s| s.0 == ui::SND_CALL), "no melody for it on return");
    }

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

    #[test]
    fn a_new_call_plays_the_call_melody_once() {
        let mut g = Game::new(evolve::tests::data(), 3);
        g.pet.def = g.data.character(2113).unwrap();
        let t = 86_400 * 3 + 12 * 3600;
        g.frame(t, 0);
        g.take_sounds();
        g.pet.raise(pet::req::ONAKA);
        g.frame(t, 0);
        assert!(g.take_sounds().iter().any(|s| s.0 == ui::SND_CALL));
        g.frame(t, 0);
        assert!(!g.take_sounds().contains(&(ui::SND_CALL, 100)), "only when the call starts");
    }
}
