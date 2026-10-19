//! Versioned save data. JSON so it can be inspected and migrated by hand.

use crate::data::Data;
use crate::pet::Pet;
use crate::Game;
use serde::{Deserialize, Serialize};

/// Bump when `Pet` changes shape, and add a migration in `Game::load`.
pub const SAVE_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct Save {
    version: u32,
    pet: Pet,
}

#[derive(Debug)]
pub enum LoadError {
    Json(serde_json::Error),
    Version(u32),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            LoadError::Json(e) => write!(f, "bad save: {e}"),
            LoadError::Version(v) => write!(f, "unsupported save version {v} (expected {SAVE_VERSION})"),
        }
    }
}

impl Game {
    pub fn save(&self) -> String {
        serde_json::to_string(&Save { version: SAVE_VERSION, pet: self.pet.clone() }).unwrap()
    }

    /// Restore a save. Call `advance_to(now)` afterwards to catch up the time the page was closed.
    pub fn load(data: Data, json: &str) -> Result<Game, LoadError> {
        let v: serde_json::Value = serde_json::from_str(json).map_err(LoadError::Json)?;
        let version = v.get("version").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        if version != SAVE_VERSION {
            return Err(LoadError::Version(version));
        }
        let save: Save = serde_json::from_value(v).map_err(LoadError::Json)?;
        let mut pet = save.pet;
        // Character numbers come from the data file, so data changes apply to existing saves.
        if let Some(def) = data.character(pet.def.id) {
            pet.def = def;
        }
        Ok(Game::with_pet(data, pet))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evolve::tests::data;
    use crate::Button;

    const NOON: u64 = 86_400 + 12 * 3600;

    fn game() -> Game {
        let mut g = Game::new(data(), 9);
        g.advance_to(NOON);
        g.advance_to(NOON + 3 * 3600);
        g.press(Button::A);
        g
    }

    #[test]
    fn roundtrip() {
        let g = game();
        let back = Game::load(data(), &g.save()).unwrap();
        assert_eq!(back.pet, g.pet);
    }

    #[test]
    fn closed_page_catches_up_like_staying_open() {
        let mut open = game();
        let mut closed = Game::load(data(), &open.save()).unwrap();
        let t = open.pet.last.unwrap();
        // Pet level: Game also completes evolution when the pet is on screen, which a closed page can't.
        for s in (60..=5 * 3600).step_by(60) {
            open.pet.advance_to(t + s);
        }
        closed.pet.advance_to(t + 5 * 3600);
        assert_eq!(closed.pet, open.pet);
    }

    #[test]
    fn rejects_unknown_version() {
        let json = game().save().replacen("\"version\":1", "\"version\":99", 1);
        assert!(matches!(Game::load(data(), &json), Err(LoadError::Version(99))));
    }

    #[test]
    fn character_numbers_come_from_current_data() {
        let g = game();
        let mut d = data();
        let id = g.pet.def.id;
        d.characters.iter_mut().find(|c| c.id == id).unwrap().hungry_min = 77;
        assert_eq!(Game::load(d, &g.save()).unwrap().pet.def.hungry_min, 77);
    }
}
