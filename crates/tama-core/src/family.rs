//! Generations: releasing to the field (grazing), offline breeding, graves (05-content §2.10).

use crate::data::Data;
use crate::pet::{status, Pet};
use crate::progress::m;
use serde::{Deserialize, Serialize};

/// Grazing slots per field (ring of 8; up to 4 shown).
pub const GRAZING_SLOTS: usize = 8;
pub const GRAZING_SHOWN: usize = 4;
pub const BREEDS_PER_DAY: u8 = 3;
/// Offline breeding NPC id bases per field (`FUN_1004e1ec`): 16 adults each.
const NPC_BASE: [u16; 3] = [0x1000, 0x1016, 0xfea];

/// A tama living on in the field or in the graves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ghost {
    pub id: u16,
    pub color: u8,
    pub generation: u8,
}

/// A grave record (`g_death_log` 0x18000df0).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grave {
    pub ghost: Ghost,
    /// 1 illness, 3 hunger.
    pub cause: u8,
    pub days: u16,
}

impl Pet {
    fn ghost(&self) -> Ghost {
        Ghost { id: self.def.id, color: self.body_color(), generation: self.generation }
    }

    pub fn grazing_in(&self, field: u8) -> usize {
        self.grazing.get(field as usize).map_or(0, |g| g.len())
    }

    /// Grazing tamas shown in a field (the newest is not shown, at most 4; `FUN_10018978`).
    pub fn grazing_shown(&self, field: u8) -> Vec<Ghost> {
        let g = &self.grazing[field as usize];
        g.iter().rev().skip(1).take(GRAZING_SHOWN).copied().collect()
    }

    pub(crate) fn release_to(&mut self, field: u8) {
        self.add_grazing(field);
        self.adults_raised[field as usize] = (self.adults_raised[field as usize] + 1).min(99);
    }

    fn add_grazing(&mut self, field: u8) {
        let ghost = self.ghost();
        let g = &mut self.grazing[field as usize];
        if g.len() == GRAZING_SLOTS {
            g.remove(0);
        }
        g.push(ghost);
    }

    /// Generation change raises planet rank at 3 and 5–9.
    fn generation_rank_up(&mut self) {
        if self.rank == 3 || (5..=9).contains(&self.rank) {
            self.rank_up();
        }
    }

    /// Record the death in the 5-grave ring (`death_log_add` 0x1000affc).
    pub(crate) fn add_grave(&mut self) {
        let cause = if self.death_cause == crate::pet::DeathCause::Illness { 1 } else { 3 };
        if self.graves.len() == 5 {
            self.graves.remove(0);
        }
        self.graves.push(Grave { ghost: self.ghost(), cause, days: self.days });
    }

    /// NEW EGG (HELP_GRAZING, `FUN_1004dcd0`): the adult goes to its home field, generation
    /// restarts at 1 with the default colour.
    pub fn release_to_field(&mut self, data: &Data) -> bool {
        if self.def.stage != 5 {
            return false;
        }
        let field = crate::data::attr_to_field(self.def.attribute).unwrap_or(self.field);
        self.add_grazing(field);
        self.adults_raised[field as usize] = (self.adults_raised[field as usize] + 1).min(99);
        self.generation_rank_up();
        self.new_egg(data);
        self.generation = 1;
        true
    }

    /// Breeding candidates (4 NPC adults of the current field, unseen first, close rank first).
    pub fn breed_candidates(&self, data: &Data) -> Vec<u16> {
        let base = NPC_BASE[self.field as usize];
        let my_rank = self.def.rank as i32;
        let mut c: Vec<(bool, i32, u16)> = (base..base + 16)
            .filter_map(|id| data.character(id))
            .filter(|d| d.stage == 5)
            .map(|d| (self.seen_partners.contains(&d.id), (d.rank as i32 - my_rank).abs(), d.id))
            .collect();
        c.sort_unstable();
        c.into_iter().take(4).map(|(_, _, id)| id).collect()
    }

    /// Chance (0–100+) that a breeding attempt with `partner` succeeds.
    pub fn breed_chance(&self, data: &Data, partner: u16) -> i32 {
        let Some(p) = data.character(partner) else { return 0 };
        let mut chance = if self.def.rank == 5 {
            80
        } else {
            match (p.rank as i32 - self.def.rank as i32).abs() {
                0 => 80,
                1 => 50,
                2 => 40,
                _ => 20,
            }
        };
        chance += match self.onaka {
            0..=2 => -5,
            5.. => 5,
            _ => 0,
        };
        chance += match self.gokigen {
            0..=5 => -5,
            16.. => 5,
            _ => 0,
        };
        if self.status & status::LOVE != 0 {
            chance += 100;
        }
        chance
    }

    /// BREED (HELP_BREED `FUN_1004e1ec`): 3 tries a day; on success the adult goes to the field
    /// and an egg of the next generation inherits colour (50%) and looks (50%) from the partner.
    pub fn breed(&mut self, data: &Data, partner: u16) -> Option<bool> {
        if self.def.stage != 5 || self.breeds_today >= BREEDS_PER_DAY {
            return None;
        }
        self.breeds_today += 1;
        if !self.seen_partners.contains(&partner) {
            self.seen_partners.push(partner);
        }
        if (self.rand100() as i32) >= self.breed_chance(data, partner) {
            return Some(false);
        }
        let partner_def = data.character(partner)?;
        let field = crate::data::attr_to_field(self.def.attribute).unwrap_or(self.field);
        self.add_grazing(field);
        self.adults_raised[field as usize] = (self.adults_raised[field as usize] + 1).min(99);
        self.generation_rank_up();
        self.count_mission(m::BREEDS, 1);
        let generation = (self.generation + 1).min(99);
        let (keep_color, keep_looks) = (self.rand100() < 50, self.rand100() < 50);
        let color = if keep_color { self.body_color() } else { partner_def.color.min(15) };
        self.new_egg(data);
        self.generation = generation;
        self.color = color;
        self.collect_mission(m::COLORS, color as u32);
        self.looks = if keep_looks { None } else { Some(partner) };
        Some(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pet::tests::def;
    use crate::pet::NO_COLOR;

    fn data() -> Data {
        let mut d = crate::evolve::tests::data();
        for i in 0..16u16 {
            let mut c = def(0x1016 + i, 5, 60, 30, 0);
            c.rank = (i % 5) as u8 + 1;
            c.color = 3;
            d.characters.push(c);
        }
        d
    }

    fn adult(d: &Data) -> Pet {
        let mut a = def(4119, 5, 60, 30, 0);
        a.rank = 2;
        let mut p = Pet::new(a, 7);
        p.field = 1;
        p.rank = 3;
        let _ = d;
        p
    }

    #[test]
    fn release_to_field_starts_a_new_generation() {
        let d = data();
        let mut p = adult(&d);
        p.generation = 4;
        assert!(p.release_to_field(&d));
        assert_eq!((p.def.id, p.generation, p.rank), (1000, 1, 4));
        assert_eq!(p.grazing_in(1), 1);
        assert_eq!(p.color, NO_COLOR);
    }

    #[test]
    fn breeding_odds_and_daily_limit() {
        let d = data();
        let mut p = adult(&d);
        let c = p.breed_candidates(&d);
        assert_eq!(c.len(), 4);
        assert_eq!(d.character(c[0]).unwrap().rank, 2, "same rank first");
        p.onaka = 6;
        p.gokigen = 20;
        assert_eq!(p.breed_chance(&d, c[0]), 90);
        p.status = status::LOVE;
        assert!(p.breed_chance(&d, c[0]) > 100);
        assert_eq!(p.breed(&d, c[0]), Some(true));
        assert_eq!((p.def.id, p.generation, p.rank), (1000, 2, 4));
        assert_eq!(p.mission_progress(m::BREEDS), 1);
        assert_eq!(p.grazing_in(1), 1);
    }

    #[test]
    fn three_breeding_tries_a_day() {
        let d = data();
        let mut p = adult(&d);
        let worst = 0x1016 + 4; // rank 5 vs 2 → 20%
        p.onaka = 0;
        p.gokigen = 0;
        let mut tries = 0;
        while let Some(ok) = p.breed(&d, worst) {
            tries += 1;
            if ok {
                break;
            }
        }
        assert!(tries <= 3);
    }

    #[test]
    fn graves_keep_the_last_five() {
        let d = data();
        let mut p = adult(&d);
        for i in 0..7 {
            p.days = i;
            p.add_grave();
        }
        assert_eq!(p.graves.len(), 5);
        assert_eq!(p.graves[0].days, 2);
    }
}
