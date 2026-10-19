//! Pure game logic: button input + wall-clock time in, framebuffer out.

pub mod data;
pub(crate) mod evolve;
pub mod home;
pub mod motion;
pub mod pet;
pub mod save;
pub mod screen;
pub mod sprite;
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
const ANCHOR: [(i32, i32); 3] = [(64, 96), (64, 100), (64, 100)];

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
        if matches!(self.scene, Scene::Home) {
            if self.pet.is_pending(pet::req::EVOLUTION) || self.pet.is_pending(pet::req::METAMORPHO) {
                self.pet.evolve(&self.data);
            }
        }
        self.update_scene(pressed);
        if matches!(self.scene, Scene::Home) {
            self.update_home(now);
            self.home.tick(&self.data.motions);
        }
        self.render();
    }

    fn rand(&mut self) -> u32 {
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

    /// Sprite indexes the screen tried to draw that have not been added yet.
    pub fn missing_sprites(&self) -> Vec<u16> {
        self.screen.wanted.iter().copied().filter(|id| !self.screen.sprites.contains_key(id)).collect()
    }

    /// Script character id → (ChrID to draw, is the player's pet, face shown).
    fn resolve_chr(&self, chr: u16) -> (u16, bool, bool) {
        match chr {
            // 0 = the pet, 1 = the pet inside the cell (medicine), 0xbd–0xdc = pet slots.
            motion::PET | motion::PET_IN_CELL | 0xbd..=0xdc => (self.pet.def.chr_id, true, true),
            motion::PET_NO_FACE => (self.pet.def.chr_id, true, false),
            c => (c, false, true),
        }
    }

    /// Draw one character pose (`chara_draw_pose` 0x100075c4).
    pub(crate) fn draw_chr(&mut self, d: &Draw, chr: u16, anim: u16, eyes: i16, mouth: i16) -> bool {
        let (chr_id, is_pet, face) = self.resolve_chr(chr);
        let Some(parts) = self.data.model(chr_id).and_then(|m| m.anims.get(anim as usize)).cloned() else { return false };
        let p = &self.pet;
        let pal = (is_pet && p.def.stage > 1 && p.def.color < 16).then_some((COLOR_PALETTES, p.def.color as usize));
        let dirty = is_pet && p.status & status::DIRTY != 0;
        let mut drawn = false;
        for part in parts {
            let frame = match part.kind {
                PART_BODY => part.frame,
                PART_EYES if face => if eyes == KEEP { part.frame } else { eyes as u16 },
                PART_MOUTH if face => if mouth == KEEP { part.frame } else { mouth as u16 },
                PART_DIRT if dirty => part.frame,
                // 0x10 is never drawn; 4/6/8 are worn items (not implemented).
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
        match self.level {
            Level::Cell => self.screen.rect(0, 0, WIDTH as i32, HEIGHT as i32, [250, 220, 230]),
            _ => self.draw_field_bg(),
        }
        if self.level == Level::Field {
            self.draw_poop();
        }
        let (ax, ay) = ANCHOR[self.level as usize];
        let mut draws = Vec::new();
        self.home.draws(ax, ay, &mut draws);
        for d in &draws {
            match d.look {
                Look::Chr { chr, anim, eyes, mouth } => {
                    self.draw_chr(d, chr, anim, eyes, mouth);
                }
                Look::Sprite { id, frame } => {
                    self.screen.blit(id, frame, d.x, d.y, d.alpha, d.flip, None);
                }
                Look::None => {}
            }
        }
        self.draws = draws;
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
