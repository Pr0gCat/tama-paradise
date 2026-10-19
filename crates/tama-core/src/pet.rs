//! Pet care simulation, following docs/game-spec.md §4–5 (firmware `care_tick` 0x100134bc).
//!
//! Time is in seconds of local wall-clock time (`t % 86400` = time of day).
//! The platform supplies `now`; the simulation advances in ≤60 s chunks like the original.

/// Request ids (EM_REQ_*), same numbering as the firmware.
pub mod req {
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
    pub const NAP: u8 = 0x1a;
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

/// Character definition record (game-spec §6). Only the fields care needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CharDef {
    pub id: u16,
    pub stage: u8,
    /// Minutes per hunger heart lost; 0 = never.
    pub hungry_min: u16,
    /// Minutes per mood heart lost; 0 = never.
    pub happy_min: u16,
    /// % chance of a cold on each new care mistake.
    pub disease: u8,
    /// Liked / disliked food group (ItemGrp / EvolBranch).
    pub likes: u8,
    pub dislikes: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoodKind {
    Meal,
    Snack,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Food {
    pub kind: FoodKind,
    pub group: u8,
    pub onaka: u8,
    pub gokigen: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cond {
    Normal,
    Cold,
    Injury,
    Death,
    Dying,
    Dead,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeathCause {
    Illness,
    Hunger,
}

#[derive(Clone, Debug)]
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
    raised: u64,
    done: u64,
    rng: u32,
    /// Last processed time; `None` until the first `advance_to`.
    pub last: Option<u64>,
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

    fn clear(&mut self, id: u8) {
        self.raised &= !(1 << id);
        self.done &= !(1 << id);
    }

    /// `req_raise` 0x10069218 with its side effects.
    fn raise(&mut self, id: u8) {
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
            }
            req::ONAKA => {
                if self.death_by_hungry == 0 {
                    self.death_by_hungry = DEATH_COUNTDOWN;
                }
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

    fn rand100(&mut self) -> u32 {
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
        if self.cold % 5 == 0 {
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

    /// Sleep/wake requests by time of day (from `care_time_update`).
    // The firmware evaluates this once per `care_time_update`; we run it per chunk so
    // long offline catch-up follows the daily schedule instead of the state at wake-up.
    fn schedule(&mut self, t: u64) {
        if self.def.stage <= 2 || self.is_done(req::NAP) {
            return;
        }
        let p = Self::period(t);
        if p == 4 {
            for id in [req::WAKE, req::SLEEP] {
                if self.is_pending(id) {
                    self.mark_done(id);
                }
            }
            return;
        }
        if !self.was_raised(req::WAKE) {
            self.raise(req::WAKE);
        }
        if p != 0 && self.is_pending(req::WAKE) {
            self.mark_done(req::WAKE);
        }
        if p == 3 && !self.was_raised(req::SLEEP) {
            self.raise(req::SLEEP);
        }
    }

    /// `care_day_change` 0x10013234.
    fn day_change(&mut self) {
        if !self.is_done(req::OTEIRE) && self.cond != Cond::Dead {
            self.yogore = (self.yogore + 1).min(3);
        }
        for id in [req::WAKE, req::SLEEP, req::OTEIRE] {
            self.clear(id);
        }
    }

    /// Advance to wall-clock `now` (`care_time_update` 0x10012c90).
    /// Catch-up is capped at 1440 chunks of 60 s plus one; older time is dropped.
    pub fn advance_to(&mut self, now: u64) {
        let Some(last) = self.last else {
            self.last = Some(now);
            return;
        };
        if now <= last {
            self.last = Some(now);
            return;
        }
        let mut t = last;
        let mut dt = now - last;
        let mut chunks = 0;
        while dt > CHUNK && chunks < MAX_CHUNKS {
            self.schedule(t);
            self.tick(t, CHUNK as u32);
            t += CHUNK;
            dt -= CHUNK;
            chunks += 1;
        }
        self.schedule(t);
        self.tick(t, dt.min(CHUNK) as u32);
        if now / DAY != last / DAY {
            self.day_change();
        }
        self.last = Some(now);
    }

    /// One simulation step of `dt` ≤ 60 s starting at `t` (`care_tick` 0x100134bc).
    pub fn tick(&mut self, t: u64, dt: u32) {
        let sleeping = self.is_sleeping_at(t);
        let nap = self.is_done(req::NAP);
        let awake = !sleeping && !nap;

        if self.cond != Cond::Dead && (self.def.stage != 3 || self.age < 86_400 || awake) {
            self.age = (self.age + dt).min(AGE_MAX);
        }
        if awake {
            self.clean_timer = (self.clean_timer + dt).min(14_400);
        }
        if nap {
            self.nap_timer += dt;
            if self.nap_timer > 59 {
                self.nap_timer = 0;
                self.clear(req::NAP);
            }
        }

        if self.death_remain > 0 {
            self.death_remain = self.death_remain.saturating_sub(dt);
            if self.death_remain == 0 {
                if self.is_pending(req::DEATH) && !self.was_raised(req::DYING) {
                    self.raise(req::DYING);
                } else if self.cond == Cond::Dying {
                    // The firmware leaves this to the dying scene; the core ends it itself.
                    self.raise(req::DEAD);
                }
            }
        }

        if matches!(self.cond, Cond::Death | Cond::Dying | Cond::Dead) {
            return;
        }

        self.day_secs += dt;
        if self.day_secs >= DAY as u32 {
            self.day_secs -= DAY as u32;
            self.days = (self.days + 1).min(999);
        }
        if self.death_by_cold > 0 {
            self.death_by_cold = self.death_by_cold.saturating_sub(dt);
            if self.death_by_cold == 0 {
                self.death_cause = DeathCause::Illness;
                self.raise(req::DEATH_DELAY);
            }
        }
        if self.death_by_hungry > 0 {
            self.death_by_hungry = self.death_by_hungry.saturating_sub(dt);
            if self.death_by_hungry == 0 {
                self.death_cause = DeathCause::Hunger;
                self.raise(req::DEATH_DELAY);
            }
        }

        if !awake {
            return;
        }

        if self.is_pending(req::DEATH_DELAY) {
            self.clear(req::DEATH_DELAY);
            self.raise(req::DEATH);
            return;
        }

        self.onaka_timer += dt;
        self.gokigen_timer += dt;

        if self.is_pending(req::SOWASOWA) {
            self.sowasowa_timer += dt;
            if self.sowasowa_timer >= SOWASOWA_DELAY {
                if self.unchi.len() < UNCHI_MAX {
                    self.unchi.push(0);
                }
                self.raise(req::UNCHI);
                self.clear(req::SOWASOWA);
            }
        }

        let old_before = self.unchi.iter().filter(|&&a| a >= UNCHI_OLD).count();
        for a in &mut self.unchi {
            *a = (*a + dt as u16).min(UNCHI_OLD);
        }
        let old_after = self.unchi.iter().filter(|&&a| a >= UNCHI_OLD).count();
        if old_before < UNCHI_MAX && old_after == UNCHI_MAX {
            self.cold_up(ColdSource::Poop);
        }

        let misses_before = self.osewamiss;

        let hungry = self.def.hungry_min as u32 * 60;
        if hungry != 0 && self.onaka_timer >= hungry {
            self.onaka_timer = 0;
            if self.onaka > 0 {
                self.add_onaka(-1);
                self.hunger_drops += 1;
                if self.hunger_drops >= 2 {
                    self.hunger_drops = 0;
                    self.sowasowa_timer = 0;
                    self.raise(req::SOWASOWA);
                }
            }
            if self.onaka == 0 {
                self.raise(req::ONAKA);
                self.raise(req::HOMETRG);
            }
        }
        if self.is_pending(req::ONAKA) && self.onaka_timer >= CALL_TIMEOUT {
            self.clear(req::ONAKA);
            if self.def.stage > 2 {
                self.add_mistake();
            }
        }

        let happy = self.def.happy_min as u32 * 60;
        if happy != 0 && self.gokigen_timer >= happy {
            self.gokigen_timer = 0;
            self.add_gokigen(-1);
            if self.gokigen == 0 {
                self.raise(req::GOKIGEN);
                self.raise(req::HOMETRG);
            }
        }
        if self.is_pending(req::GOKIGEN) && self.gokigen_timer >= CALL_TIMEOUT {
            self.clear(req::GOKIGEN);
            if self.def.stage > 2 {
                self.add_mistake();
            }
        }

        if self.def.stage == 2 && !self.was_raised(req::COLD) && self.age > 600 {
            self.raise(req::COLD);
        }

        if self.def.stage >= 3 && self.osewamiss > misses_before {
            let r = self.rand100();
            if self.osewamiss % 6 == 0 || r < self.def.disease as u32 {
                self.cold_up(ColdSource::Mistake);
            }
        }
    }

    // ---- player actions -------------------------------------------------

    fn alive(&self) -> bool {
        !matches!(self.cond, Cond::Death | Cond::Dying | Cond::Dead)
    }

    /// Feed a meal or snack (`care_eat_item` 0x1001021c).
    pub fn feed(&mut self, f: Food) {
        if !self.alive() {
            return;
        }
        self.add_onaka(f.onaka as i32);
        let pref = if f.group == self.def.likes {
            1
        } else if f.group == self.def.dislikes {
            -1
        } else {
            0
        };
        match f.kind {
            FoodKind::Meal => self.add_gokigen(pref),
            FoodKind::Snack => self.add_gokigen(f.gokigen as i32),
        }
    }

    /// Take the pet to the toilet while it is restless: no poop, mood +1.
    pub fn toilet(&mut self) -> bool {
        if !self.is_pending(req::SOWASOWA) {
            return false;
        }
        self.clear(req::SOWASOWA);
        self.add_gokigen(1);
        true
    }

    /// Clean all poop (`care_clean_unchi` 0x100108d4). Returns how many were removed.
    pub fn clean(&mut self) -> usize {
        let n = self.unchi.len();
        self.unchi.clear();
        self.clean_timer = 0;
        self.clear(req::UNCHI);
        n
    }

    /// One dose of medicine cures a cold.
    pub fn medicine(&mut self) -> bool {
        if self.cond != Cond::Cold {
            return false;
        }
        self.mark_done(req::COLD);
        self.add_gokigen(1);
        self.cond = if self.is_pending(req::INJURY) { Cond::Injury } else { Cond::Normal };
        self.death_by_cold = 0;
        true
    }

    /// Grooming (OTEIRE): clears dirt.
    pub fn groom(&mut self) {
        if !self.alive() {
            return;
        }
        self.status &= !status::DIRTY;
        self.yogore = 0;
        self.add_gokigen(1);
        self.mark_done(req::OTEIRE);
    }

    /// Wake the pet when it is asking to get up.
    pub fn wake(&mut self) -> bool {
        if !self.is_pending(req::WAKE) {
            return false;
        }
        self.mark_done(req::WAKE);
        true
    }

    /// Rescue during the DEATH window (`req_done(DEATH)`).
    pub fn rescue(&mut self) -> bool {
        if self.cond != Cond::Death {
            return false;
        }
        self.mark_done(req::DEATH);
        self.clear(req::DEATH);
        self.cond = Cond::Normal;
        self.death_remain = 0;
        self.death_by_cold = 0;
        self.cold = 0;
        self.death_by_hungry = if self.onaka == 0 { DEATH_COUNTDOWN } else { 0 };
        true
    }

    /// Tama days ≥ 30 at death counts as old age (daiojo).
    pub fn daiojo(&self) -> bool {
        self.cond == Cond::Dead && self.days >= 30
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BABY: CharDef = CharDef { id: 1001, stage: 2, hungry_min: 3, happy_min: 5, disease: 0, likes: 0, dislikes: 0 };
    const KID: CharDef = CharDef { id: 2113, stage: 3, hungry_min: 45, happy_min: 20, disease: 40, likes: 6, dislikes: 0 };

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
        let mut p = pet(KID);
        run(&mut p, 45 * 60 - 1);
        assert_eq!(p.onaka, 6);
        run(&mut p, 1);
        assert_eq!(p.onaka, 5);
        assert_eq!(p.gokigen, 18, "mood drops every 20 min");
    }

    #[test]
    fn two_hunger_drops_then_poop_after_20s() {
        let mut p = pet(BABY);
        run(&mut p, 2 * 180);
        assert!(p.is_pending(req::SOWASOWA));
        run(&mut p, 19);
        assert!(p.unchi.is_empty());
        run(&mut p, 1);
        assert_eq!(p.unchi.len(), 1);
    }

    #[test]
    fn toilet_prevents_poop() {
        let mut p = pet(BABY);
        run(&mut p, 2 * 180);
        assert!(p.toilet());
        run(&mut p, 60);
        assert!(p.unchi.is_empty());
    }

    #[test]
    fn unanswered_hunger_call_is_a_mistake_from_child_stage() {
        let mut p = pet(KID);
        p.onaka = 1;
        run(&mut p, 45 * 60);
        assert!(p.is_pending(req::ONAKA));
        run(&mut p, CALL_TIMEOUT as u64);
        assert_eq!(p.osewamiss, 1);
        assert!(!p.is_pending(req::ONAKA));
    }

    #[test]
    fn feeding_answers_the_call() {
        let mut p = pet(KID);
        p.onaka = 1;
        run(&mut p, 45 * 60);
        p.feed(Food { kind: FoodKind::Meal, group: 6, onaka: 2, gokigen: 0 });
        assert_eq!(p.onaka, 2);
        assert!(!p.is_pending(req::ONAKA));
        assert_eq!(p.death_by_hungry, 0);
        run(&mut p, CALL_TIMEOUT as u64);
        assert_eq!(p.osewamiss, 0);
    }

    #[test]
    fn baby_catches_a_cold_at_600s_and_medicine_cures_it() {
        let mut p = pet(BABY);
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
        let mut p = pet(KID);
        p.cold = 4;
        p.cold_up(ColdSource::Mistake);
        assert_eq!(p.cond, Cond::Death);
        assert_eq!(p.death_cause, DeathCause::Illness);
    }

    #[test]
    fn starving_36h_leads_to_death_then_dying_then_dead() {
        let mut p = pet(KID);
        p.onaka = 0;
        // Missed calls would otherwise give colds, and an illness countdown can win the race.
        p.status = status::IMMUNITY;
        p.raise(req::ONAKA);
        // Sleep freezes the awake-only steps; offline catch-up still counts down.
        for _ in 0..2 {
            let t = p.last.unwrap();
            p.advance_to(t + DAY);
        }
        assert!(matches!(p.cond, Cond::Death | Cond::Dying | Cond::Dead), "{:?}", p.cond);
        let t = p.last.unwrap();
        p.advance_to(t + 2 * 3600);
        assert_eq!(p.cond, Cond::Dead);
        assert_eq!(p.death_cause, DeathCause::Hunger);
    }

    #[test]
    fn rescue_during_death_window() {
        let mut p = pet(KID);
        p.raise(req::DEATH);
        run(&mut p, 100);
        assert!(p.rescue());
        assert_eq!(p.cond, Cond::Normal);
    }

    #[test]
    fn sleep_freezes_hunger() {
        let mut p = Pet::new(KID, 1);
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
        let mut p = Pet::new(KID, 1);
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
        let mut p = pet(KID);
        p.def.hungry_min = 0;
        p.def.happy_min = 0;
        p.advance_to(NOON + 10 * DAY);
        assert_eq!(p.age as u64, CHUNK * (MAX_CHUNKS + 1));
    }

    #[test]
    fn clock_going_backwards_credits_nothing() {
        let mut p = pet(KID);
        p.advance_to(NOON - 3600);
        assert_eq!(p.age, 0);
        assert_eq!(p.last, Some(NOON - 3600));
    }

    #[test]
    fn day_change_runs_once_for_many_days() {
        let mut p = pet(KID);
        p.advance_to(NOON + 3 * DAY);
        assert_eq!(p.yogore, 1);
    }

    #[test]
    fn dirt_shows_after_three_days_on_waking() {
        let mut p = pet(KID);
        p.yogore = 3;
        p.raise(req::WAKE);
        assert!(p.wake());
        assert_ne!(p.status & status::DIRTY, 0);
        p.groom();
        assert_eq!(p.status & status::DIRTY, 0);
    }
}
