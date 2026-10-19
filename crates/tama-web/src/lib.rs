use tama_core::pet::CharDef;
use tama_core::{Button, Game};
use wasm_bindgen::prelude::*;

/// Frozen kid (2113) from docs/spec/data/characters.csv, until data files are loaded.
const FROZEN_KID: CharDef = CharDef { id: 2113, stage: 3, hungry_min: 45, happy_min: 20, disease: 40, likes: 0, dislikes: 0 };

#[wasm_bindgen]
pub struct Tama(Game);

#[wasm_bindgen]
impl Tama {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u32) -> Tama {
        let mut g = Game::new(FROZEN_KID, seed);
        g.pet.onaka = 6;
        g.pet.gokigen = 20;
        Tama(g)
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

    /// Local wall-clock time in seconds.
    pub fn advance_to(&mut self, now: f64) {
        self.0.advance_to(now as u64);
    }

    /// One-line pet status for the page.
    pub fn status(&self) -> String {
        let p = &self.0.pet;
        format!(
            "{:?} onaka {} gokigen {} poop {} miss {} cold {} days {}",
            p.cond,
            p.onaka,
            p.gokigen,
            p.unchi.len(),
            p.osewamiss,
            p.cold,
            p.days
        )
    }

    /// Copy of the RGBA framebuffer.
    pub fn framebuffer(&self) -> Vec<u8> {
        self.0.framebuffer().to_vec()
    }
}
