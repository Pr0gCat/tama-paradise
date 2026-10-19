//! Play-together game 07: SHIROTAMAS (09-minigames-together §2.5). The pet walks on its own at
//! 2 px/frame, the dial sets its heading; touch 5 shirotamas (T) within 60 s, avoid the 2 G.
//! World positions are 16.16 pixels; the pet's world point is always drawn at (64,64).

use super::{cos16, sin16, sprite, Hud, MiniGame};
use crate::data::Data;
use crate::input::{DOWN, UP};
use crate::motion::{Draw, Look, Node};

const ONE: i32 = 0x10000;
/// Scene time per frame (16.16 seconds).
const FRAME: i32 = 0xccc;
const CELL: i32 = 32;
const SPEED: i32 = 2 * ONE;
/// Hit radius² around the point 20 px below the pet's world point (`osewagame07_object_cb`).
const HIT_R2: i32 = 676;
/// Stunned by a G / cheering a catch: 2 s.
const STUN: i32 = 2 * ONE;
const LIMIT: i32 = 60 * ONE;
const COUNTDOWN: i32 = 51 * ONE;
/// Field looks (XIP 0x1009474a; G via elems 0x595–0x597): ground, G and wall sprites.
const LOOKS: [(u16, u16, u16); 3] = [(0x8b7, 0x8bb, 0x8be), (0x8b8, 0x8bc, 0x8bf), (0x8b9, 0x8bd, 0x8c0)];
const OPENING: u16 = 0xaeb;
const STAND: u16 = 0xaef;
const WALK_DOWN: u16 = 0xaf0;
const WALK_UP: u16 = 0xaf1;
const WALK_LEFT: u16 = 0xaf2;
const WALK_RIGHT: u16 = 0xaf3;
const BUMPED: u16 = 0xaf4;
const GOT: u16 = 0xaf5;
const HAPPY: u16 = 0xaf6;
const SAD: u16 = 0xaf7;
const SHIROTAMA: u16 = 0xaf8;
const SPARKLE: u16 = 0xaf9;
const FOLLOW_DOWN: u16 = 0xafa;
const FOLLOW_UP: u16 = 0xafb;
const FOLLOW_LEFT: u16 = 0xafc;
const FOLLOW_RIGHT: u16 = 0xafd;
const CHEER: u16 = 0xafe;
const SULK: u16 = 0xaff;
/// Result scripts by tier 0..3 (0xb03 = failed).
const RESULTS: [u16; 4] = [0xb03, 0xb02, 0xb01, 0xb00];
/// Opening / result actor (elems 0x57c, 0x59a–0x59d) and the walking pet (elem 0x584).
const ACTOR: (i32, i32) = (64, 88);
const PET: (i32, i32) = (64, 80);

/// (x, y) pixels and heading (256 = one turn), newest first.
type Step = (i32, i32, i32);

fn node(script: u16) -> Node {
    Node::new(script)
}

/// `osewagame07_pick_walk_anim` 0x10006fe4 (float thresholds, asymmetric as in the firmware).
fn walk_anim(r: i32) -> u16 {
    let f = r as f32 / 256.0;
    if (-0.211..=0.2).contains(&f) {
        WALK_UP
    } else if f > 0.2 && f <= 0.414 {
        WALK_LEFT
    } else if f > 0.414 || f <= -0.417 {
        WALK_DOWN
    } else {
        WALK_RIGHT
    }
}

/// `osewagame07_follower_anims` 0x10062258.
fn follow_anim(r: i32) -> u16 {
    let f = r as f32 / 256.0;
    if (-0.125..=0.125).contains(&f) {
        FOLLOW_UP
    } else if f > 0.125 && f <= 0.375 {
        FOLLOW_LEFT
    } else if (-0.375..-0.125).contains(&f) {
        FOLLOW_RIGHT
    } else {
        FOLLOW_DOWN
    }
}

struct Obj {
    shirotama: bool,
    /// Cell centre, world pixels.
    x: i32,
    y: i32,
    node: Node,
    hit: bool,
    /// G fading out after a bump (frames), T collected.
    fade: Option<u8>,
    gone: bool,
}

struct Follower {
    hist: [Step; 16],
    pos: (i32, i32),
    visible: bool,
    node: Node,
    anim: u16,
}

/// The sparkle flying from a caught T to its follower (`osewagame07_capture_fx_cb` 0x1000b788), screen pixels.
struct Fx {
    at: (i32, i32),
    to: Option<(i32, i32)>,
    t0: i32,
    node: Node,
}

pub struct Together07 {
    map: Vec<Vec<u8>>,
    look: (u16, u16, u16),
    /// `osewagame07_tick` 0x1005a9b4 state and its wait.
    state: u16,
    wait: u16,
    time: i32,
    t0: i32,
    x: i32,
    y: i32,
    /// Heading, 256 = one turn: 0 = up, 128 = down (node rotation).
    r: i32,
    /// Dial position 0..23 (reset when the field is built).
    // ponytail: the dial's absolute position is rebuilt from UP/DOWN pulses, one pulse = one 15° step.
    dial: i32,
    v: i32,
    /// Pet node callback attached (removed on time out).
    moving: bool,
    /// Frames to the next walking sound.
    step: u16,
    hist: [Step; 16],
    objs: Vec<Obj>,
    followers: Vec<Follower>,
    fx: Option<Fx>,
    /// The 5th shirotama (it gets no follower).
    fifth: Option<usize>,
    pet: Node,
    /// Walk animation latched by `walk_anim` (0 = none).
    walk: u16,
    bumped: bool,
    collecting: bool,
    stunned: bool,
    stun_t0: i32,
    hits: u16,
    got: u16,
    timer: Option<i32>,
    /// Timer checks skipped while a catch is shown (the clock itself keeps running).
    pause: bool,
    timeout: bool,
    countdown: Option<u32>,
    built: bool,
}

impl Together07 {
    /// `map` = maps[plays % 3] (19 rows × 18), `field` = current field type 0..2.
    pub fn new(map: Vec<String>, field: u8) -> Self {
        let mut pet = node(OPENING);
        pet.place(ACTOR.0, ACTOR.1);
        Together07 {
            map: map.into_iter().map(String::into_bytes).collect(),
            look: LOOKS[(field as usize).min(2)],
            state: 0,
            wait: 0,
            time: 0,
            t0: 0,
            x: 0,
            y: 0,
            r: 128,
            dial: 0,
            v: 0,
            moving: false,
            step: 0,
            hist: [(0, 0, 128); 16],
            objs: Vec::new(),
            followers: Vec::new(),
            fx: None,
            fifth: None,
            pet,
            walk: 0,
            bumped: false,
            collecting: false,
            stunned: false,
            stun_t0: 0,
            hits: 0,
            got: 0,
            timer: None,
            pause: false,
            timeout: false,
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
        self.pet = node(script);
        self.pet.place(PET.0, PET.1);
    }

    /// Screen position of a world point.
    fn screen(&self, wx: i32, wy: i32) -> (i32, i32) {
        (64 + wx - (self.x >> 16), 64 + wy - (self.y >> 16))
    }

    /// State 2000: objects, the pet on 'S' facing down, the dial at 0.
    fn build(&mut self) {
        for cy in 0..self.map.len() as i32 {
            for cx in 0..18 {
                let (wx, wy) = (cx * CELL + 16, cy * CELL + 16);
                match self.cell(cx, cy) {
                    b'S' => (self.x, self.y) = (wx << 16, wy << 16),
                    c @ (b'T' | b'G') => {
                        let t = c == b'T';
                        let n = node(if t { SHIROTAMA } else { 0xffff });
                        self.objs.push(Obj {
                            shirotama: t,
                            x: wx,
                            y: wy,
                            node: n,
                            hit: false,
                            fade: None,
                            gone: false,
                        });
                    }
                    _ => {}
                }
            }
        }
        // ponytail: the firmware's trail starts zeroed; start it on the pet so nothing flashes at (0,0).
        self.hist = [(self.x >> 16, self.y >> 16, self.r); 16];
        self.set_pet(STAND);
        self.dial = 0;
        self.moving = true;
        self.built = true;
    }

    fn pick_walk(&mut self) {
        let a = walk_anim(self.r);
        if self.walk != a {
            self.walk = a;
            self.set_pet(a);
        }
    }

    /// `osewagame07_stun_update` 0x10007330.
    fn stun_update(&mut self, sounds: &mut Vec<(u16, u8)>) {
        if !(self.bumped || self.collecting) {
            return;
        }
        self.v = 0;
        self.step = 19;
        if !self.stunned {
            self.stunned = true;
            self.set_pet(if self.bumped { BUMPED } else { GOT });
            sounds.push((if self.bumped { 16 } else { 52 }, 100));
            self.stun_t0 = self.time;
        } else if self.time - self.stun_t0 > STUN {
            if self.bumped {
                self.bumped = false
            } else {
                self.collecting = false
            }
            self.v = SPEED;
            self.stunned = false;
            self.pick_walk();
            self.stun_t0 = self.time;
        }
    }

    /// `osewagame07_pet_move_cb` 0x10081768: walk, stopped per axis by walls, and record the trail.
    fn walk(&mut self, data: &Data, sounds: &mut Vec<(u16, u8)>) {
        if self.v != 0 {
            if self.step == 0 {
                sounds.push((29, 100));
                self.step = 19;
            } else {
                self.step -= 1;
            }
        }
        let a = ((self.r << 8) as u32) & 0xffff;
        let mut dx = -((self.v as i64 * sin16(&data.mini, a) as i64) >> 16) as i32;
        let mut dy = -((self.v as i64 * cos16(&data.mini, a) as i64) >> 16) as i32;
        let (cx, cy) = (self.x >> 21, self.y >> 21);
        let (ix, iy) = (self.x % (CELL << 16), self.y % (CELL << 16));
        if dx < 0 && self.cell(cx - 1, cy) == b'w' && ix < 10 * ONE
            || dx > 0 && self.cell(cx + 1, cy) == b'w' && ix > 22 * ONE
        {
            dx = 0;
        }
        if dy < 0 && self.cell(cx, cy - 1) == b'w' && iy < 10 * ONE
            || dy > 0 && self.cell(cx, cy + 1) == b'w' && iy > 22 * ONE
        {
            dy = 0;
        }
        self.x += dx;
        self.y += dy;
        if self.v > 0 {
            self.hist.rotate_right(1);
            self.hist[0] = (self.x >> 16, self.y >> 16, self.r);
        }
    }

    /// `osewagame07_object_cb` 0x1006d424: touching a G or a T.
    fn collide(&mut self) {
        if self.timeout {
            return;
        }
        for i in 0..self.objs.len() {
            let o = &self.objs[i];
            if o.gone || o.hit {
                continue;
            }
            let dx = ((o.x + 64) * ONE - self.x) >> 16;
            let dy = ((o.y + 64) * ONE - self.y) >> 16;
            let (sx, sy) = (dx, dy);
            let (dx, dy) = (dx - 64, dy - 84);
            if dx * dx + dy * dy >= HIT_R2 || self.stunned {
                continue;
            }
            if !o.shirotama {
                self.v = 0;
                self.bumped = true;
                self.walk = 0;
                self.objs[i].hit = true;
                self.objs[i].fade = Some(0);
                self.hits += 1;
            } else if self.got < 5 {
                self.v = 0;
                self.pause = true;
                self.collecting = true;
                self.walk = 0;
                if self.got < 4 {
                    let s = self.followers.last().map_or(self.hist[13], |f| f.hist[7]);
                    let n = node(FOLLOW_DOWN);
                    self.followers.push(Follower { hist: [s; 16], pos: (s.0, s.1), visible: false, node: n, anim: 0 });
                    self.fx = Some(Fx { at: (sx, sy), to: None, t0: self.time, node: node(SPARKLE) });
                    self.objs[i].gone = true;
                } else {
                    self.fifth = Some(i);
                }
                self.got += 1;
            }
        }
    }

    /// `osewagame07_follower_cb` 0x10016fc0: each follower walks the trail of the one in front.
    fn follow(&mut self) {
        let mut parent = self.hist[13];
        for f in &mut self.followers {
            f.pos = (parent.0, parent.1);
            if self.v > 0 {
                f.hist.rotate_right(1);
                f.hist[0] = parent;
            }
            parent = f.hist[7];
        }
    }

    fn follower_screen(&self, f: &Follower) -> (i32, i32) {
        let (x, y) = self.screen(f.pos.0, f.pos.1);
        (x, y + 16)
    }

    /// `osewagame07_capture_fx_cb` 0x1000b788: wait 1 s, fly to the new follower in 1 s, then it appears.
    fn sparkle(&mut self) {
        let Some(fx) = &self.fx else { return };
        let k = self.got as usize;
        if self.time - fx.t0 <= ONE || k == 0 || k > self.followers.len() {
            return;
        }
        if fx.to.is_none() {
            let to = self.follower_screen(&self.followers[k - 1]);
            let fx = self.fx.as_mut().unwrap();
            (fx.to, fx.t0) = (Some(to), self.time);
        } else {
            self.pause = false;
            self.followers[k - 1].visible = true;
            self.fx = None;
        }
    }

    fn fx_pos(&self, fx: &Fx) -> (i32, i32) {
        match fx.to {
            // ponytail: linear; the firmware uses tween curve 7 (S-shaped).
            Some(to) => {
                let t = (self.time - fx.t0).clamp(0, ONE);
                (fx.at.0 + (to.0 - fx.at.0) * t / ONE, fx.at.1 + (to.1 - fx.at.1) * t / ONE)
            }
            None => fx.at,
        }
    }

    /// `osewagame07_timer_cb` 0x1008e364.
    fn clock(&mut self) {
        let Some(start) = self.timer else { return };
        let t = if self.timeout || self.pause { 0 } else { self.time - start };
        if t > COUNTDOWN {
            self.countdown = Some((60 - (t >> 16)).clamp(0, 9) as u32);
        }
        if t > LIMIT {
            self.v = 0;
            self.timeout = true;
            self.moving = false;
            self.timer = None;
            if self.got < 2 {
                self.state = 0x1004;
            }
        }
    }

    /// Main loop (state 0xbba).
    fn play(&mut self, dial_moved: bool, sounds: &mut Vec<(u16, u8)>) {
        self.stun_update(sounds);
        if self.got < 5 && !self.timeout {
            if dial_moved && !self.bumped && !self.collecting {
                let old = self.r;
                self.r = ((0x8000 - self.dial * 0x10000 / 24) * 0x100) >> 16;
                if self.r != old {
                    self.pick_walk();
                }
            }
            for f in self.followers.iter_mut().filter(|f| f.visible) {
                let a = follow_anim(f.hist[0].2);
                if f.anim != a {
                    f.anim = a;
                    f.node = node(a);
                }
            }
        } else {
            self.v = 0;
            self.t0 = self.time;
            if let Some(i) = self.fifth {
                self.objs[i].node = node(SPARKLE);
            }
            self.state = 4000;
        }
    }

    fn cheer(&mut self, pet: u16, follower: u16) {
        self.set_pet(pet);
        for f in &mut self.followers {
            f.node = node(follower);
        }
    }

    /// Result tier 1..3 when the game ends with at least 2 caught (`osewagame07_tick` 0xfa3).
    pub fn tier(&self) -> u8 {
        match self.got {
            5 if self.hits == 0 => 3,
            4 | 5 => 2,
            _ => 1,
        }
    }
}

impl MiniGame for Together07 {
    fn update(&mut self, pressed: u8, _held: u8, data: &Data, sounds: &mut Vec<(u16, u8)>) -> Option<u8> {
        let turn = (pressed & UP != 0) as i32 - (pressed & DOWN != 0) as i32;
        self.dial = (self.dial + turn).rem_euclid(24);
        if self.wait > 0 {
            self.wait -= 1;
        } else {
            match self.state {
                // ponytail: the white fades between opening, field and result are not drawn.
                0 if self.pet.is_finished() => {
                    self.build();
                    self.t0 = self.time;
                    self.state = 0x7d1;
                }
                0x7d1 if self.time - self.t0 > ONE => {
                    self.v = SPEED;
                    self.state = 0xbb8;
                }
                0xbb8 => {
                    self.set_pet(WALK_DOWN);
                    self.wait = 5;
                    self.state = 0xbb9;
                }
                0xbb9 => {
                    self.timer = Some(self.time);
                    self.state = 0xbba;
                    self.play(turn != 0, sounds);
                }
                0xbba => self.play(turn != 0, sounds),
                4000 if self.time - self.t0 > ONE => {
                    self.cheer(HAPPY, CHEER);
                    if let Some(i) = self.fifth {
                        self.objs[i].node = node(CHEER);
                    }
                    self.state = 4001;
                }
                4001 if self.pet.is_finished() => return Some(self.tier()),
                0x1004 => {
                    self.cheer(SAD, SULK);
                    self.state = 0x1005;
                }
                0x1005 if self.pet.is_finished() => return Some(0),
                _ => {}
            }
        }
        if self.built {
            if self.moving {
                self.walk(data, sounds);
            }
            self.collide();
            self.follow();
            self.sparkle();
            self.clock();
        }
        for o in &mut self.objs {
            o.node.tick(&data.motions);
            if let Some(f) = &mut o.fade {
                *f += 1;
                o.gone |= *f >= 20;
            }
        }
        for f in &mut self.followers {
            f.node.tick(&data.motions);
        }
        if let Some(fx) = &mut self.fx {
            fx.node.tick(&data.motions);
        }
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
        let (px, py) = (self.x >> 16, self.y >> 16);
        // Ground (`osewagame07_ground_scroll_cb`): 128 px tiles scrolling against the walk.
        let (gx, gy) = ((-px).rem_euclid(128), (-py).rem_euclid(128));
        for (i, j) in [(0, 0), (-128, 0), (0, -128), (-128, -128)] {
            out.push(sprite(self.look.0, 0, 64 + gx + i, 64 + gy + j));
        }
        // Walls in the 7×7 cells around the pet, objects, followers and the pet, sorted by screen y.
        let mut items: Vec<(i32, Vec<Draw>)> = Vec::new();
        for j in -3..=3 {
            for i in -3..=3 {
                let (cx, cy) = ((px >> 5) + i, (py >> 5) + j);
                if self.cell(cx, cy) == b'w' {
                    let (sx, sy) = self.screen(cx * CELL + 16, cy * CELL + 16);
                    items.push((sy + 24, vec![sprite(self.look.2, 0, sx, sy)]));
                }
            }
        }
        for o in self.objs.iter().filter(|o| !o.gone) {
            let (sx, sy) = self.screen(o.x, o.y);
            let mut d = Vec::new();
            if o.shirotama {
                o.node.draws(sx, sy, &mut d);
            } else {
                let alpha = 64 - (64 * o.fade.unwrap_or(0) as u32 / 20) as u8;
                d.push(Draw {
                    look: Look::Sprite { id: self.look.1, frame: 0 },
                    x: sx,
                    y: sy,
                    flip: false,
                    alpha,
                    palette: 0,
                });
            }
            items.push((sy, d));
        }
        for f in self.followers.iter().filter(|f| f.visible) {
            let (sx, sy) = self.follower_screen(f);
            let mut d = Vec::new();
            f.node.draws(sx, sy, &mut d);
            items.push((sy, d));
        }
        let mut d = Vec::new();
        self.pet.draws(0, 0, &mut d);
        items.push((PET.1, d));
        items.sort_by_key(|i| i.0);
        out.extend(items.into_iter().flat_map(|i| i.1));
        if let Some(fx) = &self.fx {
            let (x, y) = self.fx_pos(fx);
            fx.node.draws(x, y, out);
        }
    }

    fn hud(&self) -> Vec<Hud> {
        if !self.built {
            return Vec::new();
        }
        let mut h =
            vec![Hud::Sprite { id: 0x80e, frame: 0, x: 15, y: 11 }, Hud::Number { n: self.got as u32, x: 14, y: 11 }];
        if let Some(n) = self.countdown {
            h.push(Hud::Sprite { id: 0x80f, frame: 0, x: 120, y: 11 });
            h.push(Hud::Number { n, x: 120, y: 11 });
        }
        h
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

    /// The 12×13 open field with 3 rows of wall around it, and `objs` (x, y, char).
    fn field(objs: &[(usize, usize, u8)]) -> Vec<String> {
        let mut rows = vec![vec![b'w'; 18]; 19];
        for row in &mut rows[3..16] {
            row[3..15].fill(b' ');
        }
        for &(x, y, c) in objs {
            rows[y][x] = c;
        }
        rows.into_iter().map(|r| String::from_utf8(r).unwrap()).collect()
    }

    fn run(g: &mut Together07, frames: usize, pressed: u8) -> Option<u8> {
        let d = crate::mini::tests::data();
        let mut r = None;
        for _ in 0..frames {
            r = r.or(g.update(pressed, 0, &d, &mut Vec::new()));
        }
        r
    }

    /// Past the opening and the 1 s hold: walking.
    fn walking(objs: &[(usize, usize, u8)]) -> Together07 {
        let mut g = Together07::new(field(objs), 1);
        run(&mut g, 30, 0);
        assert_eq!((g.state, g.v), (0xbba, SPEED));
        g
    }

    #[test]
    fn anims_by_heading() {
        for (r, a) in
            [(128, WALK_DOWN), (0, WALK_UP), (51, WALK_UP), (52, WALK_LEFT), (105, WALK_LEFT), (106, WALK_DOWN)]
        {
            assert_eq!(walk_anim(r), a, "r {r}");
        }
        for (r, a) in [(-54, WALK_UP), (-55, WALK_RIGHT), (-106, WALK_RIGHT), (-107, WALK_DOWN), (-128, WALK_DOWN)] {
            assert_eq!(walk_anim(r), a, "r {r}");
        }
        for (r, a) in [
            (0, FOLLOW_UP),
            (32, FOLLOW_UP),
            (33, FOLLOW_LEFT),
            (-33, FOLLOW_RIGHT),
            (-97, FOLLOW_DOWN),
            (128, FOLLOW_DOWN),
        ] {
            assert_eq!(follow_anim(r), a, "r {r}");
        }
    }

    #[test]
    fn the_dial_steers_and_walls_stop() {
        let mut g = walking(&[(9, 9, b'S')]);
        let (x, y) = (g.x, g.y);
        run(&mut g, 1, 0);
        assert_eq!((g.x, g.y - y), (x, SPEED), "starts walking down");
        // 12 steps = half a turn: up.
        for _ in 0..12 {
            run(&mut g, 1, UP);
        }
        assert_eq!((g.r, g.walk), (0, WALK_UP));
        let x = g.x;
        run(&mut g, 200, 0);
        assert_eq!(g.x, x, "straight up");
        assert!((3 * CELL + 8..3 * CELL + 10).contains(&(g.y >> 16)), "stopped by the wall row: {}", g.y >> 16);
        // A quarter turn back: left, sliding along the top wall.
        for _ in 0..6 {
            run(&mut g, 1, DOWN);
        }
        assert_eq!((g.r, g.walk), (64, WALK_LEFT));
        run(&mut g, 200, 0);
        assert!((3 * CELL + 8..3 * CELL + 10).contains(&(g.x >> 16)), "corner: {}", g.x >> 16);
    }

    #[test]
    fn a_shirotama_joins_the_line() {
        let mut g = walking(&[(9, 6, b'S'), (9, 8, b'T')]);
        let mut n = 0;
        while g.got == 0 {
            run(&mut g, 1, 0);
            n += 1;
            assert!(n < 40);
        }
        assert!(g.objs[0].gone && g.pause && g.fx.is_some());
        assert_eq!((g.followers.len(), g.followers[0].visible), (1, false));
        run(&mut g, 2, 0);
        assert_eq!((g.v, g.pet.script()), (0, GOT), "cheering");
        run(&mut g, 41, 0);
        assert_eq!(g.v, SPEED, "walks on after 2 s");
        run(&mut g, 25, 0);
        assert!(g.followers[0].visible && !g.pause && g.fx.is_none());
    }

    #[test]
    fn a_g_stuns_once() {
        let mut g = walking(&[(9, 6, b'S'), (9, 8, b'G')]);
        run(&mut g, 20, 0);
        assert_eq!((g.hits, g.v, g.pet.script()), (1, 0, BUMPED));
        run(&mut g, 45, 0);
        assert_eq!((g.hits, g.v, g.walk), (1, SPEED, WALK_DOWN));
    }

    #[test]
    fn time_out_and_tiers() {
        let mut g = walking(&[(9, 9, b'S')]);
        let r = run(&mut g, 60 * 20 + 10, 0);
        assert_eq!((r, g.countdown), (Some(0), Some(0)), "nothing caught: failed");
        for (got, hits, t) in [(5, 0, 3), (5, 1, 2), (4, 0, 2), (3, 0, 1), (2, 2, 1)] {
            (g.got, g.hits) = (got, hits);
            assert_eq!(g.tier(), t);
        }
        // Two caught when time runs out: the game ends with tier 1.
        let mut g = walking(&[(9, 9, b'S')]);
        g.got = 2;
        assert_eq!(run(&mut g, 60 * 20 + 40, 0), Some(1));
    }
}
