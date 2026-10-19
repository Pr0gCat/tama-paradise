//! Lab game 00: feed the babies that wake up (09-minigames-lab-a §2.1).

use super::{Hud, MiniGame};
use crate::data::Data;
use crate::motion::{Draw, Node};

const FRAMES: u16 = 500;
const ROW_EVERY: u16 = 20;
const THRESHOLDS: [u16; 3] = [10, 8, 1];
/// Hole positions (UI elements 0x3eb–0x3f0) and bottle positions (0x3f1–0x3f6).
const HOLES: [(i32, i32); 6] = [(20, 51), (62, 51), (104, 51), (20, 99), (62, 99), (104, 99)];
const CURSOR: [(i32, i32); 6] = [(36, 62), (78, 62), (120, 62), (36, 110), (78, 110), (120, 110)];
/// Hole state scripts: asleep, waking, peeking, fed, woken by mistake.
const SCRIPTS: [u16; 5] = [0x977, 0x978, 0x979, 0x97b, 0x97d];
const BACKGROUND: u16 = 0x96c;
const BOTTLE: u16 = 0x976;
const RESULTS: [u16; 4] = [0x996, 0x98f, 0x988, 0x980];

pub struct Lab00 {
    board: Vec<[i16; 6]>,
    row: usize,
    frame: u16,
    score: u16,
    cursor: usize,
    end_wait: Option<u16>,
    holes: [(u8, u16); 6],
    hole_nodes: Vec<Node>,
    bottle: Node,
    bg: Node,
}

impl Lab00 {
    /// `board` = boards[level*3 + plays % 3].
    pub fn new(board: Vec<[i16; 6]>) -> Self {
        let hole_nodes = (0..6)
            .map(|i| {
                let mut n = Node::new(SCRIPTS[0]);
                n.place(HOLES[i].0, HOLES[i].1);
                n.palette = i as u8;
                n
            })
            .collect();
        let mut bottle = Node::new(BOTTLE);
        bottle.place(CURSOR[0].0, CURSOR[0].1);
        Lab00 { board, row: 0, frame: 0, score: 0, cursor: 0, end_wait: None, holes: [(0, 0); 6], hole_nodes, bottle, bg: Node::new(BACKGROUND) }
    }

    fn set(&mut self, i: usize, state: u8, timer: u16) {
        self.holes[i] = (state, timer);
        let mut n = Node::new(SCRIPTS[state as usize]);
        n.place(HOLES[i].0, HOLES[i].1);
        n.palette = i as u8;
        self.hole_nodes[i] = n;
    }

    fn finish(&mut self, sounds: &mut Vec<(u16, u8)>, sound: u16) {
        sounds.push((sound, 100));
        self.end_wait = Some(60);
    }

    pub fn result(&self) -> u8 {
        THRESHOLDS.iter().position(|&t| self.score >= t).map_or(0, |i| 3 - i as u8)
    }
}

impl MiniGame for Lab00 {
    fn update(&mut self, pressed: u8, _held: u8, data: &Data, sounds: &mut Vec<(u16, u8)>) -> Option<u8> {
        for n in self.hole_nodes.iter_mut().chain([&mut self.bottle, &mut self.bg]) {
            n.tick(&data.motions);
            n.take_sounds(sounds);
        }
        if let Some(w) = self.end_wait {
            self.end_wait = Some(w - 1);
            self.tick_holes(true);
            return (w == 1).then(|| self.result());
        }
        self.frame += 1;
        if self.frame == FRAMES {
            self.finish(sounds, 0x4a);
            return None;
        }
        if self.frame % ROW_EVERY == 0 {
            if let Some(row) = self.board.get(self.row).copied() {
                for (i, &v) in row.iter().enumerate() {
                    if v != 0 {
                        self.set(i, 0, 1);
                    }
                }
            }
            self.row += 1;
        }
        if pressed & crate::input::A != 0 {
            sounds.push((0, 100));
            self.cursor = (self.cursor + 1) % 6;
            self.bottle.place(CURSOR[self.cursor].0, CURSOR[self.cursor].1);
        } else if pressed & crate::input::B != 0 {
            match self.holes[self.cursor].0 {
                1 | 2 => {
                    sounds.push((0x34, 100));
                    self.set(self.cursor, 3, 50);
                    self.score += 1;
                    if self.score >= 10 {
                        self.finish(sounds, 0x4a);
                    }
                }
                0 => {
                    sounds.push((0x30, 100));
                    self.set(self.cursor, 4, 50);
                    self.finish(sounds, 0x4a);
                }
                _ => {}
            }
        }
        self.tick_holes(false);
        None
    }

    fn draws(&self, out: &mut Vec<Draw>) {
        self.bg.draws(64, 64, out);
        for n in &self.hole_nodes {
            n.draws(0, 0, out);
        }
        let (state, timer) = self.holes[self.cursor];
        if self.end_wait.is_none() && !(state == 3 && timer > 29) {
            self.bottle.draws(0, 0, out);
        }
    }

    fn hud(&self) -> Vec<Hud> {
        let mut h = vec![Hud::Sprite { id: 2062, frame: 0, x: 15, y: 11 }, Hud::Number { n: self.score as u32, x: 15, y: 11 }];
        let left = (FRAMES.saturating_sub(self.frame)) / 20;
        if left < 10 {
            h.push(Hud::Sprite { id: 2063, frame: 0, x: 120, y: 11 });
            h.push(Hud::Number { n: left as u32, x: 120, y: 11 });
        }
        h
    }

    fn result_script(&self, result: u8) -> u16 {
        RESULTS[result as usize]
    }
}

impl Lab00 {
    /// Per-hole timers (`FUN_100675ac`): asleep → waking (20 f) → peeking (60 f) → asleep.
    fn tick_holes(&mut self, ending: bool) {
        for i in 0..6 {
            let (state, timer) = self.holes[i];
            if timer == 0 {
                continue;
            }
            if timer > 1 {
                self.holes[i].1 -= 1;
                continue;
            }
            let state = if ending { 3 } else { state };
            match state {
                0 => self.set(i, 1, 20),
                1 => self.set(i, 2, 60),
                _ => self.set(i, 0, 0),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{A, B};

    fn board() -> Vec<[i16; 6]> {
        let mut b = vec![[0; 6]; 24];
        b[0][0] = 1; // baby 0 wakes at frame 20
        b[1][1] = 1; // baby 1 at frame 40
        b
    }

    fn run(g: &mut Lab00, frames: usize, pressed: u8) {
        let d = crate::evolve::tests::data();
        let mut s = Vec::new();
        for i in 0..frames {
            let p = if i == 0 { pressed } else { 0 };
            g.update(p, 0, &d, &mut s);
        }
    }

    #[test]
    fn feeding_a_peeking_baby_scores() {
        let mut g = Lab00::new(board());
        run(&mut g, 45, 0);
        assert_eq!(g.holes[0].0, 2, "awake 20 frames after waking");
        run(&mut g, 1, B);
        assert_eq!((g.score, g.holes[0].0), (1, 3));
    }

    #[test]
    fn feeding_a_sleeping_baby_ends_the_game() {
        let mut g = Lab00::new(board());
        run(&mut g, 45, 0);
        run(&mut g, 1, A); // cursor → hole 1 (just woke at 40, state 1)
        run(&mut g, 1, B);
        assert_eq!(g.score, 1);
        run(&mut g, 1, A); // hole 2, asleep
        run(&mut g, 1, B);
        assert!(g.end_wait.is_some());
        let d = crate::evolve::tests::data();
        let mut r = None;
        for _ in 0..60 {
            r = r.or(g.update(0, 0, &d, &mut Vec::new()));
        }
        assert_eq!(r, Some(1));
    }

    #[test]
    fn time_runs_out_after_500_frames() {
        let mut g = Lab00::new(vec![[0; 6]; 24]);
        run(&mut g, 499, 0);
        assert!(g.end_wait.is_none());
        run(&mut g, 1, 0);
        assert!(g.end_wait.is_some());
        assert_eq!(g.result(), 0);
    }
}
