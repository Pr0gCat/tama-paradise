//! Time simulation: the daily sleep schedule, catch-up and the 60 s `care_tick` step
//! (docs/game-spec.md §4, firmware 0x10012c90 / 0x100134bc).

use super::*;

impl Pet {
    /// Sleep/wake requests by time of day (from `care_time_update`).
    // The firmware evaluates this once per `care_time_update`; we run it per chunk so
    // long offline catch-up follows the daily schedule instead of the state at wake-up.
    pub(super) fn schedule(&mut self, t: u64) {
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
        if p == 3 && self.cond == Cond::Sitter {
            self.cond = Cond::Normal;
            self.add_gokigen(-5);
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

    /// Advance to wall-clock `now` (`care_time_update` 0x10012c90).
    /// Catch-up is capped at 1440 chunks of 60 s plus one; older time is dropped.
    pub fn advance_to(&mut self, now: u64) {
        let Some(last) = self.last else {
            self.last = Some(now);
            self.raise_calendar(now);
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
            self.day_change(now);
        }
        self.last = Some(now);
    }

    /// One simulation step of `dt` ≤ 60 s starting at `t` (`care_tick` 0x100134bc).
    pub fn tick(&mut self, t: u64, dt: u32) {
        let sleeping = self.is_sleeping_at(t);
        let nap = self.is_done(req::NAP);
        self.dododo(t, !sleeping && !nap);
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
                // DYING then waits at 0 for the dying scene (`end_dying`).
                if self.is_pending(req::DEATH) && !self.was_raised(req::DYING) {
                    self.raise(req::DYING);
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

        if self.cond == Cond::Sitter {
            // The sitter keeps the pet fed; time only fills the UNCHI METER.
            self.onaka_timer += dt;
            let hungry = self.def.hungry_min as u32 * 60;
            if hungry != 0 && self.onaka_timer >= hungry {
                self.onaka_timer = 0;
                self.hunger_drops += 1;
                if self.hunger_drops >= 2 {
                    self.hunger_drops = 0;
                    self.unchi_meter = (self.unchi_meter + 1).min(60);
                }
            }
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

        self.evolution_tick(dt);

        if self.def.stage == 2 && !self.was_raised(req::COLD) && self.age > 600 {
            self.raise(req::COLD);
        }

        if self.def.stage >= 3 && self.osewamiss > misses_before {
            let r = self.rand100();
            if self.osewamiss.is_multiple_of(6) || r < self.def.disease as u32 {
                self.cold_up(ColdSource::Mistake);
            }
        }
    }
}
