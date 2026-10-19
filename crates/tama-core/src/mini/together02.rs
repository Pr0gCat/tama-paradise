//! Play-together game 02 "3 UFOS" (scene 0x67, tick `FUN_10054364`; 09-minigames-together §2.7).
//! The pet hides under UFO 2; the UFOs swap along a fixed sequence, then the player picks one.
//! Three rounds, one point per round.

use super::together04::Tween;
use super::{push_sprite, Hud, MiniGame};
use crate::data::Data;
use crate::input::{A, B};
use crate::motion::{Draw, Node};

const INTRO: u16 = 0xab8;
/// The pet slips under a UFO / is found / shows where it was.
const HIDE: u16 = 0xabd;
const FOUND: u16 = 0xac1;
const MISSED: u16 = 0xac4;
const CURSOR: u16 = 0xac6;
/// Result scripts by score 0..3 (XIP 0x10095738, elem 0x54c).
const RESULTS: [u16; 4] = [0xaca, 0xac9, 0xac8, 0xac7];
const UFO: u16 = 0x89e;
/// Positions 0..2: UFOs (elems 0x543–0x545) at y 32, cursor (0x549–0x54b) at y 48, pet (0x546–0x548) at y 64.
const X: [i32; 3] = [22, 64, 106];
const UFO_Y: i32 = 32;
const CURSOR_Y: i32 = 48;
const PET_Y: i32 = 64;
/// UFO with the pet inside.
const PRIZE: u8 = 2;
/// Swap time by round (XIP 0x1009c598: 1.0 / 0.6 / 0.3 s as 16.16), and the arc height.
const MOVE: [i32; 3] = [0x10000, 0x9999, 0x4ccc];
const ARC: i32 = 20;
const AUTO_PICK: u16 = 200;
const STEP: i32 = 0xccc;
const PX: i32 = 0x10000;

pub struct Together02 {
    /// This play's sequence: `seq[step][pos]` = UFO id 1..3 standing at pos.
    seq: Vec<[u8; 3]>,
    /// Swaps per round.
    swaps: [u8; 3],
    state: u16,
    wait: u16,
    clock: i32,
    pet: Node,
    cursor: Option<Node>,
    /// UFO id k+1: position tweens and their positions this frame.
    ufo_x: [Tween; 3],
    ufo_y: [Tween; 3],
    at: [(i32, i32); 3],
    /// Next arc goes up (`ctx+0x38..0x3a`).
    up: [bool; 3],
    ufos: bool,
    step: usize,
    count: u8,
    round: usize,
    pick: usize,
    timer: u16,
    /// Frames since the reveal finished (`ctx+0x2c`) and whether its sound played.
    after: u16,
    told: bool,
    score: u8,
}

impl Together02 {
    /// `seq` = layouts[plays % 5], `swaps` = swaps[plays % 3].
    pub fn new(seq: Vec<[u8; 3]>, swaps: [u8; 3]) -> Self {
        let mut pet = Node::new(INTRO);
        pet.place(64, 64);
        Together02 {
            seq,
            swaps,
            state: 1000,
            wait: 0,
            clock: 0,
            pet,
            cursor: None,
            ufo_x: X.map(Tween::at),
            ufo_y: [Tween::at(UFO_Y); 3],
            at: X.map(|x| (x, UFO_Y)),
            up: [false; 3],
            ufos: false,
            step: 0,
            count: 0,
            round: 0,
            pick: 0,
            timer: 0,
            after: 0,
            told: false,
            score: 0,
        }
    }

    fn layout(&self, step: usize) -> [u8; 3] {
        self.seq.get(step).copied().unwrap_or([1, 2, 3])
    }

    /// Position of UFO `id` at `step`.
    fn pos(&self, step: usize, id: u8) -> usize {
        self.layout(step).iter().position(|&u| u == id).unwrap_or(0)
    }

    fn play(&mut self, script: u16, pos: usize) {
        self.pet = Node::new(script);
        self.pet.place(X[pos], PET_Y);
    }

    fn tween(&self, from: i32, to: i32, dur: i32) -> Tween {
        Tween { from, to: to * PX, start: self.clock, dur, curve: 1 }
    }

    fn ended(t: &[Tween; 3], clock: i32) -> bool {
        t.iter().all(|t| t.start + t.dur <= clock)
    }

    /// State 0x3eb: every UFO whose position changes slides there over an arc —
    /// up when moving right, down when moving left.
    fn swap(&mut self) {
        let dur = MOVE[self.round.min(2)];
        let prev = (self.step + 19) % 20;
        for k in 0..3 {
            let id = k as u8 + 1;
            let p = self.pos(self.step, id);
            match p {
                0 => self.up[k] = false,
                2 => self.up[k] = true,
                _ => match self.pos(prev, id) {
                    0 => self.up[k] = true,
                    2 => self.up[k] = false,
                    _ => {}
                },
            }
            if self.ufo_x[k].to != X[p] * PX {
                self.ufo_x[k] = self.tween(self.ufo_x[k].to, X[p], dur);
                let y = UFO_Y + if self.up[k] { -ARC } else { ARC };
                self.ufo_y[k] = self.tween(self.at[k].1 * PX, y, dur / 2);
            }
        }
    }

    fn set_cursor(&mut self) {
        let mut n = Node::new(CURSOR);
        n.place(X[self.pick], CURSOR_Y);
        self.cursor = Some(n);
    }

    fn state_tick(&mut self, pressed: u8, sounds: &mut Vec<(u16, u8)>) -> Option<u8> {
        match self.state {
            // ponytail: the opening is cut 30 frames in, as in the firmware; the fades are not drawn.
            1000 => (self.wait, self.state) = (30, 0x3e9),
            0x3e9 => {
                self.play(HIDE, 1);
                sounds.push((61, 100));
                self.ufos = true;
                self.state = 0x3ea;
            }
            0x3ea if self.pet.is_finished() => {
                (self.step, self.count) = (1, 1);
                self.state = 0x3eb;
            }
            0x3eb => {
                self.swap();
                self.state = 0x3ec;
            }
            0x3ec if Self::ended(&self.ufo_y, self.clock) => {
                let half = MOVE[self.round.min(2)] / 2;
                for k in 0..3 {
                    self.ufo_y[k] = self.tween(self.at[k].1 * PX, UFO_Y, half);
                }
                self.state = 0x3ed;
            }
            0x3ed if Self::ended(&self.ufo_x, self.clock) => {
                if self.count < self.swaps[self.round.min(2)] {
                    self.step = (self.step + 1) % 20;
                    self.count += 1;
                    self.state = 0x3eb;
                } else {
                    self.state = 2000;
                }
            }
            2000 => {
                self.set_cursor();
                self.state = 0x7d1;
            }
            0x7d1 => {
                self.timer += 1;
                if self.timer == AUTO_PICK {
                    sounds.push((1, 100));
                    self.state = 0x7d2;
                }
                if pressed & A != 0 {
                    self.timer = 0;
                    self.pick = (self.pick + 1) % 3;
                    self.set_cursor();
                    sounds.push((0, 100));
                } else if pressed & B != 0 {
                    sounds.push((1, 100));
                    self.state = 0x7d2;
                }
            }
            0x7d2 => {
                self.timer = 0;
                self.cursor = None;
                (self.wait, self.state) = (20, 0x7d3);
            }
            0x7d3 => self.state = 0x7d4,
            0x7d4 => {
                if self.layout(self.step)[self.pick] == PRIZE {
                    self.play(FOUND, self.pick);
                    self.score += 1;
                } else {
                    self.play(MISSED, self.pos(self.step, PRIZE));
                }
                sounds.push((61, 100));
                self.state = 0x7d5;
            }
            0x7d5 if self.pet.is_finished() => {
                let hit = self.layout(self.step)[self.pick] == PRIZE;
                if !self.told && hit && self.after > 24 {
                    sounds.push((52, 100));
                    self.told = true;
                } else if !self.told && !hit {
                    sounds.push((13, 100));
                    self.told = true;
                }
                self.after += 1;
                if self.told {
                    self.wait = 30;
                    self.state = if self.round < 2 { 3000 } else { 4000 };
                }
            }
            3000 => {
                self.play(HIDE, self.pos(self.step, PRIZE));
                sounds.push((61, 100));
                self.state = 3001;
            }
            // The step is kept, so the next round's first swap stands still.
            3001 if self.pet.is_finished() => {
                (self.pick, self.count, self.after, self.told) = (0, 0, 0, false);
                self.round += 1;
                (self.wait, self.state) = (10, 0x3eb);
            }
            4000 => (self.wait, self.state) = (20, 4001),
            4001 => return Some(self.score),
            _ => {}
        }
        None
    }
}

impl MiniGame for Together02 {
    fn update(&mut self, pressed: u8, _held: u8, data: &Data, sounds: &mut Vec<(u16, u8)>) -> Option<u8> {
        let r = if self.wait > 0 {
            self.wait -= 1;
            None
        } else {
            self.state_tick(pressed, sounds)
        };
        self.clock += STEP;
        for k in 0..3 {
            self.at[k] =
                (self.ufo_x[k].value(&data.mini, self.clock) >> 16, self.ufo_y[k].value(&data.mini, self.clock) >> 16);
        }
        self.pet.tick(&data.motions);
        self.pet.take_sounds(sounds);
        if let Some(c) = &mut self.cursor {
            c.tick(&data.motions);
        }
        r
    }

    fn draws(&self, out: &mut Vec<Draw>) {
        self.pet.draws(0, 0, out);
        if self.ufos {
            for &(x, y) in &self.at {
                push_sprite(out, UFO, 0, x, y, 64);
            }
        }
        if let Some(c) = &self.cursor {
            c.draws(0, 0, out);
        }
    }

    fn hud(&self) -> Vec<Hud> {
        Vec::new()
    }

    fn result_script(&self, result: u8) -> u16 {
        RESULTS[result as usize]
    }

    fn field_bg(&self) -> bool {
        true
    }

    fn result_at(&self) -> (i32, i32) {
        (64, 88)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sequence rotating right every step.
    fn seq() -> Vec<[u8; 3]> {
        let rots = [[1, 2, 3], [3, 1, 2], [2, 3, 1]];
        (0..20).map(|i| rots[i % 3]).collect()
    }

    fn step(g: &mut Together02, pressed: u8) -> Option<u8> {
        g.update(pressed, 0, &crate::mini::tests::data(), &mut Vec::new())
    }

    fn to_pick(g: &mut Together02) {
        for _ in 0..2000 {
            if g.state == 0x7d1 {
                return;
            }
            step(g, 0);
        }
        panic!("no pick");
    }

    /// Move the cursor to `pos` and confirm, then run to the next pick or the end.
    fn pick(g: &mut Together02, pos: usize) -> Option<u8> {
        to_pick(g);
        while g.pick != pos {
            step(g, A);
        }
        step(g, B);
        (0..500).find_map(|_| if g.state == 0x7d1 { Some(None) } else { step(g, 0).map(Some) }).flatten()
    }

    #[test]
    fn swaps_follow_the_sequence() {
        let mut g = Together02::new(seq(), [4, 2, 3]);
        to_pick(&mut g);
        // Round 0: steps 1..=4, UFO 2 ends where step 4 puts it.
        assert_eq!((g.step, g.count), (4, 4));
        assert_eq!(g.at, [(64, 32), (106, 32), (22, 32)], "settled at step 4 = [3,1,2]");
        // Finding it scores; round 1 starts on the same step (a still swap) and does 2 more.
        assert_eq!(pick(&mut g, 2), None);
        assert_eq!((g.score, g.round, g.step), (1, 1, 6));
        assert_eq!(pick(&mut g, 0), None, "wrong UFO");
        assert_eq!(g.score, 1);
        assert_eq!(pick(&mut g, 1), Some(2), "round 2 ends on step 9 = [1,2,3]");
    }

    #[test]
    fn arcs_go_up_moving_right() {
        let mut g = Together02::new(seq(), [1, 1, 1]);
        g.ufo_x = X.map(Tween::at);
        g.step = 1;
        g.swap();
        // [1,2,3] -> [3,1,2]: UFO 1 and 2 move right (up), UFO 3 moves left (down).
        assert_eq!(g.up, [true, true, false]);
        assert_eq!(g.ufo_y.map(|t| t.to >> 16), [12, 12, 52]);
    }

    #[test]
    fn waiting_picks_the_cursor() {
        let mut g = Together02::new(seq(), [1, 1, 1]);
        to_pick(&mut g);
        step(&mut g, A);
        for _ in 0..AUTO_PICK {
            step(&mut g, 0);
        }
        assert_eq!((g.state, g.pick), (0x7d2, 1));
    }
}
