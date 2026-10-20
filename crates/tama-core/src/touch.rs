//! Touch controls (the remake's, for the Apple Watch): the platform reports gestures and the
//! game turns them into its buttons for the screen it is on. Swipe up / down turn the dial, a
//! tap selects, holding is the long press, swipe left goes back; swipe right does nothing.

use crate::input::{A, B, C, DOWN, PUSH, UP};
use crate::ui::Scene;
use crate::Game;

/// One gesture this frame (`Game::frame_touch`).
pub const NONE: u8 = 0;
pub const SWIPE_UP: u8 = 1;
pub const SWIPE_DOWN: u8 = 2;
pub const SWIPE_LEFT: u8 = 3;
pub const TAP: u8 = 4;

/// The buttons each gesture means. `up` / `down` / `tap` / `back` are pressed for one frame;
/// `hold` stays down while the finger does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Map {
    pub up: u8,
    pub down: u8,
    pub tap: u8,
    pub hold: u8,
    pub back: u8,
}

impl Map {
    pub const DEFAULT: Map = Map { up: UP, down: DOWN, tap: B, hold: B, back: C };
}

impl Game {
    /// The buttons for the touch gestures on the current screen.
    pub fn touch_map(&self) -> Map {
        let base = Map { hold: PUSH, ..Map::DEFAULT };
        match &self.scene {
            // HOME: a tap opens the menu (A), holding opens the lab (the long press).
            Scene::Home => Map { tap: A, ..base },
            // Star pages turn with A.
            Scene::Stars { page, .. } if *page > 0 => Map { tap: A, ..base },
            // SOUND: up louder (B), down quieter (A), tap done.
            Scene::Level { .. } => Map { up: B, down: A, tap: C, ..base },
            Scene::Mini { session, .. } => session.game.touch_map(),
            _ => base,
        }
    }

    /// One 20 fps frame driven by touch: `gesture` (`SWIPE_UP`… or `NONE`) happened this
    /// frame, `holding` while a finger rests past the platform's long-press delay.
    pub fn frame_touch(&mut self, now: u64, gesture: u8, holding: bool) {
        let m = self.touch_map();
        let mut held = match gesture {
            SWIPE_UP => m.up,
            SWIPE_DOWN => m.down,
            TAP => m.tap,
            SWIPE_LEFT => m.back,
            _ => 0,
        };
        if holding {
            held |= m.hold;
        }
        // A swipe left is a whole cancel where the firmware wants C twice (date and birthday).
        self.touch_back = gesture == SWIPE_LEFT;
        self.frame(now, held);
        self.touch_back = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::Action;

    #[test]
    fn gestures_follow_the_screen() {
        let mut g = Game::new(crate::evolve::tests::data(), 1);
        let t = 86_400 * 3 + 12 * 3600;
        g.pet.def.stage = 3;
        g.frame(t, 0);
        assert_eq!(g.touch_map().tap, A, "HOME: tap opens the menu");
        for _ in 0..15 {
            g.frame_touch(t, NONE, true);
        }
        assert!(!matches!(g.scene, Scene::Home), "holding opens the lab");
        g.scene = g.open(Action::Settings);
        g.frame_touch(t, NONE, false);
        g.frame_touch(t, SWIPE_UP, false);
        g.frame_touch(t, NONE, false);
        g.frame_touch(t, SWIPE_LEFT, false);
        g.frame_touch(t, NONE, false);
        assert!(!matches!(g.scene, Scene::Menu { back: Some(Action::Lab), .. }), "swipe left goes back");
        g.scene = Scene::Birthday { m: 1, d: 1, pos: 0, intro: false };
        g.frame_touch(t, NONE, false);
        g.frame_touch(t, SWIPE_LEFT, false);
        assert!(!matches!(g.scene, Scene::Birthday { .. }), "one swipe left cancels");
    }
}

#[cfg(test)]
mod with_rom_data {
    use super::*;
    use crate::ui::Action;

    /// Needs data/game.json (`cargo test -- --ignored`): every mini-game ends when played with
    /// random gestures only.
    #[test]
    #[ignore]
    fn every_game_ends_with_gestures_only() {
        let data = crate::data::Data::from_json(&std::fs::read_to_string("../../data/game.json").unwrap()).unwrap();
        let t = 86_400 * 3 + 12 * 3600;
        let mut games: Vec<Action> = (0..7).map(|g| Action::PlayLab(g, 0)).collect();
        games.extend([0, 1, 2, 3, 4, 6, 7].map(Action::PlayTogether));
        games.extend([0, 1].map(Action::PlayArcade));
        for a in games {
            let mut g = Game::new(data.clone(), 1);
            g.pet.def = g.data.character(2113).unwrap();
            g.frame(t, 0);
            g.scene = g.open(a);
            let mut seed = 7u32;
            let mut frames = 0;
            while matches!(g.scene, Scene::Mini { .. }) && frames < 20_000 {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let gesture = if frames.is_multiple_of(5) { [SWIPE_UP, SWIPE_DOWN, TAP, TAP][(seed >> 9) as usize % 4] } else { NONE };
                g.frame_touch(t + frames / 20, gesture, seed.is_multiple_of(7));
                frames += 1;
            }
            assert!(frames < 20_000, "{a:?} never ended");
        }
    }
}
