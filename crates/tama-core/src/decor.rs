//! Outside toys, planet outfits and travel (docs/spec/10-decor-planet.md).

use crate::data::Data;
use crate::pet::Pet;
use crate::progress::m;

/// Toy slots per area: 0–3 small (kind 3), 4–5 large (kind 4); large 4+k covers small 2k, 2k+1.
pub const SLOTS: usize = 6;
pub const TRAVEL_COST: u8 = 60;
/// Romtype class for planet looks and home field (`romtype_field_class` 0x1002b314).
pub fn romtype_class(rom_type: u8) -> usize {
    crate::data::attr_to_field(rom_type).unwrap_or(0) as usize
}

impl Pet {
    /// Items currently placed or worn (`item_inuse_count` 0x1001e628).
    pub fn in_use(&self, id: u16) -> u8 {
        let placed = self.toys.iter().flatten().flatten().filter(|&&t| t == id).count();
        let worn = self.equip.iter().filter(|&&e| e == id).count();
        (placed + worn) as u8
    }

    /// How many of an item are free to place, wear or sell.
    pub fn free(&self, id: u16) -> u8 {
        self.count(id).saturating_sub(self.in_use(id))
    }

    /// Slots a toy kind can go in.
    pub fn toy_slots(kind: u8) -> &'static [usize] {
        if kind == 4 { &[4, 5] } else { &[0, 1, 2, 3] }
    }

    /// Slots that must be emptied to place a toy in `slot` (`toy_place_conflicts` 0x1006d804).
    fn conflicts(slot: usize) -> Vec<usize> {
        if slot >= 4 {
            let k = slot - 4;
            vec![slot, 2 * k, 2 * k + 1]
        } else {
            vec![4 + slot / 2, slot]
        }
    }

    /// Place a toy (SETSUBI): whatever is in the way goes back to storage. Returns removed toys.
    pub fn place_toy(&mut self, data: &Data, field: u8, area: u8, slot: usize, id: u16) -> Option<Vec<u16>> {
        let item = data.item(id)?;
        if !matches!(item.kind, 3 | 4) || !Pet::toy_slots(item.kind).contains(&slot) || self.free(id) == 0 {
            return None;
        }
        let area_slots = &mut self.toys[field as usize][area as usize];
        let mut removed = Vec::new();
        for c in Pet::conflicts(slot) {
            if area_slots[c] != 0 {
                removed.push(area_slots[c]);
                area_slots[c] = 0;
            }
        }
        area_slots[slot] = id;
        Some(removed)
    }

    /// A bad event wrecks this field's outside toys: each placed one is lost (`FUN_10016900`).
    pub fn lose_field_toys(&mut self) {
        for area in 0..2 {
            for s in 0..SLOTS {
                let id = std::mem::take(&mut self.toys[self.field as usize][area][s]);
                if id != 0 {
                    self.give(id, -1);
                }
            }
        }
    }

    pub fn remove_toy(&mut self, field: u8, area: u8, slot: usize) -> Option<u16> {
        let s = &mut self.toys[field as usize][area as usize][slot];
        (*s != 0).then(|| std::mem::take(s))
    }

    /// Wear an outfit on the planet (DECO PLANET): slot 0 hat, 1 glasses, 2 accessory; 0 = remove.
    pub fn wear(&mut self, data: &Data, slot: usize, id: u16) -> bool {
        if id != 0 {
            let Some(item) = data.item(id) else { return false };
            if item.kind as usize != 5 + slot || (self.equip[slot] != id && self.free(id) == 0) {
                return false;
            }
        }
        self.equip[slot] = id;
        true
    }

    /// Visitors for a matching outfit set (`equip_set_match` 0x1002820c).
    pub fn outfit_visitors(&self, data: &Data) -> Option<[u16; 2]> {
        data.decor.equip_sets.iter().find_map(|s| {
            let ok = (0..3).all(|k| s[k] == 0 || s[k] == self.equip[k]) && self.equip.iter().any(|&e| e != 0);
            ok.then_some([s[3], s[4]])
        })
    }

    /// Destinations open at this rank (`travel_dest_list` 0x1006f4f0).
    pub fn destinations(&self, data: &Data) -> Vec<usize> {
        (0..data.decor.travel.len()).filter(|&i| self.rank >= data.decor.travel[i].rank).collect()
    }

    /// Travel (`travel_reward` 0x1006666c): costs a full UNCHI METER, gives one of the place's two
    /// souvenirs in turn, mood +1, mission 8.
    pub fn travel(&mut self, data: &Data, dest: usize) -> Option<u16> {
        let d = *data.decor.travel.get(dest)?;
        if self.unchi_meter < TRAVEL_COST || self.rank < d.rank || self.def.stage < 3 {
            return None;
        }
        self.unchi_meter -= TRAVEL_COST;
        let toggle = (self.travel_toggle >> dest & 1) as usize;
        self.travel_toggle ^= 1 << dest;
        let souvenir = d.souvenirs[toggle];
        if let Some(item) = data.item(souvenir) {
            self.give_capped(&item, 1);
        }
        self.collect_mission(m::TRAVEL, dest as u32);
        self.happy(1);
        Some(souvenir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{Destination, Item};
    use crate::pet::tests::def;

    fn data() -> Data {
        let mut d = crate::evolve::tests::data();
        d.items = vec![
            Item { id: 7001, kind: 3, ..Default::default() },
            Item { id: 7002, kind: 4, ..Default::default() },
            Item { id: 8001, kind: 5, ..Default::default() },
            Item { id: 8002, kind: 7, ..Default::default() },
        ];
        d.decor.equip_sets = vec![[8001, 0, 8002, 9000, 9001]];
        d.decor.travel = vec![Destination { item: 9901, rank: 3, anim: 0x866, souvenirs: [8001, 8002], npcs: [167, 168] }];
        d
    }

    fn pet() -> Pet {
        let mut p = Pet::new(def(2113, 3, 0, 0, 0), 1);
        p.rank = 3;
        p
    }

    #[test]
    fn large_toy_pushes_small_toys_back() {
        let d = data();
        let mut p = pet();
        p.give(7001, 2);
        p.give(7002, 1);
        assert_eq!(p.place_toy(&d, 1, 0, 0, 7001), Some(vec![]));
        assert_eq!(p.place_toy(&d, 1, 0, 1, 7001), Some(vec![]));
        assert_eq!(p.free(7001), 0);
        assert_eq!(p.place_toy(&d, 1, 0, 2, 7001), None, "none left to place");
        assert_eq!(p.place_toy(&d, 1, 0, 4, 7002), Some(vec![7001, 7001]));
        assert_eq!(p.free(7001), 2);
        assert_eq!(p.place_toy(&d, 1, 0, 5, 7001), None, "small toys only in 0-3");
    }

    #[test]
    fn worn_items_are_in_use_and_sets_bring_visitors() {
        let d = data();
        let mut p = pet();
        p.give(8001, 1);
        p.give(8002, 1);
        assert!(!p.wear(&d, 1, 8001), "a hat is not glasses");
        assert!(p.wear(&d, 0, 8001));
        assert_eq!(p.free(8001), 0);
        assert_eq!(p.outfit_visitors(&d), None);
        assert!(p.wear(&d, 2, 8002));
        assert_eq!(p.outfit_visitors(&d), Some([9000, 9001]));
        assert!(p.wear(&d, 0, 0));
        assert_eq!(p.free(8001), 1);
    }

    #[test]
    fn travel_needs_a_full_meter_and_alternates_souvenirs() {
        let d = data();
        let mut p = pet();
        assert_eq!(p.travel(&d, 0), None);
        p.unchi_meter = 60;
        assert_eq!(p.travel(&d, 0), Some(8001));
        p.unchi_meter = 60;
        assert_eq!(p.travel(&d, 0), Some(8002));
        assert_eq!(p.mission_progress(m::TRAVEL), 1);
        assert_eq!(p.count(8001), 1);
    }
}
