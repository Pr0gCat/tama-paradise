//! Shop, points spending, shop codes, cooking, area expansion and the tama sitter
//! (05-content §2.2–2.4, firmware `FUN_1004a438` and neighbours).

use crate::data::{Data, Item};
use crate::pet::{date, Cond, Pet};

pub const POINTS_MAX: u32 = 99_999;
pub const AREA_PRICE: u32 = 10_000;
pub const SITTER_PRICE: u32 = 300;

/// Shop tabs: food, snacks, outside toys, deco items, with the planet rank each needs.
pub const TABS: [(&[u8], u8); 4] = [(&[1], 1), (&[2], 1), (&[3, 4], 2), (&[5, 6, 7], 3)];

/// Per-kind inventory cap (XIP 0x10093d2c).
pub fn cap(kind: u8) -> u8 {
    match kind {
        1..=4 => 10,
        5..=7 => 5,
        8 => 1,
        _ => 10,
    }
}

/// Shop rotation day group 1–5: `(month + day) % 5 + 1`.
pub fn day_group(t: u64) -> u8 {
    let (_, m, d) = date(t);
    ((m + d) % 5 + 1) as u8
}

#[derive(Debug, PartialEq, Eq)]
pub enum ShopError {
    NotEnoughPoints,
    Full,
    NotOwned,
    Unavailable,
}

impl Pet {
    pub fn spend(&mut self, n: u32) -> bool {
        if n > self.points {
            return false;
        }
        self.points -= n;
        true
    }

    pub fn earn(&mut self, n: u32) {
        self.points = (self.points + n).min(POINTS_MAX);
        self.count_mission(crate::progress::m::EARNED, n);
    }

    /// Items for sale in `tab` today (`FUN_1001e7e0` / `FUN_1007d730`), sorted by id.
    pub fn shop_items(&self, data: &Data, tab: usize, now: u64) -> Vec<u16> {
        let (kinds, _) = TABS[tab];
        let today = day_group(now);
        let mut ids: Vec<u16> = data
            .items
            .iter()
            .filter(|i| kinds.contains(&i.kind) && i.buy != 0xffff && i.flags & 2 != 0)
            .filter(|i| self.rank >= i.rank && (i.day_group == 0 || i.day_group == today))
            .filter(|i| i.flags & 1 == 0 || self.unlocked.contains(&i.id))
            .map(|i| i.id)
            .collect();
        ids.sort_unstable();
        ids
    }

    pub fn tab_open(&self, tab: usize) -> bool {
        self.rank >= TABS[tab].1
    }

    /// Price today: half on days ending in 5 (food, snacks) or 0 (toys, deco items).
    pub fn price(item: &Item, now: u64) -> u32 {
        let d = date(now).2 % 10;
        let half = (d == 5 && matches!(item.kind, 1 | 2)) || (d == 0 && matches!(item.kind, 3..=7));
        if half { item.buy as u32 >> 1 } else { item.buy as u32 }
    }

    pub fn buy(&mut self, item: &Item, now: u64) -> Result<(), ShopError> {
        if self.count(item.id) >= cap(item.kind) {
            return Err(ShopError::Full);
        }
        if !self.spend(Self::price(item, now)) {
            return Err(ShopError::NotEnoughPoints);
        }
        self.give_capped(item, 1);
        Ok(())
    }

    pub fn sell(&mut self, item: &Item) -> Result<(), ShopError> {
        if self.free(item.id) == 0 {
            return Err(ShopError::NotOwned);
        }
        if item.sell == 0xffff || item.flags & 8 != 0 {
            return Err(ShopError::Unavailable);
        }
        self.give(item.id, -1);
        self.earn(item.sell as u32);
        Ok(())
    }

    /// Add items respecting the per-kind cap (`item_give` 0x1001e0d8).
    pub fn give_capped(&mut self, item: &Item, n: i32) {
        let room = cap(item.kind) as i32 - self.count(item.id) as i32;
        self.give(item.id, n.min(room));
    }

    /// Enter an 8-digit shop code (`FUN_1004bcf4`): unlocks its item.
    pub fn enter_code(&mut self, data: &Data, code: &str) -> Result<u16, ShopError> {
        let Some(&(item, _)) = data.shop_codes.iter().find(|(_, c)| c == code) else {
            return Err(ShopError::Unavailable);
        };
        if self.used_codes.iter().any(|c| c == code) {
            return Err(ShopError::Full);
        }
        self.used_codes.push(code.to_string());
        if !self.unlocked.contains(&item) {
            self.unlocked.push(item);
        }
        Ok(item)
    }

    /// Recipes the pet can cook: (product, material, materials owned).
    pub fn recipes(&self, data: &Data) -> Vec<(u16, u16, u8)> {
        data.recipes.iter().map(|&(p, m)| (p, m, self.count(m))).collect()
    }

    /// Cook (`FUN_1004b624`): uses 3 materials and eats the product at once.
    pub fn cook(&mut self, data: &Data, product: u16) -> Result<(), ShopError> {
        let &(_, material) = data.recipes.iter().find(|r| r.0 == product).ok_or(ShopError::Unavailable)?;
        if self.count(material) < 3 {
            return Err(ShopError::NotOwned);
        }
        if self.onaka >= 6 {
            return Err(ShopError::Full);
        }
        let item = data.item(product).ok_or(ShopError::Unavailable)?;
        self.give(material, -3);
        self.feed(&item);
        if let Some(k) = data.recipes.iter().position(|r| r.0 == product) {
            self.collect_mission(crate::progress::m::RECIPES, k as u32);
        }
        Ok(())
    }

    /// Buy the expansion area of `field` (`FUN_1004c1f8`).
    pub fn unlock_area(&mut self, field: u8) -> Result<(), ShopError> {
        if self.areas & 1 << field != 0 {
            return Err(ShopError::Full);
        }
        if !self.spend(AREA_PRICE) {
            return Err(ShopError::NotEnoughPoints);
        }
        self.areas |= 1 << field;
        Ok(())
    }

    /// Call the tama sitter (`FUN_1004d304`): child or older, 06:00–18:58, not hungry or sad,
    /// awake, 300 points.
    pub fn call_sitter(&mut self, now: u64) -> Result<(), ShopError> {
        let mins = now % 86_400 / 60;
        let ok = self.def.stage >= 3
            && (6 * 60..18 * 60 + 58).contains(&mins)
            && self.onaka > 0
            && self.gokigen > 0
            && self.cond == Cond::Normal
            && !self.is_sleeping_at(now);
        if !ok {
            return Err(ShopError::Unavailable);
        }
        if !self.spend(SITTER_PRICE) {
            return Err(ShopError::NotEnoughPoints);
        }
        self.cond = Cond::Sitter;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::KIND_MEAL;
    use crate::pet::tests::def;

    const DAY: u64 = 86_400;
    /// 2026-10-15 12:00 (a day ending in 5).
    const OCT15: u64 = 20_741 * DAY + 12 * 3600;

    fn data() -> Data {
        let mut d = crate::evolve::tests::data();
        let item = |id, kind, buy, rank, day_group, flags| Item { id, kind, buy, sell: buy / 2, rank, day_group, flags, onaka: 1, ..Default::default() };
        d.items = vec![
            item(5010, KIND_MEAL, 100, 1, 0, 2),
            item(5011, KIND_MEAL, 100, 3, 0, 2),
            item(5012, KIND_MEAL, 100, 0, 1, 2),
            item(5013, KIND_MEAL, 100, 0, 0, 3),
            item(5056, KIND_MEAL, 50, 0, 0, 2),
            item(5062, KIND_MEAL, 0xffff, 0, 0, 0),
            item(7001, 3, 400, 2, 0, 2),
        ];
        d.shop_codes = vec![(5013, "12345678".into())];
        d.recipes = vec![(5062, 5056)];
        d
    }

    fn pet() -> Pet {
        let mut p = Pet::new(def(2113, 3, 45, 20, 0), 1);
        p.rank = 1;
        p.points = 1000;
        p
    }

    #[test]
    fn shop_lists_by_rank_day_and_code() {
        let d = data();
        let mut p = pet();
        assert_eq!(date(OCT15), (2026, 10, 15));
        // day group (10 + 15) % 5 + 1 = 1
        assert_eq!(p.shop_items(&d, 0, OCT15), vec![5010, 5012, 5056]);
        assert!(!p.tab_open(2));
        p.enter_code(&d, "12345678").unwrap();
        assert_eq!(p.enter_code(&d, "12345678"), Err(ShopError::Full), "codes work once");
        assert!(p.shop_items(&d, 0, OCT15).contains(&5013));
    }

    #[test]
    fn half_price_on_days_ending_in_5_for_food() {
        let d = data();
        let food = d.item(5010).unwrap();
        assert_eq!(Pet::price(&food, OCT15), 50);
        assert_eq!(Pet::price(&food, OCT15 + DAY), 100);
        assert_eq!(Pet::price(&d.item(7001).unwrap(), OCT15 - 5 * DAY), 200, "toys half on day 10");
    }

    #[test]
    fn buy_and_sell() {
        let d = data();
        let mut p = pet();
        let food = d.item(5010).unwrap();
        p.buy(&food, OCT15 + DAY).unwrap();
        assert_eq!((p.points, p.count(5010)), (900, 1));
        p.points = 10;
        assert_eq!(p.buy(&food, OCT15 + DAY), Err(ShopError::NotEnoughPoints));
        p.sell(&food).unwrap();
        assert_eq!((p.points, p.count(5010)), (60, 0));
        assert_eq!(p.sell(&food), Err(ShopError::NotOwned));
    }

    #[test]
    fn inventory_cap_by_kind() {
        let d = data();
        let mut p = pet();
        p.points = 99_999;
        let food = d.item(5010).unwrap();
        for _ in 0..10 {
            p.buy(&food, OCT15 + DAY).unwrap();
        }
        assert_eq!(p.buy(&food, OCT15 + DAY), Err(ShopError::Full));
    }

    #[test]
    fn cooking_uses_three_materials() {
        let d = data();
        let mut p = pet();
        p.give(5056, 2);
        assert_eq!(p.cook(&d, 5062), Err(ShopError::NotOwned));
        p.give(5056, 1);
        p.cook(&d, 5062).unwrap();
        assert_eq!((p.count(5056), p.onaka), (0, 1));
    }

    #[test]
    fn area_and_sitter() {
        let mut p = pet();
        p.points = 10_300;
        p.unlock_area(1).unwrap();
        assert_eq!(p.unlock_area(1), Err(ShopError::Full));
        p.onaka = 3;
        p.gokigen = 3;
        let noon = 2 * DAY + 12 * 3600;
        p.advance_to(noon);
        p.call_sitter(noon).unwrap();
        assert_eq!((p.cond, p.points), (Cond::Sitter, 0));
        // The sitter feeds the pet until 19:00, then hands it back with mood −5.
        let start = p.last.unwrap();
        for s in (60..=7 * 3600 + 120).step_by(60) {
            p.advance_to(start + s);
        }
        assert_eq!(p.onaka, 3);
        assert_eq!((p.cond, p.gokigen), (Cond::Normal, 0));
    }
}
