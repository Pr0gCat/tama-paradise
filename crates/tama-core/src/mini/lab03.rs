//! Lab game 03: the bird runs along three lanes collecting fruit (09-minigames-lab-a §2.4).

use super::{atan2_16, cos16, sin16, Hud, MiniGame};
use crate::data::Data;
use crate::input::{A, B, DOWN, UP};
use crate::motion::{Draw, Look, Node};

/// Distance per layout row, goal flag spawn, finish line.
const ROW: i32 = 64;
const GOAL_SPAWN: i32 = 0x1300;
const GOAL: i32 = 0x1380;
/// Lane y (UI elements 0x4f7–0x4f9 / 0x4f1–0x4f3).
const LANE_Y: [i32; 3] = [42, 76, 110];
/// Bird anchor (element 0x4ee); its node offset is `lane_y − 44`.
const BIRD: (i32, i32) = (108, 42);
/// Bird hit box (T3 element 0x4f0): x = 96 − 8, w 8, h 32, y = lane_y − 20.
const BIRD_BOX: (i32, i32, i32) = (88, 8, 32);
const SPAWN_X: i32 = -16;
const DESPAWN_X: i32 = 144;
const GOAL_Y: i32 = 73;
const BIRD_RUN: u16 = 0xa64;
const BIRD_MAGNET: u16 = 0xa65;
const ITEM: u16 = 2191;
const FLAG: u16 = 2194;
/// Background sprites (XIP 0x10091cc4): slow, medium in/loop, fast in/loop.
const BG: [u16; 5] = [2183, 2184, 2185, 2186, 2187];
const MAGNET: i8 = 3;
/// Magnet lasts until its counter passes 60.
const MAGNET_FRAMES: u16 = 60;
const THRESHOLDS: [u16; 3] = [30, 16, 1];
const RESULTS: [u16; 4] = [0xa6f, 0xa6d, 0xa6b, 0xa66];
/// Run-off after the finish: bird x tween to −96 over 1 s.
const END_FRAMES: u16 = 20;

#[derive(Clone, Copy, Debug)]
struct Item {
    /// 16.16 position.
    x: i32,
    y: i32,
    kind: i8,
    /// Frames since a magnet started pulling it (`+0xdc` bit0), None = not pulled.
    pull: Option<u16>,
}

pub struct Lab03 {
    layout: Vec<[i8; 2]>,
    dist: i32,
    speed: i32,
    /// Background transition step (`+0x4a`) and the two tiles' sprites.
    step: u8,
    bg: [u16; 2],
    lane: usize,
    /// 0 off, 1 just picked up, 2 active (`+0x46`), and its counter.
    magnet: u8,
    magnet_t: u16,
    items: [Option<Item>; 6],
    goal: Option<i32>,
    count: u16,
    bird: Node,
    /// Bird node y offset: tween (from, to, t) over 4 frames.
    bird_y: (i32, i32, u16),
    /// Collected items flying off: (x, y, kind, frame).
    fx: Vec<(i32, i32, i8, u16)>,
    end: Option<u16>,
}

impl Lab03 {
    /// `layout` = layouts[level*2 + plays % 2].
    pub fn new(layout: Vec<[i8; 2]>) -> Self {
        let y = LANE_Y[1] - 44;
        let mut g = Lab03 {
            layout,
            dist: 0,
            speed: 0,
            step: 0,
            bg: [BG[0]; 2],
            lane: 1,
            magnet: 0,
            magnet_t: 0,
            items: [None; 6],
            goal: None,
            count: 0,
            bird: Node::new(BIRD_RUN),
            bird_y: (y, y, 4),
            fx: Vec::new(),
            end: None,
        };
        g.place_bird();
        g
    }

    fn bird_off_y(&self) -> i32 {
        let (from, to, t) = self.bird_y;
        if t >= 4 {
            to
        } else {
            from + (to - from) * t as i32 / 4
        }
    }

    fn place_bird(&mut self) {
        let run = self.end.map_or(0, |t| -96 * t as i32 / END_FRAMES as i32);
        self.bird.place(BIRD.0 + run, BIRD.1 + self.bird_off_y());
    }

    pub fn result(&self) -> u8 {
        THRESHOLDS.iter().position(|&t| self.count >= t).map_or(0, |i| 3 - i as u8)
    }

    /// `FUN_1008f854`: two 128-px tiles scrolling right; a speed-up swaps in the transition tile.
    fn background(&mut self) {
        let phase = self.dist % 128;
        let (base, trans, edge) = match self.speed {
            5 => (BG[2], BG[1], 123),
            7 => (BG[4], BG[3], 120),
            _ => {
                self.bg = [BG[0]; 2];
                return;
            }
        };
        match self.step {
            0 => self.bg = [base; 2],
            1 => self.bg[1] = trans,
            2 => self.bg = [trans, base],
            _ => {
                self.bg[0] = base;
                self.step = 0;
            }
        }
        if phase > edge && matches!(self.step, 1 | 2) {
            self.step += 1;
        }
    }

    /// `FUN_1007cee4`: A/dial up = lane up, B/dial down = lane down; magnet on/off swaps the bird.
    fn input(&mut self, pressed: u8, sounds: &mut Vec<(u16, u8)>) {
        let (a, b) = (pressed & A != 0, pressed & B != 0);
        let up = if a != b {
            a
        } else if pressed & UP != 0 {
            true
        } else if pressed & DOWN != 0 {
            false
        } else {
            return self.magnet_tick();
        };
        let mut moved = false;
        if up && self.lane > 0 {
            self.lane -= 1;
            moved = true;
        } else if !up && self.lane < 2 {
            self.lane += 1;
            moved = true;
        }
        if moved {
            sounds.push((1, 100));
            let y = LANE_Y[self.lane] - 44;
            self.bird_y = (self.bird_off_y(), y, 0);
        }
        self.magnet_tick();
    }

    fn magnet_tick(&mut self) {
        if self.magnet == 1 {
            (self.magnet, self.magnet_t) = (2, 0);
            self.swap_bird(BIRD_MAGNET);
        }
        if self.magnet == 2 {
            self.magnet_t += 1;
            if self.magnet_t - 1 > MAGNET_FRAMES {
                (self.magnet, self.magnet_t) = (0, 0);
                self.swap_bird(BIRD_RUN);
            }
        }
    }

    /// A new bird node snaps to its lane.
    fn swap_bird(&mut self, script: u16) {
        self.bird = Node::new(script);
        let y = LANE_Y[self.lane] - 44;
        self.bird_y = (y, y, 4);
        self.place_bird();
    }

    /// `FUN_1008f344`: move, pull (magnet), collect or recycle the 6 pool items.
    fn items(&mut self, data: &Data, sounds: &mut Vec<(u16, u8)>) {
        let (bx, bw, bh) = BIRD_BOX;
        let by = LANE_Y[self.lane] - 20;
        let bird_y = self.bird_off_y() << 16;
        for slot in 0..6 {
            let Some(mut it) = self.items[slot] else { continue };
            if self.magnet != 0 && it.kind != MAGNET && it.x > 0x9ffff && it.pull.is_none() {
                it.pull = Some(0);
            }
            let mut x = it.x + (self.speed << 16);
            let mut y = it.y;
            if let Some(t) = it.pull {
                // Step toward the bird node (x offset 0, y offset bird_y) by a radius that tweens
                // 0 → 10 px in 0.1 s (`+0x5c`), as the firmware computes it.
                let r = (t as i32 * 0x500).min(0xa00) << 8;
                let a = atan2_16(&data.mini, bird_y - it.y + 0x2c_0000, -it.x);
                x -= ((cos16(&data.mini, a) as i64 * r as i64) >> 16) as i32;
                y += ((sin16(&data.mini, a) as i64 * r as i64) >> 16) as i32;
                it.pull = Some(t + 1);
            }
            (it.x, it.y) = (x, y);
            self.items[slot] = None;
            if x >= DESPAWN_X << 16 {
                continue;
            }
            let (half, size) = if it.pull.is_some() { (16, 32) } else { (8, 16) };
            let (left, top) = ((x >> 16) - half, (y >> 16) - half);
            if left - bw < bx && bx < left + size && top - bh < by && by < top + size {
                if it.kind == MAGNET {
                    self.magnet = 1;
                    sounds.push((0x3b, 100));
                } else {
                    self.count += 1;
                    sounds.push((10, 100));
                }
                self.fx.push((x >> 16, y >> 16, it.kind, 0));
            } else {
                self.items[slot] = Some(it);
            }
        }
    }

    fn spawn(&mut self, lane: i8, kind: i8) {
        if let Some(slot) = self.items.iter_mut().find(|s| s.is_none()) {
            *slot = Some(Item { x: SPAWN_X << 16, y: LANE_Y[lane as usize] << 16, kind, pull: None });
        }
    }
}

impl MiniGame for Lab03 {
    fn update(&mut self, pressed: u8, _held: u8, data: &Data, sounds: &mut Vec<(u16, u8)>) -> Option<u8> {
        self.bird.tick(&data.motions);
        self.bird.take_sounds(sounds);
        self.bird_y.2 = self.bird_y.2.saturating_add(1);
        self.fx.retain_mut(|f| {
            f.3 += 1;
            f.3 < 4
        });
        if let Some(t) = self.end {
            self.dist += self.speed;
            self.background();
            self.goal = self.goal.map(|g| g + (self.speed << 16));
            self.end = Some(t + 1);
            self.place_bird();
            return (t + 1 >= END_FRAMES).then(|| self.result());
        }
        let old_speed = self.speed;
        self.speed = if self.dist < 1152 {
            3
        } else if self.dist < 2816 {
            5
        } else {
            7
        };
        if self.dist > 0 && old_speed != self.speed {
            self.step += 1;
            sounds.push((0x12, 100));
        }
        let old = self.dist;
        self.dist += self.speed;
        self.background();
        self.input(pressed, sounds);
        self.items(data, sounds);
        self.goal = self.goal.map(|g| g + (self.speed << 16));
        let row = self.dist / ROW;
        if old / ROW < row {
            if let Some(&[lane, kind]) = self.layout.get(row as usize) {
                if lane >= 0 {
                    self.spawn(lane, kind);
                }
            }
        }
        if self.goal.is_none() && self.dist >= GOAL_SPAWN {
            self.goal = Some(SPAWN_X << 16);
        }
        if self.dist >= GOAL {
            sounds.push((0x3d, 100));
            self.end = Some(0);
        }
        self.place_bird();
        None
    }

    fn draws(&self, out: &mut Vec<Draw>) {
        let sprite = |id, frame, x, y, alpha| Draw {
            look: Look::Sprite { id, frame },
            x,
            y,
            flip: false,
            alpha,
            palette: 0,
            scale: 256,
        };
        let phase = self.dist % 128;
        for k in 0..2 {
            out.push(sprite(self.bg[k], 0, 64 + phase - 128 * k as i32, 64, 64));
        }
        if let Some(g) = self.goal {
            out.push(sprite(FLAG, 0, g >> 16, GOAL_Y, 64));
        }
        self.bird.draws(0, 0, out);
        for it in self.items.iter().flatten() {
            out.push(sprite(ITEM, it.kind as u16, it.x >> 16, it.y >> 16, 64));
        }
        // Collected item: x tween +16 and fade out over 0.2 s.
        for &(x, y, kind, t) in &self.fx {
            out.push(sprite(ITEM, kind as u16, x + 4 * t as i32, y, (64 - 16 * t) as u8));
        }
    }

    fn hud(&self) -> Vec<Hud> {
        Vec::new()
    }

    fn result_script(&self, result: u8) -> u16 {
        RESULTS[result as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mini::tests::data;

    fn run(g: &mut Lab03, frames: usize, pressed: u8) -> Option<u8> {
        let d = data();
        let mut r = None;
        for i in 0..frames {
            r = r.or(g.update(if i == 0 { pressed } else { 0 }, 0, &d, &mut Vec::new()));
        }
        r
    }

    fn layout(rows: &[(usize, [i8; 2])]) -> Vec<[i8; 2]> {
        let mut l = vec![[-1, 0]; 76];
        for &(i, r) in rows {
            l[i] = r;
        }
        l
    }

    #[test]
    fn lanes_and_speed() {
        let mut g = Lab03::new(layout(&[]));
        run(&mut g, 1, A);
        assert_eq!(g.lane, 0);
        run(&mut g, 1, A);
        assert_eq!(g.lane, 0, "top lane is the limit");
        run(&mut g, 1, DOWN);
        run(&mut g, 1, B);
        assert_eq!(g.lane, 2);
        run(&mut g, 1, A | B);
        assert_eq!(g.lane, 2, "A+B together does nothing");
        assert_eq!(g.speed, 3);
        while g.dist < 1152 {
            run(&mut g, 1, 0);
        }
        run(&mut g, 1, 0);
        assert_eq!((g.speed, g.step), (5, 1));
    }

    #[test]
    fn collecting_in_the_same_lane_only() {
        // Row 1 spawns at distance 64 in lane 1 (the bird's), row 2 in lane 0.
        let mut g = Lab03::new(layout(&[(1, [1, 0]), (2, [0, 1])]));
        run(&mut g, 100, 0);
        assert_eq!(g.count, 1);
        assert!(g.items.iter().all(Option::is_none), "lane 0 fruit scrolled off");
    }

    #[test]
    fn magnet_pulls_other_lanes() {
        let mut g = Lab03::new(layout(&[(1, [1, 3]), (3, [0, 0]), (4, [2, 1])]));
        run(&mut g, 60, 0);
        assert_eq!(g.magnet, 2, "picked up the magnet");
        run(&mut g, 90, 0);
        assert_eq!(g.magnet, 0, "magnet over after ~61 frames");
        assert_eq!(g.count, 2, "fruit in lanes 0 and 2 pulled in");
    }

    #[test]
    fn finish_and_result() {
        let mut g = Lab03::new(layout(&[]));
        let frames = (0..).position(|_| {
            run(&mut g, 1, 0);
            g.end.is_some()
        });
        assert!(frames.unwrap() > 1000, "≈51 s run");
        assert!(g.goal.is_some());
        assert_eq!(run(&mut g, 20, 0), Some(0));
        g.count = 30;
        assert_eq!(g.result(), 3);
        g.count = 16;
        assert_eq!(g.result(), 2);
    }
}
