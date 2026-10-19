//! Calendar: dates, daily events (`FUN_10083984`), day change and event outcomes.

use super::*;

/// Calendar date (year, month 1–12, day 1–31) of local seconds since 1970-01-01.
pub fn date(t: u64) -> (i64, u32, u32) {
    // Howard Hinnant's civil_from_days.
    let z = (t / DAY) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + (m <= 2) as i64, m, d)
}

/// Calendar events of the day (`FUN_10083984`); the player's birthday is added by `raise_calendar`.
pub(super) fn calendar_events(t: u64) -> Vec<u8> {
    let (_, m, d) = date(t);
    let mut ev = Vec::new();
    if (m, d) == (1, 1) {
        ev.push(req::NEWYEAR);
    }
    if d % 10 == 9 {
        ev.push(req::METEOR);
    }
    if d % 7 == 0 {
        ev.push(req::SHOOTINGSTAR);
    }
    if (m, d) == (11, 23) {
        ev.push(req::UFO);
    }
    match (m, d) {
        (12, 1..=23) => ev.push(req::SANTA),
        (12, 24) => ev.push(req::EVE),
        (12, 25) => ev.push(req::XMAS),
        _ => {}
    }
    ev
}

pub const EVENTS: [u8; 9] = [
    req::BIRTHDAY,
    req::NEWYEAR,
    req::DODODO,
    req::METEOR,
    req::SHOOTINGSTAR,
    req::UFO,
    req::SANTA,
    req::EVE,
    req::XMAS,
];

impl Pet {
    /// `care_day_change` 0x10013234; `t` is the new day.
    pub(super) fn day_change(&mut self, t: u64) {
        if !self.is_done(req::OTEIRE) && self.cond != Cond::Dead {
            self.yogore = (self.yogore + 1).min(3);
        }
        for id in [req::WAKE, req::SLEEP, req::OTEIRE].into_iter().chain(EVENTS) {
            self.clear(id);
        }
        self.play_days = (self.play_days + 1).min(999);
        self.no_wake_today = false;
        self.no_sleep_today = false;
        self.lab_bonus = false;
        self.hunt_used = [0; 3];
        self.breeds_today = 0;
        self.raise_calendar(t);
    }

    /// Today's calendar events, plus the player's birthday (`FUN_1006d904`: 2/29 falls on 3/1 in
    /// other years).
    pub(super) fn raise_calendar(&mut self, t: u64) {
        for id in calendar_events(t) {
            self.raise(id);
        }
        let (y, m, d) = date(t);
        let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
        let bd =
            self.birthday.map(|(bm, bd)| if (bm, bd) == (2, 29) && !leap { (3, 1) } else { (bm as u32, bd as u32) });
        if bd == Some((m, d)) {
            self.raise(req::BIRTHDAY);
        }
    }

    /// An event scene finished (`FUN_1003f1f0` / `FUN_1003efc4`): apply its reward.
    pub fn finish_event(&mut self, id: u8) {
        match id {
            req::NEWYEAR => self.points = (self.points + 1000).min(99_999),
            req::XMAS => self.add_gokigen(20),
            req::DODODO => self.add_gokigen(10),
            _ => {}
        }
        self.mark_done(id);
    }

    /// End of the meteor event game (`labo05_tick` 0xdad): ±10 mood; a hit also injures the pet.
    pub fn finish_meteor(&mut self, survived: bool) {
        if survived {
            self.add_gokigen(10);
        } else {
            self.add_gokigen(-10);
            if self.cond == Cond::Normal {
                self.raise(req::INJURY);
            }
            self.lose_field_toys();
        }
        self.mark_done(req::METEOR);
    }

    /// DODODO (`FUN_10012c90`): raised on the 13th at 17:00 for an awake pet at rank ≥ 6; still
    /// unanswered at 17:15 it becomes DODODO_IGNORE: mood −10, an injury, and this field's
    /// outside toys are lost.
    pub(super) fn dododo(&mut self, t: u64, awake: bool) {
        let (_, _, d) = date(t);
        let min = t % DAY / 60;
        if d != 13 || !(17 * 60..18 * 60).contains(&min) {
            return;
        }
        if min < 17 * 60 + 15 {
            if awake && self.rank >= 6 && self.def.stage >= 3 && !self.was_raised(req::DODODO) {
                self.raise(req::DODODO);
            }
        } else if self.is_pending(req::DODODO) {
            self.add_gokigen(-10);
            if self.cond == Cond::Normal {
                self.raise(req::INJURY);
            }
            self.lose_field_toys();
            self.mark_done(req::DODODO);
        }
    }
}
