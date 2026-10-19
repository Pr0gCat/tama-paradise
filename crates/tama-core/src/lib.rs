//! Pure game logic: button input + wall-clock time in, framebuffer out.

pub mod data;
mod evolve;
pub mod pet;

use data::{Data, KIND_MEAL};
use pet::{Cond, Pet};

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
    /// RGBA8888, row-major.
    fb: Vec<u8>,
}

impl Game {
    /// Start a new game with an egg in the device's home area.
    pub fn new(data: Data, seed: u32) -> Self {
        let mut pet = Pet::new(data.character(1000).expect("egg 1000 in data"), seed);
        pet.field = data::attr_to_field(data.rom_type).unwrap_or(0);
        let mut g = Game { data, pet, fb: vec![0; WIDTH * HEIGHT * 4] };
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
        let (onaka, gokigen, unchi, stage) = (p.onaka as usize, p.gokigen as usize, p.unchi.len(), p.def.stage as usize);
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
        // Pet: size grows with stage; jitters when restless.
        let size = 12 + stage * 8;
        let off = if restless { 4 } else { 0 };
        self.rect(64 - size / 2 + off, 64 - size / 2, size, size, body);
        for i in 0..6 {
            let c = if i < onaka { [240, 160, 60] } else { [60, 60, 70] };
            self.rect(8 + i * 19, 8, 15, 10, c);
        }
        for i in 0..20 {
            let c = if i < gokigen { [240, 100, 160] } else { [60, 60, 70] };
            self.rect(4 + i * 6, 24, 5, 6, c);
        }
        for i in 0..unchi {
            self.rect(8 + i * 20, 108, 14, 14, [130, 90, 40]);
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
