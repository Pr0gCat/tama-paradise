//! Pure game logic: button input + wall-clock time in, framebuffer out.

pub mod pet;

use pet::{CharDef, Cond, Food, FoodKind, Pet};

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

/// Placeholder meal until item data is loaded: +1 hunger heart.
const MEAL: Food = Food { kind: FoodKind::Meal, group: 0, onaka: 1, gokigen: 0 };

pub struct Game {
    pub pet: Pet,
    /// RGBA8888, row-major.
    fb: Vec<u8>,
}

impl Game {
    pub fn new(def: CharDef, seed: u32) -> Self {
        let mut g = Game { pet: Pet::new(def, seed), fb: vec![0; WIDTH * HEIGHT * 4] };
        g.render();
        g
    }

    /// A = feed, B = toilet if restless else clean, C = medicine / wake / rescue.
    pub fn press(&mut self, b: Button) {
        let p = &mut self.pet;
        match b {
            Button::A => p.feed(MEAL),
            Button::B => {
                if !p.toilet() {
                    p.clean();
                }
            }
            Button::C => {
                let _ = p.medicine() || p.wake() || p.rescue();
            }
        }
        self.render();
    }

    /// Advance the pet to local wall-clock seconds `now` and redraw.
    pub fn advance_to(&mut self, now: u64) {
        self.pet.advance_to(now);
        self.render();
    }

    pub fn framebuffer(&self) -> &[u8] {
        &self.fb
    }

    fn rect(&mut self, x: usize, y: usize, w: usize, h: usize, c: [u8; 3]) {
        for yy in y..(y + h).min(HEIGHT) {
            for xx in x..(x + w).min(WIDTH) {
                let i = (yy * WIDTH + xx) * 4;
                self.fb[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
    }

    // ponytail: debug meters only; real sprites arrive with the asset pipeline (M2).
    fn render(&mut self) {
        let p = &self.pet;
        let (onaka, gokigen, unchi) = (p.onaka as usize, p.gokigen as usize, p.unchi.len());
        let sleeping = p.last.is_some_and(|t| p.is_sleeping_at(t));
        let body = match p.cond {
            Cond::Normal if sleeping => [80, 80, 160],
            Cond::Normal => [120, 200, 240],
            Cond::Cold | Cond::Injury => [120, 200, 120],
            Cond::Death | Cond::Dying => [200, 120, 120],
            Cond::Dead => [90, 90, 90],
        };
        let restless = p.is_pending(pet::req::SOWASOWA);
        self.rect(0, 0, WIDTH, HEIGHT, [24, 24, 32]);
        // Pet: a square that jitters when restless.
        let off = if restless { 4 } else { 0 };
        self.rect(44 + off, 44, 40, 40, body);
        // Hunger: 6 hearts on top.
        for i in 0..6 {
            let c = if i < onaka { [240, 160, 60] } else { [60, 60, 70] };
            self.rect(8 + i * 19, 8, 15, 10, c);
        }
        // Mood: 20 small cells.
        for i in 0..20 {
            let c = if i < gokigen { [240, 100, 160] } else { [60, 60, 70] };
            self.rect(4 + i * 6, 24, 5, 6, c);
        }
        // Poop along the bottom.
        for i in 0..unchi {
            self.rect(8 + i * 20, 104, 14, 14, [130, 90, 40]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KID: CharDef = CharDef { id: 2113, stage: 3, hungry_min: 45, happy_min: 20, disease: 40, likes: 6, dislikes: 0 };

    #[test]
    fn deterministic() {
        let run = || {
            let mut g = Game::new(KID, 3);
            g.advance_to(86_400 + 12 * 3600);
            g.press(Button::A);
            g.advance_to(10 * 86_400);
            (g.framebuffer().to_vec(), g.pet.onaka, g.pet.osewamiss, g.pet.cold)
        };
        assert_eq!(run(), run());
    }
}
