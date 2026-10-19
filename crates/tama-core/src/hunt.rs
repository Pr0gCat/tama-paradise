//! Egg hunt (PSID_OSEWA_HUNT, `FUN_10041df8`; 05-content §2.8).

use crate::data::Data;
use crate::pet::{req, Cond, Pet};
use crate::progress::m;

pub const GOLDEN_PEACH: u16 = 6048;

#[derive(Debug, PartialEq, Eq)]
pub enum HuntError {
    /// Only 06:00–18:59.
    WrongTime,
    TooYoung,
    Rank,
    Busy,
    /// Fewer than 3 team members free today.
    NoSlots,
}

#[derive(Debug, PartialEq, Eq)]
pub struct HuntResult {
    /// 0 great, 1 good, 2 poor, 3 injured.
    pub result: u8,
    pub items: Vec<u16>,
}

/// NPC helper on the team list (TamaID 9014).
pub const NPC: u16 = 9014;

/// One entry of the team list (state 3000): who, and its bit in the daily use mask.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Member {
    /// `None` = the player's pet; otherwise a grazing tama or the NPC helper.
    pub tama: Option<crate::family::Ghost>,
    pub slot: u32,
}

impl Pet {
    /// Team list: the pet (slot 0), this field's grazing tamas newest first (slots 1–8), then
    /// 2–4 NPC helpers by planet rank, numbered up to slot 12.
    pub fn hunt_roster(&self) -> Vec<Member> {
        let mut r = vec![Member { tama: None, slot: 0 }];
        let grazing = self.grazing[self.field as usize].iter().rev().take(8);
        r.extend(grazing.enumerate().map(|(j, &g)| Member { tama: Some(g), slot: 1 + j as u32 }));
        let npcs = match self.rank {
            0..=5 => 2,
            6..=9 => 3,
            _ => 4,
        };
        let npc = crate::family::Ghost { id: NPC, color: 0xff, generation: 0 };
        r.extend((13 - npcs..13).map(|slot| Member { tama: Some(npc), slot }));
        r
    }

    pub fn hunt_slot_used(&self, slot: u32) -> bool {
        self.hunt_used[self.field as usize] & 1 << slot != 0
    }

    pub fn can_hunt(&self, now: u64) -> Result<(), HuntError> {
        if self.def.stage < 3 {
            return Err(HuntError::TooYoung);
        }
        if self.rank < 1 {
            return Err(HuntError::Rank);
        }
        if Pet::period(now) > 2 {
            return Err(HuntError::WrongTime);
        }
        if self.cond != Cond::Normal || self.is_sleeping_at(now) {
            return Err(HuntError::Busy);
        }
        if self.hunt_roster().iter().filter(|m| !self.hunt_slot_used(m.slot)).count() < 3 {
            return Err(HuntError::NoSlots);
        }
        Ok(())
    }

    /// Go hunting at VOLCANO (kind 0) or SPRING (1) with three team members (roster indices).
    /// Only the pet's mood matters, and only when it goes along; finds go to the inventory.
    pub fn hunt(&mut self, data: &Data, now: u64, kind: u8, team: [usize; 3]) -> Result<HuntResult, HuntError> {
        self.can_hunt(now)?;
        let roster = self.hunt_roster();
        let members: Vec<Member> = team.iter().filter_map(|&i| roster.get(i).copied()).collect();
        if members.len() != 3 || members.iter().any(|m| self.hunt_slot_used(m.slot)) {
            return Err(HuntError::NoSlots);
        }
        let f = self.field as usize;
        for m in &members {
            // The pet and NPCs are used up in every field, grazing tamas only in this one.
            let everywhere = m.tama.is_none_or(|t| t.id == NPC);
            for field in 0..3 {
                if everywhere || field == f {
                    self.hunt_used[field] |= 1 << m.slot;
                }
            }
        }
        let result = if members.iter().any(|m| m.tama.is_none()) {
            let r = self.rand100() + 1;
            match self.gokigen {
                20.. if r <= 50 => 0,
                1.. => 1,
                _ if r <= 10 => 3,
                _ => 2,
            }
        } else {
            1
        };
        let mut items = Vec::new();
        if result < 2 {
            let count = if result == 0 { 9 } else { 7 + self.rand100() % 2 };
            for _ in 0..count {
                let k = (self.rand100() % 4) as usize;
                if let Some(&[_, item, bit]) = data.hunt_finds.get(f * 8 + kind as usize * 4 + k) {
                    items.push(item);
                    self.collect_mission(m::HUNT_FOODS, bit as u32);
                }
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
            let free: Vec<usize> =
                (0..p.hunt_roster().len()).filter(|&i| !p.hunt_slot_used(p.hunt_roster()[i].slot)).collect();
            let r = p.hunt(&d, NOON, 0, [free[0], free[1], free[2]]).unwrap();
            hunts += 1;
            assert!(r.result <= 1);
            assert!(r.items.len() >= 7);
            greats += (r.result == 0) as u32;
        }
        // The pet and 2 helpers: one hunt a day.
        assert_eq!(hunts, 1);
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
            let r = p.hunt(&d, NOON, 1, [0, 1, 2]).unwrap();
            assert!(r.result >= 2);
            if r.result == 3 {
                assert_eq!(p.cond, Cond::Injury);
                hurt = true;
            }
        }
        assert!(hurt);
    }

    #[test]
    fn grazing_tamas_go_in_their_field_and_without_the_pet_the_result_is_good() {
        let d = data();
        let mut p = pet();
        p.gokigen = 0;
        p.field = 1;
        for id in [4119, 4120] {
            p.grazing[1].push(crate::family::Ghost { id, color: 0, generation: 1 });
        }
        let roster = p.hunt_roster();
        assert_eq!(roster.len(), 1 + 2 + 2);
        assert_eq!(roster.iter().map(|m| m.slot).collect::<Vec<_>>(), [0, 1, 2, 11, 12]);
        let r = p.hunt(&d, NOON, 1, [1, 2, 3]).unwrap();
        assert_eq!(r.result, 1, "the sad pet stayed home");
        assert!(r.items.iter().all(|i| (5068..5072).contains(i)), "FROZEN SPRING finds: {:?}", r.items);
        assert_eq!(p.hunt_used, [1 << 11, 0b110 | 1 << 11, 1 << 11]);
        assert_eq!(p.can_hunt(NOON), Err(HuntError::NoSlots), "only the pet and one helper left");
    }
}
