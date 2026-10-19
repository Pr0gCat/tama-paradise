//! Lab menu, shop (buy / cook / sell / shop code / expansion) and the tama sitter screens.

use super::{Action, CardKind, Scene};
use crate::input::{A, B, C, DOWN, PUSH, UP};
use crate::shop::ShopError;
use crate::Game;

mod t {
    pub const INFORMATION: u16 = 0x00;
    pub const COLLECTION: u16 = 0x01;
    pub const MINI_GAMES: u16 = 0x02;
    pub const SHOP: u16 = 0x03;
    pub const HELP: u16 = 0x05;
    pub const SETTINGS: u16 = 0x06;
    pub const FIELD_NAMES: [u16; 3] = [0x0d, 0x0e, 0x0f];
    pub const TABS: [u16; 4] = [0x45, 0x46, 0x35, 0x36];
    pub const EXPANSION: u16 = 0x4d;
    pub const BUY: u16 = 0xc2;
    pub const PURCHASE_MADE: u16 = 0xc4;
    pub const COOK: u16 = 0xc6;
    pub const NOT_ENOUGH_FOOD: u16 = 0xc9;
    pub const SELL: u16 = 0xca;
    pub const SALE_MADE: u16 = 0xcb;
    pub const SHOP_CODE: u16 = 0xcc;
    pub const UNLOCK_ITEM: u16 = 0xcd;
    pub const INPUT_CODE: u16 = 0xcf;
    pub const WRONG_CODE: u16 = 0xd0;
    pub const CODE_USED: u16 = 0xd1;
    pub const UNAVAILABLE: u16 = 0xd2;
    pub const CALL_SITTER: u16 = 0xd3;
    pub const CANT_CALL_SITTER: u16 = 0xd6;
    pub const NOT_ENOUGH_POINTS: u16 = 0xd8;
}

const COOK_ICON_MATERIAL_Y: i32 = 112;

impl Game {
    /// Run a menu action and return the scene it opens.
    pub(crate) fn open(&mut self, a: Action) -> Scene {
        let now = self.pet.last.unwrap_or(0);
        match a {
            Action::Feed => Scene::FoodCategory { idx: 0 },
            Action::Groom => {
                self.play(&[super::script::GROOM_INTRO, super::script::GROOM], (4, super::OnPress::Groom), None, Scene::Home)
            }
            Action::CleanPoop => self.start_poop_clean(),
            Action::Lab => Scene::Menu {
                items: vec![
                    (t::INFORMATION, Some(Action::Missions)),
                    (t::MINI_GAMES, None),
                    (t::SHOP, Some(Action::Shop)),
                    (t::HELP, Some(Action::Help)),
                    (t::COLLECTION, None),
                    (t::SETTINGS, None),
                ],
                idx: 2,
                back: None,
            },
            Action::Shop => Scene::Menu {
                items: vec![
                    (t::BUY, Some(Action::BuyMenu)),
                    (t::COOK, Some(Action::Cook)),
                    (t::SELL, Some(Action::Sell)),
                    (t::SHOP_CODE, Some(Action::Code)),
                    (t::EXPANSION, Some(Action::Expand)),
                ],
                idx: 0,
                back: Some(Action::Lab),
            },
            Action::BuyMenu => Scene::Menu {
                items: (0..4).map(|i| (t::TABS[i], self.pet.tab_open(i).then_some(Action::BuyTab(i as u8)))).collect(),
                idx: 0,
                back: Some(Action::Shop),
            },
            Action::BuyTab(tab) => {
                let items = self.pet.shop_items(&self.data, tab as usize, now);
                if items.is_empty() {
                    return self.message(t::UNAVAILABLE, Action::BuyMenu);
                }
                Scene::Card { kind: CardKind::Buy(tab), items, idx: 0 }
            }
            Action::Sell => {
                let items = self.sellable();
                if items.is_empty() {
                    return self.message(t::UNAVAILABLE, Action::Shop);
                }
                Scene::Card { kind: CardKind::Sell, items, idx: 0 }
            }
            Action::Cook => {
                let items = self.pet.recipes(&self.data).into_iter().map(|r| r.0).collect();
                Scene::Card { kind: CardKind::Cook, items, idx: 0 }
            }
            Action::Code => Scene::Code { digits: [0; 8], pos: 0 },
            Action::Expand => Scene::Menu {
                items: (0..3u8)
                    .map(|f| (t::FIELD_NAMES[f as usize], (self.pet.areas & 1 << f == 0).then_some(Action::ExpandField(f))))
                    .collect(),
                idx: self.pet.field as usize,
                back: Some(Action::Shop),
            },
            Action::ExpandField(f) => match self.pet.unlock_area(f) {
                Ok(()) => self.message(t::PURCHASE_MADE, Action::Shop),
                Err(_) => self.message(t::NOT_ENOUGH_POINTS, Action::Expand),
            },
            Action::Missions => Scene::Missions { idx: 0 },
            Action::Help => Scene::Ask { text: t::CALL_SITTER, yes: true, on_yes: Action::CallSitter, back: Action::Lab },
            Action::CallSitter => match self.pet.call_sitter(now) {
                Ok(()) => Scene::Home,
                Err(ShopError::NotEnoughPoints) => self.message(t::NOT_ENOUGH_POINTS, Action::Lab),
                Err(_) => self.message(t::CANT_CALL_SITTER, Action::Lab),
            },
        }
    }

    fn message(&mut self, text: u16, then: Action) -> Scene {
        let then = self.open(then);
        Scene::Message { text, then: Box::new(then) }
    }

    /// Inventory items that can be sold (`FUN_1004af0c`).
    fn sellable(&self) -> Vec<u16> {
        let mut ids: Vec<u16> = self
            .pet
            .inventory
            .iter()
            .filter(|(id, n)| *n > 0 && self.data.item(*id).is_some_and(|i| i.sell != 0xffff && i.flags & 8 == 0))
            .map(|(id, _)| *id)
            .collect();
        ids.sort_unstable();
        ids
    }

    pub(super) fn update_card(&mut self, kind: CardKind, items: Vec<u16>, mut idx: usize, pressed: u8) -> Scene {
        let n = items.len().max(1);
        if pressed & (A | UP) != 0 {
            idx = (idx + 1) % n;
        }
        if pressed & DOWN != 0 {
            idx = (idx + n - 1) % n;
        }
        let back = match kind {
            CardKind::Buy(_) => Action::BuyMenu,
            _ => Action::Shop,
        };
        if pressed & C != 0 {
            return self.open(back);
        }
        if pressed & (B | PUSH) == 0 {
            return Scene::Card { kind, items, idx };
        }
        let Some(item) = items.get(idx).and_then(|&id| self.data.item(id)) else { return self.open(back) };
        let now = self.pet.last.unwrap_or(0);
        let stay = |g: &mut Game| match kind {
            CardKind::Sell => g.open(Action::Sell),
            _ => Scene::Card { kind, items: items.clone(), idx },
        };
        let (text, ok) = match kind {
            CardKind::Buy(_) => match self.pet.buy(&item, now) {
                Ok(()) => (t::PURCHASE_MADE, true),
                Err(ShopError::NotEnoughPoints) => (t::NOT_ENOUGH_POINTS, false),
                Err(_) => (t::UNAVAILABLE, false),
            },
            CardKind::Sell => match self.pet.sell(&item) {
                Ok(()) => (t::SALE_MADE, true),
                Err(_) => (t::UNAVAILABLE, false),
            },
            CardKind::Cook => match self.pet.cook(&self.data, item.id) {
                Ok(()) => {
                    // Cooking serves the dish at once, like feeding.
                    let then = Scene::Card { kind, items, idx };
                    return self.play(&[super::script::SERVE[0], super::script::EAT[0][0]], (0, super::OnPress::None), Some(item.eat_sprite), then);
                }
                Err(ShopError::NotOwned) => (t::NOT_ENOUGH_FOOD, false),
                Err(_) => (t::UNAVAILABLE, false),
            },
        };
        self.sfx(if ok { 1 } else { 0x0b }, 100);
        let then = stay(self);
        Scene::Message { text, then: Box::new(then) }
    }

    pub(super) fn update_code(&mut self, mut digits: [u8; 8], mut pos: usize, pressed: u8) -> Scene {
        if pressed & (A | UP) != 0 {
            digits[pos] = (digits[pos] + 1) % 10;
        }
        if pressed & DOWN != 0 {
            digits[pos] = (digits[pos] + 9) % 10;
        }
        if pressed & C != 0 {
            if pos == 0 {
                return self.open(Action::Shop);
            }
            pos -= 1;
        }
        if pressed & (B | PUSH) != 0 {
            pos += 1;
            if pos == 8 {
                let code: String = digits.iter().map(|d| char::from(b'0' + d)).collect();
                let text = match self.pet.enter_code(&self.data, &code) {
                    Ok(_) => t::UNLOCK_ITEM,
                    Err(ShopError::Full) => t::CODE_USED,
                    Err(_) => t::WRONG_CODE,
                };
                return self.message(text, Action::Shop);
            }
        }
        Scene::Code { digits, pos }
    }

    pub(super) fn draw_card(&mut self, kind: CardKind, items: &[u16], idx: usize) {
        self.draw_food_card(items, idx);
        let Some(item) = items.get(idx).and_then(|&id| self.data.item(id)) else { return };
        let now = self.pet.last.unwrap_or(0);
        match kind {
            CardKind::Buy(_) => {
                let price = crate::pet::Pet::price(&item, now);
                self.draw_number(price, 64, 115, 1);
            }
            CardKind::Sell => self.draw_number(item.sell as u32, 64, 115, 1),
            CardKind::Cook => {
                let material = self.data.recipes.iter().find(|r| r.0 == item.id).map_or(0, |r| r.1);
                if let Some(m) = self.data.item(material) {
                    self.screen.blit(m.icon_sprite, 0, 40, COOK_ICON_MATERIAL_Y, 64, false, None);
                }
                self.draw_number(self.pet.count(material) as u32, 72, 115, 1);
            }
        }
        // Points balance, top right.
        self.draw_number(self.pet.points, 108, 8, 1);
    }

    /// One mission: description, progress / target, "CLEARED" when paid out.
    pub(super) fn draw_mission(&mut self, i: usize) {
        use crate::progress::{TARGETS, TEXT_FIRST};
        self.draw_list(&[], 0, Some(TEXT_FIRST + i as u16));
        let (have, need) = (self.pet.mission_progress(i), TARGETS[i]);
        self.draw_number(i as u32 + 1, 12, 120, 0);
        self.draw_number(have.min(need), 46, 102, 0);
        self.draw_number(need, 94, 102, 0);
        self.screen.rect(68, 101, 4, 2, [255, 255, 255]);
        if self.pet.claimed & 1 << i != 0 {
            self.draw_text(0xa5, 80, 120, true, 0);
        }
    }

    pub(super) fn draw_code(&mut self, digits: &[u8; 8], pos: usize) {
        self.draw_list(&[], 0, Some(t::INPUT_CODE));
        for (i, &d) in digits.iter().enumerate() {
            let x = 16 + i as i32 * 13;
            if i == pos {
                self.screen.rect(x - 6, 96, 12, 2, [40, 40, 120]);
            }
            self.draw_number(d as u32, x, 86, 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::data::{Item, KIND_MEAL};
    use crate::input::{B, PUSH};
    use crate::Game;

    const NOON: u64 = 86_400 * 3 + 12 * 3600;

    fn game() -> Game {
        let mut d = crate::evolve::tests::data();
        d.items = vec![Item { id: 5010, kind: KIND_MEAL, buy: 100, sell: 50, flags: 2, onaka: 1, ..Default::default() }];
        let mut g = Game::new(d, 1);
        g.pet.def = g.data.character(2113).unwrap();
        g.pet.rank = 1;
        g.pet.points = 500;
        g.pet.onaka = 6;
        g.pet.gokigen = 20;
        g.frame(NOON, 0);
        g
    }

    fn press(g: &mut Game, bits: u8) {
        g.frame(NOON, bits);
        g.frame(NOON, 0);
    }

    #[test]
    fn long_press_opens_lab_and_buys_food() {
        let mut g = game();
        for _ in 0..14 {
            g.frame(NOON, PUSH);
        }
        g.frame(NOON, 0);
        assert!(matches!(g.scene, super::Scene::Menu { idx: 2, .. }), "lab menu with SHOP selected");
        press(&mut g, B); // SHOP
        press(&mut g, B); // BUY
        press(&mut g, B); // FOOD tab
        assert!(matches!(g.scene, super::Scene::Card { .. }));
        press(&mut g, B); // buy
        assert_eq!((g.pet.points, g.pet.count(5010)), (400, 1));
        assert!(matches!(g.scene, super::Scene::Message { .. }));
    }
}
