//! Pet care simulation, following docs/game-spec.md §4–5 (firmware `care_tick` 0x100134bc).
//!
//! Time is in seconds of local wall-clock time (`t % 86400` = time of day).
//! The platform supplies `now`; the simulation advances in ≤60 s chunks like the original.

use crate::data::{CharDef, Item, KIND_MEAL, KIND_SNACK};
use serde::{Deserialize, Serialize};

mod calendar;
mod care;
mod sim;

pub use calendar::{date, days_from_civil, EVENTS};

/// Request ids (EM_REQ_*), same numbering as the firmware.
pub mod req {
    pub const EVOLUTION: u8 = 0x01;
    pub const WAKE: u8 = 0x02;
    pub const SLEEP: u8 = 0x03;
    pub const OTEIRE: u8 = 0x05;
    pub const DEATH: u8 = 0x08;
    pub const DEATH_DELAY: u8 = 0x09;
    pub const COLD: u8 = 0x0a;
    pub const INJURY: u8 = 0x0b;
    pub const DYING: u8 = 0x0c;
    pub const DEAD: u8 = 0x0d;
    pub const SOWASOWA: u8 = 0x0e;
    pub const HOMETRG: u8 = 0x0f;
    pub const UNCHI: u8 = 0x10;
    pub const ONAKA: u8 = 0x11;
    pub const GOKIGEN: u8 = 0x12;
    pub const EVOL_SUPPORT: u8 = 0x13;
    pub const METAMORPHO: u8 = 0x14;
    pub const COLOR: u8 = 0x15;
    pub const LOVE: u8 = 0x16;
    pub const IMMUNITY: u8 = 0x17;
    pub const HAPPY: u8 = 0x18;
    pub const DOSSARI: u8 = 0x19;
    pub const NAP: u8 = 0x1a;
    pub const REDU_CM: u8 = 0x1b;
    pub const BIRTHDAY: u8 = 0x06;
    pub const NEWYEAR: u8 = 0x07;
    /// DODODO: the 13th of each month at 17:00 (rank ≥ 6); unanswered by 17:15 it turns bad.
    pub const DODODO: u8 = 0x1c;
    pub const METEOR: u8 = 0x1e;
    pub const SHOOTINGSTAR: u8 = 0x1f;
    pub const UFO: u8 = 0x20;
    pub const SANTA: u8 = 0x21;
    pub const EVE: u8 = 0x22;
    pub const XMAS: u8 = 0x23;
    /// Effects shown after eating, in the order the feeding screen completes them.
    pub const EFFECTS: [u8; 5] = [COLOR, LOVE, IMMUNITY, HAPPY, REDU_CM];
}

/// Status flags (`g_tama_status`).
pub mod status {
    pub const LOVE: u8 = 0x01;
    pub const IMMUNITY: u8 = 0x02;
    pub const HAPPY: u8 = 0x04;
    pub const DIRTY: u8 = 0x10;
    pub const DANCE: u8 = 0x20;
    pub const KIRAKIRA: u8 = 0x40;
}

pub const ONAKA_MAX: u8 = 6;
pub const GOKIGEN_MAX: u8 = 20;
pub const UNCHI_MAX: usize = 6;
pub const CALL_TIMEOUT: u32 = 900;
pub const SOWASOWA_DELAY: u32 = 20;
pub const UNCHI_OLD: u16 = 3600;
pub const DEATH_COUNTDOWN: u32 = 129_600;
pub const DEATH_RESCUE: u32 = 3540;
pub const DYING_TIME: u32 = 60;
pub const AGE_MAX: u32 = 1_209_600;
pub const CHUNK: u64 = 60;
pub const MAX_CHUNKS: u64 = 1440;
const DAY: u64 = 86_400;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cond {
    Normal,
    /// Left with the tama sitter (firmware cond 1): only the hunger timer runs, until 19:00.
    Sitter,
    Cold,
    Injury,
    Death,
    Dying,
    Dead,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeathCause {
    Illness,
    Hunger,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pet {
    pub def: CharDef,
    pub cond: Cond,
    pub status: u8,
    pub onaka: u8,
    pub gokigen: u8,
    pub onaka_timer: u32,
    pub gokigen_timer: u32,
    /// Times hunger/mood was topped up to full during child/teen (evolution quality).
    pub onaka_max: u8,
    pub gokigen_max: u8,
    pub age: u32,
    pub days: u16,
    pub day_secs: u32,
    pub hunger_drops: u8,
    pub sowasowa_timer: u32,
    /// Age of each poop, len = poop count.
    pub unchi: Vec<u16>,
    pub yogore: u8,
    pub clean_timer: u32,
    pub nap_timer: u32,
    pub osewamiss: u8,
    /// Logged mistakes used by evolution (0–6).
    pub miss_count: u8,
    pub cold: u8,
    pub death_remain: u32,
    pub death_by_cold: u32,
    pub death_by_hungry: u32,
    pub death_cause: DeathCause,
    /// Current area 0–2 (tropical, frozen, forest).
    pub field: u8,
    /// Awake seconds in each area (baby: logs every 900 s; child: metamorphosis at 3600 s).
    pub field_timer: [u32; 3],
    /// Baby area log (≤ 11); index 0 is the child's home area after evolving.
    pub field_log: Vec<u8>,
    pub support_timer: u32,
    pub support_miss: u8,
    /// Food points per category 1–18; every 5 pushes the category into `food_log`.
    pub food_pts: [u8; 18],
    /// Last 4 native-food categories (0-based) eaten before the teen stage.
    pub food_log: Vec<u8>,
    /// ROM types met over link during the teen stage (rare evolution).
    pub comm: [u8; 2],
    pub points: u32,
    pub generation: u8,
    /// Body colour 0–15; 0x1f = the character's default (`g_tama_color`).
    #[serde(default = "no_color")]
    pub color: u8,
    /// Colour a juice will apply when its COLOR effect completes.
    #[serde(default = "no_color")]
    pub next_color: u8,
    /// Planet rank 0–10 (05-content §2.6) and the last rank shown to the player.
    #[serde(default)]
    pub rank: u8,
    #[serde(default)]
    pub rank_seen: u8,
    /// Mission counters / collection bits (`progress::TARGETS`) and the ones paid out.
    #[serde(default)]
    pub missions: [u32; 22],
    #[serde(default)]
    pub claimed: u32,
    /// Completed missions whose page has been opened (0x18000ce0; unseen ones show "NEW").
    #[serde(default)]
    pub missions_viewed: u32,
    /// Items unlocked with shop codes, and the codes already used.
    #[serde(default)]
    pub unlocked: Vec<u16>,
    #[serde(default)]
    pub used_codes: Vec<String>,
    /// Expansion areas bought, bit per field.
    #[serde(default)]
    pub areas: u8,
    /// Today's weather pattern 0–5 and whether HOME shows the field's expansion area.
    #[serde(default)]
    pub weather: u8,
    #[serde(default)]
    pub weather_day: u64,
    #[serde(default)]
    pub in_area2: bool,
    /// Tamas released to each field (grazing), graves, breeding state.
    #[serde(default)]
    pub grazing: [Vec<crate::family::Ghost>; 3],
    #[serde(default)]
    pub graves: Vec<crate::family::Grave>,
    #[serde(default)]
    pub adults_raised: [u8; 3],
    #[serde(default)]
    pub breeds_today: u8,
    #[serde(default)]
    pub seen_partners: Vec<u16>,
    /// Social: this device's id, the current character's id, friends and gift bookkeeping.
    #[serde(default)]
    pub device_uid: u32,
    #[serde(default)]
    pub chara_uid: u32,
    #[serde(default)]
    pub friends: Vec<crate::social::Friend>,
    #[serde(default)]
    pub gifts_sent: u16,
    #[serde(default)]
    pub gifts_received: Vec<String>,
    /// Looks inherited from a breeding partner (drawn instead of the own character's eyes).
    #[serde(default)]
    pub looks: Option<u16>,
    /// Outside toys `[field][area][slot]` (0 = empty), planet outfits (hat, glasses, accessory),
    /// and which souvenir each destination gives next.
    #[serde(default)]
    pub toys: [[[u16; 6]; 2]; 3],
    #[serde(default)]
    pub equip: [u16; 3],
    #[serde(default)]
    pub travel_toggle: u8,
    /// Times each lab game was played (picks its fixed layouts in turn).
    #[serde(default)]
    pub mini_plays: [u8; 7],
    /// Play-together cycle counters by game number 0–7 (09-minigames-together C10).
    #[serde(default)]
    pub together_plays: [u8; 8],
    /// The player's birthday (month, day), set in SETTINGS (`DAT_18000c6a`).
    #[serde(default)]
    pub birthday: Option<(u8, u8)>,
    /// New-game setup date (month, day) and days played since (USER INFO, `c0c` / `c98`).
    #[serde(default)]
    pub start_date: Option<(u8, u8)>,
    #[serde(default)]
    pub play_days: u16,
    /// Planet name as font glyph codes (`0x18000c58`, up to 8), set in SETTINGS.
    #[serde(default)]
    pub planet_name: Vec<u16>,
    /// Language (`data::LANGS` index; the firmware keeps it in 0x18000c6a bits 9–12).
    #[serde(default = "english")]
    pub lang: u8,
    /// SETTINGS (`0x18000bec`): sound level 0–3, date order 0–2, and the remake's offset from
    /// the platform clock set under DATE/TIME (seconds). Brightness is not kept: always full.
    #[serde(default = "two")]
    pub volume: u8,
    #[serde(default)]
    pub date_order: u8,
    #[serde(default)]
    pub clock_offset: i64,
    /// The remake's touch controls on the screen.
    #[serde(default = "yes")]
    pub touch: bool,
    /// The new-game setup (language, birthday, planet name) is finished; a page reloaded in the
    /// middle of it starts it again.
    #[serde(default = "yes")]
    pub set_up: bool,
    /// Egg-hunt slots used today per field (13 bits each).
    #[serde(default)]
    pub hunt_used: [u32; 3],
    /// UNCHI METER (0x18000d13), 0–60.
    #[serde(default)]
    pub unchi_meter: u8,
    /// Inventory: (item id, count).
    #[serde(default)]
    pub inventory: Vec<(u16, u8)>,
    /// The player answered NO to the wake / sleep prompt today (cleared at day change).
    #[serde(default)]
    pub no_wake_today: bool,
    #[serde(default)]
    pub no_sleep_today: bool,
    /// MICROSCOPE: lab-code characters found (bit = code index, `0x18001684`), and today's
    /// NO DATA bonus taken (`0x18000be9` bit0, cleared at day change).
    #[serde(default)]
    pub lab_cells: u64,
    #[serde(default)]
    pub lab_bonus: bool,
    raised: u64,
    done: u64,
    pub(crate) rng: u32,
    /// Last processed time; `None` until the first `advance_to`.
    pub last: Option<u64>,
}

pub const NO_COLOR: u8 = 0x1f;

fn english() -> u8 {
    1
}

fn yes() -> bool {
    true
}

fn two() -> u8 {
    2
}

fn no_color() -> u8 {
    NO_COLOR
}

enum ColdSource {
    Poop,
    Mistake,
}

impl Pet {
    pub fn new(def: CharDef, seed: u32) -> Self {
        Pet {
            def,
            cond: Cond::Normal,
            status: 0,
            onaka: 0,
            gokigen: 0,
            onaka_timer: 0,
            gokigen_timer: 0,
            onaka_max: 0,
            gokigen_max: 0,
            age: 0,
            days: 0,
            day_secs: 0,
            hunger_drops: 0,
            sowasowa_timer: 0,
            unchi: Vec::new(),
            yogore: 0,
            clean_timer: 0,
            nap_timer: 0,
            osewamiss: 0,
            miss_count: 0,
            cold: 0,
            death_remain: 0,
            death_by_cold: 0,
            death_by_hungry: 0,
            death_cause: DeathCause::Hunger,
            field: 0,
            field_timer: [0; 3],
            field_log: Vec::new(),
            support_timer: 0,
            support_miss: 0,
            food_pts: [0; 18],
            food_log: Vec::new(),
            comm: [0; 2],
            points: 0,
            generation: 1,
            color: NO_COLOR,
            next_color: NO_COLOR,
            rank: 0,
            rank_seen: 0,
            missions: [0; 22],
            claimed: 0,
            missions_viewed: 0,
            unlocked: Vec::new(),
            used_codes: Vec::new(),
            areas: 0,
            weather: 0,
            weather_day: 0,
            in_area2: false,
            hunt_used: [0; 3],
            mini_plays: [0; 7],
            together_plays: [0; 8],
            birthday: None,
            start_date: None,
            play_days: 0,
            planet_name: Vec::new(),
            lang: 1,
            volume: 2,
            date_order: 0,
            clock_offset: 0,
            touch: true,
            set_up: true,
            toys: [[[0; 6]; 2]; 3],
            equip: [0; 3],
            travel_toggle: 0,
            grazing: Default::default(),
            graves: Vec::new(),
            adults_raised: [0; 3],
            breeds_today: 0,
            seen_partners: Vec::new(),
            looks: None,
            device_uid: seed.rotate_left(13) ^ 0x5a5a_1234,
            chara_uid: seed.rotate_left(7) ^ 0x1357_9bdf,
            friends: Vec::new(),
            gifts_sent: 0,
            gifts_received: Vec::new(),
            unchi_meter: 0,
            inventory: Vec::new(),
            lab_cells: 0,
            lab_bonus: false,
            no_wake_today: false,
            no_sleep_today: false,
            raised: 0,
            done: 0,
            rng: seed | 1,
            last: None,
        }
    }

    // ---- requests -------------------------------------------------------

    pub fn was_raised(&self, id: u8) -> bool {
        self.raised & (1 << id) != 0
    }

    pub fn is_done(&self, id: u8) -> bool {
        self.done & (1 << id) != 0
    }

    pub fn is_pending(&self, id: u8) -> bool {
        self.was_raised(id) && !self.is_done(id)
    }

    pub(crate) fn clear(&mut self, id: u8) {
        self.raised &= !(1 << id);
        self.done &= !(1 << id);
    }

    /// `req_raise` 0x10069218 with its side effects.
    pub(crate) fn raise(&mut self, id: u8) {
        match id {
            req::COLD => {
                if !self.is_done(id) {
                    if self.death_by_cold == 0 {
                        self.death_by_cold = DEATH_COUNTDOWN;
                    }
                    self.cond = Cond::Cold;
                    self.status &= status::DIRTY;
                }
            }
            req::INJURY => {
                self.cond = Cond::Injury;
                self.status &= status::DIRTY;
            }
            req::DEATH => {
                self.clear(req::COLD);
                self.clear(req::INJURY);
                self.raise(req::HOMETRG);
                self.death_remain = DEATH_RESCUE;
                self.cond = Cond::Death;
                self.status &= status::DIRTY;
            }
            req::DYING => {
                self.clear(req::DEATH);
                self.death_remain = DYING_TIME;
                self.cond = Cond::Dying;
            }
            req::DEAD => {
                self.clear(req::DYING);
                self.cond = Cond::Dead;
                self.add_grave();
            }
            req::ONAKA if self.death_by_hungry == 0 => {
                self.death_by_hungry = DEATH_COUNTDOWN;
            }
            _ => {}
        }
        self.raised |= 1 << id;
    }

    fn mark_done(&mut self, id: u8) {
        self.done |= 1 << id;
        if id == req::WAKE && self.yogore > 2 {
            self.status |= status::DIRTY;
        }
    }

    // ---- small helpers --------------------------------------------------

    /// Seed for other random sources (e.g. HOME behaviour) derived from this pet.
    pub(crate) fn rng_seed(&self) -> u32 {
        self.rng
    }

    pub(crate) fn rand100(&mut self) -> u32 {
        // xorshift32; the firmware's rand() depends on frame count, so bit-exactness is not a goal.
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        x % 100
    }

    fn growing(&self) -> bool {
        matches!(self.def.stage, 3 | 4)
    }

    /// `care_add_onaka` 0x1000f3a0.
    fn add_onaka(&mut self, d: i32) {
        let before = self.onaka;
        self.onaka = (self.onaka as i32 + d).clamp(0, ONAKA_MAX as i32) as u8;
        if d > 0 {
            self.clear(req::ONAKA);
            self.onaka_timer = 0;
            self.death_by_hungry = 0;
            if self.growing() && before < ONAKA_MAX && self.onaka == ONAKA_MAX {
                self.onaka_max = (self.onaka_max + 1).min(99);
            }
        }
    }

    /// `care_add_gokigen` 0x1000f23c.
    fn add_gokigen(&mut self, d: i32) {
        if d < 0 && self.status & status::HAPPY != 0 {
            return;
        }
        let before = self.gokigen;
        self.gokigen = (self.gokigen as i32 + d).clamp(0, GOKIGEN_MAX as i32) as u8;
        if d > 0 {
            if self.is_pending(req::GOKIGEN) {
                self.clear(req::GOKIGEN);
                self.gokigen_timer = 0;
            }
            if self.growing() && before < GOKIGEN_MAX && self.gokigen == GOKIGEN_MAX {
                self.gokigen_max = (self.gokigen_max + 1).min(99);
            }
        }
    }

    fn add_mistake(&mut self) {
        self.osewamiss = self.osewamiss.saturating_add(1).min(99);
        self.miss_count = (self.miss_count + 1).min(6);
    }

    fn cold_up(&mut self, src: ColdSource) {
        if self.cond == Cond::Cold || self.status & status::IMMUNITY != 0 {
            return;
        }
        self.cold += 1;
        if self.cold.is_multiple_of(5) {
            self.cold = 0;
            self.death_cause = DeathCause::Illness;
            match src {
                ColdSource::Poop => self.raise(req::DEATH_DELAY),
                ColdSource::Mistake => self.raise(req::DEATH),
            }
        } else {
            self.clear(req::COLD);
            self.raise(req::COLD);
            self.raise(req::HOMETRG);
        }
    }

    // ---- time -----------------------------------------------------------

    /// 0: 06–09, 1: 10–15, 2: 16–18, 3: 19–21, 4: 22–05 (`tama_time_period` 0x1008e704).
    pub fn period(t: u64) -> u8 {
        match (t % DAY) / 3600 {
            6..=9 => 0,
            10..=15 => 1,
            16..=18 => 2,
            19..=21 => 3,
            _ => 4,
        }
    }

    /// `tama_is_sleeping_at` 0x1007dc1c.
    pub fn is_sleeping_at(&self, t: u64) -> bool {
        if matches!(self.cond, Cond::Death | Cond::Dying | Cond::Dead) {
            return false;
        }
        match self.def.stage {
            1 => false,
            2 => (1800..2100).contains(&self.age),
            _ => match Self::period(t) {
                0 => self.is_pending(req::WAKE),
                3 => !self.is_pending(req::SLEEP),
                4 => true,
                _ => false,
            },
        }
    }

    fn alive(&self) -> bool {
        !matches!(self.cond, Cond::Death | Cond::Dying | Cond::Dead)
    }

    /// Colour actually drawn: the chosen colour or the character's default.
    pub fn body_color(&self) -> u8 {
        if self.color < 16 {
            self.color
        } else {
            self.def.color
        }
    }

    pub fn count(&self, id: u16) -> u8 {
        self.inventory.iter().find(|e| e.0 == id).map_or(0, |e| e.1)
    }

    /// Add (or remove with negative `n`) items; per-item cap 10 (`item_give` 0x1001e0d8).
    pub fn give(&mut self, id: u16, n: i32) {
        match self.inventory.iter_mut().find(|e| e.0 == id) {
            Some(e) => e.1 = (e.1 as i32 + n).clamp(0, 10) as u8,
            None if n > 0 => self.inventory.push((id, n.min(10) as u8)),
            None => {}
        }
        self.inventory.retain(|e| e.1 > 0);
    }

    /// Tama days ≥ 30 at death counts as old age (daiojo).
    pub fn daiojo(&self) -> bool {
        self.cond == Cond::Dead && self.days >= 30
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::calendar::calendar_events;
    use super::*;

    pub(crate) fn def(id: u16, stage: u8, hungry_min: u16, happy_min: u16, disease: u8) -> CharDef {
        CharDef {
            id,
            chr_id: 0,
            name_id: 0,
            stage,
            species: 0,
            rank: 0,
            likes: 6,
            attribute: 6,
            gender: 0,
            hungry_min,
            happy_min,
            disease,
            dislikes: 2,
            color: 0,
        }
    }

    pub(crate) fn meal(onaka: u8, group: u8) -> Item {
        Item { id: 5000, kind: KIND_MEAL, group, onaka, ..Default::default() }
    }

    const BABY: fn() -> CharDef = || def(1001, 2, 3, 5, 0);
    const KID: fn() -> CharDef = || def(2113, 3, 45, 20, 40);

    /// 12:00 on day 1: awake period for stage ≥ 3.
    const NOON: u64 = DAY + 12 * 3600;

    fn pet(def: CharDef) -> Pet {
        let mut p = Pet::new(def, 7);
        p.onaka = ONAKA_MAX;
        p.gokigen = GOKIGEN_MAX;
        p.advance_to(NOON);
        p
    }

    fn run(p: &mut Pet, secs: u64) {
        let start = p.last.unwrap();
        for s in 1..=secs {
            p.advance_to(start + s);
        }
    }

    #[test]
    fn hunger_drops_every_hungry_minutes() {
        let mut p = pet(KID());
        run(&mut p, 45 * 60 - 1);
        assert_eq!(p.onaka, 6);
        run(&mut p, 1);
        assert_eq!(p.onaka, 5);
        assert_eq!(p.gokigen, 18, "mood drops every 20 min");
    }

    #[test]
    fn two_hunger_drops_then_poop_after_20s() {
        let mut p = pet(BABY());
        run(&mut p, 2 * 180);
        assert!(p.is_pending(req::SOWASOWA));
        run(&mut p, 19);
        assert!(p.unchi.is_empty());
        run(&mut p, 1);
        assert_eq!(p.unchi.len(), 1);
    }

    #[test]
    fn toilet_prevents_poop() {
        let mut p = pet(BABY());
        run(&mut p, 2 * 180);
        assert!(p.toilet());
        run(&mut p, 60);
        assert!(p.unchi.is_empty());
    }

    #[test]
    fn unanswered_hunger_call_is_a_mistake_from_child_stage() {
        let mut p = pet(KID());
        p.onaka = 1;
        run(&mut p, 45 * 60);
        assert!(p.is_pending(req::ONAKA));
        run(&mut p, CALL_TIMEOUT as u64);
        assert_eq!(p.osewamiss, 1);
        assert!(!p.is_pending(req::ONAKA));
    }

    #[test]
    fn feeding_answers_the_call() {
        let mut p = pet(KID());
        p.onaka = 1;
        run(&mut p, 45 * 60);
        assert!(p.feed(&meal(2, 6)));
        assert_eq!(p.onaka, 2);
        assert!(!p.is_pending(req::ONAKA));
        assert_eq!(p.death_by_hungry, 0);
        run(&mut p, CALL_TIMEOUT as u64);
        assert_eq!(p.osewamiss, 0);
    }

    #[test]
    fn baby_catches_a_cold_at_600s_and_medicine_cures_it() {
        let mut p = pet(BABY());
        p.def.hungry_min = 0;
        p.def.happy_min = 0;
        run(&mut p, 601);
        assert_eq!(p.cond, Cond::Cold);
        assert_eq!(p.death_by_cold, DEATH_COUNTDOWN);
        assert!(p.medicine());
        assert_eq!(p.cond, Cond::Normal);
        assert_eq!(p.death_by_cold, 0);
    }

    #[test]
    fn fifth_cold_is_fatal() {
        let mut p = pet(KID());
        p.cold = 4;
        p.cold_up(ColdSource::Mistake);
        assert_eq!(p.cond, Cond::Death);
        assert_eq!(p.death_cause, DeathCause::Illness);
    }

    #[test]
    fn starving_36h_leads_to_death_then_dying_then_dead() {
        let mut p = pet(KID());
        p.onaka = 0;
        // Missed calls would otherwise give colds, and an illness countdown can win the race.
        p.status = status::IMMUNITY;
        p.raise(req::ONAKA);
        // Sleep freezes the awake-only steps; offline catch-up still counts down.
        for _ in 0..2 {
            let t = p.last.unwrap();
            p.advance_to(t + DAY);
        }
        assert!(matches!(p.cond, Cond::Death | Cond::Dying), "{:?}", p.cond);
        let t = p.last.unwrap();
        p.advance_to(t + 2 * 3600);
        assert_eq!((p.cond, p.death_remain), (Cond::Dying, 0), "waits for the dying scene");
        p.end_dying(None);
        assert_eq!(p.cond, Cond::Dead);
        assert_eq!(p.death_cause, DeathCause::Hunger);
        assert!(!p.graves[0].spirit, "died young");
    }

    #[test]
    fn an_old_age_ancestor_saves_a_dying_pet_once() {
        let mut p = pet(KID());
        p.days = 30;
        p.raise(req::DYING);
        p.end_dying(None);
        assert!(p.daiojo() && p.graves[0].spirit);
        p.cond = Cond::Normal;
        p.raise(req::DYING);
        assert_eq!(p.dying_ancestor(), Some(0));
        p.end_dying(Some(0));
        assert_eq!((p.cond, p.dying_ancestor(), p.graves.len()), (Cond::Normal, None, 1));
    }

    #[test]
    fn rescue_during_death_window() {
        let mut p = pet(KID());
        p.raise(req::DEATH);
        run(&mut p, 100);
        assert!(p.rescue());
        assert_eq!(p.cond, Cond::Normal);
    }

    #[test]
    fn sleep_freezes_hunger() {
        let mut p = Pet::new(KID(), 1);
        p.onaka = 6;
        p.advance_to(DAY + 23 * 3600);
        let start = p.last.unwrap();
        for s in 1..=3 * 3600 {
            p.advance_to(start + s);
        }
        assert!(p.is_sleeping_at(p.last.unwrap()));
        assert_eq!(p.onaka, 6);
    }

    #[test]
    fn wakes_at_10_and_sleeps_at_22() {
        let mut p = Pet::new(KID(), 1);
        p.advance_to(DAY + 5 * 3600);
        p.advance_to(DAY + 7 * 3600);
        assert!(p.is_sleeping_at(DAY + 7 * 3600), "06–09 asleep until woken");
        p.advance_to(DAY + 10 * 3600);
        assert!(!p.is_sleeping_at(DAY + 10 * 3600));
        p.advance_to(DAY + 22 * 3600);
        assert!(p.is_sleeping_at(DAY + 22 * 3600));
    }

    #[test]
    fn catch_up_is_capped_at_24h() {
        let mut p = pet(KID());
        p.def.hungry_min = 0;
        p.def.happy_min = 0;
        p.advance_to(NOON + 10 * DAY);
        assert_eq!(p.age as u64, CHUNK * (MAX_CHUNKS + 1));
    }

    #[test]
    fn clock_going_backwards_credits_nothing() {
        let mut p = pet(KID());
        p.advance_to(NOON - 3600);
        assert_eq!(p.age, 0);
        assert_eq!(p.last, Some(NOON - 3600));
    }

    #[test]
    fn day_change_runs_once_for_many_days() {
        let mut p = pet(KID());
        p.advance_to(NOON + 3 * DAY);
        assert_eq!(p.yogore, 1);
    }

    #[test]
    fn juice_changes_colour_when_effect_completes() {
        let mut p = pet(KID());
        let juice = Item { id: 6003, kind: KIND_SNACK, gokigen: 1, ..Default::default() };
        assert!(p.feed(&juice));
        assert_eq!(p.body_color(), 0, "default colour until the effect plays");
        assert_eq!(p.complete_effect(), Some(req::COLOR));
        assert_eq!(p.body_color(), 4);
        assert_eq!(p.complete_effect(), None);
    }

    #[test]
    fn happy_pancakes_set_happy_and_stop_mood_loss() {
        let mut p = pet(KID());
        p.gokigen = 0;
        p.feed(&Item { id: 6010, kind: KIND_SNACK, ..Default::default() });
        assert_eq!(p.gokigen, 20);
        assert_eq!(p.complete_effect(), Some(req::HAPPY));
        run(&mut p, 3600);
        assert_eq!(p.gokigen, 20);
    }

    #[test]
    fn meteor_day_and_outcomes() {
        let ninth = 20_806 * DAY + 12 * 3600; // 2026-12-19
        assert_eq!(date(ninth).2, 19);
        assert_eq!(calendar_events(ninth), vec![req::METEOR, req::SANTA]);
        let mut p = Pet::new(KID(), 1);
        p.gokigen = 15;
        p.raise(req::METEOR);
        p.finish_meteor(false);
        assert_eq!((p.gokigen, p.cond), (5, Cond::Injury));
        assert!(!p.is_pending(req::METEOR) && p.is_pending(req::INJURY));
    }

    #[test]
    fn birthday_is_an_event_and_feb_29_moves_to_march_1() {
        let mut p = Pet::new(KID(), 1);
        p.birthday = Some((2, 29));
        let mar1_2027 = 20_878 * DAY; // 2027-03-01
        assert_eq!(date(mar1_2027), (2027, 3, 1));
        p.advance_to(mar1_2027 - 3600);
        assert!(!p.was_raised(req::BIRTHDAY));
        p.advance_to(mar1_2027 + 3600);
        assert!(p.is_pending(req::BIRTHDAY));
        p.finish_event(req::BIRTHDAY);
        p.advance_to(mar1_2027 + DAY + 3600);
        assert!(!p.was_raised(req::BIRTHDAY), "cleared the next day");
    }

    #[test]
    fn dododo_on_the_13th_at_five_and_its_penalty() {
        let thirteenth = 20_800 * DAY; // 2026-12-13
        assert_eq!(date(thirteenth).2, 13);
        let mut p = Pet::new(KID(), 1);
        p.rank = 6;
        p.gokigen = 15;
        p.field = 1;
        p.give(7001, 1);
        p.toys[1][0][0] = 7001;
        p.advance_to(thirteenth + 16 * 3600 + 59 * 60);
        p.cond = Cond::Normal;
        p.advance_to(thirteenth + 17 * 3600 + 2 * 60);
        assert!(p.is_pending(req::DODODO));
        let mut answered = p.clone();
        answered.finish_event(req::DODODO);
        answered.advance_to(thirteenth + 17 * 3600 + 20 * 60);
        assert_eq!(answered.toys[1][0][0], 7001);
        p.advance_to(thirteenth + 17 * 3600 + 20 * 60);
        assert!(!p.is_pending(req::DODODO));
        assert_eq!(p.toys[1][0][0], 0);
        assert_eq!(p.count(7001), 0);
        assert!(p.was_raised(req::INJURY) || p.cond == Cond::Injury);
        assert!(answered.gokigen > p.gokigen);
    }

    #[test]
    fn calendar() {
        assert_eq!(date(0), (1970, 1, 1));
        // 2026-12-25 12:00
        let xmas = 20_812 * DAY + 12 * 3600;
        assert_eq!(date(xmas), (2026, 12, 25));
        assert_eq!(calendar_events(xmas), vec![req::XMAS]);
        let mut p = Pet::new(KID(), 1);
        p.advance_to(xmas - DAY);
        assert!(p.is_pending(req::EVE) && !p.is_pending(req::XMAS));
        p.advance_to(xmas);
        assert!(p.is_pending(req::XMAS) && !p.is_pending(req::EVE));
        p.gokigen = 0;
        p.finish_event(req::XMAS);
        assert_eq!(p.gokigen, 20);
        assert!(!p.is_pending(req::XMAS));
    }

    #[test]
    fn dirt_shows_after_three_days_on_waking() {
        let mut p = pet(KID());
        p.yogore = 3;
        p.raise(req::WAKE);
        assert!(p.wake());
        assert_ne!(p.status & status::DIRTY, 0);
        p.groom();
        assert_eq!(p.status & status::DIRTY, 0);
    }
}
