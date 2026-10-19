//! CONNECTION on the planet level (`tushin_state` 0x1008cc54): PLAYDATE, GIFTS, PLANETS.
//! The link cable is replaced by friend / gift codes, which the platform shows and reads
//! (`Game::take_connect`).

use super::{Action, CardKind, Scene};
use crate::Game;

mod t {
    pub const PLAYDATE: u16 = 0x27;
    pub const GIFTS: u16 = 0x28;
    pub const PLANETS: u16 = 0x29;
    pub const GIFT_THIS: u16 = 0x70;
    pub const WAIT_UNTIL_AWAKE: u16 = 0x69;
    pub const UNAVAILABLE: u16 = 0xd2;
}

/// Playdate animations by HOW PLAY 0–6 (0x10093912): play ×4, fight, eat, eaten.
pub(crate) const PLAYDATE_ANIM: [u16; 7] = [0x8c9, 0x8ca, 0x8cb, 0x8cc, 0x8cd, 0x8ce, 0x8cf];

impl Game {
    pub(super) fn open_connect(&mut self, a: Action) -> Scene {
        match a {
            Action::Connect => {
                if self.pet.last.is_some_and(|t| self.pet.is_sleeping_at(t)) {
                    return self.message(t::WAIT_UNTIL_AWAKE, Action::SpaceMenu);
                }
                Scene::Menu {
                    items: vec![
                        (t::PLAYDATE, Some(Action::ConnectPlaydate)),
                        (t::GIFTS, Some(Action::ConnectGifts)),
                        (t::PLANETS, Some(Action::Planets)),
                    ],
                    idx: 0,
                    back: Some(Action::SpaceMenu),
                }
            }
            Action::ConnectPlaydate => {
                self.connect = Some("playdate".into());
                Scene::Home
            }
            Action::ConnectGifts => {
                let items = self.giftable();
                if items.is_empty() {
                    self.connect = Some("receive".into());
                    return Scene::Home;
                }
                Scene::Card { kind: CardKind::Gift, items, idx: 0 }
            }
            Action::GiftItem(id) => match self.pet.gift(id) {
                Some(code) => {
                    self.connect = Some(format!("gift:{code}"));
                    Scene::Home
                }
                None => self.message(t::UNAVAILABLE, Action::Connect),
            },
            Action::Planets => {
                if self.pet.friends.is_empty() {
                    return self.message(t::UNAVAILABLE, Action::Connect);
                }
                Scene::Friends { idx: 0 }
            }
            _ => Scene::Home,
        }
    }

    /// Items that can be given away: owned and not placed or worn.
    fn giftable(&self) -> Vec<u16> {
        let mut ids: Vec<u16> = self.pet.inventory.iter().map(|&(id, _)| id).filter(|&id| self.pet.free(id) > 0).collect();
        ids.sort_unstable();
        ids
    }

    pub(super) fn pick_gift(&mut self, items: &[u16], idx: usize) -> Scene {
        Scene::Ask { text: t::GIFT_THIS, yes: true, on_yes: Action::GiftItem(items[idx]), back: Action::ConnectGifts }
    }

    /// PLANETS: one friend a page, their tama and friendship (1–4).
    pub(super) fn draw_friend(&mut self, idx: usize) {
        self.draw_list(&[], 0, Some(t::PLANETS));
        let Some(f) = self.pet.friends.get(idx).copied() else { return };
        self.draw_character(f.tama, Some(f.color), 64, 92);
        self.draw_digits(f.friendship as u32, 64, 114);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{A, B};
    use crate::Level;

    const NOON: u64 = 86_400 * 3 + 12 * 3600;

    #[test]
    fn gifts_from_the_planet_menu_hand_a_code_to_the_platform() {
        let mut d = crate::evolve::tests::data();
        d.items = vec![crate::data::Item { id: 5010, kind: 1, ..Default::default() }];
        let mut g = Game::new(d, 1);
        g.pet.def = g.data.character(2113).unwrap();
        g.pet.rank = 3;
        g.pet.rank_seen = 3;
        g.pet.onaka = 6;
        g.pet.gokigen = 20;
        g.pet.inventory.clear();
        g.pet.give(5010, 1);
        g.level = Level::Space;
        g.frame(NOON, 0);
        let press = |g: &mut Game, b: u8| {
            g.frame(NOON, b);
            g.frame(NOON, 0);
        };
        press(&mut g, A); // planet menu, CONNECTION
        press(&mut g, B);
        press(&mut g, A); // GIFTS
        press(&mut g, B);
        assert!(matches!(g.scene, Scene::Card { kind: CardKind::Gift, .. }));
        press(&mut g, B); // GIFT THIS ITEM?
        press(&mut g, B); // YES
        let req = g.take_connect().expect("a gift code for the platform");
        assert!(req.starts_with("gift:TG1-"), "{req}");
        assert_eq!(g.pet.count(5010), 0);
        assert_eq!(g.take_connect(), None);
    }
}
