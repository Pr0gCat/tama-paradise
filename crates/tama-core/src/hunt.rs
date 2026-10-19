//! Egg hunt (PSID_OSEWA_HUNT, `FUN_10041df8`; 05-content §2.8).

use crate::data::Data;
use crate::pet::{req, Cond, Pet};
use crate::progress::m;

pub const GOLDEN_PEACH: u16 = 6048;
/// Daily slots per field (`DAT_18000ce8[field]`, 13 bits).
pub const SLOTS: u32 = 13;

#[derive(Debug, PartialEq, Eq)]
pub enum HuntError {
    /// Only 06:00–18:59.
    WrongTime,
    TooYoung,
    Rank,
    Busy,
    /// Fewer than 3 free slots in this field today.
    NoSlots,
}

#[derive(Debug, PartialEq, Eq)]
pub struct HuntResult {
    /// 0 great, 1 good, 2 poor, 3 injured.
    pub result: u8,
    pub items: Vec<u16>,
}

impl Pet {
    /// Party size: the pet, the field's grazing tamas, and NPC helpers by planet rank.
    pub fn hunt_party(&self) -> u32 {
        let npcs = match self.rank {
            0..=5 => 2,
            6..=9 => 3,
            _ => 4,
        };
        1 + self.grazing_in(self.field) as u32 + npcs
    }

    pub fn can_hunt(&self, now: u64) -> Result<(), HuntError> {
        if Pet::period(now) > 2 {
            return Err(HuntError::WrongTime);
        }
        if self.def.stage < 3 {
            return Err(HuntError::TooYoung);
        }
        if self.rank < 1 {
            return Err(HuntError::Rank);
        }
        if self.cond != Cond::Normal || self.is_sleeping_at(now) {
            return Err(HuntError::Busy);
        }
        if SLOTS - self.hunt_used[self.field as usize].count_ones() < 3 {
            return Err(HuntError::NoSlots);
        }
        Ok(())
    }

    /// Go hunting: the result depends on mood and a 1–100 roll; finds go to the inventory.
    pub fn hunt(&mut self, data: &Data, now: u64) -> Result<HuntResult, HuntError> {
        self.can_hunt(now)?;
        let f = self.field as usize;
        let party = self.hunt_party().min(SLOTS);
        let used = &mut self.hunt_used[f];
        let mut n = 0;
        for bit in 0..SLOTS {
            if n < party && *used & 1 << bit == 0 {
                *used |= 1 << bit;
                n += 1;
            }
        }
        let r = self.rand100() + 1;
        let result = match self.gokigen {
            20.. if r <= 50 => 0,
            1.. => 1,
            _ if r <= 10 => 3,
            _ => 2,
        };
        let count = if result == 0 { 9 } else { 7 + self.rand100() % 2 };
        let mut items = Vec::new();
        for _ in 0..count {
            let kind = (self.rand100() % 2) as usize;
            let k = (self.rand100() % 4) as usize;
            if let Some(&[_, item, bit]) = data.hunt_finds.get(f * 8 + kind * 4 + k) {
                items.push(item);
                self.collect_mission(m::HUNT_FOODS, bit as u32);
            }
        }
        if result == 0 {
            self.count_mission(m::HUNT_SUCCESS, 1);
            items.push(GOLDEN_PEACH);
        }
        if result == 3 && self.cond == Cond::Normal {
            self.raise(req::INJURY);
        }
        for &id in &items {
            if let Some(item) = data.item(id) {
                self.give_capped(&item, 1);
            }
        }
        Ok(HuntResult { result, items })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::Item;
    use crate::pet::tests::def;

    const NOON: u64 = 3 * 86_400 + 12 * 3600;

    fn data() -> Data {
        let mut d = crate::evolve::tests::data();
        d.hunt_finds = (0..24).map(|i| [0, 5056 + i, i]).chain([[0, GOLDEN_PEACH, 0]]).collect();
        d.items = (5056..5080).chain([GOLDEN_PEACH]).map(|id| Item { id, kind: 1, ..Default::default() }).collect();
        d
    }

    fn pet() -> Pet {
        let mut p = Pet::new(def(2113, 3, 0, 0, 0), 1);
        p.rank = 1;
        p.advance_to(NOON);
        p
    }

    #[test]
    fn needs_daytime_rank_and_child_stage() {
        let mut p = pet();
        assert_eq!(p.can_hunt(NOON + 8 * 3600), Err(HuntError::WrongTime));
        p.rank = 0;
        assert_eq!(p.can_hunt(NOON), Err(HuntError::Rank));
    }

    #[test]
    fn happy_pet_finds_more_and_slots_run_out() {
        let d = data();
        let mut p = pet();
        p.gokigen = 20;
        let mut greats = 0;
        let mut hunts = 0;
        while p.can_hunt(NOON).is_ok() {
            let r = p.hunt(&d, NOON).unwrap();
            hunts += 1;
            assert!(r.result <= 1);
            assert!(r.items.len() >= 7);
            greats += (r.result == 0) as u32;
        }
        // 3 slots per hunt (pet + 2 helpers) out of 13 → 4 hunts a day.
        assert_eq!(hunts, 4);
        assert_eq!(p.mission_progress(m::HUNT_SUCCESS), greats);
        assert!(p.mission_progress(m::HUNT_FOODS) > 0);
        assert!(!p.inventory.is_empty());
    }

    #[test]
    fn sad_pet_can_get_hurt() {
        let d = data();
        let mut hurt = false;
        for seed in 1..200 {
            let mut p = Pet::new(def(2113, 3, 0, 0, 0), seed);
            p.rank = 1;
            p.advance_to(NOON);
            let r = p.hunt(&d, NOON).unwrap();
            assert!(r.result >= 2);
            if r.result == 3 {
                assert_eq!(p.cond, Cond::Injury);
                hurt = true;
            }
        }
        assert!(hurt);
    }
}
