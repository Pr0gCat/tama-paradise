//! EGG HUNT screens (PSID_OSEWA_HUNT, `FUN_10041df8`; 09-minigames-together §2.9).

use super::{Action, Scene};
use crate::hunt::HuntError;
use crate::input::{A, B, C, DOWN, PUSH, UP};
use crate::Game;

mod t {
    pub const VOLCANO: u16 = 0x42;
    pub const SPRING: u16 = 0x43;
    pub const BRING_THEM: u16 = 0x9a;
    pub const CANT_BRING_NOW: u16 = 0x9b;
    pub const NO_MORE_TODAY: u16 = 0x9c;
    pub const WAIT_UNTIL_AWAKE: u16 = 0x69;
    pub const STILL_CANT_USE: u16 = 0x6b;
    pub const FROM_LEVEL_1: u16 = 0xf2;
}

const SCENERY: u16 = 0x7e;
const PANEL: u16 = 0x85;
const CARD: u16 = 0x83;
const CURSOR: u16 = 0x80;
const ORDER_DIGITS: u16 = 0x7f;
const CARD_POS: [(i32, i32); 4] = [(30, 34), (87, 34), (30, 92), (87, 92)];
const DEPART: u16 = 0x7ab;
/// Trip animation `[field][kind][result]` (0x10093920).
const TRIP: [[[u16; 4]; 2]; 3] = [
    [[0x754, 0x756, 0x758, 0x75a], [0x755, 0x757, 0x759, 0x75a]],
    [[0x75d, 0x75f, 0x761, 0x763], [0x75e, 0x760, 0x762, 0x763]],
    [[0x78e, 0x790, 0x792, 0x794], [0x78f, 0x791, 0x793, 0x794]],
];
/// Way home `[field][result]` (0x10095778).
const HOME: [[u16; 4]; 3] = [[0x75b, 0x75c, 0x7c7, 0x7c8], [0x764, 0x765, 0x7c7, 0x7c8], [0x795, 0x796, 0x7c7, 0x7c8]];
/// Egg sprites by field (64×64, 3 frames cracking open).
const EGG: [u16; 3] = [0x8b, 0x8c, 0x8a];
const PEACH_SPRITE: u16 = 0x706;

impl Game {
    pub(super) fn open_hunt(&mut self, a: Action) -> Scene {
        let now = self.pet.last.unwrap_or(0);
        match a {
            Action::Hunt => match self.pet.can_hunt(now) {
                Err(HuntError::Busy) => self.hunt_message(t::WAIT_UNTIL_AWAKE, Scene::Home),
                Err(HuntError::TooYoung) => self.hunt_message(t::STILL_CANT_USE, Scene::Home),
                Err(HuntError::Rank) => self.hunt_message(t::FROM_LEVEL_1, Scene::Home),
                _ => Scene::Menu {
                    items: vec![(t::VOLCANO, Some(Action::HuntKind(0))), (t::SPRING, Some(Action::HuntKind(1)))],
                    idx: 0,
                    back: None,
                },
            },
            Action::HuntKind(kind) => match self.pet.can_hunt(now) {
                Ok(()) => Scene::HuntTeam { kind, idx: 0, picked: Vec::new() },
                Err(HuntError::NoSlots) => self.hunt_message(t::NO_MORE_TODAY, Scene::Home),
                Err(_) => self.hunt_message(t::CANT_BRING_NOW, Scene::Home),
            },
            Action::HuntGo { kind, team } => {
                let mut team = team.map(|i| i as usize);
                // Role 0 is the pet when it goes along; the others fill roles 1 and 2.
                team.sort_by_key(|&i| i != 0);
                let roster = self.pet.hunt_roster();
                for (r, &i) in team.iter().enumerate() {
                    self.roles[r] = roster.get(i).and_then(|m| m.tama).and_then(|g| self.data.character(g.id)).map(|c| c.chr_id);
                }
                match self.pet.hunt(&self.data, now, kind, team) {
                    Ok(r) => {
                        let f = self.pet.field as usize;
                        let then = if r.result < 2 { Scene::HuntEggs { items: r.items, t: 0, peach: false } } else { Scene::Home };
                        let trip = TRIP[f][kind as usize & 1][r.result as usize];
                        self.play(&[DEPART, trip, HOME[f][r.result as usize]], (0, super::OnPress::None), None, then)
                    }
                    Err(_) => self.hunt_message(t::CANT_BRING_NOW, Scene::Home),
                }
            }
            _ => Scene::Home,
        }
    }

    fn hunt_message(&mut self, text: u16, then: Scene) -> Scene {
        Scene::Message { text, then: Box::new(then) }
    }

    /// Team picker (`FUN_1002e120`): 2×2 cards a page; B picks, C drops the last pick.
    pub(super) fn update_hunt_team(&mut self, kind: u8, mut idx: usize, mut picked: Vec<usize>, pressed: u8) -> Scene {
        let roster = self.pet.hunt_roster();
        let n = roster.len();
        if pressed & (A | UP) != 0 {
            idx = (idx + 1) % n;
            self.sfx(0, 100);
        }
        if pressed & DOWN != 0 {
            idx = (idx + n - 1) % n;
            self.sfx(0, 100);
        }
        if pressed & C != 0 {
            self.sfx(2, 100);
            if picked.pop().is_none() {
                return self.open(Action::Hunt);
            }
        }
        if pressed & (B | PUSH) != 0 {
            if picked.contains(&idx) || self.pet.hunt_slot_used(roster[idx].slot) {
                self.sfx(0x0b, 100);
            } else {
                picked.push(idx);
                self.sfx(1, 100);
                if picked.len() == 3 {
                    let team = [picked[0] as u8, picked[1] as u8, picked[2] as u8];
                    return Scene::Ask { text: t::BRING_THEM, yes: true, on_yes: Action::HuntGo { kind, team }, back: Action::HuntKind(kind) };
                }
            }
        }
        Scene::HuntTeam { kind, idx, picked }
    }

    pub(super) fn draw_hunt_team(&mut self, idx: usize, picked: &[usize]) {
        self.screen.blit(SCENERY, 0, 64, 64, 64, false, None);
        let roster = self.pet.hunt_roster();
        let page = idx / 4;
        for (k, &(x, y)) in CARD_POS.iter().enumerate() {
            let i = page * 4 + k;
            let Some(m) = roster.get(i) else { break };
            let gray = picked.contains(&i) || self.pet.hunt_slot_used(m.slot);
            self.screen.blit(CARD, 0, x, y, 64, false, gray.then_some((CARD, 1)));
            match m.tama {
                None => self.draw_character_pose(self.pet.def.id, Some(self.pet.body_color()), 30, x + 1, y + 19),
                Some(g) => self.draw_character_pose(g.id, Some(g.color), 30, x + 1, y + 19),
            }
            if let Some(order) = picked.iter().position(|&p| p == i) {
                self.screen.blit(ORDER_DIGITS, order as u16 + 1, x + 16, y - 15, 64, false, None);
            }
        }
        let (cx, cy) = CARD_POS[idx % 4];
        self.screen.blit(CURSOR, 0, cx, cy, 64, false, None);
    }

    pub(super) fn update_hunt_eggs(&mut self, items: Vec<u16>, t: u16, peach: bool, pressed: u8) -> Scene {
        if t == 0 {
            self.sfx(23, 100);
        }
        // Any key after 100 frames, or 200 frames: the golden peach (great result), then HOME.
        if (t >= 100 && pressed & (A | B | C | PUSH) != 0) || t >= 200 {
            return if !peach && items.contains(&crate::hunt::GOLDEN_PEACH) {
                Scene::HuntEggs { items, t: 0, peach: true }
            } else {
                Scene::Home
            };
        }
        Scene::HuntEggs { items, t: t + 1, peach }
    }

    /// Eggs crack open in a 3×3 grid, showing what was inside (state 5000).
    pub(super) fn draw_hunt_eggs(&mut self, items: &[u16], t: u16, peach: bool) {
        self.screen.blit(SCENERY, 0, 64, 64, 64, false, None);
        self.screen.blit(PANEL, 0, 64, 64, 64, false, None);
        let egg = EGG[self.pet.field as usize % 3];
        if peach {
            if t < 30 {
                self.screen.blit(egg, (t / 10) as u16, 64, 64, 64, false, None);
            } else {
                self.screen.blit(PEACH_SPRITE, 0, 64, 64, 64, false, None);
            }
            return;
        }
        let eggs = items.iter().filter(|&&i| i != crate::hunt::GOLDEN_PEACH).take(9);
        for (i, &id) in eggs.enumerate() {
            let (x, y) = (28 + (i % 3) as i32 * 36, 32 + (i / 3) as i32 * 32);
            if t < 30 {
                self.screen.blit(egg, t / 10, x, y, 64, false, None);
            } else if let Some(item) = self.data.item(id) {
                self.screen.blit(item.eat_sprite, 0, x, y, 64, false, None);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::Item;

    const NOON: u64 = 86_400 * 3 + 12 * 3600;

    #[test]
    fn hunt_from_the_field_menu() {
        let mut d = crate::evolve::tests::data();
        d.hunt_finds = (0..24).map(|i| [0, 5056 + i, i]).collect();
        d.items = (5056..5080).chain([crate::hunt::GOLDEN_PEACH]).map(|id| Item { id, kind: 1, ..Default::default() }).collect();
        let mut g = Game::new(d, 1);
        g.pet.def = g.data.character(2113).unwrap();
        g.pet.rank = 1;
        g.pet.rank_seen = 1;
        g.pet.onaka = 6;
        g.pet.gokigen = 20;
        g.frame(NOON, 0);
        g.scene = g.open(Action::Hunt);
        let press = |g: &mut Game, b: u8| {
            g.frame(NOON, b);
            g.frame(NOON, 0);
        };
        press(&mut g, B); // VOLCANO
        assert!(matches!(g.scene, Scene::HuntTeam { .. }));
        press(&mut g, B); // the pet
        press(&mut g, B); // already picked
        press(&mut g, A);
        press(&mut g, B);
        press(&mut g, A);
        press(&mut g, B);
        assert!(matches!(g.scene, Scene::Ask { .. }), "three picked: BRING THEM?");
        press(&mut g, B);
        assert!(matches!(g.scene, Scene::Play(_)));
        assert!(g.pet.hunt_used[0] & 1 != 0);
        assert!(!g.pet.inventory.is_empty(), "a happy pet always finds eggs");
    }
}
