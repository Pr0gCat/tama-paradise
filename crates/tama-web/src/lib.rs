use tama_core::data::Data;
use tama_core::Game;
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

    /// Restore from a string made by `save()`. Advance afterwards to catch up offline time.
    pub fn load(data: &str, save: &str) -> Result<Tama, JsError> {
        let data = Data::from_json(data).map_err(|e| JsError::new(&e.to_string()))?;
        Game::load(data, save).map(Tama).map_err(|e| JsError::new(&e.to_string()))
    }

    pub fn save(&self) -> String {
        self.0.save()
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

    /// One 20 fps frame. `held`: bit0 A, bit1 B, bit2 C, bit3 dial push, bit6 dial down, bit7 dial up.
    pub fn frame(&mut self, now: f64, held: u8) {
        self.0.frame(now as u64, held);
    }

    /// Local wall-clock time in seconds (catch-up without input, e.g. after loading).
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

    /// Sprite indexes to fetch (`sprites/<id>.tsp.gz`) and pass to `add_sprite`.
    pub fn missing_sprites(&self) -> Vec<u16> {
        self.0.missing_sprites()
    }

    /// `bytes` = decompressed .tsp file.
    pub fn add_sprite(&mut self, id: u16, bytes: &[u8]) -> bool {
        self.0.add_sprite(id, bytes)
    }

    /// Sounds started since the last call, as `id << 8 | volume`.
    pub fn take_sounds(&mut self) -> Vec<u32> {
        self.0.take_sounds().into_iter().map(|(id, vol)| (id as u32) << 8 | vol as u32).collect()
    }

    pub fn friend_code(&self) -> String {
        self.0.friend_code()
    }

    pub fn playdate(&mut self, code: &str, marry: bool) -> Result<(), JsError> {
        self.0.playdate(code, marry).map_err(JsError::new)
    }

    /// Gift code, or empty if the item is not owned.
    pub fn gift(&mut self, item: u16) -> String {
        self.0.gift(item).unwrap_or_default()
    }

    /// Item id received, or 0 if the code is invalid or already used.
    pub fn receive_gift(&mut self, code: &str) -> u16 {
        self.0.receive_gift(code).unwrap_or(0)
    }

    /// Inventory as JSON `[[id, count, name], ...]`.
    pub fn inventory(&self) -> String {
        let items: Vec<String> = self.0.inventory().into_iter().map(|(id, n, name)| format!("[{id},{n},{name:?}]")).collect();
        format!("[{}]", items.join(","))
    }

    /// Copy of the RGBA framebuffer.
    pub fn framebuffer(&self) -> Vec<u8> {
        self.0.framebuffer().to_vec()
    }
}
