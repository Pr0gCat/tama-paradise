//! Play-together game 04 "MATCHING" (scene 0x69, tick `FUN_10059600`; 09-minigames-together §2.2).

use super::{push_sprite, sin16, Hud, MiniGame};
use crate::data::{Data, MiniData};
use crate::input::{A, B, DOWN, PUSH, UP};
use crate::motion::{Draw, Look, Node};

const INTRO: u16 = 0xae1;
const IDLE: u16 = 0xae4;
const HAPPY: u16 = 0xae5;
const SAD: u16 = 0xae6;
/// Result scripts by score 0..3 (UI elements 0x57b..0x578, all at (64,80)).
const RESULTS: [u16; 4] = [0xaea, 0xae9, 0xae8, 0xae7];
const TABLE: u16 = 0x8b3;
/// 7 frames: 0 = back, 1–6 = faces.
const CARD: u16 = 0x8b4;
/// Card outline: palette 1 = the pet's card, palette 0 = the cursor.
const OUTLINE: u16 = 0x8b5;
const ARROW: u16 = 0x8b6;
/// Card slots (UI elements 0x567..0x569); cards slide in from x 144 (0x56a).
const SLOT_X: [i32; 3] = [32, 64, 96];
/// Scene clock step, 16.16 seconds per frame (C1).
const STEP: i32 = 0xccc;
const SEC: i32 = 0x10000;
const PX: i32 = 0x10000;

/// A `FUN_10066b34` tween of a 16.16 value over the scene clock.
#[derive(Clone, Copy, Debug, Default)]
struct Tween {
    from: i32,
    to: i32,
    start: i32,
    dur: i32,
    curve: u8,
}

impl Tween {
    fn at(px: i32) -> Self {
        Tween { from: px * PX, to: px * PX, ..Default::default() }
    }

    /// `FUN_1008ebec`.
    fn value(&self, m: &MiniData, clock: i32) -> i32 {
        if self.dur == 0 {
            return self.to;
        }
        let t = ((clock - self.start) as i64 * 0x10000 / self.dur as i64) as i32;
        if t < 1 {
            return self.from;
        }
        if t >= 0x10000 {
            return self.to;
        }
        let k = ease(m, self.curve, t) as i64;
        ((self.from as i64 * (0x10000 - k) + self.to as i64 * k) >> 16) as i32
    }
}

/// Tween curves 1 (linear), 3 (`sin(t·90°)`, `FUN_10062170`) and 7 (S shape, `FUN_10061df0`).
fn ease(m: &MiniData, curve: u8, t: i32) -> i32 {
    match curve {
        3 => sin16(m, (t >> 2) as u32),
        7 => {
            // [guess] the sin argument is lost in the decompile: u = t − sin(2πt)/2π as curve 4.
            let u = t - (sin16(m, t as u32) as i64 * 0x10000 / 0x6487e) as i32;
            let s = u * 2 - 0x10000;
            let a = (0x10000 - s.abs()) as i64;
            let v = 0x10000 - ((a * a) >> 16) as i32;
            (if s < 0 { -v } else { v } >> 1) + 0x8000
        }
        _ => t,
    }
}

pub struct Together04 {
    cards: Vec<[u8; 3]>,
    slots: Vec<u8>,
    /// Cycle counter of the first round (p = k % 10, q = k % 5, +1 every round).
    k0: usize,
    state: u16,
    wait: u16,
    clock: i32,
    /// Auto-confirm timer (`ctx[8]`, −1 every frame).
    timer: u16,
    intro: Option<Node>,
    pet: Node,
    pet_x: Tween,
    pet_y: Tween,
    card_x: [Tween; 3],
    card_y: [Tween; 3],
    /// Positions for drawing: pet, then the 3 cards.
    at: [(i32, i32); 4],
    face: [u16; 3],
    cards_on: bool,
    pet_outline: bool,
    cursor_outline: bool,
    arrow: bool,
    /// This round's card values, the pet's card and the player's.
    v: [u8; 3],
    pos: usize,
    cursor: usize,
    /// Card being turned or dropped (states 4000, 0xfa5).
    i: usize,
    round: usize,
    score: u8,
}

impl Together04 {
    /// `cards`/`slots` = together04 tables; `n` = play count (3 rounds per play).
    pub fn new(cards: Vec<[u8; 3]>, slots: Vec<u8>, n: usize) -> Self {
        Together04 {
            cards,
            slots,
            // ponytail: the firmware keeps two save counters (mod 10 and mod 5) bumped per round;
            // both start at 0, so one count of rounds played gives the same p and q.
            k0: n * 3,
            state: 1,
            wait: 0,
            clock: 0,
            timer: 0,
            intro: Some(Node::new(INTRO)),
            pet: Node::new(IDLE),
            pet_x: Tween::at(64),
            pet_y: Tween::at(80),
            card_x: [Tween::at(144); 3],
            card_y: [Tween::at(96); 3],
            at: [(64, 80), (144, 96), (144, 96), (144, 96)],
            face: [0; 3],
            cards_on: false,
            pet_outline: false,
            cursor_outline: false,
            arrow: false,
            v: [0; 3],
            pos: 0,
            cursor: 0,
            i: 0,
            round: 0,
            score: 0,
        }
    }

    fn tween(&self, from: i32, to: i32, dur: i32, curve: u8) -> Tween {
        Tween { from, to: to * PX, start: self.clock, dur, curve }
    }

    /// Pet node to x (curve 3, 1 s) with a hop on y (curve 7, 72 → 80).
    fn pet_hop(&mut self, x: i32) {
        self.pet_x = self.tween(self.pet_x.to, x, SEC, 3);
        self.pet_y = self.tween(72 * PX, 80, SEC, 7);
    }

    fn pet_rise(&mut self, y: i32) {
        self.pet_y = self.tween(self.pet_y.to, y, SEC / 2, 3);
    }

    /// Next cursor in direction `d` (1 or 2 = −1), skipping the pet's card.
    fn step_cursor(&mut self, d: usize) {
        self.cursor = (self.cursor + d) % 3;
        if self.cursor == self.pos {
            self.cursor = (self.cursor + d) % 3;
        }
    }

    fn state_tick(&mut self, pressed: u8, sounds: &mut Vec<(u16, u8)>) -> Option<u8> {
        match self.state {
            1000 => self.state = 0x3e9,
            0x3e9 => {
                let k = self.k0 + self.round;
                self.v = self.cards.get(k % 10).copied().unwrap_or([0, 0, 1]);
                self.pos = self.slots.get(k % 5).map_or(1, |&s| s as usize).min(2);
                self.cards_on = true;
                self.face = [0; 3];
                self.card_x = [Tween::at(144); 3];
                self.card_y = [Tween::at(96); 3];
                (self.wait, self.state) = (20, 0x3ea);
            }
            0x3ea => {
                self.card_x = SLOT_X.map(|x| self.tween(144 * PX, x, SEC, 3));
                (self.wait, self.state) = (20, 0x3eb);
            }
            0x3eb => (self.wait, self.state) = (10, 2000),
            // The pet goes to its card and takes it.
            2000 => {
                sounds.push((9, 100));
                if self.pos == 1 {
                    self.pet_rise(72);
                } else {
                    self.pet_hop(SLOT_X[self.pos]);
                }
                self.pet_outline = true;
                (self.wait, self.state) = (10, 0x7d1);
            }
            0x7d1 => {
                if self.pos == 1 {
                    self.pet_rise(80);
                }
                (self.wait, self.state) = (20, 0x7d2);
            }
            0x7d2 => {
                self.card_y[self.pos] = self.tween(96 * PX, 88, SEC / 2, 1);
                sounds.push((1, 100));
                (self.wait, self.state) = (20, 0x7d3);
            }
            0x7d3 => {
                sounds.push((9, 100));
                if self.pos == 1 {
                    self.pet_rise(72);
                } else {
                    self.pet_hop(64);
                }
                (self.wait, self.state) = (10, 0x7d4);
            }
            0x7d4 => {
                if self.pos == 1 {
                    self.pet_rise(80);
                }
                (self.wait, self.state) = (10, 3000);
            }
            3000 => {
                self.arrow = true;
                self.cursor_outline = true;
                self.cursor = if self.pos == 0 { 1 } else { 0 };
                self.timer = 200;
                (self.wait, self.state) = (10, 0xbb9);
            }
            0xbb9 => {
                if pressed & (A | UP | DOWN) != 0 {
                    sounds.push((0, 100));
                    self.timer = 200;
                    self.step_cursor(if pressed & (A | UP) != 0 { 1 } else { 2 });
                } else if pressed & (B | PUSH) != 0 || self.timer == 0 {
                    self.state = 0xbba;
                }
            }
            0xbba => {
                sounds.push((1, 100));
                self.card_y[self.cursor] = self.tween(96 * PX, 104, SEC / 2, 3);
                self.arrow = false;
                self.i = 0;
                (self.wait, self.state) = (20, 4000);
            }
            // Turn the cards one by one; faces (0x10091f10) = (1,2), (3,4), (5,6) by round.
            4000 => {
                self.face[self.i] = 1 + 2 * self.round as u16 + self.v[self.i] as u16;
                sounds.push((10, 100));
                self.i += 1;
                self.wait = 20;
                if self.i == 3 {
                    self.state = 0xfa2;
                }
            }
            0xfa2 => {
                let hit = self.v[self.cursor] == self.v[self.pos];
                self.score += hit as u8;
                self.pet = Node::new(if hit { HAPPY } else { SAD });
                self.state = 0xfa3;
            }
            0xfa3 if self.pet.is_finished() => {
                self.pet = Node::new(IDLE);
                self.i = 0;
                self.state = 0xfa5;
            }
            0xfa5 => {
                let y = self.card_y[self.i].to;
                self.card_y[self.i] = self.tween(y, 146, SEC / 2, 3);
                self.i += 1;
                self.wait = 5;
                if self.i == 3 {
                    self.round += 1;
                    self.wait = 20;
                    self.state = if self.round < 3 { 0xfa7 } else { 5000 };
                }
            }
            0xfa7 => {
                (self.cards_on, self.pet_outline, self.cursor_outline) = (false, false, false);
                self.state = 0x3e9;
            }
            5000 => return Some(self.score),
            _ => {}
        }
        None
    }
}

impl MiniGame for Together04 {
    fn update(&mut self, pressed: u8, _held: u8, data: &Data, sounds: &mut Vec<(u16, u8)>) -> Option<u8> {
        self.timer = self.timer.saturating_sub(1);
        if let Some(intro) = &mut self.intro {
            intro.tick(&data.motions);
            intro.take_sounds(sounds);
            if intro.is_finished() {
                self.intro = None;
                self.state = 1000;
            }
        } else {
            self.pet.tick(&data.motions);
            self.pet.take_sounds(sounds);
            if self.wait > 0 {
                self.wait -= 1;
            } else if let Some(r) = self.state_tick(pressed, sounds) {
                return Some(r);
            }
        }
        self.clock += STEP;
        let (m, c) = (&data.mini, self.clock);
        self.at[0] = (self.pet_x.value(m, c) >> 16, self.pet_y.value(m, c) >> 16);
        for i in 0..3 {
            self.at[i + 1] = (self.card_x[i].value(m, c) >> 16, self.card_y[i].value(m, c) >> 16);
        }
        None
    }

    fn draws(&self, out: &mut Vec<Draw>) {
        if let Some(intro) = &self.intro {
            return intro.draws(64, 64, out);
        }
        self.pet.draws(self.at[0].0, self.at[0].1, out);
        push_sprite(out, TABLE, 0, 64, 102, 64);
        if self.cards_on {
            for i in 0..3 {
                let (x, y) = self.at[i + 1];
                push_sprite(out, CARD, self.face[i], x, y, 64);
                for (on, palette) in [(self.pet_outline && i == self.pos, 1), (self.cursor_outline && i == self.cursor, 0)] {
                    if on {
                        out.push(Draw { look: Look::Sprite { id: OUTLINE, frame: 0 }, x, y, flip: false, alpha: 64, palette });
                    }
                }
            }
        }
        if self.arrow {
            push_sprite(out, ARROW, 0, SLOT_X[self.cursor], 116, 64);
        }
    }

    fn hud(&self) -> Vec<Hud> {
        if self.intro.is_some() {
            return Vec::new();
        }
        // ponytail: the "\x9a" icon glyph before the number is not drawn (no text in Hud).
        vec![Hud::Sprite { id: 0x80e, frame: 0, x: 15, y: 11 }, Hud::Number { n: self.score as u32, x: 14, y: 11 }]
    }

    fn result_script(&self, result: u8) -> u16 {
        RESULTS[result as usize]
    }

    /// C only quits from state 1000 on, not during the opening.
    fn uses_c(&self) -> bool {
        self.intro.is_some()
    }

    fn field_bg(&self) -> bool {
        true
    }

    fn result_at(&self) -> (i32, i32) {
        (64, 80)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CARDS: [[u8; 3]; 10] = [[0, 0, 1], [1, 0, 1], [0, 1, 0], [0, 1, 0], [1, 0, 0], [0, 1, 1], [0, 1, 0], [1, 0, 1], [0, 1, 1], [1, 1, 0]];
    const SLOTS: [u8; 5] = [1, 2, 0, 2, 1];

    fn game(n: usize) -> Together04 {
        Together04::new(CARDS.to_vec(), SLOTS.to_vec(), n)
    }

    /// Run until the cursor is live, pick card `pick` (by A presses, then B), and finish the round.
    fn play_round(g: &mut Together04, d: &Data, pick: Option<usize>) -> Option<u8> {
        let mut s = Vec::new();
        while g.state != 0xbb9 || g.wait > 0 {
            if let Some(r) = g.update(0, 0, d, &mut s) {
                return Some(r);
            }
        }
        let round = g.round;
        if let Some(p) = pick {
            assert_ne!(p, g.pos, "the pet's card cannot be picked");
            while g.cursor != p {
                g.update(A, 0, d, &mut s);
            }
            g.update(B, 0, d, &mut s);
        }
        while g.round == round {
            if let Some(r) = g.update(0, 0, d, &mut s) {
                return Some(r);
            }
        }
        None
    }

    fn match_of(g: &Together04) -> usize {
        (0..3).find(|&c| c != g.pos && g.v[c] == g.v[g.pos]).expect("winnable")
    }

    #[test]
    fn rounds_follow_the_cycle_counter() {
        let d = super::super::tests::data();
        let mut g = game(1);
        play_round(&mut g, &d, None);
        // k = 3: p = 3, q = 3 → pet at slot 2.
        assert_eq!((g.v, g.pos), (CARDS[3], 2));
        assert_eq!(g.face, [1, 2, 1], "round 0 faces are frames 1/2");
    }

    #[test]
    fn matching_cards_score_and_three_wins_is_perfect() {
        let d = super::super::tests::data();
        let mut g = game(0);
        let mut r = None;
        for _ in 0..3 {
            let mut s = Vec::new();
            while g.state != 0xbb9 {
                g.update(0, 0, &d, &mut s);
            }
            let p = match_of(&g);
            r = play_round(&mut g, &d, Some(p));
        }
        assert_eq!(g.score, 3);
        let r = r.or_else(|| (0..100).find_map(|_| g.update(0, 0, &d, &mut Vec::new())));
        assert_eq!(r, Some(3));
    }

    #[test]
    fn wrong_card_scores_nothing_and_timer_confirms() {
        let d = super::super::tests::data();
        let mut g = game(0);
        let mut s = Vec::new();
        while g.state != 0xbb9 {
            g.update(0, 0, &d, &mut s);
        }
        let wrong = (0..3).find(|&c| c != g.pos && g.v[c] != g.v[g.pos]).unwrap();
        play_round(&mut g, &d, Some(wrong));
        assert_eq!(g.score, 0);
        // No input: the 200-frame timer confirms the default cursor.
        while g.state != 0xbb9 || g.wait > 0 {
            g.update(0, 0, &d, &mut s);
        }
        let mut frames = 0;
        while g.state == 0xbb9 {
            g.update(0, 0, &d, &mut s);
            frames += 1;
        }
        assert_eq!(frames, 190, "200 frames from state 3000, 10 of them waiting");
    }

    #[test]
    fn cursor_skips_the_pets_card() {
        let mut g = game(0);
        g.pos = 1;
        g.cursor = 0;
        g.step_cursor(1);
        assert_eq!(g.cursor, 2);
        g.step_cursor(1);
        assert_eq!(g.cursor, 0);
        g.step_cursor(2);
        assert_eq!(g.cursor, 2);
    }

    #[test]
    fn synced_counters_always_leave_a_match() {
        for k in 0..30 {
            let (v, pos) = (CARDS[k % 10], SLOTS[k % 5] as usize);
            assert!((0..3).any(|c| c != pos && v[c] == v[pos]), "k {k}");
        }
    }

    #[test]
    fn tween_curves_end_points() {
        let d = super::super::tests::data();
        let m = &d.mini;
        for c in [1, 3, 7] {
            assert_eq!(ease(m, c, 0), 0);
            assert!((ease(m, c, 0x10000) - 0x10000).abs() < 0x100, "curve {c}");
            assert!((ease(m, c, 0x8000) - 0x8000).abs() < 0x3000 || c == 3, "curve {c} middle");
        }
        assert!(ease(m, 3, 0x8000) > 0xb000, "ease-out is ahead at half time");
    }
}
