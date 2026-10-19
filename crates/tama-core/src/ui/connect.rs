//! CONNECTION on the planet level (`tushin_state` 0x1008cc54): PLAYDATE, GIFTS, PLANETS and
//! MINI GAMES (Tama Space Arcade, 09-arcade). The link cable is replaced by friend / gift codes,
//! which the platform shows and reads (`Game::take_connect`); the arcade's friend is a CPU.

use super::{Action, CardKind, Scene};
use crate::Game;

mod t {
    pub const PLAYDATE: u16 = 0x27;
    pub const GIFTS: u16 = 0x28;
    pub const PLANETS: u16 = 0x29;
    pub const GIFT_THIS: u16 = 0x70;
    pub const WAIT_UNTIL_AWAKE: u16 = 0x69;
    pub const UNAVAILABLE: u16 = 0xd2;
    /// CONNECTION menu entry 5 (XIP 0x1009bcb8) and the arcade menu (0x10091ca0).
    pub const MINI_GAMES: u16 = 0x4e;
    pub const HOCKEY: u16 = 0x335;
    pub const SHOOTING: u16 = 0x336;
    pub const STILL_CANT: u16 = 0x6b;
}

/// Arcade stand-in when no friend is known yet: Mametchi.
const DEFAULT_FRIEND: u16 = 4022;

/// Playdate animations by HOW PLAY 0–6 (0x10093912): play ×4, fight, eat, eaten.
/// Getting cleaned up before a connection.
const CLEAN_UP: u16 = 0x8e0;

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
                        (t::MINI_GAMES, Some(Action::Arcade)),
                    ],
                    idx: 0,
                    back: Some(Action::SpaceMenu),
                }
            }
            Action::ConnectPlaydate => {
                self.connect = Some("playdate".into());
                self.clean_up_first(Scene::Home)
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
            // `FUN_1008cffc` case 4: babies cannot play (stage < 3), then scene 0x83's menu.
            Action::Arcade => {
                if self.pet.def.stage < 3 {
                    return self.message(t::STILL_CANT, Action::Connect);
                }
                let menu = Scene::Menu {
                    items: vec![(t::HOCKEY, Some(Action::PlayArcade(0))), (t::SHOOTING, Some(Action::PlayArcade(1)))],
                    idx: 0,
                    back: Some(Action::Connect),
                };
                self.clean_up_first(menu)
            }
            Action::PlayArcade(g) => {
                // The CPU stands in for the most recent friend (role 1 in the scripts).
                let tama = self.pet.friends.last().map_or(DEFAULT_FRIEND, |f| f.tama);
                self.roles[1] = self.data.character(tama).map(|c| c.chr_id);
                let seed = self.rand();
                let game: Box<dyn crate::mini::MiniGame> = if g == 0 {
                    Box::new(crate::mini::arcade01::Arcade01::new(seed))
                } else {
                    // ponytail: the firmware picks the table with a rolling counter (`FUN_10002fe4(3)`); random here.
                    let tables = &self.data.mini.arcade02_patterns;
                    let pattern = tables.get(seed as usize % tables.len().max(1)).cloned().unwrap_or_default();
                    Box::new(crate::mini::arcade02::Arcade02::new(pattern, seed))
                };
                let session = crate::mini::Session { arcade: Some(g), ..crate::mini::Session::together(game) };
                Scene::Mini { session, lab: None }
            }
            _ => Scene::Home,
        }
    }

    /// After an arcade game (scene 0x84 `FUN_100883e0`): points and mission 11, then the arcade menu.
    pub(crate) fn after_arcade(&mut self, game: u8, result: u8) -> Scene {
        let points = self.pet.arcade_reward(game as usize, result as usize);
        let then = self.open(Action::Arcade);
        Scene::Points { points, then: Box::new(then) }
    }

    /// A dirty tama is cleaned before meeting a friend (`FUN_1008cffc` state 2000: OTEIRE done,
    /// anim 0x8e0), then the connection goes on.
    fn clean_up_first(&mut self, then: Scene) -> Scene {
        if self.pet.status & crate::pet::status::DIRTY == 0 {
            return then;
        }
        self.pet.groom();
        self.play(&[CLEAN_UP], (0, super::OnPress::None), None, then)
    }

    /// Items that can be given away: owned and not placed or worn.
    fn giftable(&self) -> Vec<u16> {
        let mut ids: Vec<u16> =
            self.pet.inventory.iter().map(|&(id, _)| id).filter(|&id| self.pet.free(id) > 0).collect();
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

impl Game {
    pub(super) fn update_friends(&mut self, idx: usize, pressed: u8) -> Scene {
        let idx = super::cycle(idx, self.pet.friends.len(), pressed);
        if pressed & crate::input::C != 0 {
            self.open(Action::Connect)
        } else {
            Scene::Friends { idx }
        }
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

    #[test]
    fn arcade_runs_as_a_mini_game_and_pays_points() {
        let mut g = Game::new(crate::evolve::tests::data(), 1);
        g.pet.def = g.data.character(2113).unwrap();
        g.pet.rank = 3;
        g.pet.rank_seen = 3;
        g.pet.onaka = 6;
        g.pet.gokigen = 20;
        g.level = Level::Space;
        g.frame(NOON, 0);
        let press = |g: &mut Game, b: u8| {
            g.frame(NOON, b);
            g.frame(NOON, 0);
        };
        press(&mut g, A); // planet menu, CONNECTION
        press(&mut g, B);
        for _ in 0..3 {
            press(&mut g, A); // MINI GAMES
        }
        press(&mut g, B);
        assert!(matches!(&g.scene, Scene::Menu { items, .. } if items[1].0 == t::SHOOTING));
        press(&mut g, A);
        press(&mut g, B);
        let Scene::Mini { session, .. } = &g.scene else { panic!("arcade game") };
        assert_eq!(session.arcade, Some(1));
        let pts = g.pet.points;
        let Scene::Points { points, .. } = g.after_arcade(0, 1) else { panic!("points") };
        assert_eq!((points, g.pet.points - pts), (100, 100));
        assert_eq!(g.pet.mission_progress(crate::progress::m::LINKS), 1);
        // The CPU's friend record: HOCKEY +1, SHOOTING with result >= 1 +2, capped at 4, result 5.
        let f = crate::social::Friend { device_uid: 7, chara_uid: 8, tama: 4022, color: 0, friendship: 1, result: 0 };
        g.pet.friends.push(f);
        g.after_arcade(0, 1);
        assert_eq!((g.pet.friends[0].friendship, g.pet.friends[0].result), (2, 5));
        g.after_arcade(1, 0);
        assert_eq!(g.pet.friends[0].friendship, 3);
        g.after_arcade(1, 2);
        assert_eq!(g.pet.friends[0].friendship, 4);
    }

    #[test]
    fn a_dirty_tama_is_cleaned_before_the_arcade() {
        let mut g = Game::new(crate::evolve::tests::data(), 1);
        g.pet.def = g.data.character(2113).unwrap();
        g.pet.status |= crate::pet::status::DIRTY;
        g.scene = g.open(Action::Arcade);
        assert!(matches!(g.scene, Scene::Play(_)), "the clean-up animation first");
        assert_eq!(g.pet.status & crate::pet::status::DIRTY, 0);
    }
}
