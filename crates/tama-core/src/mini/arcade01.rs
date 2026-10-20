//! Tama Space Arcade 01 "HOCKEY" (scene 0x85, `arcade_pong_play` 0x100860f8; 09-arcade §2).
//!
//! Air hockey for two devices on a link cable: each screen shows its own half of one field.
//! Offline the friend's half is played by a CPU paddle (`Cpu`), everything else follows the
//! firmware's host side. Coordinates are the host's: my screen is y 0..128, the friend's half is
//! mirrored above it (field centre y = −8).

use super::{sprite, Hud, MiniGame};
use crate::data::Data;
use crate::input::{A, B, C, DOWN, PUSH, UP};
use crate::motion::{Draw, Node};

/// Host opening (both tamas, role 1 = the friend), elem 0x5d4.
pub const INTRO: u16 = 0xb4b;
/// ARCADE_RESULT scripts (XIP 0x10095750, host row) by result 0 win / 1 lose / 2 draw.
pub const RESULTS: [u16; 3] = [0xb27, 0xb28, 0xb29];
const BG: u16 = 0x8d3;
const PADDLE: u16 = 0x8d4;
const BALL: u16 = 0x8d6;
const CORNER: u16 = 0x8d7;
/// Full charge sparkle (elem 0x5db), hit sparks (elem 0x5dd / 0x5de, RW 0x180006da).
const CHARGED: u16 = 0xb57;
const HIT_FX: u16 = 0xb60;
const WALL_FX: u16 = 0xb61;
/// Hit sounds by kind (RW 0x180006e0): paddle, smashing paddle, wall / corner.
const HIT_SND: [u16; 3] = [0xa, 0x3d, 0];
/// Goal banners (elems 0x5e2 / 0x5e3) and the conceded effect (elem 0x5e1 at (64, 96)).
const SCORED: u16 = 0xb62;
const CONCEDED: u16 = 0xb64;
const CONCEDED_FX: u16 = 0xb5d;
/// In-game result panel (`FUN_10002cec`, RW 0x18000000 game 0): sprite and pet pose by result.
const PANEL: [u16; 3] = [0x8d0, 0x8d2, 0x8d1];
const PANEL_PET: [u16; 3] = [0xb2f, 0xb31, 0xb30];
/// 2400 ticks; one tick per 20 fps frame (the clock shows ceil(ticks / 20) s, `FUN_1008fb7c`).
pub const TICKS: u16 = 2400;
/// Paddle step per frame for A / C and per dial pulse (±0x60000).
const STEP: f32 = 6.0;
const MIN_SPEED: f32 = 4.0;
/// Ball radius, and centre distance at which ball and paddle touch (`FUN_10003cf8`).
const R: f32 = 8.0;
const REACH: f32 = 21.0;
const MY_Y: f32 = 100.0;
const CPU_Y: f32 = -116.0;
/// Goal lines (`FUN_10003cb8`): past 144 the friend scores, above −159 I score.
const MY_GOAL: f32 = 144.0;
const CPU_GOAL: f32 = -159.0;
/// Corner blocks 32×6 (`FUN_100683cc`), top-left corners.
const CORNERS: [(f32, f32); 4] = [(-8.0, 122.0), (104.0, 122.0), (-8.0, -144.0), (104.0, -144.0)];
/// Serve position and direction by who conceded (`FUN_100040f8` / `FUN_10004140`, speed 4.0).
const SERVE_ME: ((f32, f32), u32) = ((6.0, 36.0), 0x5000);
const SERVE_CPU: ((f32, f32), u32) = ((122.0, -52.0), 0xd000);
/// CPU paddle speed (the player's is 6) and how often it re-aims, frames.
// ponytail: fixed CPU skill; scale CPU_STEP / CPU_THINK if it needs difficulty levels.
const CPU_STEP: f32 = 4.0;
const CPU_THINK: u8 = 4;

/// The 3-2-1 countdown before play (`FUN_10002a24`, sprite 0x8c8 frame = seconds − 1, 60 ticks).
pub(super) const COUNT_SPRITE: u16 = 0x8c8;
pub(super) const COUNT: u8 = 60;

pub(super) fn count_hud(n: u8) -> Option<Draw> {
    let s = (n as i32 + 19) / 20;
    (1..4).contains(&s).then(|| sprite(COUNT_SPRITE, s as u16 - 1, 64, 64))
}

/// xorshift32 (the firmware rng depends on timing, so bit-exactness is not a goal).
pub(super) fn rand(s: &mut u32) -> u32 {
    *s ^= *s << 13;
    *s ^= *s >> 17;
    *s ^= *s << 5;
    *s
}

/// In-game result panel (`FUN_10002cec`, update callback `FUN_1006b5d0`): dim overlay, banner,
/// scores and the pet's reaction. The whole panel fades in (0.15 s, linear), then the content
/// slides in from one screen to the left (0.25 s, ease-in, sound 53); 40 frames later any key
/// closes it, or 5 s without input.
pub(super) struct Panel {
    pub sprite: u16,
    pub pet: Node,
    pub t: u16,
}

const PANEL_FADE: u16 = 3;
const PANEL_SLIDE: u16 = 5;
/// The slide starts, then the panel ignores keys for 40 frames (`FUN_1006b6fc` countdown).
const PANEL_LOCK: u16 = PANEL_FADE + 1 + 40;
/// `idle_exceeds(5)`: 5 s with no input after the lock.
const PANEL_IDLE: u16 = 100;

impl Panel {
    pub fn new(sprite: u16, pet: u16) -> Self {
        Panel { sprite, pet: Node::new(pet), t: 0 }
    }

    /// True once closed: any of A/B/C/PUSH (key mask 0xf) after the lock, or idle for 5 s.
    pub fn update(&mut self, pressed: u8, data: &Data, sounds: &mut Vec<(u16, u8)>) -> bool {
        self.pet.tick(&data.motions);
        if self.t == PANEL_FADE {
            sounds.push((0x35, 100));
        }
        self.t += 1;
        self.t > PANEL_LOCK && (pressed & (A | B | C | PUSH) != 0 || self.t > PANEL_LOCK + PANEL_IDLE)
    }

    /// Content x offset: −128 → 0 with 1 − cos(t·90°) once the fade is done.
    pub fn dx(&self) -> i32 {
        let k = self.t.saturating_sub(PANEL_FADE).min(PANEL_SLIDE) as f32 / PANEL_SLIDE as f32;
        (-128.0 * (k * std::f32::consts::FRAC_PI_2).cos()).round() as i32
    }

    pub fn draws(&self, out: &mut Vec<Draw>) {
        let fade = (self.t.min(PANEL_FADE) * 32 / PANEL_FADE) as u8;
        out.push(Draw { alpha: fade, ..sprite(0x8cc, 0, 64, 64) });
        let dx = self.dx();
        out.push(sprite(self.sprite, 0, 64 + dx, 64));
        self.pet.draws(64 + dx, 94, out);
    }
}

fn len(v: (f32, f32)) -> f32 {
    (v.0 * v.0 + v.1 * v.1).sqrt()
}

/// Angle 0x10000 = one turn; the firmware's launch is (cos, sin)(a − 0x4000).
fn launch(a: u32, speed: f32) -> (f32, f32) {
    let r = (a as f32 - 16384.0) / 65536.0 * std::f32::consts::TAU;
    (r.cos() * speed, r.sin() * speed)
}

/// A paddle (`FUN_100241bc`): moves on x, charges while held, lunges 32 px forward on release.
#[derive(Clone, Debug)]
struct Paddle {
    x: f32,
    base: f32,
    /// −1 for my paddle (lunges up), +1 for the friend's.
    fwd: f32,
    prev: (f32, f32),
    charge: f32,
    full: bool,
    smashing: bool,
    /// Lunge tween: offset from → to over len frames.
    tw: (f32, f32, u8, u8),
}

impl Paddle {
    fn new(base: f32, fwd: f32) -> Self {
        Paddle {
            x: 64.0,
            base,
            fwd,
            prev: (64.0, base),
            charge: 0.0,
            full: false,
            smashing: false,
            tw: (0.0, 0.0, 0, 0),
        }
    }

    fn off(&self) -> f32 {
        let (a, b, t, n) = self.tw;
        if t >= n {
            b
        } else {
            a + (b - a) * t as f32 / n as f32
        }
    }

    fn pos(&self) -> (f32, f32) {
        (self.x, self.base + self.fwd * self.off())
    }

    fn update(&mut self, dx: f32, charge: bool) {
        self.prev = self.pos();
        let (_, _, t, n) = self.tw;
        if t >= n {
            if !self.smashing {
                if !charge {
                    if self.charge > 0.0 {
                        // 0.35 s − 0.2 s × charge (0x5999 − charge × 0x3333).
                        let frames = ((0.35 - 0.2 * self.charge) * 20.0).round() as u8;
                        self.tw = (self.off(), 32.0, 0, frames);
                        (self.smashing, self.charge, self.full) = (true, 0.0, false);
                    }
                } else {
                    self.charge = (self.charge + 0.15).min(1.0);
                    self.full = self.charge >= 1.0;
                }
            } else {
                self.tw = (self.off(), 0.0, 0, 3);
                (self.smashing, self.full) = (false, false);
            }
        }
        self.tw.2 = self.tw.2.saturating_add(1);
        self.x = (self.x + dx).clamp(13.0, 115.0);
    }
}

#[derive(Clone, Debug, Default)]
struct Ball {
    pos: (f32, f32),
    prev: (f32, f32),
    v: (f32, f32),
    on: bool,
    /// Time-up: speed and alpha fade to 0 over 20 frames (`FUN_100043e4`).
    fade: Option<(u8, f32)>,
}

impl Ball {
    /// `FUN_1000418c`: keep at least 4 px/frame, lose 1 % a frame above it, bounce off the sides.
    fn step(&mut self) -> Option<(f32, f32)> {
        if !self.on {
            return None;
        }
        self.prev = self.pos;
        let s = len(self.v);
        if s > 0.0 {
            let ns = match &mut self.fade {
                Some((n, s0)) => {
                    *n = n.saturating_sub(1);
                    *s0 * *n as f32 / 20.0
                }
                None => (s * 0.99).max(MIN_SPEED),
            };
            self.v = (self.v.0 / s * ns, self.v.1 / s * ns);
        }
        self.pos = (self.pos.0 + self.v.0, self.pos.1 + self.v.1);
        if self.pos.0 < R {
            self.pos.0 = R;
            self.v.0 = -self.v.0;
            return Some((0.0, self.pos.1));
        }
        if self.pos.0 > 128.0 - R {
            self.pos.0 = 128.0 - R;
            self.v.0 = -self.v.0;
            return Some((128.0, self.pos.1));
        }
        None
    }

    /// `FUN_10003cf8`: sweep ball and paddle in sub-steps; reflect the relative motion (×1.98).
    // ponytail: the extra push-out by penetration depth is left out (a fraction of a pixel).
    fn hit(&mut self, p: &Paddle) -> Option<(f32, f32)> {
        let pp = p.pos();
        let bd = (self.pos.0 - self.prev.0, self.pos.1 - self.prev.1);
        let pd = (pp.0 - p.prev.0, pp.1 - p.prev.1);
        let n = ((len(bd) + len(pd)) / 8.0).floor().max(1.0) as i32;
        for i in 1..=n {
            let f = i as f32 / n as f32;
            let b = (self.prev.0 + bd.0 * f, self.prev.1 + bd.1 * f);
            let q = (p.prev.0 + pd.0 * f, p.prev.1 + pd.1 * f);
            let d = (b.0 - q.0, b.1 - q.1);
            let dist = len(d);
            if dist > REACH {
                continue;
            }
            let nrm = if dist > 0.0 { (d.0 / dist, d.1 / dist) } else { (0.0, p.fwd) };
            let dot = nrm.0 * (bd.0 - pd.0) + nrm.1 * (bd.1 - pd.1);
            if dot < 0.0 {
                self.v = (self.v.0 - 1.98 * dot * nrm.0, self.v.1 - 1.98 * dot * nrm.1);
                self.pos = (q.0 + nrm.0 * REACH, q.1 + nrm.1 * REACH);
                return Some((q.0 + nrm.0 * 13.0, q.1 + nrm.1 * 13.0));
            }
        }
        None
    }

    /// `FUN_100684e4`: ten sub-steps against the corner blocks, mirror on the contact normal.
    fn corners(&mut self) -> Option<(f32, f32)> {
        for i in 1..=10 {
            let f = i as f32 / 10.0;
            let b = (self.prev.0 + (self.pos.0 - self.prev.0) * f, self.prev.1 + (self.pos.1 - self.prev.1) * f);
            for &(x, y) in &CORNERS {
                let c = (b.0.clamp(x, x + 32.0), b.1.clamp(y, y + 6.0));
                let d = (b.0 - c.0, b.1 - c.1);
                let dist = len(d);
                if dist >= R {
                    continue;
                }
                if dist == 0.0 {
                    return None;
                }
                let n = (d.0 / dist, d.1 / dist);
                let dot = n.0 * self.v.0 + n.1 * self.v.1;
                self.v = (self.v.0 - 2.0 * dot * n.0, self.v.1 - 2.0 * dot * n.1);
                self.pos = (b.0 + n.0 * (R - dist), b.1 + n.1 * (R - dist));
                return Some(c);
            }
        }
        None
    }
}

enum St {
    Intro,
    Count(u8),
    /// The serve marker flies in for 10 frames (`FUN_1000f040`, 0.5 s); `me` = served on my side.
    Serve(u8, bool),
    Play,
    Goal {
        me: bool,
        banner: Node,
        fx: Option<Node>,
    },
    /// Time is up: the ball fades out (state 9000).
    End,
    Panel(Panel),
}

pub struct Arcade01 {
    st: St,
    intro: Node,
    me: Paddle,
    cpu: Paddle,
    ball: Ball,
    /// Goals: mine (0x162) and the friend's (0x164).
    score: [u16; 2],
    /// Score shown in the corner, updated at each goal for me (elem 0x5e5).
    shown: u16,
    t: u16,
    fx: Vec<(Node, i32, i32)>,
    charged: Node,
    cpu_aim: f32,
    cpu_think: u8,
    rng: u32,
}

impl Arcade01 {
    pub fn new(seed: u32) -> Self {
        Arcade01 {
            st: St::Intro,
            intro: Node::new(INTRO),
            me: Paddle::new(MY_Y, -1.0),
            cpu: Paddle::new(CPU_Y, 1.0),
            ball: Ball::default(),
            score: [0; 2],
            shown: 0,
            t: TICKS,
            fx: Vec::new(),
            charged: Node::new(CHARGED),
            cpu_aim: 64.0,
            cpu_think: 0,
            rng: seed | 1,
        }
    }

    /// `FUN_1007afb8`: 0 win, 1 lose, 2 draw (500 / 100 / 250 points).
    pub fn result(&self) -> u8 {
        match self.score[0].cmp(&self.score[1]) {
            std::cmp::Ordering::Greater => 0,
            std::cmp::Ordering::Less => 1,
            std::cmp::Ordering::Equal => 2,
        }
    }

    fn serve(&mut self, me: bool) {
        let ((x, y), a) = if me { SERVE_ME } else { SERVE_CPU };
        self.ball = Ball { pos: (x, y), prev: (x, y), v: launch(a, MIN_SPEED), on: true, fade: None };
    }

    /// The friend's paddle, offline: follow the ball when it comes up, else drift back to the
    /// middle; lunge (one frame of charge, then release) when the ball is close in front.
    fn cpu_input(&mut self) -> (f32, bool) {
        let b = &self.ball;
        let coming = b.on && b.v.1 < 0.0;
        if self.cpu_think == 0 {
            self.cpu_think = CPU_THINK;
            let miss = (rand(&mut self.rng) % 17) as f32 - 8.0;
            self.cpu_aim = if coming && b.pos.1 < 60.0 { b.pos.0 + miss } else { 64.0 };
        }
        self.cpu_think -= 1;
        let d = self.cpu_aim - self.cpu.x;
        let dx = if d.abs() < 1.0 { 0.0 } else { d.clamp(-CPU_STEP, CPU_STEP) };
        let cp = self.cpu.pos();
        let close = coming && b.pos.1 - cp.1 < 40.0 && (b.pos.0 - cp.0).abs() < 20.0;
        (dx, close && self.cpu.charge == 0.0)
    }

    fn spark(&mut self, kind: usize, at: (f32, f32), sounds: &mut Vec<(u16, u8)>) {
        sounds.push((HIT_SND[kind], 100));
        let script = if kind == 2 { WALL_FX } else { HIT_FX };
        self.fx.push((Node::new(script), at.0 as i32, at.1 as i32));
    }

    /// Ball, collisions and goals (host side of state 3000 / 9000).
    fn physics(&mut self, goals: bool, sounds: &mut Vec<(u16, u8)>) -> Option<bool> {
        if let Some(at) = self.ball.step() {
            self.spark(2, at, sounds);
        }
        for i in 0..2 {
            let p = if i == 0 { &self.me } else { &self.cpu };
            let smash = p.smashing as usize;
            if let Some(at) = self.ball.hit(p) {
                self.spark(smash, at, sounds);
            }
        }
        if let Some(at) = self.ball.corners() {
            self.spark(2, at, sounds);
        }
        let y = self.ball.pos.1;
        match () {
            _ if !goals || !self.ball.on => None,
            _ if y >= MY_GOAL => Some(false),
            _ if y < CPU_GOAL => Some(true),
            _ => None,
        }
    }

    fn paddles(&mut self, pressed: u8, held: u8) {
        let mut dx = 0.0;
        if held & A != 0 {
            dx -= STEP;
        }
        if held & C != 0 {
            dx += STEP;
        }
        if pressed & UP != 0 {
            dx += STEP;
        } else if pressed & DOWN != 0 {
            dx -= STEP;
        }
        self.me.update(dx, held & (B | PUSH) != 0);
        let (cdx, charge) = self.cpu_input();
        self.cpu.update(cdx, charge);
    }
}

impl MiniGame for Arcade01 {
    /// Touch: C moves right, so swipe right does nothing; holding B charges the smash.
    fn touch_map(&self) -> crate::touch::Map {
        crate::touch::Map { back: 0, ..crate::touch::Map::DEFAULT }
    }
    fn update(&mut self, pressed: u8, held: u8, data: &Data, sounds: &mut Vec<(u16, u8)>) -> Option<u8> {
        for (n, ..) in &mut self.fx {
            n.tick(&data.motions);
        }
        self.fx.retain(|(n, ..)| !n.is_finished());
        self.charged.tick(&data.motions);
        // Paddles and the clock run in states 3000..9000 (play and goal banners).
        if matches!(self.st, St::Play | St::Goal { .. }) {
            self.paddles(pressed, held);
            self.t = self.t.saturating_sub(1);
        }
        match &mut self.st {
            St::Intro => {
                self.intro.tick(&data.motions);
                self.intro.take_sounds(sounds);
                if self.intro.is_finished() {
                    self.st = St::Count(COUNT);
                }
            }
            St::Count(n) => {
                if *n % 20 == 0 {
                    sounds.push((1, 100));
                }
                *n -= 1;
                if *n == 0 {
                    self.st = St::Serve(0, true);
                }
            }
            St::Serve(n, me) => {
                *n += 1;
                if *n >= 10 {
                    let me = *me;
                    self.serve(me);
                    self.st = St::Play;
                }
            }
            St::Play if self.t == 0 => {
                self.ball.fade = Some((20, len(self.ball.v)));
                self.st = St::End;
            }
            St::Play => {
                if let Some(me) = self.physics(true, sounds) {
                    self.ball.on = false;
                    self.score[!me as usize] += 1;
                    let fx = if me {
                        self.shown = self.score[0];
                        sounds.push((0x13, 100));
                        None
                    } else {
                        Some(Node::new(CONCEDED_FX))
                    };
                    self.st = St::Goal { me, banner: Node::new(if me { SCORED } else { CONCEDED }), fx };
                }
            }
            St::Goal { me, banner, fx } => {
                banner.tick(&data.motions);
                banner.take_sounds(sounds);
                if let Some(f) = fx {
                    f.tick(&data.motions);
                }
                if banner.is_finished() && fx.as_ref().is_none_or(|f| f.is_finished()) {
                    // The side that conceded serves.
                    self.st = St::Serve(0, !*me);
                }
            }
            St::End => {
                self.physics(false, sounds);
                if self.ball.fade.is_some_and(|(n, _)| n == 0) {
                    let r = self.result() as usize;
                    self.st = St::Panel(Panel::new(PANEL[r], PANEL_PET[r]));
                }
            }
            St::Panel(p) => {
                if p.update(pressed, data, sounds) {
                    return Some(self.result());
                }
            }
        }
        None
    }

    fn draws(&self, out: &mut Vec<Draw>) {
        if let St::Intro = self.st {
            self.intro.draws(64, 64, out);
            return;
        }
        out.push(sprite(BG, 0, 64, 64));
        // Only my half is on this screen; the friend's half is the other device's.
        // ponytail: offline the CPU half stays off-screen like the friend's device; no split view.
        for &(x, y) in &CORNERS[..2] {
            out.push(sprite(CORNER, 0, x as i32 + 16, y as i32 + 3));
        }
        let (bx, by) = (self.ball.pos.0 as i32, self.ball.pos.1 as i32);
        match &self.st {
            St::Serve(n, true) => {
                // From (x + 48, y − 16) to the serve spot, fading in over 5 frames.
                let ((x, y), _) = SERVE_ME;
                let k = 10 - *n as i32;
                let alpha = (*n as u32 * 64 / 5).min(64) as u8;
                out.push(Draw { alpha, ..sprite(BALL, 0, x as i32 + 48 * k / 10, y as i32 - 16 * k / 10) });
            }
            St::Play | St::End if self.ball.on => {
                let alpha = self.ball.fade.map_or(64, |(n, _)| (n as u32 * 64 / 20) as u8);
                out.push(Draw { alpha, ..sprite(BALL, 0, bx, by) });
            }
            _ => {}
        }
        let (px, py) = self.me.pos();
        out.push(sprite(PADDLE, 0, px as i32, py as i32));
        if self.me.full && self.me.charge > 0.0 {
            self.charged.draws(px as i32, py as i32, out);
        }
        for (n, x, y) in &self.fx {
            n.draws(*x, *y, out);
        }
        match &self.st {
            St::Count(n) => out.extend(count_hud(*n)),
            St::Goal { banner, fx, .. } => {
                if let Some(f) = fx {
                    f.draws(64, 96, out);
                }
                banner.draws(64, 64, out);
            }
            St::Panel(p) => p.draws(out),
            _ => {}
        }
    }

    fn hud(&self) -> Vec<Hud> {
        match self.st {
            St::Intro => Vec::new(),
            St::Panel(ref p) => vec![
                Hud::Number { n: self.score[0] as u32, x: 42 + p.dx(), y: 48 },
                Hud::Number { n: self.score[1] as u32, x: 86 + p.dx(), y: 48 },
            ],
            _ => {
                let mut h = vec![
                    Hud::Sprite { id: 0x80e, frame: 0, x: 15, y: 11 },
                    Hud::Number { n: self.shown as u32, x: 14, y: 11 },
                ];
                let s = (self.t as u32).div_ceil(20);
                if s < 10 {
                    h.extend([Hud::Sprite { id: 0x80f, frame: 0, x: 120, y: 11 }, Hud::Number { n: s, x: 120, y: 11 }]);
                }
                h
            }
        }
    }

    fn result_script(&self, result: u8) -> u16 {
        RESULTS[result as usize % 3]
    }

    fn uses_c(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn play() -> Arcade01 {
        let mut g = Arcade01::new(7);
        g.st = St::Play;
        g
    }

    #[test]
    fn result_by_goals() {
        let mut g = play();
        for (s, r) in [([3, 1], 0), ([1, 2], 1), ([2, 2], 2)] {
            g.score = s;
            assert_eq!(g.result(), r);
        }
    }

    #[test]
    fn ball_keeps_a_minimum_speed_and_slows_towards_it() {
        let mut b = Ball { pos: (64.0, 50.0), prev: (64.0, 50.0), v: (0.0, 2.0), on: true, fade: None };
        b.step();
        assert!((len(b.v) - MIN_SPEED).abs() < 1e-4);
        b.v = (0.0, 10.0);
        b.step();
        assert!((len(b.v) - 9.9).abs() < 1e-4);
        // Side walls bounce.
        b.pos = (9.0, 50.0);
        b.v = (-4.0, 0.0);
        assert!(b.step().is_some());
        assert!(b.v.0 > 0.0 && b.pos.0 == R);
    }

    #[test]
    fn a_lunge_sends_the_ball_back_faster() {
        let mut p = Paddle::new(MY_Y, -1.0);
        for _ in 0..7 {
            p.update(0.0, true);
        }
        assert!(p.full);
        p.update(0.0, false);
        assert!(p.smashing);
        // Ball coming straight down onto the lunging paddle.
        let mut b = Ball { pos: (64.0, 60.0), prev: (64.0, 60.0), v: (0.0, 4.0), on: true, fade: None };
        let mut hit = false;
        for _ in 0..10 {
            p.update(0.0, false);
            b.step();
            hit |= b.hit(&p).is_some();
            if hit {
                break;
            }
        }
        assert!(hit && b.v.1 < -MIN_SPEED, "{:?}", b.v);
    }

    #[test]
    fn cpu_follows_the_ball_and_returns_it() {
        let d = crate::evolve::tests::data();
        let mut g = play();
        g.ball = Ball { pos: (100.0, 40.0), prev: (100.0, 40.0), v: (0.0, -4.0), on: true, fade: None };
        let mut returned = false;
        for _ in 0..60 {
            g.update(0, 0, &d, &mut Vec::new());
            if g.ball.v.1 > 0.0 {
                returned = true;
                break;
            }
        }
        assert!(returned, "cpu at {} ball {:?}", g.cpu.x, g.ball.pos);
        assert!(g.cpu.x > 80.0);
    }

    #[test]
    fn goals_count_and_the_loser_serves() {
        let d = crate::evolve::tests::data();
        let mut g = play();
        // Through the friend's goal mouth, out of the CPU's reach.
        g.cpu.x = 13.0;
        g.cpu_think = 200;
        g.ball = Ball { pos: (64.0, -150.0), prev: (64.0, -150.0), v: (0.0, -6.0), on: true, fade: None };
        g.update(0, 0, &d, &mut Vec::new());
        assert_eq!(g.score, [0, 0]);
        g.update(0, 0, &d, &mut Vec::new());
        assert_eq!(g.score, [1, 0]);
        assert!(matches!(g.st, St::Goal { me: true, .. }));
        for _ in 0..20 {
            g.update(0, 0, &d, &mut Vec::new());
        }
        assert!(g.ball.on && g.ball.pos.1 < 0.0, "served on the friend's side");
    }

    #[test]
    fn panel_fades_slides_and_waits_before_closing() {
        let d = crate::evolve::tests::data();
        let mut p = Panel::new(PANEL[0], PANEL_PET[0]);
        let mut snd = Vec::new();
        assert_eq!(p.dx(), -128);
        let mut closed_at = None;
        for f in 1..=60 {
            if p.update(C, &d, &mut snd) {
                closed_at = Some(f);
                break;
            }
            if f == PANEL_FADE + PANEL_SLIDE {
                assert_eq!(p.dx(), 0, "slid in");
            }
        }
        assert_eq!(snd, vec![(0x35, 100)]);
        assert_eq!(closed_at, Some(PANEL_LOCK + 1), "keys ignored for 40 frames");
        let mut p = Panel::new(PANEL[0], PANEL_PET[0]);
        let n = (1..400).find(|_| p.update(0, &d, &mut snd)).unwrap();
        assert_eq!(n, PANEL_LOCK + PANEL_IDLE + 1, "closes by itself after 5 s idle");
    }

    #[test]
    fn time_up_ends_with_the_panel() {
        let d = crate::evolve::tests::data();
        let mut g = Arcade01::new(1);
        let mut r = None;
        let mut frames = 0;
        while r.is_none() && frames < 4000 {
            r = g.update(0, 0, &d, &mut Vec::new());
            frames += 1;
        }
        assert!(r.is_some() && frames > TICKS as usize, "{frames}");
        assert_eq!(g.t, 0);
    }
}
