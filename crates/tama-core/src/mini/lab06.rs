//! Lab game 06: CLOUDY COIN COLLECTION (09-minigames-lab-b §2.4). Positions are 16.16 fixed point.

use super::{sprite, Hud, MiniGame};
use crate::data::Data;
use crate::input::A;
use crate::motion::{Draw, Node};

const ONE: i32 = 0x10000;
const TIME: i16 = 2000;
const GOAL: i32 = 6500;
/// Pet anchor (0x4bb) and where it slides in from / is knocked back to.
const PET_X: i32 = 104;
const PET_Y: i32 = 102;
const OFF_X: i32 = PET_X + 64;
const MIN_Y: i32 = 32 * ONE;
const MAX_Y: i32 = 120 * ONE;
/// Hold A: −0.24 px/f², release: +0.24; falling adds 0.1 px/f to the scroll speed each frame.
const LIFT: i32 = 0x3d70;
const DIVE: i32 = 0x1999;
const SPEED: i32 = 4 * ONE;
/// Knockback: first frame −5 px, then +0.4 px/f²; x tween to 168 over 1.0 s.
const KNOCK_UP: i32 = -5 * ONE;
const KNOCK_FALL: i32 = 0x6666;
const KNOCK_FRAMES: u16 = 20;
/// Slide in / back to x 104 (0.5 s).
const SLIDE_FRAMES: u16 = 10;
const INVULN: u8 = 20;
/// The camera follows the pet above y 48.
const CAMERA_Y: i32 = 48 * ONE;
/// Objects spawn 256 px ahead; recycled past these bounds.
const AHEAD: i32 = 256;
const SLOTS: usize = 32;
// ponytail: fades are not drawn; the ~8 frames the original waits for each one are kept.
const FADE: u16 = 8;
const THRESHOLDS: [u32; 3] = [40, 25, 1];
/// Pose → cloud frame (XIP 0x100938ee). Poses 0–2 make the sage bob, 3–5 show sage frame 1.
const CLOUD_FRAME: [u16; 6] = [0, 0, 1, 1, 0, 0];
const BACKGROUND: u16 = 0x86b;
const CLOUD: u16 = 0x86d;
/// The sage (origin (0,−15)).
const SAGE: u16 = 0x86c;
const COIN: u16 = 0x872;
const ROCK: u16 = 0x873;
const FLAG: u16 = 0x86e;
const GO: u16 = 0x95c;
const RESULTS: [u16; 4] = [0xa3f, 0xa3c, 0xa3a, 0xa36];
pub const OPENING: u16 = 0xa2e;
const KIND_GOAL: i16 = 2;
const KIND_COIN: i16 = 9;

#[derive(Clone, Copy, Debug, Default)]
struct Tween {
    from: i32,
    to: i32,
    t: u16,
    len: u16,
}

impl Tween {
    fn at(v: i32) -> Self {
        Tween { from: v, to: v, t: 0, len: 0 }
    }

    /// Slides ease out (tween curve 3).
    fn to(&self, to: i32, len: u16) -> Self {
        Tween { from: self.value(), to, t: 0, len }
    }

    fn value(&self) -> i32 {
        if self.done() {
            self.to
        } else {
            crate::ease::ease_fixed(3, self.from, self.to, self.t as u32, self.len as u32)
        }
    }

    fn done(&self) -> bool {
        self.t >= self.len
    }
}

#[derive(Clone, Copy, Debug)]
struct Obj {
    x: i32,
    y: i32,
    y_prev: i32,
    accel: i32,
    kind: i16,
    /// Obstacle knocked away, coin collected: no more collisions.
    bumped: bool,
    got: bool,
    front: bool,
}

impl Obj {
    fn new(x: i32, y: i32, kind: i16) -> Self {
        Obj { x: x * ONE, y: y * ONE, y_prev: y * ONE, accel: 0, kind, bumped: false, got: false, front: false }
    }

    /// `labo06_obj_scroll` 0x10026224 for one object. Returns false when it is recycled.
    fn scroll(&mut self, delta: i32, pet_y: i32) -> bool {
        if self.bumped {
            let p = self.y_prev;
            self.y_prev = self.y;
            self.x += delta - 4 * ONE;
            self.y += self.y - p + self.accel;
            self.accel = ONE / 2;
            self.front = true;
        } else if self.got {
            self.x += delta - 3 * ONE / 2;
            self.front = true;
        } else {
            self.x += delta;
            self.y_prev = self.y;
            self.front = pet_y < self.y;
        }
        !(self.x > 0xbf_ffff || self.y > 0xbf_ffff || self.y < -0x7f_ffff)
    }

    /// Hit box (0x10094768; coins 24×24).
    fn size(&self) -> (i32, i32) {
        match self.kind {
            0 => (12, 24),
            k if k >= KIND_COIN => (24, 24),
            _ => (12, 12),
        }
    }
}

pub struct Lab06 {
    course: Vec<[i16; 3]>,
    next: usize,
    objs: Vec<Obj>,
    goal: Option<Obj>,
    /// `labo06_main_cb` 0x10079f0c state and its wait counter.
    state: u16,
    wait: u16,
    scroll: i32,
    time: i16,
    coins: u32,
    x: Tween,
    y: i32,
    y_prev: i32,
    /// y to land back on after a knockback, and the draw-layer split for objects.
    base_y: i32,
    accel: i32,
    dive: i32,
    /// Knockback bounce running (status bits 5–6) and its x tween not finished (bit 7).
    knock: bool,
    busy: bool,
    invuln: u8,
    pose: usize,
    cloud: u16,
    frame: u32,
    go: Option<Node>,
}

impl Lab06 {
    /// `course` = courses[level*3 + plays % 3].
    pub fn new(course: Vec<[i16; 3]>) -> Self {
        let y = PET_Y * ONE;
        Lab06 {
            course,
            next: 0,
            objs: Vec::new(),
            goal: None,
            state: 1000,
            wait: 0,
            scroll: 0,
            time: TIME,
            coins: 0,
            x: Tween::at(OFF_X * ONE),
            y,
            y_prev: y,
            base_y: y,
            accel: 0,
            dive: 0,
            knock: false,
            busy: false,
            invuln: 0,
            pose: 0,
            cloud: 0,
            frame: 0,
            go: None,
        }
    }

    pub fn result(&self) -> u8 {
        THRESHOLDS.iter().position(|&t| self.coins >= t).map_or(0, |i| 3 - i as u8)
    }

    fn set_pose(&mut self, pose: usize) {
        self.pose = pose;
        self.cloud = CLOUD_FRAME[pose];
    }

    /// `labo06_knockback` 0x100194a0.
    fn knockback(&mut self) {
        if self.busy {
            return;
        }
        self.knock = true;
        self.y_prev = self.y;
        self.accel = KNOCK_UP;
        self.base_y = self.y;
        self.x = self.x.to(OFF_X * ONE, KNOCK_FRAMES);
        self.busy = true;
    }

    /// `labo06_pet_physics` 0x100196f0 (Verlet).
    fn physics(&mut self, held: u8) {
        self.invuln = self.invuln.saturating_sub(1);
        if self.busy && self.x.done() {
            self.accel = 0;
            self.knock = false;
            self.busy = false;
        }
        let old = self.y;
        if !self.knock {
            if self.accel == 0 {
                self.y_prev = self.y;
            } else {
                self.y = (self.y + (self.y - self.y_prev) + self.accel).clamp(MIN_Y, MAX_Y);
                if self.accel < 1 || self.y <= self.y_prev {
                    self.cloud = (held & A != 0) as u16;
                    self.dive = 0;
                } else {
                    self.cloud = 2;
                    self.dive += DIVE;
                }
                self.y_prev = old;
            }
            self.base_y = self.y;
            self.accel = 0;
        } else {
            self.y += self.y - self.y_prev + self.accel;
            let falling = self.y > self.y_prev;
            self.cloud = 0;
            self.y_prev = old;
            self.accel = KNOCK_FALL;
            if falling && self.base_y <= self.y {
                if !self.busy {
                    self.set_pose(1);
                }
                self.y = self.base_y;
                self.y_prev = self.y;
                self.accel = 0;
                self.knock = false;
            }
        }
    }

    /// `labo06_collide` 0x100179d0: coins count; an obstacle knocks the pet back unless invulnerable.
    fn collide(&mut self, sounds: &mut Vec<(u16, u8)>) -> bool {
        let (px, py) = ((self.x.value() >> 16) - 20, (self.y >> 16) - 16);
        for o in self.objs.iter_mut().filter(|o| !o.bumped && !o.got) {
            let (w, h) = o.size();
            let (l, t) = ((o.x >> 16) - (w >> 1), (o.y >> 16) - (h >> 1));
            if !(l - 24 < px && px < l + w && t - 8 < py && py < t + h) {
                continue;
            }
            if o.kind >= KIND_COIN {
                o.got = true;
                sounds.push((0x3b, 100));
                self.coins += 1;
            } else {
                // ponytail: the bounce height is rand(4.0..6.0), only visual; fixed at 5.
                o.bumped = true;
                o.accel = -5 * ONE;
                if self.invuln == 0 {
                    sounds.push((0xe, 100));
                    return true;
                }
            }
        }
        false
    }

    /// Spawn course objects coming within 256 px, then the goal flag.
    fn spawn(&mut self) {
        let s = self.scroll >> 16;
        while let Some(&[x, y, kind]) = self.course.get(self.next).filter(|e| (e[0] as i32) < s + AHEAD) {
            if self.objs.len() < SLOTS {
                self.objs.push(Obj::new(s + PET_X - x as i32, y as i32, kind));
            }
            self.next += 1;
        }
        if self.next == self.course.len() && GOAL < s + AHEAD {
            self.goal = Some(Obj::new(s + PET_X - GOAL, 104, KIND_GOAL));
            self.next += 1;
        }
    }

    fn bob(&self) -> i32 {
        // ponytail: tween mode 10 (1 s float, −1 px) as a square wave.
        -((self.frame / 10 % 2) as i32)
    }
}

impl MiniGame for Lab06 {
    fn update(&mut self, _pressed: u8, held: u8, data: &Data, sounds: &mut Vec<(u16, u8)>) -> Option<u8> {
        self.frame += 1;
        if let Some(n) = &mut self.go {
            n.tick(&data.motions);
            n.take_sounds(sounds);
            if n.done {
                self.go = None;
            }
        }
        let mut delta = 0;
        if self.wait > 0 {
            self.wait -= 1;
        } else {
            match self.state {
                1000 => {
                    self.x = self.x.to(PET_X * ONE, SLIDE_FRAMES);
                    self.state = 1001;
                }
                1001 if self.x.done() => {
                    let mut n = Node::new(GO);
                    n.place(64, 64);
                    self.go = Some(n);
                    self.wait = 60;
                    self.state = 1002;
                }
                1002 => {
                    self.set_pose(1);
                    self.state = 2000;
                }
                2000 => {
                    // The knockback flag is always clear here (the x tween ends before 3005), so the
                    // original's 2.5 px/f knockback speed never applies.
                    delta = SPEED + self.dive;
                    self.scroll += delta;
                    if (self.scroll >> 16).min(GOAL) > GOAL - 1 || self.time < 1 {
                        self.state = 4000;
                    } else {
                        self.accel = if held & A != 0 { -LIFT } else { LIFT };
                    }
                }
                3000 => {
                    self.knockback();
                    self.state = 3001;
                }
                3001 if !self.busy => {
                    self.wait = FADE;
                    self.state = 3002;
                }
                3002 => {
                    self.set_pose(0);
                    self.y = PET_Y * ONE;
                    self.wait = 20 + FADE;
                    self.state = 3004;
                }
                3004 => {
                    self.x = self.x.to(PET_X * ONE, SLIDE_FRAMES);
                    self.state = 3005;
                }
                3005 if self.x.done() => {
                    self.set_pose(1);
                    self.invuln = INVULN;
                    self.state = 2000;
                }
                4000 => {
                    sounds.push((0x35, 100));
                    self.set_pose(0);
                    self.wait = 50;
                    self.state = 4001;
                }
                4001 => return Some(self.result()),
                _ => {}
            }
        }
        if self.state >= 4000 {
            delta = ONE / 2;
            self.scroll += delta;
        }
        self.spawn();
        self.physics(held);
        let base = self.base_y;
        self.objs.retain_mut(|o| o.scroll(delta, base));
        if self.goal.as_mut().is_some_and(|g| !g.scroll(delta, base)) {
            self.goal = None;
        }
        self.x.t = self.x.t.saturating_add(1);
        if self.state == 2000 && !self.knock && self.collide(sounds) {
            self.set_pose(4);
            self.state = 3000;
        }
        if (2000..4000).contains(&self.state) && self.time > 0 {
            self.time -= 1;
        }
        None
    }

    fn draws(&self, out: &mut Vec<Draw>) {
        let cam = (CAMERA_Y - self.y).max(0) >> 16;
        // Parallax background (`labo06_bg_cb` 0x1006eb24): half the scroll speed, sky row above.
        let s = (self.scroll >> 17).rem_euclid(256);
        let (a, b) = (if s < 128 { s + 64 } else { s - 192 }, s - 64);
        for (x, frame) in [(a, 4), (b, 5)] {
            out.push(sprite(BACKGROUND, frame, x, 64 + cam));
            out.push(sprite(BACKGROUND, 0, x, cam - 64));
        }
        let bob = self.bob();
        let obj = |o: &Obj, out: &mut Vec<Draw>| {
            let (id, bob) = match o.kind {
                KIND_GOAL => (FLAG, bob),
                k if k >= KIND_COIN => (COIN, bob),
                _ => (ROCK, 0),
            };
            // ponytail: a collected coin shrinks away with sparkles; it just disappears here.
            if !o.got {
                out.push(sprite(id, 0, o.x >> 16, (o.y >> 16) + bob + cam));
            }
        };
        let objs = || self.goal.iter().chain(&self.objs);
        for o in objs().filter(|o| !o.front) {
            obj(o, out);
        }
        let (x, y) = (self.x.value() >> 16, (self.y >> 16) + cam);
        out.push(sprite(CLOUD, self.cloud, x, y));
        let still = self.pose >= 3;
        out.push(sprite(SAGE, still as u16, x, y - 15 + if still { 0 } else { bob }));
        for o in objs().filter(|o| o.front) {
            obj(o, out);
        }
        if let Some(n) = &self.go {
            n.draws(0, 0, out);
        }
    }

    fn hud(&self) -> Vec<Hud> {
        let mut h =
            vec![Hud::Sprite { id: 0x80e, frame: 0, x: 15, y: 11 }, Hud::Number { n: self.coins, x: 14, y: 11 }];
        let secs = (self.time as i32 + 19) / 20;
        if secs < 10 {
            h.push(Hud::Sprite { id: 0x80f, frame: 0, x: 120, y: 11 });
            h.push(Hud::Number { n: secs as u32, x: 120, y: 11 });
        }
        h
    }

    fn result_script(&self, result: u8) -> u16 {
        RESULTS[result as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(g: &mut Lab06, frames: usize, held: u8) -> Option<u8> {
        let d = crate::evolve::tests::data();
        let mut r = None;
        for _ in 0..frames {
            r = r.or(g.update(0, held, &d, &mut Vec::new()));
        }
        r
    }

    /// Past the slide-in and GO: playing.
    fn playing(course: Vec<[i16; 3]>) -> Lab06 {
        let mut g = Lab06::new(course);
        run(&mut g, 12 + 60, 0);
        assert_eq!((g.state, g.x.value() >> 16), (2000, PET_X));
        g
    }

    #[test]
    fn holding_a_rises_and_falling_dives() {
        let mut g = playing(vec![]);
        run(&mut g, 1, A);
        assert_eq!(g.y, PET_Y * ONE - LIFT);
        run(&mut g, 1, A);
        assert_eq!(g.y, PET_Y * ONE - 3 * LIFT, "Verlet: speed builds up");
        run(&mut g, 60, A);
        assert_eq!(g.y, MIN_Y);
        let s = g.scroll;
        run(&mut g, 5, 0);
        assert_eq!(g.cloud, 2);
        assert!(g.dive > 0 && g.scroll - s > 5 * SPEED);
        run(&mut g, 60, 0);
        assert_eq!((g.y, g.dive), (MAX_Y, 0), "landed at the bottom: no more dive bonus");
    }

    #[test]
    fn coins_are_collected_when_the_scroll_reaches_them() {
        // Pet falls to y 120 at once; coins on its line at distance 200 and 300.
        let mut g = playing(vec![[200, 120, KIND_COIN], [300, 120, KIND_COIN], [400, 40, KIND_COIN]]);
        run(&mut g, 120, 0);
        assert_eq!(g.coins, 2);
        assert_eq!(g.result(), 1);
    }

    #[test]
    fn an_obstacle_knocks_back_then_gives_invulnerability() {
        let mut g = playing(vec![[200, 120, 0], [204, 120, 0]]);
        let mut n = 0;
        while g.state == 2000 {
            run(&mut g, 1, 0);
            n += 1;
            assert!(n < 100);
        }
        assert_eq!((g.state, g.pose), (3000, 4));
        let t = g.time;
        let scroll = g.scroll;
        run(&mut g, 2, 0);
        assert!(g.knock && g.y < MAX_Y, "bounced up");
        while g.state != 2000 {
            run(&mut g, 1, 0);
        }
        assert_eq!(g.scroll, scroll, "no progress while knocked back");
        assert!(g.time < t - 40, "but the clock runs");
        assert_eq!((g.invuln, g.x.value() >> 16), (INVULN - 1, PET_X), "set, then one physics tick");
    }

    #[test]
    fn reaching_the_goal_ends_the_game() {
        let mut g = playing(vec![]);
        let r = run(&mut g, 1700, 0);
        assert!(r.is_some());
        assert!(g.time > 0 && g.scroll >> 16 >= GOAL);
        let mut g = playing(vec![]);
        g.time = 1;
        assert_eq!(run(&mut g, 60, 0), Some(0), "time out");
        for (c, r) in [(40, 3), (39, 2), (25, 2), (24, 1), (1, 1), (0, 0)] {
            g.coins = c;
            assert_eq!(g.result(), r);
        }
    }
}
