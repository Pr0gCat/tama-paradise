//! LABOMENU (`FUN_10045db0`): the "TAMAGOTCHI Lab" logo when coming from HOME, then a 3×2 grid
//! of round icons with the selected item's name on a band (docs/spec/08-care-ui.md §2.21).

use super::*;

/// The logo shows for 20 frames (its 8-frame fade-in included).
const SPLASH_FRAMES: u16 = 20;
/// Fade level step per frame (`fade_set(.., speed 4, ..)` over 0..32).
pub(crate) const FADE_STEP: u8 = 4;
pub(crate) const FADE_FULL: u8 = 32;

mod elem {
    pub const LOGO: u16 = 0x19c;
    pub const BG: u16 = 0x19d;
    pub const BAND: u16 = 0x19f;
    pub const LABEL: u16 = 0x19e;
    /// INFORMATION, MINI GAMES, SHOP, HELP, LAB LOG, SETTINGS (frame 0 normal, 1 selected).
    pub const ICONS: [u16; 6] = [0x1a0, 0x1a1, 0x1a2, 0x1a3, 0x1a4, 0x1a5];
}

/// Item names and where they go (table XIP 0x10094188).
const ITEMS: [(u16, Action); 6] = [
    (0, Action::Info),
    (2, Action::MiniGames),
    (3, Action::Shop),
    (5, Action::Help),
    (1, Action::Book),
    (6, Action::Settings),
];

impl Game {
    /// Long press at HOME: the logo, then the menu on INFORMATION.
    pub(crate) fn enter_lab(&mut self) -> Scene {
        self.lab_idx = 0;
        self.fade = FADE_FULL;
        Scene::LabSplash { t: 0 }
    }

    /// Back from a lab screen: the grid fades in on that screen's icon.
    pub(super) fn open_lab(&mut self) -> Scene {
        self.fade = FADE_FULL;
        Scene::LabMenu { idx: self.lab_idx }
    }

    pub(super) fn update_lab_splash(&mut self, t: u16) -> Scene {
        if t + 1 >= SPLASH_FRAMES {
            return self.open_lab();
        }
        Scene::LabSplash { t: t + 1 }
    }

    /// A / UP next, DOWN previous (wrapping), B / PUSH opens, C back to HOME.
    pub(super) fn update_lab_menu(&mut self, idx: u8, pressed: u8) -> Scene {
        if pressed & C != 0 {
            self.sfx(2, 100);
            self.fade = FADE_FULL;
            return Scene::Home;
        }
        if pressed & (B | PUSH) != 0 {
            self.sfx(1, 100);
            self.lab_idx = idx;
            return self.open(ITEMS[idx as usize].1);
        }
        let next = cycle(idx as usize, ITEMS.len(), pressed) as u8;
        if next != idx {
            self.sfx(0, 100);
        }
        Scene::LabMenu { idx: next }
    }

    pub(super) fn draw_lab_splash(&mut self) {
        self.draw_elem(elem::LOGO, None, None);
    }

    pub(super) fn draw_lab_menu(&mut self, idx: u8) {
        self.draw_elem(elem::BG, None, None);
        self.draw_elem(elem::BAND, None, None);
        for (i, id) in elem::ICONS.into_iter().enumerate() {
            self.draw_elem(id, Some((i == idx as usize) as u16), None);
        }
        let label = self.string(ITEMS[idx as usize].0);
        self.draw_text_elem(elem::LABEL, &label);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOON: u64 = 86_400 * 3 + 12 * 3600;

    fn press(g: &mut Game, b: u8) {
        g.frame(NOON, b);
        g.frame(NOON, 0);
    }

    #[test]
    fn logo_then_grid_and_back_on_the_same_icon() {
        let mut g = Game::new(crate::evolve::tests::data(), 1);
        g.pet.def = g.data.character(2113).unwrap();
        g.frame(NOON, 0);
        for _ in 0..14 {
            g.frame(NOON, PUSH);
        }
        assert!(matches!(g.scene, Scene::LabSplash { .. }), "the logo first");
        g.frame(NOON, 0);
        for _ in 0..SPLASH_FRAMES {
            g.frame(NOON, 0);
        }
        assert!(matches!(g.scene, Scene::LabMenu { idx: 0 }));
        press(&mut g, A);
        press(&mut g, A); // SHOP
        press(&mut g, B);
        assert!(!matches!(g.scene, Scene::LabMenu { .. }));
        press(&mut g, C);
        assert!(matches!(g.scene, Scene::LabMenu { idx: 2 }), "back on SHOP, no logo");
        press(&mut g, C);
        assert!(matches!(g.scene, Scene::Home));
        assert!(g.fade > 0, "HOME fades in");
    }
}
