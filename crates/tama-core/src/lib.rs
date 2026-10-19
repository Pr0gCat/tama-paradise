//! Pure game logic: button input + elapsed ticks in, framebuffer out.

// Placeholder until the real LCD resolution is known from the dump.
pub const WIDTH: usize = 128;
pub const HEIGHT: usize = 128;
/// Fixed game step, decoupled from display FPS.
pub const TICK_MS: u32 = 50;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Button {
    A,
    B,
    C,
}

pub struct Game {
    tick: u64,
    presses: u32,
    /// RGBA8888, row-major.
    fb: Vec<u8>,
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}

impl Game {
    pub fn new() -> Self {
        let mut g = Game { tick: 0, presses: 0, fb: vec![0; WIDTH * HEIGHT * 4] };
        g.render();
        g
    }

    pub fn press(&mut self, _b: Button) {
        self.presses += 1;
    }

    /// Advance `n` ticks. Rendering happens once at the end, so offline catch-up stays cheap.
    pub fn step(&mut self, n: u32) {
        self.tick += n as u64;
        self.render();
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn framebuffer(&self) -> &[u8] {
        &self.fb
    }

    // Test pattern: scrolling gradient, tinted by press count.
    fn render(&mut self) {
        let t = self.tick as usize;
        let tint = (self.presses * 40) as u8;
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let i = (y * WIDTH + x) * 4;
                self.fb[i] = ((x + t) * 2) as u8;
                self.fb[i + 1] = (y * 2) as u8;
                self.fb[i + 2] = tint;
                self.fb[i + 3] = 255;
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
            let mut g = Game::new();
            g.step(3);
            g.press(Button::A);
            g.step(100);
            g.framebuffer().to_vec()
        };
        assert_eq!(run(), run());
    }
}
