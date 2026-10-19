//! Game data loaded at runtime (`data/game.json`, produced by `tama-extract`).

use serde::{Deserialize, Serialize};

/// Character definition record (asset data table 11, 15 × u16). See game-spec §6.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharDef {
    pub id: u16,
    pub chr_id: u16,
    pub name_id: u16,
    pub stage: u8,
    pub species: u16,
    pub rank: u8,
    /// Liked food group (ItemGrp).
    pub likes: u8,
    /// Home area 1–6: land, water, sky, forest, tropical, frozen.
    pub attribute: u8,
    pub gender: u8,
    /// Minutes per hunger heart lost; 0 = never.
    pub hungry_min: u16,
    /// Minutes per mood heart lost; 0 = never.
    pub happy_min: u16,
    /// % chance of a cold on each new care mistake.
    pub disease: u8,
    /// Disliked food group (named EvolBranch in the firmware).
    pub dislikes: u8,
    /// Default colour 0–15.
    pub color: u8,
}

/// Item record (asset data table 10, 23 × u16). Only the fields the game logic uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub id: u16,
    /// 1 meal, 2 snack, 3/4 toys, 5 hat, 6 glasses, 7 accessory, 8 planet.
    pub kind: u8,
    /// 0xffff = not for sale.
    pub buy: u16,
    pub sell: u16,
    /// Food category 1–18 (feeds the evolution food log).
    pub category: u8,
    /// Planet rank needed to show in the shop.
    pub rank: u8,
    /// Compared with the character's liked/disliked group.
    pub group: u8,
    /// Snack eaten by an adult gives +1 mood.
    pub snack_flag: u8,
    pub onaka: u8,
    /// Snack: mood gain. Meal: food-log points.
    pub gokigen: u8,
    /// Shop rotation day 1–5, 0 = every day.
    pub day_group: u8,
    pub flags: u16,
}

pub const KIND_MEAL: u8 = 1;
pub const KIND_SNACK: u8 = 2;

/// Evolution targets. Indexes: field 0 tropical, 1 frozen, 2 forest; b = food branch; q = care quality.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evolution {
    /// Baby → child by dominant field (code constant at `tama_evolve` 0x1000fc98).
    pub kids: [u16; 3],
    /// `[field][b]`, XIP 0x10091314.
    pub teens: [[u16; 4]; 3],
    /// `[field][b][q]`, XIP 0x1009132c.
    pub adults: [[[u16; 4]; 4]; 3],
    /// Rare adult by field.
    pub rare: [u16; 3],
    /// Adult for a baby that never settled or a child that ran past 28 h.
    pub failed: u16,
}

/// One sprite of a character pose (keyframe, data table 15; see game-spec §8).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Part {
    pub sprite: u16,
    pub frame: u16,
    /// Sprite centre relative to the character anchor (bottom centre), pixels.
    pub x: i16,
    pub y: i16,
    /// 0 body, 1 eyes, 15 mouth, 17 shadow, …
    pub kind: u16,
    /// 0–64, 64 = opaque.
    pub alpha: u8,
}

pub const PART_BODY: u16 = 0;
pub const PART_EYES: u16 = 1;
pub const PART_MOUTH: u16 = 15;

/// Character model (data table 18): poses by animation number, only those the game uses.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Model {
    pub chr_id: u16,
    /// Idle (anim 1) and idle alternate (anim 2).
    pub idle: [Vec<Part>; 2],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Data {
    /// Device edition (flash 0xd29000): 1–6. Glacier = 6.
    pub rom_type: u8,
    pub characters: Vec<CharDef>,
    pub items: Vec<Item>,
    pub evolution: Evolution,
    #[serde(default)]
    pub models: Vec<Model>,
}

impl Data {
    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }

    pub fn character(&self, id: u16) -> Option<CharDef> {
        self.characters.iter().find(|c| c.id == id).copied()
    }

    pub fn model(&self, chr_id: u16) -> Option<&Model> {
        self.models.iter().find(|m| m.chr_id == chr_id)
    }

    pub fn item(&self, id: u16) -> Option<Item> {
        self.items.iter().find(|i| i.id == id).copied()
    }
}

/// `romtype_to_field` 0x1002b314 / `attr_to_field` 0x100039d4: edition or home area → field.
pub fn attr_to_field(a: u8) -> Option<u8> {
    match a {
        1 | 5 => Some(0),
        2 | 6 => Some(1),
        3 | 4 => Some(2),
        _ => None,
    }
}
