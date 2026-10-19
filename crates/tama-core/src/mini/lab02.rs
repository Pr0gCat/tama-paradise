//! Lab game 02: memory cards with a bomb, "MATCHING MEALS!" (09-minigames-lab-a §2.3).

use super::{sprite, Hud, MiniGame};
use crate::data::Data;
use crate::input::{A, B, DOWN, PUSH, UP};
use crate::motion::{Draw, Node};

/// Frames of the opening memorise phase: no input, clock stopped (state 0xfa2).
const MEMORISE: u32 = 200;
/// Time limit by level, seconds (XIP 0x10093fd4, floats).
const TIME_S: [i32; 3] = [50, 40, 30];
const BOMB: u8 = 7;
const BACKGROUND: u16 = 2165;
const BACK: u16 = 2166;
/// Face sprite = 0x877 + card type.
const FACE: u16 = 0x877;
const CURSOR: u16 = 2176;
/// Diver reactions (XIP 0x100a008a): idle, match, miss, bomb; all at UI elements (100, 94).
const DIVER: [u16; 4] = [0xa4b, 0xa48, 0xa49, 0xa4a];
/// Result scripts by result (UI elements 0x4e5..0x4e2).
const RESULTS: [u16; 4] = [0xa55, 0xa54, 0xa53, 0xa4c];

/// Card i (UI element 0x4cf + i); the cursor (0x4cc) uses the same grid.
fn card_xy(i: usize) -> (i32, i32) {
    (11 + 23 * (i % 5) as i32, 12 + 24 * (i / 5) as i32)
}

/// Card states (`FUN_1006a668`): 400/401/402 dealing (back, then face for 120 frames), 100 face
/// down, 200 picked, 300/301 turning back after a miss, 0 matched (stays face up).
fn face_up(state: u16) -> bool {
    matches!(state, 0 | 200 | 300 | 301 | 402)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sub {
    Play,
    /// Diver reacting (0xfa3, 0xfa4): step, frame counter.
    React(u8, u16),
    /// Judge the pair (0xfa5).
    Resolve,
    /// Wait, then continue or end (0xfa6).
    Settle(u16),
    /// Fade out before the result, frames left.
    End(u16),
}

pub struct Lab02 {
    cards: [u8; 15],
    state: [u16; 15],
    timer: [i32; 15],
    cursor: usize,
    pick: [usize; 2],
    picked: usize,
    /// Frames left (+0x184) and play frame counter (+0x188).
    time: i32,
    frame: u32,
    pairs: u8,
    bomb: bool,
    sub: Sub,
    diver: Node,
}

impl Lab02 {
    /// `cards` = lab02_boards[level*5 + plays % 5]. Dealt face down (`FUN_10069ed0`).
    pub fn new(cards: [u8; 15], level: usize) -> Self {
        let mut g = Lab02 {
            cards,
            state: [400; 15],
            timer: [0; 15],
            cursor: 0,
            pick: [0; 2],
            picked: 0,
            time: TIME_S[level.min(2)] * 20,
            frame: 0,
            pairs: 0,
            bomb: false,
            sub: Sub::Play,
            diver: Node::new(0),
        };
        g.react(0);
        g
    }

    /// `FUN_1007ad44`.
    pub fn result(&self) -> u8 {
        match self.pairs {
            _ if self.bomb => 0,
            0 => 0,
            7.. => 3,
            6 => 2,
            _ => 1,
        }
    }

    /// Diver reaction n (`FUN_10083900`).
    fn react(&mut self, n: usize) {
        self.diver = Node::new(DIVER[n]);
        self.diver.place(100, 94);
    }

    /// Move the cursor, skipping matched and picked cards (`FUN_1006a47c`).
    fn step(&mut self, by: usize) {
        for _ in 0..15 {
            self.cursor = (self.cursor + by) % 15;
            if !matches!(self.state[self.cursor], 0 | 200) {
                break;
            }
        }
    }

    fn tick_time(&mut self) {
        if self.time > 0 {
            self.time -= 1;
        }
    }

    /// `FUN_1006a668`: timers count after comparing, so 401 lasts 4i+6 frames and 402 121.
    fn tick_cards(&mut self) {
        for i in 0..15 {
            let t = self.timer[i];
            match self.state[i] {
                400 => (self.timer[i], self.state[i]) = (4 * i as i32 + 5, 401),
                401 => {
                    self.timer[i] -= 1;
                    if t == 0 {
                        (self.timer[i], self.state[i]) = (120, 402);
                    }
                }
                402 => {
                    self.timer[i] -= 1;
                    if t < 1 {
                        self.state[i] = 300;
                    }
                }
                300 => (self.timer[i], self.state[i]) = (5, 301),
                301 => {
                    self.timer[i] -= 1;
                    if t < 1 {
                        self.state[i] = 100;
                    }
                }
                _ => {}
            }
        }
    }

    /// Turn the card under the cursor (B or dial push).
    fn turn(&mut self, sounds: &mut Vec<(u16, u8)>) {
        let c = self.cursor;
        if self.state[c] == 100 {
            self.state[c] = 200;
            if self.cards[c] == BOMB {
                self.bomb = true;
            } else {
                self.pick[self.picked] = c;
                self.picked += 1;
            }
        }
        if self.bomb {
            self.react(3);
            sounds.push((0xc, 100));
            self.sub = Sub::React(0, 0);
        } else if self.picked == 2 {
            let hit = self.cards[self.pick[0]] == self.cards[self.pick[1]];
            self.react(if hit { 1 } else { 2 });
            sounds.push((if hit { 10 } else { 0xb }, 100));
            self.picked = 0;
            self.sub = Sub::React(0, 0);
        } else {
            sounds.push((1, 100));
        }
    }
}

impl MiniGame for Lab02 {
    fn update(&mut self, pressed: u8, _held: u8, data: &Data, sounds: &mut Vec<(u16, u8)>) -> Option<u8> {
        self.diver.tick(&data.motions);
        self.diver.take_sounds(sounds);
        // State function (`FUN_10050954`).
        match self.sub {
            Sub::Play => {
                if self.frame > MEMORISE {
                    if pressed & (A | UP) != 0 {
                        self.step(1);
                        sounds.push((0, 100));
                    } else if pressed & DOWN != 0 {
                        self.step(14);
                        sounds.push((0, 100));
                    }
                    if pressed & (B | PUSH) != 0 && self.picked < 2 {
                        self.turn(sounds);
                    }
                    self.tick_time();
                }
                self.tick_cards();
                if self.time < 1 && !self.bomb {
                    sounds.push((0x4a, 100));
                    self.sub = Sub::End(60);
                }
                self.frame += 1;
            }
            Sub::React(step, n) => {
                self.tick_cards();
                self.tick_time();
                if n <= 13 {
                    self.sub = Sub::React(step, n + 1);
                } else if step == 0 {
                    self.react(if self.bomb { 3 } else { 0 });
                    self.sub = Sub::React(1, 0);
                } else {
                    self.sub = Sub::Resolve;
                }
            }
            Sub::Resolve if self.bomb => self.sub = Sub::End(4),
            Sub::Resolve => {
                let [a, b] = self.pick;
                if self.cards[a] == self.cards[b] {
                    (self.state[a], self.state[b]) = (0, 0);
                    self.pairs += 1;
                } else {
                    (self.state[a], self.state[b]) = (300, 300);
                }
                self.sub = Sub::Settle(5);
            }
            Sub::Settle(n) if n > 0 => self.sub = Sub::Settle(n - 1),
            Sub::Settle(_) => {
                if self.state.iter().filter(|&&s| s == 0).count() < 14 && self.time > 0 {
                    self.react(0);
                    self.sub = Sub::Play;
                } else {
                    self.sub = Sub::End(4);
                }
            }
            Sub::End(n) if n > 1 => self.sub = Sub::End(n - 1),
            Sub::End(_) => return Some(self.result()),
        }
        None
    }

    fn draws(&self, out: &mut Vec<Draw>) {
        sprite(out, BACKGROUND, 0, 64, 64, 64);
        for i in 0..15 {
            let (x, y) = card_xy(i);
            let id = if face_up(self.state[i]) { FACE + self.cards[i] as u16 } else { BACK };
            sprite(out, id, 0, x, y, 64);
        }
        self.diver.draws(0, 0, out);
        if self.frame > MEMORISE && !self.bomb {
            let (x, y) = card_xy(self.cursor);
            sprite(out, CURSOR, 0, x, y, 64);
        }
    }

    /// Time left only (no score display), once under 10 s (`FUN_1006a8fc`).
    fn hud(&self) -> Vec<Hud> {
        let left = self.time / 20;
        if self.frame <= MEMORISE || left >= 10 {
            return Vec::new();
        }
        vec![Hud::Sprite { id: 2063, frame: 0, x: 120, y: 11 }, Hud::Number { n: left as u32, x: 120, y: 11 }]
    }

    fn result_script(&self, result: u8) -> u16 {
        RESULTS[result as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOARD: [u8; 15] = [0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7];

    fn run(g: &mut Lab02, frames: usize, pressed: u8) -> (Option<u8>, Vec<(u16, u8)>) {
        let d = crate::evolve::tests::data();
        let (mut r, mut s) = (None, Vec::new());
        for i in 0..frames {
            r = r.or(g.update(if i == 0 { pressed } else { 0 }, 0, &d, &mut s));
        }
        (r, s)
    }

    /// Turn the cards at `a` then `b` and wait for the judgement to finish.
    fn pair(g: &mut Lab02, a: usize, b: usize) {
        for c in [a, b] {
            while g.cursor != c {
                run(g, 1, A);
            }
            run(g, 1, B);
        }
        run(g, 30 + 1 + 6 + 1, 0);
    }

    #[test]
    fn cards_show_during_memorise_and_input_waits_200_frames() {
        let mut g = Lab02::new(BOARD, 0);
        run(&mut g, 5, 0);
        assert!(!face_up(g.state[0]));
        run(&mut g, 2, 0);
        assert!(face_up(g.state[0]), "card 0 face up at frame 6");
        run(&mut g, 183, B | A);
        assert!(face_up(g.state[14]), "last card turning back");
        run(&mut g, 1, 0);
        assert!(g.state.iter().all(|&s| s == 100), "all face down after 191 frames");
        run(&mut g, 10, A);
        assert_eq!((g.cursor, g.time), (0, 1000), "no input or clock before frame 201");
        run(&mut g, 1, A);
        assert_eq!((g.cursor, g.time), (1, 999));
    }

    #[test]
    fn matching_and_missing() {
        let mut g = Lab02::new(BOARD, 0);
        run(&mut g, 201, 0);
        pair(&mut g, 0, 1);
        assert_eq!((g.pairs, g.state[0], g.state[1], g.sub), (1, 0, 0, Sub::Play));
        pair(&mut g, 2, 4);
        assert_eq!(g.pairs, 1);
        run(&mut g, 7, 0);
        assert_eq!((g.state[2], g.state[4]), (100, 100), "missed cards turn back");
        assert_eq!(g.cursor, 4);
        run(&mut g, 1, A);
        assert_eq!(g.cursor, 5);
        run(&mut g, 1, DOWN);
        run(&mut g, 1, DOWN);
        assert_eq!(g.cursor, 3, "matched cards 0/1 are skipped, picked/turned ones only while 0/200");
    }

    #[test]
    fn all_pairs_wins_bomb_loses() {
        let mut g = Lab02::new(BOARD, 2);
        run(&mut g, 201, 0);
        for p in 0..7 {
            pair(&mut g, 2 * p, 2 * p + 1);
        }
        assert_eq!(g.pairs, 7);
        let mut r = None;
        for _ in 0..10 {
            r = r.or(run(&mut g, 1, 0).0);
        }
        assert_eq!(r, Some(3));

        let mut g = Lab02::new(BOARD, 0);
        run(&mut g, 201, 0);
        pair(&mut g, 0, 1);
        while g.cursor != 14 {
            run(&mut g, 1, A);
        }
        let (_, s) = run(&mut g, 1, B);
        assert!(g.bomb && s.contains(&(0xc, 100)));
        assert_eq!(run(&mut g, 40, 0).0, Some(0));
    }

    #[test]
    fn time_up_and_results() {
        let mut g = Lab02::new(BOARD, 2);
        run(&mut g, 201, 0);
        pair(&mut g, 0, 1);
        assert_eq!(g.time, 600 - 34, "3 input frames + 30 reacting + 1; judging and waiting stop the clock");
        let (mut frames, mut sounds) = (0, Vec::new());
        while !matches!(g.sub, Sub::End(_)) {
            sounds.extend(run(&mut g, 1, 0).1);
            frames += 1;
        }
        assert_eq!(frames, 566);
        assert!(sounds.contains(&(0x4a, 100)));
        assert_eq!(run(&mut g, 60, 0).0, Some(1));
        for (pairs, r) in [(0, 0), (1, 1), (5, 1), (6, 2), (7, 3)] {
            g.pairs = pairs;
            assert_eq!(g.result(), r);
        }
    }
}
