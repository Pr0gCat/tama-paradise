use tama_core::data::Data;
use tama_core::Game;
use wasm_bindgen::prelude::*;

/// QR code for `text`: first byte = width in modules, then one byte per module (1 = dark).
#[wasm_bindgen]
pub fn qr(text: &str) -> Vec<u8> {
    let Ok(code) = qrcode::QrCode::new(text.as_bytes()) else { return Vec::new() };
    let w = code.width();
    std::iter::once(w as u8).chain(code.to_colors().into_iter().map(|c| (c == qrcode::Color::Dark) as u8)).collect()
}

/// The player's own flash dump: the page reads the game data and sprites from it instead of
/// shipping them.
#[wasm_bindgen]
pub struct Dump(Vec<u8>);

#[wasm_bindgen]
impl Dump {
    #[wasm_bindgen(constructor)]
    pub fn new(bytes: Vec<u8>) -> Result<Dump, JsError> {
        if !tama_extract::is_dump(&bytes) {
            return Err(JsError::new("not a Tamagotchi Paradise flash dump"));
        }
        Ok(Dump(bytes))
    }

    /// What tama-extract writes to data/game.json.
    pub fn game_json(&self) -> String {
        serde_json::to_string(&tama_extract::extract(&self.0)).unwrap_or_default()
    }

    /// Sprite `id` as a .tsp file (empty if the dump has none).
    pub fn sprite(&self, id: usize) -> Vec<u8> {
        if id >= tama_extract::sprite_count(&self.0) {
            return Vec::new();
        }
        tama_extract::sprite(&self.0, id).map(|s| s.to_bytes()).unwrap_or_default()
    }
}

thread_local! {
    /// The published build (commit) for GAME INFO, kept for every game made after it is known.
    static BUILD: std::cell::RefCell<String> = std::cell::RefCell::new("DEV".into());
}

/// Set the build shown in GAME INFO (this and later games).
#[wasm_bindgen]
pub fn set_build(build: &str) {
    BUILD.with(|b| *b.borrow_mut() = build.into());
}

fn with_build(mut game: Game) -> Game {
    BUILD.with(|b| game.set_build(&b.borrow()));
    game
}

#[wasm_bindgen]
pub struct Tama(Game);

#[wasm_bindgen]
impl Tama {
    /// `data` is the contents of data/game.json.
    #[wasm_bindgen(constructor)]
    pub fn new(data: &str, seed: u32) -> Result<Tama, JsError> {
        let data = Data::from_json(data).map_err(|e| JsError::new(&e.to_string()))?;
        let mut game = with_build(Game::new(data, seed));
        game.begin_intro();
        Ok(Tama(game))
    }

    /// Restore from a string made by `save()`. Advance afterwards to catch up offline time.
    pub fn load(data: &str, save: &str) -> Result<Tama, JsError> {
        let data = Data::from_json(data).map_err(|e| JsError::new(&e.to_string()))?;
        Game::load(data, save).map(|g| Tama(with_build(g))).map_err(|e| JsError::new(&e.to_string()))
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

    /// While the page is hidden: advance the pet and return the request id of a care call that
    /// just started (0x0f HOME, 0x11 hunger, 0x12 mood, 0x0a cold, 0x13 evolution support,
    /// 0x1c DODODO), to show as a notification.
    pub fn poll_call(&mut self, now: f64) -> Option<u8> {
        self.0.poll_call(now as u64)
    }

    /// Local wall-clock time in seconds (catch-up without input, e.g. after loading).
    pub fn advance_to(&mut self, now: f64) {
        self.0.advance_to(now as u64);
    }

    /// The cold-start DELETE menu (CONTINUE / RESET ALL).
    pub fn begin_delete(&mut self) {
        self.0.begin_delete();
    }

    /// SETTINGS → TOUCH: swipe / tap on the screen turns and presses the dial.
    pub fn touch(&self) -> bool {
        self.0.touch()
    }

    /// The screen takes typed text (raise a keyboard; pass keys to `type_key`).
    pub fn text_entry(&self) -> bool {
        self.0.text_entry()
    }

    /// A `KeyboardEvent.key` typed into the text on screen; false if not used.
    pub fn type_key(&mut self, key: &str) -> bool {
        self.0.type_key(key)
    }

    pub fn set_build(&mut self, build: &str) {
        self.0.set_build(build);
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

    /// In-game CONNECTION request: "", "playdate", "receive" or "gift:<code>".
    pub fn take_connect(&mut self) -> String {
        self.0.take_connect().unwrap_or_default()
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
        let items: Vec<String> =
            self.0.inventory().into_iter().map(|(id, n, name)| format!("[{id},{n},{name:?}]")).collect();
        format!("[{}]", items.join(","))
    }

    /// Copy of the RGBA framebuffer.
    pub fn framebuffer(&self) -> Vec<u8> {
        self.0.framebuffer().to_vec()
    }
}
