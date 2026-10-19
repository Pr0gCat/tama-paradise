//! Planet rank and the 22 missions (05-content §2.6–2.7).

use crate::pet::Pet;

/// Mission targets (XIP 0x100942b4). Missions 8, 17–22 count collected bits.
pub const TARGETS: [u32; 22] =
    [500, 500, 100, 100, 100, 1000, 10, 7, 50, 50, 100, 50, 100, 99_999, 50, 50, 18, 12, 16, 16, 16, 16];
/// Mission text ids (XIP 0x100a4ce0): 169 + index.
pub const TEXT_FIRST: u16 = 169;
pub const REWARD: u32 = 1000;
pub const RANK_MAX: u8 = 10;

/// Mission indexes (0-based) and what they count.
pub mod m {
    pub const MEALS: usize = 0;
    pub const SNACKS: usize = 1;
    pub const GROOMED: usize = 2;
    pub const PLAY_PERFECT: usize = 3;
    pub const CLEANINGS: usize = 4;
    pub const POOP_CLEANED: usize = 5;
    pub const TOILET: usize = 6;
    pub const TRAVEL: usize = 7;
    pub const PUT_TO_BED: usize = 8;
    pub const WOKE_UP: usize = 9;
    pub const LINKS: usize = 10;
    pub const GIFTS: usize = 11;
    pub const LAB_PERFECT: usize = 12;
    pub const EARNED: usize = 13;
    pub const BREEDS: usize = 14;
    pub const HUNT_SUCCESS: usize = 15;
    pub const HUNT_FOODS: usize = 16;
    pub const RECIPES: usize = 17;
    pub const ADULTS_FIELD0: usize = 18;
    pub const COLORS: usize = 21;
}

/// Missions whose counter is a bit set (count = bits set).
fn is_bits(i: usize) -> bool {
    matches!(i, m::TRAVEL | m::HUNT_FOODS | m::RECIPES | 18..=21)
}

impl Pet {
    /// Increase counter mission `i` (capped like the firmware counters).
    pub fn count_mission(&mut self, i: usize, n: u32) {
        self.missions[i] = (self.missions[i] + n).min(TARGETS[i]);
    }

    /// Set bit `bit` of a collection mission.
    pub fn collect_mission(&mut self, i: usize, bit: u32) {
        self.missions[i] |= 1 << bit;
    }

    pub fn mission_progress(&self, i: usize) -> u32 {
        if is_bits(i) {
            self.missions[i].count_ones()
        } else {
            self.missions[i]
        }
    }

    /// Newly completed missions: marks and pays them (+1000 each, `FUN_10025368` / `FUN_10048bd0`).
    pub fn claim_missions(&mut self) -> Vec<usize> {
        let new: Vec<usize> =
            (0..22).filter(|&i| self.claimed & 1 << i == 0 && self.mission_progress(i) >= TARGETS[i]).collect();
        for &i in &new {
            self.claimed |= 1 << i;
            self.earn(REWARD);
        }
        new
    }

    /// Planet rank +1 (old value kept for the rank-up screen).
    pub fn rank_up(&mut self) {
        self.rank = (self.rank + 1).min(RANK_MAX);
    }

    /// Rank-up not shown yet (`DAT_18000c6c` high nibble < low nibble).
    /// A rank-up not shown yet: (rank last shown, new rank).
    pub fn take_rank_up(&mut self) -> Option<(u8, u8)> {
        (self.rank_seen < self.rank).then(|| {
            let old = std::mem::replace(&mut self.rank_seen, self.rank);
            (old, self.rank)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{Item, KIND_MEAL};
    use crate::pet::tests::def;

    #[test]
    fn meals_count_toward_a_mission_and_pay_once() {
        let mut p = Pet::new(def(2113, 3, 45, 20, 0), 1);
        p.missions[m::MEALS] = 499;
        p.feed(&Item { id: 5000, kind: KIND_MEAL, onaka: 1, ..Default::default() });
        assert_eq!(p.claim_missions(), vec![m::MEALS]);
        assert_eq!(p.points, 1000);
        assert!(p.claim_missions().is_empty());
        assert_eq!(p.mission_progress(m::EARNED), 1000, "earned points count too");
    }

    #[test]
    fn collections_count_bits() {
        let mut p = Pet::new(def(2113, 3, 45, 20, 0), 1);
        for b in 0..12 {
            p.collect_mission(m::RECIPES, b);
            p.collect_mission(m::RECIPES, b);
        }
        assert_eq!(p.mission_progress(m::RECIPES), 12);
        assert_eq!(p.claim_missions(), vec![m::RECIPES]);
    }

    #[test]
    fn rank_up_is_shown_once() {
        let mut p = Pet::new(def(2113, 3, 45, 20, 0), 1);
        p.rank_up();
        assert_eq!(p.take_rank_up(), Some((0, 1)));
        assert_eq!(p.take_rank_up(), None);
    }
}

#[cfg(test)]
mod evolve_tests {
    use crate::evolve::tests::data;
    use crate::pet::Pet;

    #[test]
    fn evolving_raises_planet_rank_to_three() {
        let d = data();
        let mut p = Pet::new(d.character(2113).unwrap(), 1);
        p.field = 1;
        p.field_log = vec![1; 4];
        p.age = 86_400;
        p.evolution_tick(1);
        p.evolve(&d);
        assert_eq!(p.rank, 0, "only the first child raises rank 0→1");
        let mut p = Pet::new(d.character(1001).unwrap(), 1);
        p.field = 1;
        p.field_log = vec![1; 4];
        p.raise(crate::pet::req::EVOLUTION);
        p.evolve(&d);
        assert_eq!(p.rank, 1);
        p.age = 86_400;
        p.evolution_tick(1);
        p.evolve(&d);
        assert_eq!(p.rank, 2);
        p.age = 86_400;
        p.evolution_tick(1);
        p.evolve(&d);
        assert_eq!((p.def.stage, p.rank), (5, 3));
        assert!(p.mission_progress(crate::progress::m::ADULTS_FIELD0 + 1) == 1);
    }
}
