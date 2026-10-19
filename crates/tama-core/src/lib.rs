//! Pure game logic: button input + wall-clock time in, framebuffer out.

pub mod data;
pub(crate) mod evolve;
pub mod pet;
pub mod save;
pub mod sprite;

use data::{Data, Part, KIND_MEAL, PART_BODY, PART_EYES, PART_MOUTH};
use pet::{Cond, Pet};
use sprite::Sprite;
use std::collections::HashMap;

/// Sprite with the 16 pet body colour palettes; set n = colour n (game-spec §8).
const COLOR_PALETTES: u16 = 206;
/// Eyes frame with closed eyes ("– –"), used while asleep.
// ponytail: guessed from the eyes sheet; the real sleep pose is another animation number.
const EYES_SLEEP: u16 = 7;

pub const WIDTH: usize = 128;
pub const HEIGHT: usize = 128;
/// Frame step, 20 fps like the original (game-spec §2). Pet simulation runs on wall-clock seconds.
pub const TICK_MS: u32 = 50;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Button {
    A,
    B,
    C,
}

pub struct Game {
    pub data: Data,
    pub pet: Pet,
    /// Decoded sprites by sprite-pack index, supplied by the platform (`add_sprite`).
    sprites: HashMap<u16, Sprite>,
    /// RGBA8888, row-major.
    fb: Vec<u8>,
}

impl Game {
    /// Start a new game with an egg in the device's home area.
    pub fn new(data: Data, seed: u32) -> Self {
        let mut pet = Pet::new(data.character(1000).expect("egg 1000 in data"), seed);
        pet.field = data::attr_to_field(data.rom_type).unwrap_or(0);
        Game::with_pet(data, pet)
    }

    fn with_pet(data: Data, pet: Pet) -> Self {
        let mut g = Game { data, pet, sprites: HashMap::new(), fb: vec![0; WIDTH * HEIGHT * 4] };
        g.render();
        g
    }

    /// A = feed, B = toilet if restless else clean,
    /// C = medicine / wake / rescue / evolution support / new egg after death.
    pub fn press(&mut self, b: Button) {
        let p = &mut self.pet;
        match b {
            Button::A => {
                // ponytail: first meal in the table until there is a food menu.
                if let Some(meal) = self.data.items.iter().find(|i| i.kind == KIND_MEAL) {
                    p.feed(&meal.clone());
                }
            }
            Button::B => {
                if !p.toilet() {
                    p.clean();
                }
            }
            Button::C => {
                if p.cond == Cond::Dead {
                    p.new_egg(&self.data);
                } else {
                    let _ = p.medicine() || p.wake() || p.rescue() || p.support(&self.data);
                }
            }
        }
        self.render();
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
        &self.fb
    }

    /// Current idle pose: alternates between the two idle keyframes every second.
    fn pose(&self) -> Vec<Part> {
        let Some(m) = self.data.model(self.pet.def.chr_id) else { return Vec::new() };
        let alt = self.pet.last.unwrap_or(0) % 2 == 1 && !m.idle[1].is_empty();
        // ponytail: only body/eyes/mouth; kinds 16/17 (e.g. sprite 221 dust puffs) are
        // state effects whose conditions are not traced yet.
        m.idle[alt as usize].iter().filter(|p| matches!(p.kind, PART_BODY | PART_EYES | PART_MOUTH)).copied().collect()
    }

    /// Sprite indexes the current screen needs that have not been added yet.
    pub fn missing_sprites(&self) -> Vec<u16> {
        let mut ids = vec![COLOR_PALETTES];
        if let Some(m) = self.data.model(self.pet.def.chr_id) {
            ids.extend(m.idle.iter().flatten().filter(|p| matches!(p.kind, PART_BODY | PART_EYES | PART_MOUTH)).map(|p| p.sprite));
        }
        ids.sort_unstable();
        ids.dedup();
        ids.retain(|id| !self.sprites.contains_key(id));
        ids
    }

    /// Add a sprite in `sprite::Sprite` byte format. Returns false if the bytes are invalid.
    pub fn add_sprite(&mut self, id: u16, bytes: &[u8]) -> bool {
        let Some(s) = Sprite::from_bytes(bytes) else { return false };
        self.sprites.insert(id, s);
        self.render();
        true
    }

    fn rect(&mut self, x: usize, y: usize, w: usize, h: usize, c: [u8; 3]) {
        for yy in y..(y + h).min(HEIGHT) {
            for xx in x..(x + w).min(WIDTH) {
                let i = (yy * WIDTH + xx) * 4;
                self.fb[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
    }

    /// Draw `frame` of sprite `id` centred on (cx, cy) with alpha 0–64.
    /// `palette` = (sprite, set) whose palette replaces the sprite's own set 0.
    fn blit(&mut self, id: u16, frame: u16, cx: i32, cy: i32, alpha: u8, palette: Option<(u16, usize)>) -> bool {
        let Some(s) = self.sprites.get(&id) else { return false };
        let (w, h) = (s.w as i32, s.h as i32);
        let f = (frame % s.frames) as usize * (w * h) as usize;
        let pal = palette.and_then(|(pid, set)| self.sprites.get(&pid).map(|p| (p, set)));
        for y in 0..h {
            for x in 0..w {
                let (px, py) = (cx - w / 2 + x, cy - h / 2 + y);
                if !(0..WIDTH as i32).contains(&px) || !(0..HEIGHT as i32).contains(&py) {
                    continue;
                }
                let idx = s.pixels[f + (y * w + x) as usize];
                if s.transparent && idx == s.key {
                    continue;
                }
                let c = match pal {
                    Some((p, set)) => p.rgba(set, idx),
                    None => s.rgba(0, idx),
                };
                let i = (py as usize * WIDTH + px as usize) * 4;
                let a = alpha.min(64) as u32;
                for ch in 0..3 {
                    let d = self.fb[i + ch] as u32;
                    self.fb[i + ch] = ((c[ch] as u32 * a + d * (64 - a)) / 64) as u8;
                }
            }
        }
        true
    }

    fn render(&mut self) {
        let p = &self.pet;
        let (onaka, gokigen, unchi, stage) = (p.onaka as usize, p.gokigen as usize, p.unchi.len(), p.def.stage as usize);
        let sleeping = p.last.is_some_and(|t| p.is_sleeping_at(t));
        let tint = match p.cond {
            Cond::Normal if sleeping => [24, 24, 56],
            Cond::Normal => [200, 230, 240],
            Cond::Cold | Cond::Injury => [200, 235, 200],
            Cond::Death | Cond::Dying => [240, 200, 200],
            Cond::Dead => [90, 90, 90],
        };
        let restless = p.is_pending(pet::req::SOWASOWA);
        let color = p.def.color as usize;
        self.rect(0, 0, WIDTH, HEIGHT, tint);

        // Anchor = character's bottom centre.
        let (ax, ay) = (64 + if restless { 3 } else { 0 }, 104);
        let mut drawn = false;
        for part in self.pose() {
            let frame = if sleeping && part.kind == PART_EYES { EYES_SLEEP } else { part.frame };
            // The egg keeps its own palette; hatched characters take the colour palette.
            let pal = (part.kind == PART_BODY && stage > 1 && color < 16).then_some((COLOR_PALETTES, color));
            drawn |= self.blit(part.sprite, frame, ax + part.x as i32, ay + part.y as i32, part.alpha, pal);
        }
        let (cx, cy) = (ax, ay - 32);
        if !drawn {
            let size = 12 + stage * 8;
            self.rect((cx as usize).saturating_sub(size / 2), (cy as usize).saturating_sub(size / 2), size, size, [120, 120, 140]);
        }

        for i in 0..6 {
            let c = if i < onaka { [240, 160, 60] } else { [150, 150, 160] };
            self.rect(8 + i * 19, 6, 15, 8, c);
        }
        for i in 0..20 {
            let c = if i < gokigen { [240, 100, 160] } else { [150, 150, 160] };
            self.rect(4 + i * 6, 18, 5, 5, c);
        }
        for i in 0..unchi {
            self.rect(8 + i * 20, 112, 12, 10, [130, 90, 40]);
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
                g.press(Button::A);
            }
            (g.framebuffer().to_vec(), g.pet.def.id, g.pet.onaka, g.pet.osewamiss, g.pet.cold)
        };
        assert_eq!(run(), run());
    }
}
