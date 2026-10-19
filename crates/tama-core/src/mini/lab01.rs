//! Lab game 01: jump up the platforms, "DELIVERING VITAMIN!" (09-minigames-lab-a §2.2).
//!
//! Heights are 16.16 fixed point measured upwards from the screen bottom, like the firmware.

use super::{sprite, Hud, MiniGame};
use crate::data::Data;
use crate::input::{A, B, DOWN, UP};
use crate::motion::{Draw, Node};

const ONE: i32 = 0x10000;
/// Time limit by level, seconds (XIP 0x1009572c).
const TIME_S: [i32; 3] = [100, 90, 80];
const GOAL: i32 = 6400;
/// Highest scroll (`FUN_10028498`).
const SCROLL_MAX: i32 = 0x18c0;
/// Score needed for results 3, 2, 1 (`FUN_10021178`).
const THRESHOLDS: [u32; 3] = [3000, 1800, 300];
/// Result scripts by result (XIP 0x10095740).
const RESULTS: [u16; 4] = [0x9be, 0x9b8, 0x9b6, 0x9ab];
/// 3-2-1 countdown (UI element 0x413), 60 frames before play.
const COUNTDOWN: u16 = 0x95c;
/// Platform sprites and hit widths by type (UI elements at 0x1009b272 / 0x1009b268).
const PLAT_SPRITE: [u16; 5] = [2100, 2101, 2102, 2103, 2104];
const PLAT_W: [i32; 5] = [28, 20, 30, 24, 34];
const SPRING: u8 = 2;
const CRUMBLE: u8 = 3;
const MOVING: u8 = 4;
const PLAYER: u16 = 2098;
const BACKGROUND: u16 = 2095;
const GAUGE: u16 = 2096;
const MARKER: u16 = 2097;
/// Sideways step per frame in the air, 5.6 px (`FUN_10004efc`).
const STEP: i32 = 0x59999;
/// Context clock step (16.16 s per frame), the time base of the platform tweens.
const DT: i32 = 0xccc;

/// One platform of the 32-slot pool (`FUN_1002bcb0`, ticked by `FUN_1002c148`).
#[derive(Clone, Copy, Debug)]
struct Plat {
    kind: u8,
    /// Centre x and height, 16.16.
    x: i32,
    y: i32,
    /// Frames left showing "stepped on" (flags bits 4–5).
    stepped: u8,
    /// Moving direction (bit 6), alternating by spawn order.
    right: bool,
    /// Moving: start time of the sine; None = stopped.
    osc: Option<i32>,
    offset: i32,
    /// Crumbling: start time of the 0.2 s fade.
    fade: Option<i32>,
    alpha: i32,
    /// Hit box (left, top, width), computed on the first tick after spawning.
    hit: Option<(i32, i32, i32)>,
    rider: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sub {
    /// 3-2-1 (substate 1000): frames left.
    Countdown(u16),
    Play,
    /// Fell off (substates 3000–0xbbd): step, frames left.
    Fall(u8, u16),
    /// Time up or goal reached (4000/5000), frames left.
    End(u16),
}

pub struct Lab01 {
    map: Vec<[i16; 3]>,
    sub: Sub,
    /// Context clock (+0x1c), 16.16 s.
    t: i32,
    /// Frames left (+0x50).
    time: i32,
    scroll: i32,
    /// Highest height landed on (+0x60), integer px.
    best: i32,
    score: u32,
    respawn: (i32, i32),
    plats: [Option<Plat>; 32],
    /// Spawner: next map entry and moving platforms spawned.
    next: usize,
    moving: u32,
    /// Player (`+0x11c`): x, height, previous height, acceleration (16.16) and flags.
    px: i32,
    py: i32,
    prev: i32,
    accel: i32,
    air: bool,
    spring: bool,
    falling: bool,
    /// Frames until the automatic jump after landing (bits 3–6).
    land: u8,
    /// Parachute frame counter (bits 7–11) and the resulting x jitter.
    para: u8,
    jitter: i32,
    countdown: Node,
}

/// Game score for a height (`FUN_1006aa58`).
fn height_score(h: i32) -> u32 {
    h.clamp(0, GOAL) as u32 * 3000 / GOAL as u32
}

/// Frames a sound plays (the firmware waits for the buzzer to go idle, `FUN_10024c4c`).
fn sound_frames(data: &Data, id: u16) -> u16 {
    data.melodies.get(id as usize).map_or(1, |m| {
        let ms = m.notes.iter().map(|n| n[0] as u32).sum::<u32>() * m.ms_per_step as u32;
        ms.div_ceil(50).max(1) as u16
    })
}

impl Lab01 {
    /// `map` = lab01_maps[level*3 + plays % 3]. The player starts on the first platform.
    pub fn new(map: Vec<[i16; 3]>, level: usize) -> Self {
        let [x, y, _] = map.first().copied().unwrap_or([64, 12, 0]);
        let (x, y) = (x as i32, y as i32);
        let mut countdown = Node::new(COUNTDOWN);
        countdown.place(64, 64);
        Lab01 {
            map,
            sub: Sub::Countdown(60),
            t: 0,
            time: TIME_S[level.min(2)] * 20,
            scroll: 0,
            best: 0,
            score: 0,
            respawn: (x, y),
            plats: [None; 32],
            next: 0,
            moving: 0,
            px: x * ONE,
            py: y * ONE,
            prev: y * ONE,
            accel: 0,
            air: false,
            spring: false,
            falling: false,
            land: 0,
            para: 0,
            jitter: 0,
            countdown,
        }
    }

    pub fn result(&self) -> u8 {
        THRESHOLDS.iter().position(|&t| self.score >= t).map_or(0, |i| 3 - i as u8)
    }

    /// Jump and steer (`FUN_10004efc`), only while playing.
    fn input(&mut self, pressed: u8, held: u8, sounds: &mut Vec<(u16, u8)>) {
        if !self.spring && self.land > 2 && pressed & (A | B) != 0 {
            self.land = 0;
        }
        if !self.air {
            if self.land > 0 {
                self.land -= 1;
                return;
            }
            if self.spring {
                self.accel = 25 * ONE;
                self.spring = false;
                sounds.push((9, 100));
            } else {
                self.accel = 14 * ONE;
                sounds.push((10, 100));
            }
            self.air = true;
        }
        let mut d = 0;
        if self.px > 16 * ONE && held & A != 0 {
            d = -STEP;
        }
        if self.px < 112 * ONE && held & B != 0 {
            d += STEP;
        }
        // ponytail: dial pulses come every 3 raw counts; the firmware steps on every raw count.
        if pressed & UP != 0 {
            d += STEP;
        } else if pressed & DOWN != 0 {
            d -= STEP;
        }
        // The edge clamp tests x − d (as decompiled), so x can overshoot 16..112 by up to a step.
        let nx = self.px - d;
        if d < 0 && nx < 16 * ONE {
            d = 16 * ONE - self.px;
        }
        if d > 0 && nx > 112 * ONE {
            d = 112 * ONE - self.px;
        }
        self.px += d;
    }

    /// Scroll up to follow the player (`FUN_10028498`), then tick the platforms.
    fn scroll_tick(&mut self) {
        let mut d = 0;
        if self.best - self.scroll > 12 {
            d = (self.best - self.scroll).min(4);
        }
        let y = self.py >> 16;
        if y > 80 {
            d = (y - 80).min(24);
        }
        self.scroll += d;
        if self.scroll > SCROLL_MAX {
            d -= self.scroll - SCROLL_MAX;
            self.scroll = SCROLL_MAX;
        }
        self.py -= d * ONE;
        self.prev -= d * ONE;
        for p in self.plats.iter_mut().flatten() {
            p.y -= d * ONE;
        }
        self.plat_tick();
    }

    /// Platform pool (`FUN_1002c148`): recycle, move, fade, carry the rider, update hit boxes.
    fn plat_tick(&mut self) {
        let falling = matches!(self.sub, Sub::Fall(..));
        for slot in &mut self.plats {
            let Some(p) = slot else { continue };
            if p.y < -0xfffff {
                *slot = None;
                continue;
            }
            if p.kind == MOVING {
                if !falling || p.stepped == 0 {
                    p.osc.get_or_insert(self.t);
                } else {
                    p.osc = None;
                }
            }
            if p.stepped > 0 {
                p.stepped -= 1;
                if p.kind == CRUMBLE && p.stepped == 0 {
                    p.fade = Some(self.t);
                }
            }
            // ponytail: linear fade; the firmware uses tween curve 6 (not decoded) over 0.2 s.
            p.alpha = p.fade.map_or(ONE, |t0| (ONE - ((self.t - t0) as i64 * ONE as i64 / 0x3333) as i32).max(0));
            if p.alpha < 1 {
                *slot = None;
                continue;
            }
            let w = PLAT_W[p.kind as usize];
            let half = w / 2;
            // ±32 px sine with a 3 s period (sine table 0x1009a968, 1024 × 4096).
            let mut off = p.osc.map_or(0, |t0| {
                let i = ((self.t - t0) as i64 * ONE as i64 / (3 * ONE) as i64) as u32 & 0xffff;
                let s = (f64::sin(std::f64::consts::TAU * (i >> 6) as f64 / 1024.0) * 4096.0).round() as i32;
                if p.right { s * 512 } else { -s * 512 }
            });
            if p.x + off < (16 - half) * ONE {
                off = (16 - half) * ONE - p.x;
            }
            if p.x + off > (half + 112) * ONE {
                off = (half + 112) * ONE - p.x;
            }
            if p.rider {
                self.px += off - p.offset;
                p.rider = false;
            }
            p.offset = off;
            p.hit = Some((((p.x + off) >> 16) - half, 124 - (p.y >> 16), w));
        }
    }

    /// Spawner (`FUN_10009d98`): create the platforms up to 384 px above the scroll.
    fn spawn(&mut self) {
        while let Some(&[x, y, kind]) = self.map.get(self.next) {
            let (y, kind) = (y as i32, (kind as u8).min(MOVING));
            if self.scroll + 384 <= y {
                break;
            }
            if y - self.scroll > -16 {
                let right = kind == MOVING && {
                    self.moving += 1;
                    self.moving & 1 == 0
                };
                if let Some(slot) = self.plats.iter_mut().find(|p| p.is_none()) {
                    let (x, y) = (x as i32 * ONE, (y - self.scroll) * ONE);
                    *slot = Some(Plat { kind, x, y, stepped: 0, right, osc: None, offset: 0, fade: None, alpha: ONE, hit: None, rider: false });
                }
            }
            self.next += 1;
        }
    }

    /// Player physics and landing (`FUN_100051dc` message 8).
    fn player_tick(&mut self) {
        let old = self.py;
        let v = self.py - self.prev + self.accel;
        self.py += v;
        self.falling = self.py <= self.prev;
        self.prev = old;
        self.jitter = 0;
        if self.time < 1 && self.air && self.falling {
            // Parachute: constant 0.7 px/frame, x jitters ±2 every 8 frames.
            self.prev = self.py;
            self.accel = -0xb333;
            self.para = (self.para + 1) & 31;
            self.jitter = if self.para & 8 != 0 { 2 } else { -2 };
        } else {
            self.accel = -0x16666;
            self.para = 0;
        }
        // Taller hit box at speed so fast falls cannot pass through a platform.
        let vi = v >> 16;
        let h = if vi < -15 { vi.abs() - 7 } else { 8 };
        let (bx, top) = ((self.px >> 16) - 4, 124 - ((self.py >> 16) + h));
        if !self.falling {
            return;
        }
        let hit = |p: &Option<Plat>| p.and_then(|p| p.hit).is_some_and(|(l, t, w)| l - 8 < bx && bx < l + w && t - h < top && top < t + 8);
        let Some(i) = self.plats.iter().position(hit) else { return };
        let Some(p) = self.plats[i].as_mut() else { return };
        p.stepped = 3;
        let height = (p.y >> 16) + self.scroll;
        if self.time > 0 && height > self.best {
            self.best = height;
            self.score = self.score.max(height_score(height));
            self.respawn = (p.x >> 16, height);
        }
        self.py = p.y;
        self.prev = p.y;
        if p.kind == SPRING {
            self.spring = true;
        }
        if self.air {
            // Crumbling platforms bounce at once.
            self.land = if p.kind == CRUMBLE { 0 } else { 4 };
            self.air = false;
        }
        p.rider = true;
    }

    /// Back to the highest platform reached (substate 0xbbb).
    fn respawn(&mut self) {
        self.scroll = self.best - 12;
        self.next = 0;
        self.moving = 0;
        self.plats = [None; 32];
        self.px = self.respawn.0 * ONE;
        self.py = (self.respawn.1 - self.scroll) * ONE;
        self.prev = self.py;
        self.accel = 0;
        (self.air, self.spring, self.falling, self.land, self.para) = (false, false, false, 0, 0);
    }
}

impl MiniGame for Lab01 {
    fn update(&mut self, pressed: u8, held: u8, data: &Data, sounds: &mut Vec<(u16, u8)>) -> Option<u8> {
        self.t += DT;
        self.countdown.tick(&data.motions);
        self.countdown.take_sounds(sounds);
        // State function (`FUN_10020a30`).
        match self.sub {
            Sub::Countdown(n) => self.sub = if n > 1 { Sub::Countdown(n - 1) } else { Sub::Play },
            Sub::Play => {
                if self.best >= GOAL || self.time < 1 {
                    sounds.push((0x35, 100));
                    self.sub = Sub::End(60);
                } else if self.py < -24 * ONE {
                    sounds.push((0x16, 100));
                    self.sub = Sub::Fall(0, sound_frames(data, 0x16) + 10);
                } else {
                    self.input(pressed, held, sounds);
                }
            }
            Sub::Fall(step, n) if n > 1 => self.sub = Sub::Fall(step, n - 1),
            Sub::Fall(0, _) => {
                sounds.push((0x1a, 100));
                self.sub = Sub::Fall(1, sound_frames(data, 0x1a));
            }
            Sub::Fall(1, _) => {
                self.respawn();
                self.sub = Sub::Fall(2, 20);
            }
            Sub::Fall(..) => self.sub = Sub::Play,
            Sub::End(n) if n > 1 => self.sub = Sub::End(n - 1),
            Sub::End(_) => return Some(self.result()),
        }
        // The clock runs while playing and while falling (substates 2000–3999).
        if matches!(self.sub, Sub::Play | Sub::Fall(..)) && self.time > 0 {
            self.time -= 1;
        }
        // Layer update: scroller, spawner, player.
        self.scroll_tick();
        self.spawn();
        self.player_tick();
        None
    }

    fn draws(&self, out: &mut Vec<Draw>) {
        // Curtain parallax at 1/4 speed (`FUN_10009c34`).
        let v = (self.scroll >> 2) % 256;
        sprite(out, BACKGROUND, 0, 64, v + if v < 128 { 64 } else { -192 }, 64);
        sprite(out, BACKGROUND, 0, 64, v - 64, 64);
        let frame = if !self.air { 0 } else if self.time < 1 && self.falling { 2 } else { 1 };
        sprite(out, PLAYER, frame, (self.px >> 16) + self.jitter, (96 * ONE - self.py) >> 16, 64);
        for p in self.plats.iter().flatten() {
            let frame = (p.stepped != 0) as u16;
            sprite(out, PLAT_SPRITE[p.kind as usize], frame, (p.x + p.offset) >> 16, 128 - (p.y >> 16), (p.alpha >> 10) as u8);
        }
        // Height gauge (`FUN_100245ec`): marker from y 120 (start) to 8 (goal).
        sprite(out, GAUGE, 0, 8, 64, 64);
        let frac = ((self.best as i64) << 16) / GOAL as i64;
        let y = ((120 * ONE) as i64 + (((-112 * ONE) as i64 * frac) >> 16)) >> 16;
        sprite(out, MARKER, 0, 8, y as i32, 64);
        if !self.countdown.done {
            self.countdown.draws(0, 0, out);
        }
    }

    fn hud(&self) -> Vec<Hud> {
        let left = (self.time + 19) / 20;
        if matches!(self.sub, Sub::Countdown(_)) || left >= 10 {
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

    fn run(g: &mut Lab01, frames: usize, pressed: u8, held: u8) -> Option<u8> {
        let d = crate::evolve::tests::data();
        let mut r = None;
        for i in 0..frames {
            let p = if i == 0 { pressed } else { 0 };
            r = r.or(g.update(p, held, &d, &mut Vec::new()));
        }
        r
    }

    fn map(rest: &[[i16; 3]]) -> Vec<[i16; 3]> {
        let mut m = vec![[64, 12, 0]];
        m.extend_from_slice(rest);
        m
    }

    #[test]
    fn countdown_then_a_77_px_jump() {
        let mut g = Lab01::new(map(&[]), 0);
        run(&mut g, 60, 0, 0);
        assert_eq!((g.sub, g.time, g.best, g.score), (Sub::Play, 1999, 12, 5), "landed on the start during 3-2-1");
        assert!(!g.air);
        let mut top = 0;
        let mut frames = 0;
        while frames == 0 || g.air {
            run(&mut g, 1, 0, 0);
            top = top.max((g.py >> 16) + g.scroll);
            frames += 1;
        }
        assert_eq!(frames, 22, "airborne for 21 frames");
        assert_eq!(top - 12, 77, "14 px up, 1.4 px/frame² gravity");
        assert_eq!((g.py >> 16) + g.scroll, 12, "back on the start platform");
        assert_eq!(g.scroll, 89 - 80, "the screen follows the player above 80 px");
    }

    #[test]
    fn landing_waits_four_frames_unless_a_or_b() {
        let mut g = Lab01::new(map(&[]), 0);
        run(&mut g, 61, 0, 0);
        assert!(g.air);
        while g.air {
            run(&mut g, 1, 0, 0);
        }
        assert_eq!(g.land, 4);
        run(&mut g, 1, A, 0);
        run(&mut g, 1, 0, 0);
        assert!(g.air, "A skips the wait");
    }

    #[test]
    fn climbing_scores_and_respawns_after_a_fall() {
        // A crumbling platform 60 px up: landing there bounces again at once.
        let mut g = Lab01::new(map(&[[64, 72, CRUMBLE as i16]]), 0);
        run(&mut g, 60, 0, 0);
        while g.best < 72 {
            run(&mut g, 1, 0, 0);
        }
        assert_eq!((g.score, g.land, g.respawn), (height_score(72), 0, (64, 72)));
        run(&mut g, 1, 0, 0);
        assert!(g.air, "crumbling platform: no landing wait");
        // The platform crumbles away 3 + 5 frames later; the player falls off the screen.
        let mut frames = 0;
        while !matches!(g.sub, Sub::Fall(..)) {
            run(&mut g, 1, 0, 0);
            frames += 1;
            assert!(frames < 200);
        }
        while g.sub != Sub::Play {
            run(&mut g, 1, 0, 0);
        }
        assert_eq!((g.px >> 16, g.scroll), (64, 60));
        assert_eq!(g.py >> 16, 12, "back on the best platform, 12 px above the bottom");
    }

    #[test]
    fn moving_platforms_swing_32_px() {
        let mut g = Lab01::new(map(&[[64, 100, MOVING as i16], [64, 140, MOVING as i16]]), 0);
        run(&mut g, 2, 0, 0);
        run(&mut g, 15, 0, 0);
        let offs: Vec<i32> = g.plats.iter().flatten().filter(|p| p.kind == MOVING).map(|p| p.offset >> 16).collect();
        assert_eq!(offs, [-32, 32], "quarter period, alternating directions");
    }

    #[test]
    fn time_up_ends_the_game() {
        let mut g = Lab01::new(map(&[]), 2);
        assert_eq!(g.time, 1600);
        let r = run(&mut g, 60 + 1600, 0, 0);
        assert_eq!((r, g.sub), (None, Sub::End(60)));
        assert_eq!(g.hud().len(), 2);
        assert_eq!(run(&mut g, 60, 0, 0), Some(0));
    }

    #[test]
    fn results_by_height() {
        let mut g = Lab01::new(map(&[]), 0);
        for (h, r) in [(639, 0), (640, 1), (3839, 1), (3840, 2), (6399, 2), (6400, 3)] {
            g.score = height_score(h);
            assert_eq!(g.result(), r, "height {h}");
        }
    }
}
