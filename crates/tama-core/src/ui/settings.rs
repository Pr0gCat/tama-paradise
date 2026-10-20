//! New-game setup, lab menu SETTINGS (birthday, planet name, language) and INFORMATION →
//! USER INFO (docs/spec/08-care-ui.md §2.19).

use super::*;

/// New-game setup texts: NAME YOUR PLANET, and the goal naming it.
const INTRO_NAME: u16 = 100;
const INTRO_GOAL: u16 = 101;
/// Language names, Japanese first (0x56–0x5e, in `data::LANGS` order).
const LANGUAGE_NAMES: u16 = 0x56;

/// INFORMATION submenu texts, USER INFO page titles and the DAYS label.
const USER_INFO: u16 = 7;
const MISSION: u16 = 8;
const DAYS: u16 = 161;
const EGG_BANG: u16 = 268;
const BIRTHDAY: u16 = 163;
const EDIT_BIRTHDAY: u16 = 0x0b;
const EDIT_PLANET_NAME: u16 = 0x0c;

/// USER INFO UI elements (`FUN_10047f10`).
mod elem {
    pub const BG: u16 = 0x1ce;
    /// Page arrows (anims 12 / 13; drawn still).
    pub const ARROWS: [u16; 2] = [0x1cf, 0x1d0];
    pub const TITLE_BARS: [u16; 3] = [0x1d2, 0x1dd, 0x1ec];
    pub const PLANET_NAME: u16 = 0x1d1;
    pub const DAYS_BOX: u16 = 0x1d5;
    pub const DAYS_LABEL: u16 = 0x1d3;
    pub const DAYS: u16 = 0x1d4;
    pub const POINTS_BOX: u16 = 0x1d8;
    pub const POINTS: u16 = 0x1d6;
    pub const POINTS_MARK: u16 = 0x1d7;
    pub const TITLE: u16 = 0x1eb;
    pub const BUNTING: u16 = 0x1e8;
    pub const DATE_BOX: u16 = 0x1ed;
    pub const MONTH: u16 = 0x1ee;
    pub const SLASH: u16 = 0x1ef;
    pub const DAY: u16 = 0x1f0;
}

/// Days in each month (February allows the 29th).
const MONTH_DAYS: [u8; 12] = [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

impl Game {
    /// New-game setup (after the language and clock): name the planet, then the birthday.
    /// New-game setup in the firmware's order (`FUN_10038250`..`FUN_10039034`): LANGUAGE, then
    /// BIRTHDAY, then PLANET NAME (the clock comes from the platform).
    pub fn begin_intro(&mut self) {
        self.pet.set_up = false;
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
            // INFORMATION (`FUN_10046b84`): USER INFO, MISSION; back from MISSION on MISSION.
            Action::Info | Action::InfoFromMissions => Scene::Menu {
                items: vec![(USER_INFO, Some(Action::UserInfo)), (MISSION, Some(Action::Missions))],
                idx: (a == Action::InfoFromMissions) as usize,
                back: Some(Action::Lab),
            },
            Action::UserInfo => Scene::UserInfo { page: 0 },
            // USER INFO's edit menu (RW data 0x18000708).
            Action::UserEdit => Scene::Menu {
                items: vec![
                    (EDIT_BIRTHDAY, Some(Action::EditBirthday)),
                    (EDIT_PLANET_NAME, Some(Action::EditPlanetName)),
                ],
                idx: 0,
                back: Some(Action::UserInfo),
            },
            _ => Scene::Home,
        }
    }

    /// Back from an editor to the menu it was opened from.
    fn edit_done(&mut self) -> Scene {
        self.open(Action::UserEdit)
    }

    /// USER INFO (`FUN_10047f10`): A / UP next page, DOWN previous, B the edit menu, C back.
    pub(super) fn update_user_info(&mut self, page: u8, pressed: u8) -> Scene {
        if pressed & C != 0 {
            return self.open(Action::Info);
        }
        if pressed & (B | PUSH) != 0 {
            return self.open(Action::UserEdit);
        }
        Scene::UserInfo { page: cycle(page as usize, 3, pressed) as u8 }
    }

    pub(super) fn draw_user_info(&mut self, page: u8) {
        self.draw_elem(elem::BG, None, None);
        if page == 2 {
            self.draw_elem(elem::BUNTING, None, None);
        }
        self.draw_elem(elem::TITLE_BARS[page as usize], None, None);
        let date = |d: Option<(u8, u8)>| d.map_or((0, 0), |(m, d)| (m as u32, d as u32));
        match page {
            0 => {
                let name = self.pet.planet_name.clone();
                self.draw_text_elem(elem::PLANET_NAME, &name);
                self.draw_elem(elem::DAYS_BOX, None, None);
                self.draw_text_elem(elem::DAYS_LABEL, &self.string(DAYS));
                let days = self.digits(self.pet.play_days as u32);
                self.draw_text_elem(elem::DAYS, &days);
                self.draw_elem(elem::POINTS_BOX, None, None);
                self.draw_number_elem(elem::POINTS, self.pet.points);
                self.draw_elem(elem::POINTS_MARK, None, None);
            }
            _ => {
                let (title, (m, d)) =
                    if page == 1 { (EGG_BANG, date(self.pet.start_date)) } else { (BIRTHDAY, date(self.pet.birthday)) };
                self.draw_text_elem(elem::TITLE, &self.string(title));
                self.draw_elem(elem::DATE_BOX, None, None);
                self.draw_number_elem(elem::MONTH, m);
                self.draw_elem(elem::SLASH, None, None);
                // The day starts right after the slash.
                let e = self.ui_elem(elem::DAY);
                let day = self.digits(d);
                self.screen.text(&self.data.fonts, &day, e.x as i32 - 6, e.y as i32, false, e.p[7].max(0) as usize);
            }
        }
        for (i, id) in elem::ARROWS.into_iter().enumerate() {
            let e = self.ui_elem(id);
            self.screen.blit(super::ARROWS, i as u16, e.x as i32, e.y as i32, 64, false, None);
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
                0 => self.edit_done(),
                _ => Scene::PlanetName { chars, pos: pos - 1, intro },
            };
        }
        if pressed & (B | PUSH) != 0 {
            pos += 1;
            if pos == chars.len() {
                let len = chars.iter().rposition(|&c| c != crate::screen::SPACE).map_or(0, |i| i + 1);
                self.pet.planet_name = chars[..len].to_vec();
                if intro {
                    // The new-game setup date, shown in USER INFO as the DAY OF THE EGG BANG.
                    self.pet.start_date = self.pet.last.map(|t| {
                        let (_, m, d) = crate::pet::date(t);
                        (m as u8, d as u8)
                    });
                    self.pet.set_up = true;
                    // "MAKE <planet> PROSPEROUS!", then the game starts.
                    return Scene::Message { text: INTRO_GOAL, then: Box::new(Scene::Home) };
                }
                return self.edit_done();
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
                0 => self.edit_done(),
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
                self.edit_done()
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
    fn a_reload_during_the_setup_starts_it_again() {
        let mut g = Game::new(crate::evolve::tests::data(), 1);
        g.begin_intro();
        let save = g.save();
        let g = Game::load(crate::evolve::tests::data(), &save).unwrap();
        assert!(matches!(&g.scene, Scene::Menu { items, .. } if items[0].0 == LANGUAGE_NAMES), "LANGUAGE again");
    }

    #[test]
    fn language_can_change_in_settings() {
        let mut g = Game::new(crate::evolve::tests::data(), 1);
        g.scene = g.open(Action::Settings);
        let press = |g: &mut Game, b: u8| {
            g.frame(86_400 * 3 + 12 * 3600, b);
            g.frame(86_400 * 3 + 12 * 3600, 0);
        };
        press(&mut g, DOWN); // SAVE DATA (last)
        press(&mut g, DOWN); // TOUCH
        press(&mut g, DOWN); // the language entry
        press(&mut g, B);
        assert!(matches!(&g.scene, Scene::Menu { idx: 1, .. }), "current language selected");
        press(&mut g, DOWN); // にほんご
        press(&mut g, B);
        assert_eq!(g.pet.lang, 0);
        assert!(matches!(&g.scene, Scene::Menu { items, .. } if items[2].0 == 0x56), "shows the new name");
    }

    #[test]
    fn user_info_pages_and_its_edit_menu() {
        let mut g = Game::new(crate::evolve::tests::data(), 1);
        g.scene = g.open(Action::Lab);
        let press = |g: &mut Game, b: u8| {
            g.frame(86_400 * 3 + 12 * 3600, b);
            g.frame(86_400 * 3 + 12 * 3600, 0);
        };
        g.scene = g.open(Action::Info);
        press(&mut g, B);
        assert!(matches!(g.scene, Scene::UserInfo { page: 0 }));
        press(&mut g, DOWN);
        assert!(matches!(g.scene, Scene::UserInfo { page: 2 }), "pages wrap");
        press(&mut g, B);
        press(&mut g, B); // EDIT BIRTHDAY
        press(&mut g, B);
        press(&mut g, B);
        assert_eq!(g.pet.birthday, Some((1, 1)));
        assert!(matches!(&g.scene, Scene::Menu { items, .. } if items.len() == 2), "back to the edit menu");
        press(&mut g, C);
        press(&mut g, C);
        assert!(matches!(&g.scene, Scene::Menu { idx: 0, items, .. } if items[0].0 == USER_INFO));
        press(&mut g, A);
        press(&mut g, B); // MISSION
        press(&mut g, C);
        assert!(matches!(&g.scene, Scene::Menu { idx: 1, items, .. } if items[1].0 == MISSION));
    }
}
