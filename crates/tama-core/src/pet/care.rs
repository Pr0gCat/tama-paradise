//! Player care actions: feeding and snack effects, toilet, cleaning, medicine, grooming,
//! sleep and wake, first aid and the dying / sitter endings (docs/game-spec.md §5).

use super::*;

impl Pet {
    /// Special snack effects (`care_eat_item` 0x1001021c, ids 6002–6050; 05-content §2.3).
    pub(super) fn snack_effect(&mut self, id: u16) {
        let cur = self.body_color();
        let color = |c: u8, p: &mut Pet| {
            p.next_color = c;
            p.raise(req::COLOR);
        };
        match id {
            6002..=6007 => color([0, 4, 6, 9, 11, 10][(id - 6002) as usize], self),
            6008 | 6009 => {
                let (lo, n) = if id == 6008 { (0, 9) } else { (9, 7) };
                let mut c = lo + (self.rand100() % n) as u8;
                if c == cur {
                    c = lo + (c - lo + 1) % n as u8;
                }
                color(c, self);
            }
            6046 => color(6, self),
            6049 => color(8, self),
            6050 => color(14, self),
            6010 => {
                self.add_gokigen(20);
                self.raise(req::HAPPY);
            }
            6011 if self.def.stage == 5 => self.raise(req::LOVE),
            6012 => self.raise(req::IMMUNITY),
            6013 => self.raise(req::DOSSARI),
            6014 => {
                self.status &= status::DIRTY;
                self.nap_timer = 0;
                self.raise(req::NAP);
                self.mark_done(req::NAP);
            }
            6015 | 6047 => match self.rand100() % if self.def.stage == 5 { 3 } else { 2 } {
                0 => {
                    self.add_gokigen(20);
                    self.raise(req::HAPPY);
                }
                1 => self.raise(req::IMMUNITY),
                _ => self.raise(req::LOVE),
            },
            6016 => self.status = self.status & status::DIRTY | status::DANCE,
            6017 => self.status = self.status & status::DIRTY | status::KIRAKIRA,
            6048 => self.raise(req::REDU_CM),
            _ => {}
        }
    }

    /// Complete one pending eating effect (`req_done` for COLOR/LOVE/IMMUNITY/HAPPY/REDU_CM).
    /// Returns the effect completed, if any.
    pub fn complete_effect(&mut self) -> Option<u8> {
        let id = req::EFFECTS.into_iter().find(|&r| self.is_pending(r))?;
        match id {
            req::COLOR => {
                self.color = self.next_color;
                self.collect_mission(crate::progress::m::COLORS, self.color as u32);
            }
            req::LOVE => self.status = self.status & status::DIRTY | status::LOVE,
            req::IMMUNITY => self.status = self.status & status::DIRTY | status::IMMUNITY,
            req::HAPPY => self.status = self.status & status::DIRTY | status::HAPPY,
            _ => self.miss_count = self.miss_count.saturating_sub(1),
        }
        self.mark_done(id);
        Some(id)
    }

    /// Like (+1) / dislike (−1) of a food (`food_pref_bonus` 0x100171fc).
    pub fn food_pref(&self, item: &Item) -> i32 {
        let mut pref = 0;
        if item.group != 0 && item.group == self.def.likes {
            pref += 1;
        } else if item.group != 0 && item.group == self.def.dislikes {
            pref -= 1;
        }
        if self.def.stage > 4 && item.id == 5002 {
            pref = -1;
        }
        if item.kind == KIND_SNACK && self.def.stage == 5 && item.snack_flag == 1 {
            pref += 1;
        }
        pref
    }

    pub fn feed(&mut self, item: &Item) -> bool {
        if self.cond != Cond::Normal
            || !matches!(item.kind, KIND_MEAL | KIND_SNACK)
            || (item.kind == KIND_MEAL && self.onaka == ONAKA_MAX)
        {
            return false;
        }
        let pref = self.food_pref(item);
        self.add_onaka(item.onaka as i32);
        self.log_food(item);
        self.snack_effect(item.id);
        if item.kind == KIND_SNACK {
            self.add_gokigen(item.gokigen as i32 + pref);
            self.count_mission(crate::progress::m::SNACKS, 1);
        } else {
            self.add_gokigen(pref);
            self.count_mission(crate::progress::m::MEALS, 1);
        }
        true
    }

    /// Take the pet to the toilet while it is restless: no poop, mood +1.
    pub fn toilet(&mut self) -> bool {
        if !self.is_pending(req::SOWASOWA) {
            return false;
        }
        self.clear(req::SOWASOWA);
        self.count_mission(crate::progress::m::TOILET, 1);
        self.add_gokigen(1);
        true
    }

    /// Clean all poop (`care_clean_unchi` 0x100108d4). Returns how many were removed.
    pub fn clean(&mut self) -> usize {
        let n = self.unchi.len();
        self.unchi.clear();
        // Grazing tamas' poop adds a bonus by how long since the last cleaning (`FUN_100108d4`).
        let k = (0..3).map(|f| self.grazing_in(f)).sum::<usize>().min(12);
        let bonus = match self.clean_timer {
            3600..=7199 => 2 * k,
            7200..=14_399 => k,
            _ => 0,
        };
        let reward = (n + bonus) as u32;
        self.count_mission(crate::progress::m::CLEANINGS, 1);
        self.count_mission(crate::progress::m::POOP_CLEANED, reward);
        self.unchi_meter = (self.unchi_meter as u32 + reward).min(60) as u8;
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
        self.count_mission(crate::progress::m::GROOMED, 1);
        self.add_gokigen(1);
        self.mark_done(req::OTEIRE);
    }

    /// Mood reward from a care action (e.g. +1 after cleaning poop).
    pub fn happy(&mut self, n: i32) {
        self.add_gokigen(n);
    }

    /// Put the pet to bed when it asks to sleep ("TIME TO SLEEP?" YES).
    pub fn put_to_bed(&mut self) -> bool {
        if !self.is_pending(req::SLEEP) {
            return false;
        }
        self.mark_done(req::SLEEP);
        self.count_mission(crate::progress::m::PUT_TO_BED, 1);
        self.status &= status::DIRTY;
        true
    }

    /// Treat an injury (`req_done(INJURY)`): mood +1, back to normal (or cold).
    pub fn treat_injury(&mut self) -> bool {
        if self.cond != Cond::Injury {
            return false;
        }
        self.clear(req::INJURY);
        self.add_gokigen(1);
        self.cond = if self.is_pending(req::COLD) { Cond::Cold } else { Cond::Normal };
        true
    }

    /// Wake the pet when it is asking to get up.
    pub fn wake(&mut self) -> bool {
        if !self.is_pending(req::WAKE) {
            return false;
        }
        self.mark_done(req::WAKE);
        self.count_mission(crate::progress::m::WOKE_UP, 1);
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

    /// The ancestor who comes to save a dying pet (`FUN_1000af5c`): the first grave of an
    /// old-age death whose spirit has not helped yet.
    pub fn dying_ancestor(&self) -> Option<usize> {
        self.graves.iter().position(|g| g.spirit)
    }

    /// End of the dying scene (`req_done(DYING, arg)`): saved by the ancestor in grave `saved`
    /// (its spirit is used up, cond back to normal), or dead (dirt cleared, DEAD raised).
    pub fn end_dying(&mut self, saved: Option<usize>) {
        if self.cond != Cond::Dying {
            return;
        }
        self.clear(req::DYING);
        match saved {
            Some(i) => {
                if let Some(g) = self.graves.get_mut(i) {
                    g.spirit = false;
                }
                self.cond = Cond::Normal;
                self.death_remain = 0;
            }
            None => {
                self.status &= !status::DIRTY;
                self.raise(req::DEAD);
            }
        }
    }

    /// Take the pet back from the tama sitter early (HELP_SITTER, END TAMA SITTING? → YES).
    pub fn end_sitter(&mut self) {
        if self.cond == Cond::Sitter {
            self.cond = Cond::Normal;
        }
    }
}
