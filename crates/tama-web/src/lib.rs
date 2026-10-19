use tama_core::data::Data;
use tama_core::{Button, Game};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Tama(Game);

#[wasm_bindgen]
impl Tama {
    /// `data` is the contents of data/game.json.
    #[wasm_bindgen(constructor)]
    pub fn new(data: &str, seed: u32) -> Result<Tama, JsError> {
        let data = Data::from_json(data).map_err(|e| JsError::new(&e.to_string()))?;
        Ok(Tama(Game::new(data, seed)))
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
            "#{} stage {} {:?}\nonaka {} gokigen {} poop {}\nmiss {} cold {} days {} pts {}",
            p.def.id,
            p.def.stage,
            p.cond,
            p.onaka,
            p.gokigen,
            p.unchi.len(),
            p.osewamiss,
            p.cold,
            p.days,
            p.points
        )
    }

    /// Copy of the RGBA framebuffer.
    pub fn framebuffer(&self) -> Vec<u8> {
        self.0.framebuffer().to_vec()
    }
}
