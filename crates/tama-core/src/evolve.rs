//! Growth stages and evolution, following docs/game-spec.md §6 (firmware `tama_evolve` 0x1000fc98).

use crate::data::{attr_to_field, Data, Item};
use crate::pet::{req, status, Pet};

/// Age (s) at which each stage asks to evolve, XIP 0x100913ac. Index = stage.
pub const STAGE_AGE: [u32; 6] = [0, 60, 3600, 86_400, 86_400, 1_209_600];
/// Baby is forced to evolve at 4 h; child window ends at 28 h.
pub const BABY_FORCE: u32 = 14_400;
pub const CHILD_LIMIT: u32 = 86_400 + 14_400;
pub const FIELD_LOG_EVERY: u32 = 900;
pub const FIELD_LOG_MAX: usize = 11;
pub const METAMORPHO_AFTER: u32 = 3600;
pub const SUPPORT_TIMEOUT: u32 = 900;

/// Native food categories (1-based) per field: tropical 13–15, frozen 16–18, forest 10–12.
const NATIVE_FIRST: [u8; 3] = [13, 16, 10];

impl Pet {
    pub fn stage(&self) -> u8 {
        self.def.stage
    }

    /// Move to another area (`field_set_current` 0x10010cf4).
    pub fn set_field(&mut self, f: u8) {
        if f == self.field {
            return;
        }
        self.field = f;
        self.field_timer = [0; 3];
        if self.stage() == 3 {
            self.clear(req::EVOLUTION);
        }
    }

    /// Food log for evolution (`care_eat_item` 0x1001021c, stage < 4, native food only).
    pub(crate) fn log_food(&mut self, item: &Item) {
        let Some(home) = attr_to_field(self.def.attribute) else { return };
        let first = NATIVE_FIRST[home as usize];
        if self.stage() >= 4 || !(first..first + 3).contains(&item.category) {
            return;
        }
        let i = item.category as usize - 1;
        self.food_pts[i] += item.gokigen;
        if self.food_pts[i] > 4 {
            self.food_pts[i] -= 5;
            if self.food_log.len() == 4 {
                self.food_log.remove(0);
            }
            self.food_log.push(i as u8);
        }
    }

    /// `evo_dominant_field` 0x10081c68: area with ≥ 4 log entries (highest wins), else 3.
    pub fn dominant_field(&self) -> u8 {
        (0..3u8).rev().find(|f| self.field_log.iter().filter(|&&x| x == *f).count() >= 4).unwrap_or(3)
    }

    /// `food_branch` 0x10081d70: strictly most-eaten of the field's 3 native categories, else 3.
    pub fn food_branch(&self) -> u8 {
        let first = NATIVE_FIRST[self.field as usize] - 1;
        let counts: Vec<usize> = (0..3).map(|k| self.food_log.iter().filter(|&&c| c == first + k).count()).collect();
        let max = *counts.iter().max().unwrap();
        if max == 0 || counts.iter().filter(|&&c| c == max).count() > 1 {
            return 3;
        }
        counts.iter().position(|&c| c == max).unwrap() as u8
    }

    /// Care quality 0 (best) – 3 from logged mistakes and full-heart counts.
    pub fn care_quality(&self) -> u8 {
        match self.miss_count {
            0 if self.onaka_max > 4 && self.gokigen_max > 4 => 0,
            0 | 1 => 1,
            2..=5 => 2,
            _ => 3,
        }
    }

    fn drop_dominant(&mut self) {
        let d = self.dominant_field();
        self.field_log.retain(|&x| x != d);
    }

    /// Evolution timers and requests, run on awake ticks (part of `care_tick` 0x100134bc).
    pub(crate) fn evolution_tick(&mut self, dt: u32) {
        let f = self.field as usize;
        match self.stage() {
            1 if self.age >= STAGE_AGE[1] => self.raise(req::EVOLUTION),
            2 => {
                self.field_timer[f] += dt;
                if self.field_timer[f] >= FIELD_LOG_EVERY {
                    self.field_timer[f] = 0;
                    if self.field_log.len() < FIELD_LOG_MAX {
                        self.field_log.push(self.field);
                    }
                }
                if self.is_pending(req::EVOL_SUPPORT) {
                    self.support_timer += dt;
                    if self.support_timer >= SUPPORT_TIMEOUT {
                        self.drop_dominant();
                        self.clear(req::EVOL_SUPPORT);
                        self.support_miss += 1;
                    }
                }
                if self.age >= BABY_FORCE {
                    if self.is_pending(req::EVOL_SUPPORT) {
                        self.drop_dominant();
                        self.clear(req::EVOL_SUPPORT);
                    }
                    self.raise(req::EVOLUTION);
                } else if self.dominant_field() != 3
                    && !self.is_pending(req::EVOL_SUPPORT)
                    && !self.is_pending(req::EVOLUTION)
                {
                    if self.support_miss < 2 {
                        self.support_timer = 0;
                        self.raise(req::EVOL_SUPPORT);
                    } else {
                        self.raise(req::EVOLUTION);
                    }
                }
            }
            3 => {
                let home = self.field_log.first().copied().unwrap_or(self.field);
                if self.field != home {
                    self.field_timer[f] += dt;
                    if self.field_timer[f] >= METAMORPHO_AFTER && !self.is_pending(req::METAMORPHO) {
                        self.field_timer[f] = 0;
                        self.field_log = vec![self.field; 4];
                        self.food_log.clear();
                        self.raise(req::METAMORPHO);
                    }
                }
                if self.age >= CHILD_LIMIT || (self.age >= STAGE_AGE[3] && self.field == home) {
                    self.raise(req::EVOLUTION);
                } else if self.field != home {
                    self.clear(req::EVOLUTION);
                }
            }
            4 if self.age >= STAGE_AGE[4] => self.raise(req::EVOLUTION),
            _ => {}
        }
    }

    pub fn evolution_pending(&self) -> bool {
        [req::EVOLUTION, req::EVOL_SUPPORT, req::METAMORPHO].iter().any(|&r| self.is_pending(r))
    }

    /// The player helps the baby during EVOL_SUPPORT (the original uses a mini-game).
    pub fn support(&mut self, data: &Data) -> bool {
        self.is_pending(req::EVOL_SUPPORT) && self.evolve(data)
    }

    fn add_points(&mut self, n: u32) {
        self.earn(n);
    }

    /// Complete a pending evolution and switch character (`tama_evolve` 0x1000fc98).
    /// Returns false if nothing is pending or the target is missing from the data.
    pub fn evolve(&mut self, data: &Data) -> bool {
        let e = &data.evolution;
        let (target, points) = if self.is_pending(req::METAMORPHO) {
            (e.kids[self.field_log[0] as usize], 0)
        } else if self.is_pending(req::EVOLUTION) || self.is_pending(req::EVOL_SUPPORT) {
            let f = self.field as usize;
            match self.stage() {
                1 => (1001, 0),
                2 => match self.dominant_field() {
                    3 => (e.failed, 800),
                    d => (e.kids[d as usize], 300),
                },
                3 if self.age < CHILD_LIMIT => (e.teens[f][self.food_branch() as usize], 500),
                3 => (e.failed, 800),
                4 => {
                    let b = self.food_branch();
                    let rare = self.comm[0] != 0
                        && self.comm[1] != 0
                        && attr_to_field(data.rom_type) == Some(self.field)
                        && b != 3;
                    let q = self.care_quality();
                    let id = if rare { e.rare[f] } else { e.adults[f][b as usize][q as usize] };
                    if !rare {
                        self.collect_mission(crate::progress::m::ADULTS_FIELD0 + f, b as u32 * 4 + q as u32);
                    }
                    (id, 800)
                }
                _ => return false,
            }
        } else {
            return false;
        };
        let Some(def) = data.character(target) else { return false };

        if self.stage() == 2 && def.stage == 3 {
            let d = self.dominant_field();
            self.field_log = vec![d; 4];
        } else if def.stage == 5 && self.stage() == 2 {
            self.field_log.clear();
        }
        for r in [req::EVOLUTION, req::EVOL_SUPPORT, req::METAMORPHO] {
            self.clear(r);
        }
        self.status &= status::DIRTY;
        // Planet rank 0→1→2→3 for the first child, teen and adult (05-content §2.6).
        if def.stage == self.rank + 3 && def.stage >= 3 {
            self.rank_up();
        }
        if self.color == crate::pet::NO_COLOR && def.stage == 5 {
            self.collect_mission(crate::progress::m::COLORS, def.color as u32);
        }
        self.def = def;
        self.age = 0;
        self.cold = 0;
        self.field_timer = [0; 3];
        self.add_points(points);
        true
    }

    /// Start over with a new egg (`tama_birth` 0x1000f708, no partner). Player progress — points,
    /// rank, missions, inventory, unlocks, areas — carries over; the pet's own state resets.
    pub fn new_egg(&mut self, data: &Data) {
        let egg = data.character(1000).expect("egg 1000 in data");
        let mut fresh = Pet::new(egg, self.rng);
        fresh.points = self.points;
        fresh.field = self.field;
        fresh.last = self.last;
        fresh.generation = self.generation;
        fresh.rank = self.rank;
        fresh.rank_seen = self.rank_seen;
        fresh.missions = self.missions;
        fresh.claimed = self.claimed;
        fresh.unlocked = std::mem::take(&mut self.unlocked);
        fresh.used_codes = std::mem::take(&mut self.used_codes);
        fresh.areas = self.areas;
        fresh.unchi_meter = self.unchi_meter;
        fresh.inventory = std::mem::take(&mut self.inventory);
        fresh.weather = self.weather;
        fresh.weather_day = self.weather_day;
        fresh.hunt_used = self.hunt_used;
        fresh.grazing = std::mem::take(&mut self.grazing);
        fresh.graves = std::mem::take(&mut self.graves);
        fresh.adults_raised = self.adults_raised;
        fresh.breeds_today = self.breeds_today;
        fresh.seen_partners = std::mem::take(&mut self.seen_partners);
        fresh.device_uid = self.device_uid;
        fresh.mini_plays = self.mini_plays;
        fresh.together_plays = self.together_plays;
        fresh.birthday = self.birthday;
        fresh.planet_name = self.planet_name.clone();
        fresh.toys = self.toys;
        fresh.equip = self.equip;
        fresh.travel_toggle = self.travel_toggle;
        fresh.friends = std::mem::take(&mut self.friends);
        fresh.gifts_sent = self.gifts_sent;
        fresh.gifts_received = std::mem::take(&mut self.gifts_received);
        *self = fresh;
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use crate::data::{CharDef, Data, Evolution, Item, KIND_MEAL};
    use crate::pet::tests::def;
    use crate::pet::{req, Pet};

    /// Same shape as the real tables; ids encode their position for easy checking.
    pub(crate) fn data() -> Data {
        let mut characters = vec![def(1000, 1, 0, 0, 0), def(1001, 2, 3, 5, 0), def(4017, 5, 30, 30, 0)];
        let kids = [2091, 2113, 2069];
        let rare = [4112, 4134, 4090];
        for &k in kids.iter().chain(&rare) {
            characters.push(def(k, if k < 3000 { 3 } else { 5 }, 45, 20, 0));
        }
        let teens = std::array::from_fn(|f| std::array::from_fn(|b| 3000 + (f * 10 + b) as u16));
        let adults = std::array::from_fn(|f| {
            std::array::from_fn(|b| std::array::from_fn(|q| 4500 + (f * 100 + b * 10 + q) as u16))
        });
        for f in 0..3 {
            for b in 0..4 {
                characters.push(def(3000 + (f * 10 + b) as u16, 4, 60, 30, 0));
                for q in 0..4 {
                    characters.push(def(4500 + (f * 100 + b * 10 + q) as u16, 5, 60, 30, 0));
                }
            }
        }
        Data {
            rom_type: 6,
            characters,
            items: vec![],
            evolution: Evolution { kids, teens, adults, rare, failed: 4017 },
            models: vec![],
            motions: vec![],
            home_motions: vec![],
            fonts: vec![],
            strings: vec![],
            ui: vec![],
            field_bg: vec![],
            melodies: vec![],
            shop_codes: vec![],
            recipes: vec![],
            weather_months: vec![],
            weather_patterns: vec![],
            hunt_finds: vec![],
            mini: Default::default(),
            decor: Default::default(),
        }
    }

    const NOON: u64 = 86_400 + 12 * 3600;

    fn egg(d: &Data) -> Pet {
        let mut p = Pet::new(d.character(1000).unwrap(), 5);
        p.field = 1;
        p.advance_to(NOON);
        p
    }

    fn run(p: &mut Pet, secs: u64) {
        let t = p.last.unwrap();
        for s in 1..=secs {
            p.advance_to(t + s);
        }
    }

    fn food(category: u8, points: u8) -> Item {
        Item { id: 5000, kind: KIND_MEAL, category, onaka: 1, gokigen: points, ..Default::default() }
    }

    #[test]
    fn egg_hatches_after_60s() {
        let d = data();
        let mut p = egg(&d);
        run(&mut p, 59);
        assert!(!p.evolution_pending());
        run(&mut p, 1);
        assert!(p.evolve(&d));
        assert_eq!(p.def.id, 1001);
        assert_eq!(p.age, 0);
    }

    #[test]
    fn baby_settles_in_area_then_support_then_child() {
        let d = data();
        let mut p = egg(&d);
        run(&mut p, 60);
        p.evolve(&d);
        p.onaka = 6;
        p.gokigen = 20;
        p.def.hungry_min = 0;
        p.def.happy_min = 0;
        // 3600 awake seconds in one area; the baby naps at age 1800–2100.
        run(&mut p, 3900 - 1);
        assert!(!p.is_pending(req::EVOL_SUPPORT));
        run(&mut p, 1);
        assert_eq!(p.dominant_field(), 1);
        assert!(p.is_pending(req::EVOL_SUPPORT));
        assert!(p.support(&d));
        assert_eq!(p.def.id, 2113);
        assert_eq!(p.points, 300);
        assert_eq!(p.field_log, vec![1; 4]);
    }

    #[test]
    fn missed_support_twice_then_plain_evolution() {
        let d = data();
        let mut p = egg(&d);
        run(&mut p, 60);
        p.evolve(&d);
        p.def.hungry_min = 0;
        p.def.happy_min = 0;
        // Support at 3900 s times out at 4800 s; the area log is dropped and must refill.
        run(&mut p, 4800);
        assert_eq!(p.support_miss, 1);
        run(&mut p, 3600 + 900);
        assert_eq!(p.support_miss, 2);
        run(&mut p, 3600);
        assert!(p.age < 14_400);
        assert!(p.is_pending(req::EVOLUTION));
    }

    #[test]
    fn baby_forced_at_4h_without_area_becomes_failed_adult() {
        let d = data();
        let mut p = egg(&d);
        run(&mut p, 60);
        p.evolve(&d);
        p.age = 14_400;
        p.field_log.clear();
        p.raise(req::EVOLUTION);
        assert!(p.evolve(&d));
        assert_eq!(p.def.id, 4017);
    }

    #[test]
    fn food_branch_needs_strict_majority_of_native_food() {
        let d = data();
        let mut p = Pet::new(d.character(2113).unwrap(), 1);
        p.field = 1;
        for _ in 0..3 {
            p.log_food(&food(17, 5));
        }
        p.log_food(&food(16, 5));
        assert_eq!(p.food_log, vec![16, 16, 16, 15]);
        assert_eq!(p.food_branch(), 1);
        p.log_food(&food(13, 5));
        assert_eq!(p.food_log.len(), 4, "tropical food is not native for a frozen kid");
        p.food_log = vec![15, 15, 16, 16];
        assert_eq!(p.food_branch(), 3, "tie");
    }

    #[test]
    fn child_evolves_in_home_area_by_food_branch() {
        let d = data();
        let mut p = Pet::new(d.character(2113).unwrap(), 1);
        p.field = 1;
        p.field_log = vec![1; 4];
        p.food_log = vec![17, 17, 16];
        p.age = 86_400;
        p.evolution_tick(1);
        assert!(p.evolve(&d));
        assert_eq!(p.def.id, 3012, "teens[1][2]");
    }

    #[test]
    fn child_away_from_home_waits_then_fails_after_28h() {
        let d = data();
        let mut p = Pet::new(d.character(2113).unwrap(), 1);
        p.field = 0;
        p.field_log = vec![1; 4];
        p.age = 90_000;
        p.evolution_tick(1);
        assert!(!p.is_pending(req::EVOLUTION));
        p.age = 100_800;
        p.evolution_tick(1);
        assert!(p.evolve(&d));
        assert_eq!(p.def.id, 4017);
    }

    #[test]
    fn child_in_other_area_for_an_hour_metamorphoses() {
        let d = data();
        let mut p = Pet::new(d.character(2113).unwrap(), 1);
        p.field_log = vec![1; 4];
        p.set_field(2);
        p.evolution_tick(3600);
        assert!(p.is_pending(req::METAMORPHO));
        assert!(p.evolve(&d));
        assert_eq!(p.def.id, 2069);
    }

    #[test]
    fn teen_becomes_adult_by_branch_and_quality() {
        let d = data();
        let mut p = Pet::new(d.character(3012).unwrap(), 1);
        p.field = 1;
        p.food_log = vec![15, 15];
        p.miss_count = 3;
        p.age = 86_400;
        p.evolution_tick(1);
        assert!(p.evolve(&d));
        assert_eq!(p.def.id, (4500 + 100) + 2, "adults[1][b=0][q=2]");
        assert_eq!(p.points, 800);
    }

    #[test]
    fn rare_evolution_needs_two_links_and_device_home_area() {
        let d = data();
        let mut p = Pet::new(d.character(3012).unwrap(), 1);
        p.field = 1;
        p.food_log = vec![15];
        p.comm = [1, 3];
        p.age = 86_400;
        p.evolution_tick(1);
        assert!(p.evolve(&d));
        assert_eq!(p.def.id, 4134);
    }

    #[test]
    fn quality_zero_needs_no_mistakes_and_full_hearts() {
        let d = data();
        let mut p = Pet::new(d.character(3012).unwrap(), 1);
        assert_eq!(p.care_quality(), 1);
        p.onaka_max = 5;
        p.gokigen_max = 5;
        assert_eq!(p.care_quality(), 0);
        p.miss_count = 6;
        assert_eq!(p.care_quality(), 3);
    }

    #[test]
    fn new_egg_keeps_player_progress() {
        let d = data();
        let mut p = Pet::new(d.character(4017).unwrap(), 1);
        p.points = 1234;
        p.rank = 3;
        p.missions[0] = 7;
        p.give(5000, 2);
        p.unlocked.push(5033);
        p.onaka = 4;
        p.new_egg(&d);
        assert_eq!(p.def.id, 1000);
        assert_eq!((p.points, p.rank, p.missions[0], p.count(5000), p.unlocked.len()), (1234, 3, 7, 2, 1));
        assert_eq!(p.onaka, 0);
    }

    #[test]
    fn real_data_file_loads_and_resolves_all_targets() {
        let Ok(json) = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/game.json")) else {
            return; // data/game.json is generated from a dump; skip when absent.
        };
        let d = Data::from_json(&json).unwrap();
        let e = &d.evolution;
        let all =
            e.kids.iter().chain(e.teens.iter().flatten()).chain(e.adults.iter().flatten().flatten()).chain(&e.rare);
        for id in all.chain(&[e.failed]) {
            let c: CharDef = d.character(*id).unwrap_or_else(|| panic!("{id} missing"));
            assert_eq!(c.stage as u16, id / 1000 + 1, "{id}: 2xxx child, 3xxx teen, 4xxx adult");
        }
        assert_eq!(d.character(1000).unwrap().stage, 1);
        assert_eq!(d.character(1001).unwrap().stage, 2);
    }
}
