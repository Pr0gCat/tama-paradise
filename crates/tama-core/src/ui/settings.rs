//! Lab menu SETTINGS: EDIT BIRTHDAY and EDIT PLANET NAME.

use super::*;

/// New-game setup texts: NAME YOUR PLANET, and the goal naming it.
const INTRO_NAME: u16 = 100;
const INTRO_GOAL: u16 = 101;
/// Language names, Japanese first (0x56–0x5e, in `data::LANGS` order).
const LANGUAGE_NAMES: u16 = 0x56;

/// Days in each month (February allows the 29th).
const MONTH_DAYS: [u8; 12] = [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

impl Game {
    /// New-game setup (after the language and clock): name the planet, then the birthday.
    /// New-game setup in the firmware's order (`FUN_10038250`..`FUN_10039034`): LANGUAGE, then
    /// BIRTHDAY, then PLANET NAME (the clock comes from the platform).
    pub fn begin_intro(&mut self) {
        // The first-start jingle (PSID_INITIAL_LOGO `FUN_10038250`).
        self.sfx(SND_FIRST_START, 100);
        self.scene = self.open(Action::IntroLanguage);
    }

    pub(super) fn open_intro(&mut self, a: Action) -> Scene {
        match a {
            // LANGUAGE (`FUN_10038c24`): a ring menu of the nine language names.
            Action::IntroLanguage => Scene::Menu {
                items: (0..9u8).map(|i| (LANGUAGE_NAMES + i as u16, Some(Action::IntroLang(i)))).collect(),
                idx: self.pet.lang as usize,
                back: Some(Action::IntroLanguage),
            },
            Action::IntroLang(i) => {
                self.pet.lang = i;
                Scene::Birthday { m: 1, d: 1, pos: 0, intro: true }
            }
            Action::Language => Scene::Menu {
                items: (0..9u8).map(|i| (LANGUAGE_NAMES + i as u16, Some(Action::SetLang(i)))).collect(),
                idx: self.pet.lang as usize,
                back: Some(Action::Settings),
            },
            Action::SetLang(i) => {
                self.pet.lang = i;
                self.open(Action::Settings)
            }
            _ => Scene::Home,
        }
    }

    pub(super) fn update_planet_name(
        &mut self,
        mut chars: [u16; 8],
        mut pos: usize,
        intro: bool,
        pressed: u8,
    ) -> Scene {
        // Letters cycle through space, A–Z and 0–9 (font glyphs 305, 318–343, 306–315).
        let set: Vec<u16> = std::iter::once(crate::screen::SPACE).chain(318..=343).chain(306..=315).collect();
        let at = set.iter().position(|&c| c == chars[pos]).unwrap_or(0);
        chars[pos] = set[cycle(at, set.len(), pressed)];
        if pressed & C != 0 {
            return match pos {
                0 if intro => Scene::PlanetName { chars, pos, intro },
                0 => self.open(Action::Settings),
                _ => Scene::PlanetName { chars, pos: pos - 1, intro },
            };
        }
        if pressed & (B | PUSH) != 0 {
            pos += 1;
            if pos == chars.len() {
                let len = chars.iter().rposition(|&c| c != crate::screen::SPACE).map_or(0, |i| i + 1);
                self.pet.planet_name = chars[..len].to_vec();
                if intro {
                    // "MAKE <planet> PROSPEROUS!", then the game starts.
                    return Scene::Message { text: INTRO_GOAL, then: Box::new(Scene::Home) };
                }
                return self.open(Action::Settings);
            }
        }
        Scene::PlanetName { chars, pos, intro }
    }

    pub(super) fn update_birthday(&mut self, mut m: u8, mut d: u8, pos: u8, intro: bool, pressed: u8) -> Scene {
        if pos == 0 {
            m = cycle(m as usize - 1, 12, pressed) as u8 + 1;
        } else {
            d = cycle(d as usize - 1, MONTH_DAYS[m as usize - 1] as usize, pressed) as u8 + 1;
        }
        d = d.min(MONTH_DAYS[m as usize - 1]);
        if pressed & C != 0 {
            match pos {
                0 if intro => Scene::Birthday { m, d, pos, intro },
                0 => self.open(Action::Settings),
                _ => Scene::Birthday { m, d, pos: 0, intro },
            }
        } else if pressed & (B | PUSH) != 0 && pos == 0 {
            Scene::Birthday { m, d, pos: 1, intro }
        } else if pressed & (B | PUSH) != 0 {
            self.pet.birthday = Some((m, d));
            if intro {
                let name = Scene::PlanetName { chars: [crate::screen::SPACE; 8], pos: 0, intro: true };
                Scene::Message { text: INTRO_NAME, then: Box::new(name) }
            } else {
                self.open(Action::Settings)
            }
        } else {
            Scene::Birthday { m, d, pos, intro }
        }
    }

    pub(super) fn draw_planet_name(&mut self, chars: &[u16; 8], pos: usize) {
        self.draw_list(&[], 0, Some(0x0c));
        for (i, &c) in chars.iter().enumerate() {
            let x = 18 + i as i32 * 13;
            self.screen.text(&self.data.fonts, &[c], x, 86, true, 1);
            if i == pos {
                self.screen.rect(x - 5, 96, 10, 2, [40, 40, 120]);
            }
        }
    }

    pub(super) fn draw_birthday(&mut self, m: u8, d: u8, pos: u8) {
        self.draw_list(&[], 0, Some(0x60));
        for (i, (v, x)) in [(m, 40), (d, 88)].into_iter().enumerate() {
            if i == pos as usize {
                self.screen.rect(x - 12, 98, 24, 2, [40, 40, 120]);
            }
            self.draw_digits(v as u32, x, 86);
        }
        self.screen.rect(62, 85, 4, 4, [40, 40, 120]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_game_asks_language_then_birthday_then_planet_name() {
        let mut g = Game::new(crate::evolve::tests::data(), 1);
        g.begin_intro();
        let press = |g: &mut Game, b: u8| {
            g.frame(86_400 * 3 + 12 * 3600, b);
            g.frame(86_400 * 3 + 12 * 3600, 0);
        };
        assert!(matches!(&g.scene, Scene::Menu { idx: 1, .. }), "LANGUAGE, English selected");
        press(&mut g, C);
        assert!(matches!(&g.scene, Scene::Menu { .. }), "no way out of the setup");
        press(&mut g, A); // FRANÇAIS
        press(&mut g, B);
        assert_eq!(g.pet.lang, 2);
        press(&mut g, B); // month
        press(&mut g, B); // day
        assert_eq!(g.pet.birthday, Some((1, 1)));
        press(&mut g, B); // NAME YOUR PLANET
        press(&mut g, C);
        assert!(matches!(g.scene, Scene::PlanetName { intro: true, .. }), "no way back");
        press(&mut g, A); // A
        for _ in 0..8 {
            press(&mut g, B);
        }
        assert_eq!(g.pet.planet_name, vec![318]);
        assert!(matches!(g.scene, Scene::Message { text: INTRO_GOAL, .. }));
        press(&mut g, B);
        assert!(matches!(g.scene, Scene::Home));
    }

    #[test]
    fn language_can_change_in_settings() {
        let mut g = Game::new(crate::evolve::tests::data(), 1);
        g.scene = g.open(Action::Settings);
        let press = |g: &mut Game, b: u8| {
            g.frame(86_400 * 3 + 12 * 3600, b);
            g.frame(86_400 * 3 + 12 * 3600, 0);
        };
        press(&mut g, DOWN); // the language entry (last)
        press(&mut g, B);
        assert!(matches!(&g.scene, Scene::Menu { idx: 1, .. }), "current language selected");
        press(&mut g, DOWN); // にほんご
        press(&mut g, B);
        assert_eq!(g.pet.lang, 0);
        assert!(matches!(&g.scene, Scene::Menu { items, .. } if items[2].0 == 0x56), "shows the new name");
    }
}
