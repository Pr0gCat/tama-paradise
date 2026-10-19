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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub id: u16,
    /// Name string id.
    #[serde(default)]
    pub name: u16,
    /// Sprite shown while eating (frames: full, half, empty) and in lists.
    #[serde(default)]
    pub eat_sprite: u16,
    #[serde(default)]
    pub icon_sprite: u16,
    /// Outside toys: idle display animation (item +8) and play animation (+0xa).
    #[serde(default)]
    pub show_anim: u16,
    #[serde(default)]
    pub play_anim: u16,
    /// Drink (item +0x14 bit0): uses the drinking animations.
    #[serde(default)]
    pub drink: bool,
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
    /// Mirrored horizontally (keyframe [9] bit0).
    #[serde(default)]
    pub flip: bool,
}

pub const PART_BODY: u16 = 0;
pub const PART_EYES: u16 = 1;
pub const PART_MOUTH: u16 = 15;
/// Dirt splotches, shown on the player's pet while DIRTY.
pub const PART_DIRT: u16 = 0x11;

/// Character model (data table 18): 54 static poses by animation number (game-spec §8).
/// 1–20 large close-up poses, 30–53 small field poses.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Model {
    pub chr_id: u16,
    pub anims: Vec<Vec<Part>>,
}

/// Bitmap font (data tables 0/1): glyph code c = frame c of `sprite`, width = advance + adj[c].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Font {
    pub sprite: u16,
    pub advance: i16,
    pub line_height: i16,
    pub adj: Vec<i16>,
}

/// UI element (data tables 4/5, `ui_element_def_get` 0x10023124): kind 1 = sprite
/// (p[3] sprite, p[6] alpha, p[7] frame, p[8] palette), 2 = text (p[3] string), 5 = animation (p[3] script).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiElem {
    pub kind: u8,
    pub x: i16,
    pub y: i16,
    pub p: [i16; 11],
}

pub const UI_SPRITE: u8 = 1;
pub const UI_TEXT: u8 = 2;
pub const UI_ANIM: u8 = 5;

/// Buzzer melody (table XIP 0x100a0a28): notes `[steps, period µs (0 = rest), duty %]`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Melody {
    pub ms_per_step: u16,
    pub notes: Vec<[u16; 3]>,
}

pub const LANGS: [&str; 9] = ["ja", "en", "fr", "de", "pt", "es", "it", "zh", "ko"];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Data {
    /// Device edition (flash 0xd29000): 1–6. Glacier = 6.
    pub rom_type: u8,
    pub characters: Vec<CharDef>,
    pub items: Vec<Item>,
    pub evolution: Evolution,
    #[serde(default)]
    pub models: Vec<Model>,
    /// Motion scripts (data tables 13/14): raw u16 words, word 0 = (count << 1) | loop.
    #[serde(default)]
    pub motions: Vec<Vec<u16>>,
    /// HOME state → {field, close-up, cell} motion script (XIP 0x1009dbd8, 32 states).
    #[serde(default)]
    pub home_motions: Vec<[u16; 3]>,
    #[serde(default)]
    pub fonts: Vec<Font>,
    /// `strings[lang][id]` = glyph codes (see `LANGS`). 0xF000 = newline.
    #[serde(default)]
    pub strings: Vec<Vec<Vec<u16>>>,
    #[serde(default)]
    pub ui: Vec<UiElem>,
    /// Field background UI elements `[field][variant][1 + period*3 + weather]` (XIP 0x10092e06).
    #[serde(default)]
    pub field_bg: Vec<[[u16; 16]; 2]>,
    #[serde(default)]
    pub melodies: Vec<Melody>,
    /// Shop codes: (item unlocked, 8-digit code), XIP 0x100a1a20.
    #[serde(default)]
    pub shop_codes: Vec<(u16, String)>,
    /// Cooking: (product, material needed ×3), XIP 0x10091fbc.
    #[serde(default)]
    pub recipes: Vec<(u16, u16)>,
    /// Weather: per-month chances of patterns 0–4 (rest = 5), XIP 0x10091b5c; and per pattern the
    /// weather (0 clear, 1 cloudy, 2 rain/snow) for each time-of-day period, XIP 0x1009138c.
    #[serde(default)]
    pub weather_months: Vec<[u16; 6]>,
    #[serde(default)]
    pub weather_patterns: Vec<[u8; 5]>,
    /// Egg hunt finds `[sprite, item, collection bit]`, index field*8 + kind*4 + 0..3;
    /// entry 24 is the GOLDEN PEACH bonus (XIP 0x100a0600).
    #[serde(default)]
    pub hunt_finds: Vec<[u16; 3]>,
    #[serde(default)]
    pub mini: MiniData,
    #[serde(default)]
    pub decor: DecorData,
}

/// Outside toys, planet and travel (docs/spec/10-decor-planet.md).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecorData {
    /// HOME field anchors `[field][area][role]` = (x, y, z) in world coordinates:
    /// roles 0–5 toy slots, 6 pet, 7 egg, 8 visitor (XIP 0x100933ae).
    pub home_pos: Vec<[[(i16, i16, i16); 9]; 2]>,
    /// Planet ChrID by `[romtype class][rank 0..10]` (0x10094786 / 0x100947b2 / 0x1009479c).
    pub planet_chr: Vec<[u16; 11]>,
    /// Matching outfit sets: hat, glasses, accessory (0 = any), visitor A, visitor B (0x100947c8).
    pub equip_sets: Vec<[u16; 5]>,
    /// Travel destinations: (item, rank needed, animation, souvenirs A/B, local NPC ChrIDs).
    pub travel: Vec<Destination>,
    /// Planet-level goal text per rank (0x100a138e).
    pub rank_goals: Vec<u16>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Destination {
    pub item: u16,
    pub rank: u8,
    pub anim: u16,
    pub souvenirs: [u16; 2],
    pub npcs: [u16; 2],
}

/// Fixed mini-game layouts (docs/spec/09-minigames-*.md).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MiniData {
    /// Game 00: `[level*3 + n]` boards of rows × 6 holes (non-zero = a baby wakes), 24 rows read.
    #[serde(default)]
    pub lab00_boards: Vec<Vec<[i16; 6]>>,
    /// Game 01: `[level*3 + n]` maps as platforms `[x, height above the start line, type]` in
    /// spawn order, ending with the goal platform (64, 6400, 0).
    #[serde(default)]
    pub lab01_maps: Vec<Vec<[i16; 3]>>,
    /// Game 02: `[level*5 + n]` card types of the 15 cards (0–6 twice, 7 = bomb).
    #[serde(default)]
    pub lab02_boards: Vec<[u8; 15]>,
    /// Game 03: `[level*2 + n]` layouts of 76 rows `[lane, kind]` (lane −1 = empty), XIP 0x10093984.
    #[serde(default)]
    pub lab03_layouts: Vec<Vec<[i8; 2]>>,
    /// Game 04: launch angle index `[level*3 + n][round]` (XIP 0x1009bb5c).
    #[serde(default)]
    pub lab04_launch: Vec<[u8; 5]>,
    /// Game 04: prize brick 1–15 `[level*5 + n][round]` (XIP 0x10091d30).
    #[serde(default)]
    pub lab04_prize_brick: Vec<[u8; 5]>,
    /// Game 04: prize character (ChrID) by play count (XIP 0x10093320).
    #[serde(default)]
    pub lab04_prize_chr: Vec<u16>,
    /// 1024-step sine, Q12 (XIP 0x1009a968), and tan for 12-bit angles 0..=0x400, Q12 (XIP 0x1009cbd4).
    #[serde(default)]
    pub sin_q12: Vec<i16>,
    #[serde(default)]
    pub tan_q12: Vec<i32>,
    /// Game 05: `[level*3 + n]` meteor tables of 40 rows × 3 lanes `[present, speed 0..2]`.
    #[serde(default)]
    pub lab05_layouts: Vec<Vec<[[u8; 2]; 3]>>,
    /// Game 05 meteor event table (SRAM 0x18000316).
    #[serde(default)]
    pub lab05_event: Vec<[[u8; 2]; 3]>,
    /// Game 06: `[level*3 + n]` courses of objects `[x distance, y, type]` (type 0 obstacle, 9 coin), sorted by x.
    #[serde(default)]
    pub lab06_courses: Vec<Vec<[i16; 3]>>,
    /// Arcade 02 (SHOOTING): 3 spawn tables of `[side << 14 | kind, delay]` rows (XIP 0x1009c4ec).
    #[serde(default)]
    pub arcade02_patterns: Vec<Vec<[u16; 2]>>,
    /// Play-together game 07: 3 fields of 19 rows × 18 cells ('w' wall, 'S' start, 'T' shirotama,
    /// 'G' bump creature, ' ' floor), XIP 0x1009c7a8.
    #[serde(default)]
    pub together07_maps: Vec<Vec<String>>,
    /// Together game 01 (FLAGS): 3 sequences × 10 commands `[feint 1..4, answer 1..4]` (XIP 0x100a10c2).
    #[serde(default)]
    pub together01_commands: Vec<[[u8; 2]; 10]>,
    /// Together game 04 (MATCHING): card values `[p][slot]` (0x10091ece) and the pet's slot `[q]` (0x10091f94).
    #[serde(default)]
    pub together04_cards: Vec<[u8; 3]>,
    #[serde(default)]
    pub together04_pet_slot: Vec<u8>,
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

    pub fn ui(&self, id: u16) -> UiElem {
        self.ui.get(id as usize).copied().unwrap_or_default()
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
