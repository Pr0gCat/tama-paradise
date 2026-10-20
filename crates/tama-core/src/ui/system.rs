//! Lab menu SETTINGS (`FUN_10047174`): SOUND, TIME → DATE/TIME and DISPLAY (the remake drops
//! BRIGHTNESS: a screen is always at full brightness), the remake's save data menu, plus the
//! planet level's idle clock (docs/spec/08-care-ui.md §2.20, 10-decor-planet S5).

use super::*;
use crate::pet::{date, days_from_civil};

const SOUND: u16 = 28;
const TIME: u16 = 26;
const DATE_TIME: u16 = 29;
const DISPLAY: u16 = 30;
/// Date orders as the menu shows them: 2026/01/23, 23/01/2026, 01/23/2026.
const DATE_ORDERS: u16 = 31;
const PICK_DATE_ORDER: u16 = 97;
const SET_DATE_TIME: u16 = 95;
/// Clock editor field titles: YEAR, MONTH, DATE, HOUR, MINUTE, AM/PM.
const FIELD_TITLES: u16 = 235;
/// Language names (the remake's extra SETTINGS item shows the current one).
const LANGUAGE_NAMES: u16 = 0x56;
/// SOUND closes itself after 200 frames without A or B.
const LEVEL_IDLE: u16 = 200;
/// The planet level shows the clock after 200 idle frames.
pub(crate) const CLOCK_IDLE: u16 = 200;
/// Sound level 0–3 → master volume % (`snd_set_master` 0x10030bb4).
pub(crate) const MASTER_VOLUME: [u32; 4] = [0, 30, 60, 100];
const YEARS: (u16, u16) = (2025, 2079);
/// DELETE texts: CONTINUE, RESET ALL, REALLY DELETE?
const CONTINUE: u16 = 80;
const RESET_ALL: u16 = 81;
const REALLY_DELETE: u16 = 83;

mod elem {
    /// SOUND: background, speaker chick, level bar (frame = level), mute cross.
    pub const SOUND: [u16; 3] = [0x28b, 0x28c, 0x28d];
    pub const MUTE: u16 = 0x28e;
    /// Clock editor: background, title bar, title, year/month/day boxes and values, time bar,
    /// hour, colon, minute, AM/PM, key hints.
    pub const CLOCK_BG: [u16; 2] = [0x79, 0x7b];
    pub const CLOCK_TITLE: u16 = 0x7c;
    pub const DATE_BOXES: [u16; 3] = [0x7d, 0x7f, 0x81];
    pub const DATE_VALUES: [u16; 3] = [0x7e, 0x80, 0x82];
    pub const TIME_BAR: u16 = 0x83;
    pub const HOUR: u16 = 0x84;
    pub const COLON: u16 = 0x85;
    pub const MINUTE: u16 = 0x86;
    pub const AM_PM: u16 = 0x88;
    pub const KEYS: [u16; 3] = [0x89, 0x8a, 0x8b];
    /// Planet idle clock: panel, AM/PM, hour and minute digits with the colon, 10 date cells.
    pub const IDLE_PANEL: u16 = 0xfc;
    pub const IDLE_AM_PM: u16 = 0xfe;
    pub const IDLE_TIME: u16 = 0xff;
    pub const IDLE_DATE: u16 = 0x104;
}

fn days_in_month(y: u16, m: u16) -> u16 {
    match m {
        2 if y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400)) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

impl Game {
    pub(super) fn open_system(&mut self, a: Action) -> Scene {
        match a {
            Action::Settings => self.settings_menu(0),
            Action::ToggleTouch => {
                self.pet.touch = !self.pet.touch;
                self.settings_menu(3)
            }
            Action::SettingsTime => self.time_menu(0),
            Action::Volume => Scene::Level { level: self.pet.volume, idle: 0 },
            // SAVE DATA (remake): export or import a save file through the page, or start over.
            Action::SaveData => Scene::Menu {
                items: vec![
                    (REMAKE_TEXT + 3, Some(Action::ExportSave)),
                    (REMAKE_TEXT + 4, Some(Action::ImportSave)),
                    (REMAKE_TEXT + 5, Some(Action::NewGameAsk)),
                ],
                idx: 0,
                back: Some(Action::SettingsAtSaveData),
            },
            Action::ExportSave | Action::ImportSave => {
                self.connect = Some(if a == Action::ExportSave { "save-export" } else { "save-import" }.into());
                self.open(Action::SaveData)
            }
            Action::NewGameAsk => {
                Scene::Ask { text: REALLY_DELETE, yes: false, on_yes: Action::ResetAll, back: Action::SaveData }
            }
            Action::SettingsAtSaveData => self.settings_menu(4),
            Action::DateOrder => {
                let items = (0..3u8).map(|i| (DATE_ORDERS + i as u16, Some(Action::SetDateOrder(i)))).collect();
                let menu = Scene::Menu { items, idx: 0, back: Some(Action::SettingsTimeDisplay) };
                Scene::Message { text: PICK_DATE_ORDER, then: Box::new(menu) }
            }
            Action::SetDateOrder(i) => {
                self.pet.date_order = i;
                self.time_menu(1)
            }
            Action::SettingsTimeDisplay => self.time_menu(1),
            // DELETE (`delete_menu_tick` 0x10038d24): C does nothing.
            Action::Delete => Scene::Menu {
                items: vec![(CONTINUE, Some(Action::Continue)), (RESET_ALL, Some(Action::ResetAllAsk))],
                idx: 0,
                back: Some(Action::Delete),
            },
            Action::ResetAllAsk => {
                Scene::Ask { text: REALLY_DELETE, yes: false, on_yes: Action::ResetAll, back: Action::Delete }
            }
            // Everything goes (language, birthday, planet, settings too), then the new-game setup.
            Action::ResetAll => {
                let (seed, held) = (self.rand(), self.held);
                *self = Game::new(self.data.clone(), seed);
                // The confirming key is still down; it must not also answer the LANGUAGE menu.
                self.held = held;
                self.begin_intro();
                std::mem::replace(&mut self.scene, Scene::Home)
            }
            Action::DateTime | Action::Continue => {
                let now = self.pet.last.unwrap_or(0);
                let (y, m, d) = date(now);
                let h = (now % 86_400 / 3600) as u16;
                let vals = [
                    (y as u16).clamp(YEARS.0, YEARS.1),
                    m as u16,
                    d as u16,
                    if h.is_multiple_of(12) { 12 } else { h % 12 },
                    (now % 3600 / 60) as u16,
                    h / 12,
                ];
                let boot = a == Action::Continue;
                Scene::Message { text: SET_DATE_TIME, then: Box::new(Scene::Clock { vals, pos: 0, boot }) }
            }
            _ => Scene::Home,
        }
    }

    /// SETTINGS: SOUND, TIME; the remake adds the language, touch controls and save data.
    fn settings_menu(&self, idx: usize) -> Scene {
        let items = vec![
            (SOUND, Some(Action::Volume)),
            (TIME, Some(Action::SettingsTime)),
            (LANGUAGE_NAMES + self.pet.lang as u16, Some(Action::Language)),
            (REMAKE_TEXT + !self.pet.touch as u16, Some(Action::ToggleTouch)),
            (REMAKE_TEXT + 2, Some(Action::SaveData)),
        ];
        Scene::Menu { items, idx, back: Some(Action::Lab) }
    }

    fn time_menu(&self, idx: usize) -> Scene {
        let items = vec![(DATE_TIME, Some(Action::DateTime)), (DISPLAY, Some(Action::DateOrder))];
        Scene::Menu { items, idx, back: Some(Action::SettingsAtTime) }
    }

    pub(super) fn settings_at_time(&self) -> Scene {
        self.settings_menu(1)
    }

    /// SOUND (`FUN_1004fcf4`): A lowers, B raises (0–3), applied at once; C or 200 frames without
    /// A/B keeps it.
    pub(super) fn update_level(&mut self, mut level: u8, idle: u16, pressed: u8) -> Scene {
        let change = if pressed & A != 0 {
            level.checked_sub(1)
        } else if pressed & B != 0 {
            (level < 3).then_some(level + 1)
        } else {
            None
        };
        if let Some(l) = change {
            level = l;
            self.pet.volume = l;
            self.sfx(0, 100);
            return Scene::Level { level, idle: 0 };
        }
        if pressed & C != 0 || idle >= LEVEL_IDLE {
            self.sfx(1, 100);
            return self.settings_menu(0);
        }
        Scene::Level { level, idle: idle + 1 }
    }

    pub(super) fn draw_level(&mut self, level: u8) {
        let ids = elem::SOUND;
        self.draw_elem(ids[0], None, None);
        self.draw_elem(ids[1], None, None);
        self.draw_elem(ids[2], Some(level as u16), None);
        if level == 0 {
            self.draw_elem(elem::MUTE, None, None);
        }
    }

    /// DATE/TIME (`settings_clock_update` 0x1004f7d8): year, month, day, hour, minute, AM/PM in
    /// turn; B/PUSH confirms a field, C goes back one (leaves from the year). The new time
    /// starts without catching up the pet (`last_tick` is set too).
    pub(super) fn update_clock(&mut self, mut vals: [u16; 6], pos: u8, boot: bool, pressed: u8) -> Scene {
        let p = pos as usize;
        let (lo, hi) = match p {
            0 => YEARS,
            1 => (1, 12),
            2 => (1, days_in_month(vals[0], vals[1])),
            3 => (1, 12),
            4 => (0, 59),
            _ => (0, 1),
        };
        let stepped = lo + cycle((vals[p] - lo) as usize, (hi - lo + 1) as usize, pressed) as u16;
        if stepped != vals[p] {
            self.sfx(0, 100);
        }
        vals[p] = stepped;
        if pressed & C != 0 {
            self.sfx(2, 100);
            return if p == 0 { self.clock_exit(boot, false) } else { Scene::Clock { vals, pos: pos - 1, boot } };
        }
        if pressed & (B | PUSH) == 0 {
            return Scene::Clock { vals, pos, boot };
        }
        self.sfx(1, 100);
        vals[2] = vals[2].min(days_in_month(vals[0], vals[1]));
        if p < 5 {
            return Scene::Clock { vals, pos: pos + 1, boot };
        }
        let day = days_from_civil(vals[0] as i64, vals[1] as u32, vals[2] as u32);
        let hour = (vals[3] % 12 + 12 * vals[5]) as i64;
        let t = day * 86_400 + hour * 3600 + vals[4] as i64 * 60;
        let now = self.pet.last.unwrap_or(0) as i64;
        self.pet.clock_offset += t - now;
        self.pet.last = Some(t as u64);
        self.clock_exit(boot, true)
    }

    /// Leaving the clock editor: DATE/TIME back to the TIME menu; CORRECTTIME back to DELETE when
    /// cancelled, else on to HOME.
    fn clock_exit(&mut self, boot: bool, done: bool) -> Scene {
        match (boot, done) {
            (false, _) => self.time_menu(0),
            (true, false) => self.open(Action::Delete),
            (true, true) => Scene::Home,
        }
    }

    pub(super) fn draw_clock(&mut self, vals: &[u16; 6], pos: u8) {
        for id in elem::CLOCK_BG {
            self.draw_elem(id, None, None);
        }
        self.draw_text_elem(elem::CLOCK_TITLE, &self.string(FIELD_TITLES + pos as u16));
        // The field being edited blinks (10 frames on, 10 off).
        let blink_on = (self.frame_count / 10).is_multiple_of(2);
        let shown = |i: u8| i != pos || blink_on;
        for (i, (&v, (box_id, value_id))) in
            vals.iter().zip(elem::DATE_BOXES.into_iter().zip(elem::DATE_VALUES)).enumerate()
        {
            self.draw_elem(box_id, None, None);
            if shown(i as u8) {
                let digits = self.padded(v, if i == 0 { 4 } else { 2 });
                self.draw_text_elem(value_id, &digits);
            }
        }
        self.draw_elem(elem::TIME_BAR, None, None);
        for (i, id) in [(3u8, elem::HOUR), (4, elem::MINUTE)] {
            if shown(i) {
                let digits = self.padded(vals[i as usize], 2);
                self.draw_right_elem(id, &digits);
            }
        }
        self.draw_elem(elem::COLON, None, None);
        if shown(5) {
            self.draw_elem(elem::AM_PM, Some(vals[5]), None);
        }
        for id in elem::KEYS {
            self.draw_elem(id, None, None);
        }
    }

    fn padded(&self, n: u16, width: usize) -> Vec<u16> {
        format!("{n:0width$}").bytes().map(|b| DIGIT_0 + (b - b'0') as u16).collect()
    }

    /// The planet level's idle clock (`space_clock_update` 0x1006a074): 12-hour time, PM in the
    /// second palette, and the date in the order chosen under DISPLAY.
    pub(crate) fn draw_idle_clock(&mut self) {
        let now = self.pet.last.unwrap_or(0);
        let (y, m, d) = date(now);
        let (h, min) = ((now % 86_400 / 3600) as u32, (now % 3600 / 60) as u32);
        self.draw_elem(elem::IDLE_PANEL, None, None);
        self.draw_elem(elem::IDLE_AM_PM, None, (h >= 12).then_some(1));
        let h12 = if h.is_multiple_of(12) { 12 } else { h % 12 };
        for (i, digit) in [Some(h12 / 10), Some(h12 % 10), None, Some(min / 10), Some(min % 10)].into_iter().enumerate()
        {
            // Digit frames are digit + 1; the colon keeps its own frame.
            self.draw_elem(elem::IDLE_TIME + i as u16, digit.map(|n| n as u16 + 1), None);
        }
        let (y, m, d) = (format!("{:04}", y.rem_euclid(10_000)), format!("{m:02}"), format!("{d:02}"));
        let text = match self.pet.date_order {
            1 => format!("{d}/{m}/{y}"),
            2 => format!("{m}/{d}/{y}"),
            _ => format!("{y}/{m}/{d}"),
        };
        for (i, c) in text.bytes().enumerate() {
            let frame = if c == b'/' { 0xb } else { (c - b'0') as u16 + 1 };
            self.draw_elem(elem::IDLE_DATE + i as u16, Some(frame), None);
        }
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
    fn sound_level_scales_and_mutes_the_sounds() {
        let mut g = Game::new(crate::evolve::tests::data(), 1);
        g.frame(NOON, 0);
        g.scene = g.open(Action::Settings);
        press(&mut g, B); // SOUND
        assert!(matches!(g.scene, Scene::Level { level: 2, .. }));
        g.take_sounds();
        press(&mut g, A);
        assert_eq!(g.pet.volume, 1);
        assert_eq!(g.take_sounds(), vec![(0, 30)], "preview at the new level");
        press(&mut g, A);
        press(&mut g, A);
        assert_eq!(g.pet.volume, 0);
        g.take_sounds();
        press(&mut g, C);
        assert!(g.take_sounds().is_empty(), "muted");
        assert!(matches!(g.scene, Scene::Menu { idx: 0, .. }));
    }

    #[test]
    fn save_data_menu_asks_the_page_and_starts_over() {
        let mut g = Game::new(crate::evolve::tests::data(), 1);
        g.pet.points = 500;
        g.scene = g.open(Action::SaveData);
        press(&mut g, B); // EXPORT
        assert_eq!(g.take_connect().as_deref(), Some("save-export"));
        press(&mut g, A);
        press(&mut g, B); // IMPORT
        assert_eq!(g.take_connect().as_deref(), Some("save-import"));
        press(&mut g, A);
        press(&mut g, B); // NEW GAME
        press(&mut g, B); // NO
        assert_eq!(g.pet.points, 500);
        assert!(matches!(&g.scene, Scene::Menu { items, .. } if items[0].0 == REMAKE_TEXT + 3));
        press(&mut g, C);
        assert!(matches!(g.scene, Scene::Menu { idx: 4, .. }), "back on SAVE DATA");
    }

    #[test]
    fn date_time_sets_the_clock_without_catching_up() {
        let mut g = Game::new(crate::evolve::tests::data(), 1);
        g.frame(NOON, 0);
        let onaka = g.pet.onaka;
        g.scene = g.open(Action::DateTime);
        press(&mut g, B); // the notice
        let Scene::Clock { vals, .. } = &g.scene else { panic!("clock") };
        assert_eq!(vals[3..], [12, 0, 1], "12:00 PM");
        press(&mut g, A); // year 2026
        for _ in 0..5 {
            press(&mut g, B);
        }
        press(&mut g, DOWN); // AM
        press(&mut g, B);
        let t = g.pet.last.unwrap();
        assert_eq!(date(t), (2026, 1, 4));
        assert_eq!(t % 86_400, 0, "12 AM");
        g.frame(NOON, 0);
        assert_eq!(g.pet.last, Some(t), "the platform's clock is shifted from now on");
        assert_eq!(g.pet.onaka, onaka, "no catch-up");
    }

    #[test]
    fn delete_continues_or_resets_everything() {
        let mut g = Game::new(crate::evolve::tests::data(), 1);
        g.pet.points = 500;
        g.pet.lang = 3;
        g.scene = g.open(Action::Delete);
        press(&mut g, C);
        assert!(matches!(g.scene, Scene::Menu { .. }), "no way out");
        press(&mut g, A); // RESET ALL
        press(&mut g, B);
        press(&mut g, B); // NO is selected
        assert_eq!(g.pet.points, 500);
        assert!(matches!(&g.scene, Scene::Menu { idx: 0, .. }));
        press(&mut g, A);
        press(&mut g, B);
        press(&mut g, A); // YES
        g.frame(NOON, B);
        g.frame(NOON, B); // still held
        g.frame(NOON, 0);
        assert_eq!((g.pet.points, g.pet.lang, g.pet.def.id), (0, 1, 1000));
        assert!(matches!(&g.scene, Scene::Menu { items, .. } if items.len() == 9), "LANGUAGE");

        let mut g = Game::new(crate::evolve::tests::data(), 1);
        g.scene = g.open(Action::Continue);
        press(&mut g, B); // the notice
        press(&mut g, C);
        assert!(matches!(&g.scene, Scene::Menu { items, .. } if items[0].0 == CONTINUE), "back to DELETE");
        press(&mut g, B);
        press(&mut g, B);
        for _ in 0..6 {
            press(&mut g, B);
        }
        assert!(matches!(g.scene, Scene::Home));
    }

    #[test]
    fn the_dial_runs_down_a_vertical_menu() {
        let mut g = Game::new(crate::evolve::tests::data(), 1);
        g.scene = g.open(Action::Settings);
        press(&mut g, DOWN);
        assert!(matches!(g.scene, Scene::Menu { idx: 1, .. }), "DOWN (the left button) is the next item");
        press(&mut g, UP);
        assert!(matches!(g.scene, Scene::Menu { idx: 0, .. }));
        press(&mut g, A);
        assert!(matches!(g.scene, Scene::Menu { idx: 1, .. }), "A still steps forward");
    }

    #[test]
    fn touch_toggles_in_settings() {
        let mut g = Game::new(crate::evolve::tests::data(), 1);
        assert!(g.touch(), "on by default");
        g.scene = g.open(Action::Settings);
        press(&mut g, UP); // SAVE DATA (last)
        press(&mut g, UP); // TOUCH ON
        press(&mut g, B);
        assert!(!g.touch());
        assert!(matches!(&g.scene, Scene::Menu { idx: 3, items, .. } if items[3].0 == REMAKE_TEXT + 1), "TOUCH OFF");
        assert_eq!(g.string(REMAKE_TEXT + 1)[..2], [337, 332], "T, O");
        press(&mut g, B);
        assert!(g.touch());
    }

    #[test]
    fn date_order_changes_the_planet_clock() {
        let mut g = Game::new(crate::evolve::tests::data(), 1);
        g.scene = g.open(Action::DateOrder);
        press(&mut g, B); // the notice
        press(&mut g, A);
        press(&mut g, B);
        assert_eq!(g.pet.date_order, 1);
        assert!(matches!(g.scene, Scene::Menu { idx: 1, .. }));
    }
}
