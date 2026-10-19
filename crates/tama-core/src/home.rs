//! HOME pet behaviour: which looping motion plays (firmware `FUN_10078c94`, 07-animation §2.5).

use crate::pet::{req, status, Cond, Pet};

/// Random behaviour candidates, XIP 0x100a05b4.
const RANDOM_SEQ: [u8; 27] = [5, 1, 2, 6, 4, 9, 7, 3, 8, 9, 2, 7, 1, 8, 5, 6, 3, 4, 2, 5, 3, 4, 1, 8, 7, 9, 6];

/// Pending calls checked in priority order (subset of XIP 0x100a2fdf) → HOME state.
const CALLS: [(u8, u8); 4] = [(req::EVOL_SUPPORT, 0x1a), (req::ONAKA, 0x1b), (req::GOKIGEN, 0x1c), (req::SOWASOWA, 0x11)];

pub const STATE_IDLE: u8 = 8;

/// State forced by the pet's condition, or None when the behaviour is picked at random.
pub fn forced_state(p: &Pet, now: u64) -> Option<u8> {
    if p.def.stage == 1 {
        return Some(0);
    }
    if p.is_sleeping_at(now) || p.is_done(req::NAP) {
        return Some(match p.cond {
            Cond::Cold => 3,
            Cond::Injury => 4,
            _ => 2,
        });
    }
    match p.cond {
        Cond::Cold => return Some(0x17),
        Cond::Injury => return Some(0x18),
        Cond::Death | Cond::Dying => return Some(0x19),
        Cond::Dead => return Some(STATE_IDLE),
        Cond::Normal => {}
    }
    if let Some(&(_, s)) = CALLS.iter().find(|(r, _)| p.is_pending(*r)) {
        return Some(s);
    }
    for (bit, s) in [(status::LOVE, 0x12), (status::IMMUNITY, 0x13), (status::HAPPY, 0x14)] {
        if p.status & bit != 0 {
            return Some(s);
        }
    }
    if p.onaka <= 2 {
        return Some(0x0e);
    }
    None
}

/// Pick a random behaviour, walking the candidate sequence from `*seq` (advanced in place).
/// `r` is a random number for the states that choose among variants.
pub fn random_state(p: &Pet, now: u64, seq: &mut usize, r: u32) -> u8 {
    if p.gokigen <= 5 {
        return 11 + (r % 2) as u8;
    }
    for _ in 0..RANDOM_SEQ.len() {
        let c = RANDOM_SEQ[*seq % RANDOM_SEQ.len()];
        *seq += 1;
        let pick = match c {
            2 if p.onaka >= 5 => Some(0x0d),
            3 if (6..=15).contains(&p.gokigen) => Some(8 + (r % 3) as u8),
            4 if p.gokigen >= 16 => Some(5 + (r % 3) as u8),
            6 if matches!(Pet::period(now), 0 | 3) => Some(0x0f),
            8 if p.status & status::DANCE != 0 => Some(0x15),
            9 if p.status & status::KIRAKIRA != 0 => Some(0x16),
            // 1/5 need a friend in the field, 7 a 13:01–13:29 event; not implemented.
            _ => None,
        };
        if let Some(s) = pick {
            return s;
        }
    }
    STATE_IDLE
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pet::tests::def;

    const NOON: u64 = 86_400 + 12 * 3600;

    #[test]
    fn calls_and_conditions_win_over_random() {
        let mut p = Pet::new(def(2113, 3, 45, 20, 0), 1);
        p.onaka = 6;
        p.gokigen = 10;
        assert_eq!(forced_state(&p, NOON), None);
        p.onaka = 0;
        p.raise(req::ONAKA);
        assert_eq!(forced_state(&p, NOON), Some(0x1b));
        p.cond = Cond::Cold;
        assert_eq!(forced_state(&p, NOON), Some(0x17));
        assert_eq!(forced_state(&p, 86_400 + 23 * 3600), Some(3), "asleep with a cold");
    }

    #[test]
    fn random_states_follow_mood() {
        let mut p = Pet::new(def(2113, 3, 45, 20, 0), 1);
        p.onaka = 3;
        p.gokigen = 18;
        let mut seq = 0;
        let s = random_state(&p, NOON, &mut seq, 1);
        assert!((5..=7).contains(&s), "{s}");
        p.gokigen = 3;
        assert!((11..=12).contains(&random_state(&p, NOON, &mut seq, 0)));
    }
}
