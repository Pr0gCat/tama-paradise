//! Play-together game 03 "WALKING" (scene 0x68, tick `FUN_10055f30`; 09-minigames-together §2.8).
//! The pet walks on its own; the dial turns the whole maze around it. Reach a G within 60 s;
//! moving obstacles stop the pet for 2 s. World positions are 16.16 pixels, 48 px cells, and the
//! pet's world point is always drawn at (64,64).

use super::{cos16, sin16, sprite, Hud, MiniGame};
use crate::data::Data;
use crate::input::{DOWN, UP};
use crate::motion::{Draw, Node};

const ONE: i32 = 0x10000;
/// Scene time per frame (16.16 seconds).
const FRAME: i32 = 0xccc;
const CELL: i32 = 48;
const SPEED: i32 = 2 * ONE;
const ACCEL: i32 = 0x3333;
/// Stopped by an obstacle, then blinking (invincible): 2 s each.
const STUN: i32 = 2 * ONE;
const BLINK: i32 = ONE / 2;
/// Obstacle hit radius² (pixels) and its swing: triangle ±48 px over 3 s.
const HIT_R2: i32 = 1024;
const SWING: i32 = 48;
const SWING_T: i32 = 3 * ONE;
const LIMIT: i32 = 60 * ONE;
const COUNTDOWN: i32 = 51 * ONE;
/// The opening runs 40 frames (`DAT_1800efc8`), the camera pan 3 s after 0.5 s.
const OPENING_FRAMES: u16 = 40;
const PAN: i32 = 7 * ONE / 2;
/// Field looks (XIP 0x1009472c): opening, ground, obstacle, wall and goal sprites.
const LOOKS: [[u16; 5]; 3] =
    [[0xacb, 0x8a3, 0x8a9, 0x8a6, 0x8ac], [0xacc, 0x8a4, 0x8aa, 0x8a7, 0x8ad], [0xace, 0x8af, 0x8b1, 0x8b2, 0x8b0]];
const START: u16 = 0xad8;
const WALK: u16 = 0xad9;
const BUMPED: u16 = 0xada;
const HAPPY: u16 = 0xadb;
const SAD: u16 = 0xadc;
/// Result scripts by tier 0..3 (0xae0 = time out; elems 0x55f–0x562).
const RESULTS: [u16; 4] = [0xae0, 0xadf, 0xade, 0xadd];
/// Opening / result actor (elem 0x550) and the walking pet (elem 0x555).
const ACTOR: (i32, i32) = (64, 88);
const PET: (i32, i32) = (64, 80);

pub struct Together03 {
    map: Vec<Vec<u8>>,
    look: [u16; 5],
    state: u16,
    wait: u16,
    /// The game's own screen fade.
    fx: Option<super::Fade>,
    frames: u16,
    time: i32,
    built_at: i32,
    x: i32,
    y: i32,
    /// Dial position 0..23 (reset when the maze is built).
    // ponytail: the dial's absolute position is rebuilt from UP/DOWN pulses, one pulse = one 15° step.
    dial: i32,
    /// Pet heading, 256 = one turn (node rotation `(0x8000 − dial_angle)·256 >> 16`).
    r: i32,
    /// Maze rotation (16.16 cos / sin of `dial_angle >> 8`).
    rot: (i32, i32),
    v: i32,
    /// Pet node callback attached (removed at the goal / on time out).
    moving: bool,
    /// Camera pan running: no walking (`ctx+0x56`).
    frozen: bool,
    bumped: bool,
    stunned: bool,
    stun_t0: i32,
    blink: Option<i32>,
    hits: u16,
    /// Obstacles: cell centre (world pixels) and whether it swings horizontally.
    obstacles: Vec<(i32, i32, bool)>,
    /// Ground scroll, 16.16 pixels.
    ground: (i32, i32),
    pet: Node,
    timer: Option<i32>,
    /// Goal reached or time out (`ctx+0x57`): the clock stops.
    over: bool,
    countdown: Option<u32>,
    built: bool,
}

impl Together03 {
    /// `map` = maps[plays % 3] (14 rows × 12), `field` = current field type 0..2.
    pub fn new(map: Vec<String>, field: u8) -> Self {
        let look = LOOKS[(field as usize).min(2)];
        let mut pet = Node::new(look[0]);
        pet.place(ACTOR.0, ACTOR.1);
        Together03 {
            map: map.into_iter().map(String::into_bytes).collect(),
            look,
            state: 1000,
            wait: 0,
            fx: None,
            frames: 0,
            time: 0,
            built_at: 0,
            x: 0,
            y: 0,
            dial: 0,
            r: 128,
            rot: (ONE, 0),
            v: 0,
            moving: false,
            frozen: false,
            bumped: false,
            stunned: false,
            stun_t0: 0,
            blink: None,
            hits: 0,
            obstacles: Vec::new(),
            ground: (0, 0),
            pet,
            timer: None,
            over: false,
            countdown: None,
            built: false,
        }
    }

    fn cell(&self, cx: i32, cy: i32) -> u8 {
        if cx < 0 || cy < 0 {
            return b'w';
        }
        self.map.get(cy as usize).and_then(|r| r.get(cx as usize)).copied().unwrap_or(b'w')
    }

    fn set_pet(&mut self, script: u16) {
        self.pet = Node::new(script);
        self.pet.place(PET.0, PET.1);
    }

    /// Rotate a 16.16 world offset into the screen (the maze turns about the pet).
    // [guess] rotation sense chosen so the pet always walks down the screen (spec §5 q4).
    fn turn(&self, dx: i32, dy: i32) -> (i32, i32) {
        let (c, s) = (self.rot.0 as i64, self.rot.1 as i64);
        let (dx, dy) = (dx as i64, dy as i64);
        (((dx * c - dy * s) >> 16) as i32, ((dx * s + dy * c) >> 16) as i32)
    }

    /// Screen position of a world point (pixels).
    fn screen(&self, wx: i32, wy: i32) -> (i32, i32) {
        let (x, y) = self.turn((wx << 16) - self.x, (wy << 16) - self.y);
        (64 + (x >> 16), 64 + (y >> 16))
    }

    /// Obstacle position now (world pixels): a triangle wave 0 → +48 → 0 → −48.
    fn obstacle_at(&self, (x, y, h): (i32, i32, bool)) -> (i32, i32) {
        let f = (4 * SWING as i64 * (self.time - self.built_at).rem_euclid(SWING_T) as i64 / SWING_T as i64) as i32;
        let d = if f < SWING {
            f
        } else if f < 3 * SWING {
            2 * SWING - f
        } else {
            f - 4 * SWING
        };
        if h {
            (x + d, y)
        } else {
            (x, y + d)
        }
    }

    /// State 2000: obstacles, the pet on 'S', the dial at 0, the camera pan starts.
    fn build(&mut self) {
        for cy in 0..self.map.len() as i32 {
            for cx in 0..12 {
                let (wx, wy) = (cx * CELL + CELL / 2, cy * CELL + CELL / 2);
                match self.cell(cx, cy) {
                    b'S' => (self.x, self.y) = (wx << 16, wy << 16),
                    c @ (b'H' | b'V') => self.obstacles.push((wx, wy, c == b'H')),
                    _ => {}
                }
            }
        }
        self.set_pet(START);
        self.dial = 0;
        self.r = 128;
        self.rot = (ONE, 0);
        // ponytail: the camera pan itself is not drawn, only its 3.5 s hold.
        self.frozen = true;
        self.moving = true;
        self.built_at = self.time;
        self.built = true;
    }

    /// `osewagame03_pet_move_cb` 0x1008121c: stun / speed, then walk, stopped per axis by walls.
    fn walk(&mut self, data: &Data, sounds: &mut Vec<(u16, u8)>) {
        if self.bumped {
            self.v = 0;
            if !self.stunned {
                self.stunned = true;
                self.set_pet(BUMPED);
                sounds.push((16, 100));
                self.stun_t0 = self.time;
            } else if self.time - self.stun_t0 > STUN {
                self.bumped = false;
                self.set_pet(WALK);
                self.blink = Some(self.time);
                self.stun_t0 = self.time;
            }
        } else if self.stunned {
            self.v = (self.v + ACCEL).min(SPEED);
            if self.time - self.stun_t0 > STUN {
                self.stunned = false;
                self.blink = None;
            }
        } else if self.frozen {
            self.v = 0;
        } else {
            self.v = (self.v + ACCEL).min(SPEED);
        }
        let a = ((self.r << 8) as u32) & 0xffff;
        let mut dx = ((self.v as i64 * sin16(&data.mini, a) as i64) >> 16) as i32;
        let mut dy = -((self.v as i64 * cos16(&data.mini, a) as i64) >> 16) as i32;
        let (cx, cy) = ((self.x >> 16) / CELL, (self.y >> 16) / CELL);
        let (ix, iy) = (self.x % (CELL << 16), self.y % (CELL << 16));
        if dx < 0 && self.cell(cx - 1, cy) == b'w' && ix < 10 * ONE
            || dx > 0 && self.cell(cx + 1, cy) == b'w' && ix > 38 * ONE
        {
            dx = 0;
        }
        if dy < 0 && self.cell(cx, cy - 1) == b'w' && iy < 10 * ONE
            || dy > 0 && self.cell(cx, cy + 1) == b'w' && iy > 38 * ONE
        {
            dy = 0;
        }
        self.x += dx;
        self.y += dy;
        // `osewagame03_ground_scroll_cb` 0x1006bfc0: the ground slides against the on-screen move.
        let (sx, sy) = self.turn(dx, dy);
        self.ground = (self.ground.0 - sx, self.ground.1 - sy);
    }

    /// Obstacle callback: a touch while not stunned or blinking stops the pet.
    fn collide(&mut self) {
        if self.bumped || self.stunned {
            return;
        }
        for &o in &self.obstacles {
            let (ox, oy) = self.obstacle_at(o);
            let (dx, dy) = (ox - (self.x >> 16), oy - (self.y >> 16));
            if dx * dx + dy * dy < HIT_R2 {
                self.hits += 1;
                self.bumped = true;
                return;
            }
        }
    }

    /// Goal cells shown in the 5×5 tiles around the pet (`osewagame03_tile_cb` 0x1007fe08).
    fn tiles(&self) -> impl Iterator<Item = (u8, (i32, i32))> + '_ {
        let (cx, cy) = ((self.x >> 16) / CELL, (self.y >> 16) / CELL);
        (-2..=2).flat_map(move |j| {
            (-2..=2).map(move |i| {
                let (tx, ty) = (cx + i, cy + j);
                (self.cell(tx, ty), self.screen(tx * CELL + CELL / 2, ty * CELL + CELL / 2))
            })
        })
    }

    fn at_goal(&self) -> bool {
        self.tiles().any(|(c, (x, y))| c == b'G' && 48 < x && x < 96 && 52 < y && y < 100)
    }

    /// `osewagame03_timer_cb` 0x1008e21c.
    fn clock(&mut self, sounds: &mut Vec<(u16, u8)>) {
        let Some(start) = self.timer else { return };
        let t = if self.over { 0 } else { self.time - start };
        if t > COUNTDOWN {
            self.countdown = Some((60 - (t >> 16)).clamp(0, 9) as u32);
        }
        if t > LIMIT {
            self.moving = false;
            self.timer = None;
            self.over = true;
            sounds.push((74, 100));
            self.state = 0x1004;
        }
    }

    /// Result tier by obstacles hit (XIP 0x100a0c68: +10 / +5 / +1).
    pub fn tier(&self) -> u8 {
        3 - self.hits.min(2) as u8
    }
}

impl MiniGame for Together03 {
    fn update(&mut self, pressed: u8, _held: u8, data: &Data, sounds: &mut Vec<(u16, u8)>) -> Option<u8> {
        let turn = (pressed & UP != 0) as i32 - (pressed & DOWN != 0) as i32;
        self.dial = (self.dial + turn).rem_euclid(24);
        if let Some(f) = &mut self.fx {
            f.step();
            if f.done() && !f.out {
                self.fx = None;
            }
        }
        if self.wait > 0 {
            self.wait -= 1;
        } else {
            match self.state {
                // The opening fades out to white, and the maze fades in from it.
                1000 => {
                    self.frames += 1;
                    if self.frames > OPENING_FRAMES {
                        (self.wait, self.state) = (10, 2000);
                        self.fx = Some(super::Fade::out(true));
                    }
                }
                2000 => {
                    self.fx = Some(super::Fade::into(true));
                    self.build();
                    self.state = 0x7d1;
                }
                0x7d1 if self.time - self.built_at >= PAN => {
                    self.frozen = false;
                    self.v = SPEED;
                    self.state = 0xbb8;
                }
                0xbb8 => {
                    self.set_pet(WALK);
                    (self.wait, self.state) = (5, 0xbb9);
                }
                0xbb9 | 0xbba => {
                    if self.state == 0xbb9 {
                        self.timer = Some(self.time);
                        self.state = 0xbba;
                    }
                    if turn != 0 {
                        let d = self.dial * 0x10000 / 24;
                        let w = ((d >> 8) << 8) as u32;
                        self.rot = (cos16(&data.mini, w), sin16(&data.mini, w));
                        self.r = ((0x8000 - d) * 0x100) >> 16;
                    }
                }
                4000 => {
                    self.set_pet(HAPPY);
                    self.state = 4001;
                }
                4001 if self.pet.is_finished() => return Some(self.tier()),
                0x1004 => {
                    self.set_pet(SAD);
                    self.state = 0x1005;
                }
                0x1005 if self.pet.is_finished() => return Some(0),
                _ => {}
            }
        }
        if self.built && self.moving {
            self.walk(data, sounds);
            self.collide();
            if self.at_goal() {
                self.moving = false;
                self.over = true;
                sounds.push((15, 100));
                self.state = 4000;
            }
        }
        self.clock(sounds);
        self.pet.tick(&data.motions);
        self.pet.take_sounds(sounds);
        self.time += FRAME;
        None
    }

    fn draws(&self, out: &mut Vec<Draw>) {
        if !self.built {
            self.pet.draws(0, 0, out);
            return;
        }
        let (gx, gy) = ((self.ground.0 >> 16).rem_euclid(128), (self.ground.1 >> 16).rem_euclid(128));
        for (i, j) in [(0, 0), (-128, 0), (0, -128), (-128, -128)] {
            out.push(sprite(self.look[1], 0, 64 + gx + i, 64 + gy + j));
        }
        // Maze sprites turn with the maze (256 = a full turn, clockwise like `turn`).
        let rot = ((self.rot.1 as f64).atan2(self.rot.0 as f64) / std::f64::consts::TAU * 256.0).round() as i16;
        let turned = |id: u16, x: i32, y: i32| Draw { rot, ..sprite(id, 0, x, y) };
        let mut items: Vec<(i32, Vec<Draw>)> = Vec::new();
        for (c, (x, y)) in self.tiles() {
            match c {
                b'G' => out.push(turned(self.look[4], x, y)),
                b'w' => items.push((y, vec![turned(self.look[3], x, y)])),
                _ => {}
            }
        }
        for &o in &self.obstacles {
            let (ox, oy) = self.obstacle_at(o);
            let (x, y) = self.screen(ox, oy);
            if (-64..192).contains(&x) && (-64..192).contains(&y) {
                items.push((y, vec![turned(self.look[2], x, y)]));
            }
        }
        let hidden = self.blink.is_some_and(|t0| (self.time - t0) % BLINK >= BLINK / 2);
        if !hidden {
            let mut d = Vec::new();
            self.pet.draws(0, 0, &mut d);
            items.push((PET.1, d));
        }
        items.sort_by_key(|i| i.0);
        out.extend(items.into_iter().flat_map(|i| i.1));
    }

    fn hud(&self) -> Vec<Hud> {
        match self.countdown {
            Some(n) => vec![Hud::Sprite { id: 0x80f, frame: 0, x: 120, y: 11 }, Hud::Number { n, x: 120, y: 11 }],
            None => Vec::new(),
        }
    }

    fn screen_fade(&self) -> Option<super::Fade> {
        self.fx
    }

    fn result_fade(&self) -> Option<bool> {
        Some(true)
    }

    fn result_script(&self, result: u8) -> u16 {
        RESULTS[result as usize]
    }

    fn field_bg(&self) -> bool {
        true
    }

    fn result_at(&self) -> (i32, i32) {
        ACTOR
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An open 10×12 room inside the border wall, with `objs` (x, y, char).
    fn maze(objs: &[(usize, usize, u8)]) -> Vec<String> {
        let mut rows = vec![vec![b'w'; 12]; 14];
        for row in &mut rows[1..13] {
            row[1..11].fill(b' ');
        }
        for &(x, y, c) in objs {
            rows[y][x] = c;
        }
        rows.into_iter().map(|r| String::from_utf8(r).unwrap()).collect()
    }

    fn run(g: &mut Together03, frames: usize, pressed: u8) -> Option<u8> {
        let d = crate::mini::tests::data();
        let mut r = None;
        for _ in 0..frames {
            r = r.or(g.update(pressed, 0, &d, &mut Vec::new()));
        }
        r
    }

    /// Past the opening and the camera hold: walking.
    fn walking(objs: &[(usize, usize, u8)]) -> Together03 {
        let mut g = Together03::new(maze(objs), 0);
        let mut n = 0;
        while g.state != 0xbba {
            run(&mut g, 1, 0);
            n += 1;
            assert!(n < 200);
        }
        assert_eq!(g.v, SPEED);
        g
    }

    #[test]
    fn walls_stop_and_the_dial_turns_the_maze() {
        let mut g = walking(&[(5, 2, b'S')]);
        let y = g.y;
        run(&mut g, 1, 0);
        assert_eq!(g.y - y, SPEED, "starts walking down");
        run(&mut g, 300, 0);
        assert!((12 * CELL + 38..=12 * CELL + 40).contains(&(g.y >> 16)), "stopped by the bottom wall: {}", g.y >> 16);
        // A quarter turn: the pet now walks along x in the maze, but still down the screen.
        for _ in 0..6 {
            run(&mut g, 1, UP);
        }
        assert_eq!(g.r, 64);
        let (x, y) = (g.x, g.y);
        let before = g.screen(0, 0);
        run(&mut g, 1, 0);
        assert_eq!((g.x - x, g.y - y), (SPEED, 0));
        let after = g.screen(0, 0);
        assert_eq!((after.0 - before.0, after.1 - before.1), (0, -2), "the maze slides up the screen");
    }

    #[test]
    fn an_obstacle_stops_the_pet_once() {
        let mut g = walking(&[(5, 1, b'S'), (5, 4, b'V')]);
        let mut n = 0;
        while g.hits == 0 {
            run(&mut g, 1, 0);
            n += 1;
            assert!(n < 100);
        }
        run(&mut g, 1, 0);
        assert_eq!((g.v, g.pet.script(), g.stunned), (0, BUMPED, true));
        run(&mut g, 45, 0);
        assert!(!g.bumped && g.stunned && g.blink.is_some() && g.v > 0, "blinking, walking again");
        run(&mut g, 30, 0);
        assert_eq!((g.hits, g.stunned), (1, true), "invincible while blinking");
        run(&mut g, 10, 0);
        assert!(g.blink.is_none(), "blinking ends after 2 s");
    }

    #[test]
    fn reaching_a_goal_ends_the_game() {
        let mut g = walking(&[(5, 2, b'S'), (5, 4, b'G')]);
        let r = run(&mut g, 100, 0);
        assert_eq!((r, g.moving), (Some(3), false));
        assert!((180..190).contains(&(g.y >> 16)));
        for (hits, t) in [(1, 2), (2, 1), (7, 1)] {
            g.hits = hits;
            assert_eq!(g.tier(), t);
        }
    }

    #[test]
    fn time_out() {
        let mut g = walking(&[(5, 2, b'S')]);
        let r = run(&mut g, 60 * 20 + 10, 0);
        assert_eq!((r, g.countdown), (Some(0), Some(0)));
    }
}
