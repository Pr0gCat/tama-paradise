use tama_core::{Button, Game};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Tama(Game);

#[wasm_bindgen]
impl Tama {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Tama {
        Tama(Game::new())
    }

    pub fn width() -> usize {
        tama_core::WIDTH
    }

    pub fn height() -> usize {
        tama_core::HEIGHT
    }

    pub fn tick_ms() -> u32 {
        tama_core::TICK_MS
    }

    /// 0 = A, 1 = B, 2 = C.
    pub fn press(&mut self, b: u8) {
        let b = match b {
            0 => Button::A,
            1 => Button::B,
            _ => Button::C,
        };
        self.0.press(b);
    }

    pub fn step(&mut self, n: u32) {
        self.0.step(n);
    }

    /// Copy of the RGBA framebuffer (64 KB at 128x128; zero-copy view if it ever matters).
    pub fn framebuffer(&self) -> Vec<u8> {
        self.0.framebuffer().to_vec()
    }
}
