//! Lab game 04: curling breakout, find the character hidden under one of 15 ice blocks
//! (09-minigames-lab-b §2.1). Positions are 16.16 fixed point like the firmware.

use super::{atan2_16, cos16, sin16, Hud, MiniGame};
use crate::data::{Data, MiniData};
use crate::input::{A, B, DOWN, UP};
use crate::motion::{Draw, Look, Node, KEEP};

const ONE: i32 = 0x10000;
/// Ball start (`labo04_ball_create` 0x1006fb74) and walls x ∈ [12, 116], top 12, lost below 158.
const BALL0: (i32, i32) = (88, 124);
const WALL: (i32, i32, i32) = (12, 116, 12);
const LOST_Y: i32 = 158;
const SPEED: i32 = 3 * ONE;
/// Deceleration once the prize is hit or time is up: 0.15 px/f².
const DECEL: i32 = 0x2666;
/// Paddle (elements 0x510/0x512): y 124, x ∈ [16, 112], start 88; half width 14 + 6 px slack.
const PADDLE_Y: i32 = 124;
const PADDLE_X: (i32, i32) = (16, 112);
const PADDLE_HALF: i32 = 14;
// ponytail: the firmware compares 16.16 speed with raw −4/5, so holding A/B accelerates 1 px/f
// without limit (09-minigames-lab-b §5.1). Capped at the apparent intent of 5 px/f.
const PADDLE_CAP: i32 = 5 * ONE;
/// `dial_delta() × 5.0` per raw count; one UP/DOWN pulse = 3 raw counts.
const DIAL_STEP: i32 = 15 * ONE;
/// Launch angles (f32 turns at XIP 0x1009bb50): up-left, up, up-right.
const ANGLES: [u32; 3] = [0x6000, 0x4000, 0x2000];
/// Frames per round by level (XIP 0x1009c5a4).
const ROUND_FRAMES: [u16; 3] = [1200, 1000, 800];
const ROUNDS: u8 = 5;
/// Blocks appear 0.6 s after the round is set up; the ball leaves after 22 frames.
const APPEAR: u16 = 12;
const SETUP: u16 = 22;
/// Prize character: shown 11 frames, then the reveal pose for 30 frames.
const PRIZE_SHOW: u16 = 10;
const REVEAL: u16 = 30;
const PRIZE_ANIM: [u16; 2] = [30, 33];
const BG: u16 = 2195;
/// Sprites and their y origin (drawn centre = logic y + origin, 09-minigames-lab-b C8).
const BALL: (u16, i32) = (2196, -16);
const PADDLE: (u16, i32) = (2197, -8);
const BRICK: (u16, i32) = (2198, -16);
const BREAK: u16 = 0xa8a;
const CLOCK: u16 = 2063;
const RESULTS: [u16; 4] = [0xa87, 0xa85, 0xa7f, 0xa78];

fn brick_xy(i: usize) -> (i32, i32) {
    (12 + 26 * (i % 5) as i32, 32 + 24 * (i / 5) as i32)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    /// Round set-up (states 1000–0x3eb), frames so far.
    Setup(u16),
    Play,
    /// Prize revealed: wait for the ball to stop (0x44c).
    WaitStop,
    /// Between rounds (0x44d) or before the result (3000), frames left.
    Next(u16),
    Over(u16),
}

#[derive(Clone, Copy, Debug, Default)]
struct Brick {
    /// Frames until it can be hit; None = broken.
    appear: Option<u16>,
    /// Breaking fade-out frame.
    fade: Option<u16>,
}

pub struct Lab04 {
    level: usize,
    launch: [u8; 5],
    prize: [u8; 5],
    chr: u16,
    state: State,
    round: u8,
    /// Round timer and the "running" flag (`+0x35`): off when time is up or the prize shows.
    t: u16,
    timing: bool,
    clock: Option<u16>,
    ball: (i32, i32),
    prev: (i32, i32),
    speed: i32,
    angle: u32,
    hit: bool,
    ball_flip: bool,
    paddle: i32,
    paddle_v: i32,
    paddle_flip: bool,
    bricks: [Brick; 15],
    /// Frames the prize character (0x513) and its reveal (0x514) have been shown.
    shown: Option<u16>,
    revealed: Option<u16>,
    effects: Vec<Node>,
}

impl Lab04 {
    /// `launch` = lab04_launch[level*3 + plays % 3], `prize` = lab04_prize_brick[level*5 + plays % 5],
    /// `chr` = lab04_prize_chr[plays % 49].
    pub fn new(level: usize, launch: [u8; 5], prize: [u8; 5], chr: u16) -> Self {
        let mut g = Lab04 {
            level: level.min(2),
            launch,
            prize,
            chr,
            state: State::Setup(0),
            round: 0,
            t: 0,
            timing: true,
            clock: None,
            ball: (0, 0),
            prev: (0, 0),
            speed: 0,
            angle: 0,
            hit: false,
            ball_flip: false,
            paddle: 0,
            paddle_v: 0,
            paddle_flip: false,
            bricks: [Brick::default(); 15],
            shown: None,
            revealed: None,
            effects: Vec::new(),
        };
        g.new_round();
        g
    }

    /// States 1000/0x3e9: ball and paddle back to the start, 15 fresh blocks.
    fn new_round(&mut self) {
        let r = self.round as usize;
        self.ball = (BALL0.0 * ONE, BALL0.1 * ONE);
        self.prev = self.ball;
        self.speed = 0;
        self.angle = ANGLES[self.launch[r] as usize % 3];
        self.hit = false;
        self.paddle = 88 * ONE;
        self.bricks = [Brick { appear: Some(APPEAR), fade: None }; 15];
        self.shown = None;
        self.revealed = None;
        self.state = State::Setup(0);
    }

    pub fn result(&self) -> u8 {
        match self.round {
            0 => 0,
            1..=2 => 1,
            3..=4 => 2,
            _ => 3,
        }
    }

    fn snd(&self, id: u16, sounds: &mut Vec<(u16, u8)>) {
        sounds.push((id, 100));
    }

    /// `labo04_play` 0x100788d0.
    fn play(&mut self, pressed: u8, held: u8, sounds: &mut Vec<(u16, u8)>) {
        let limit = ROUND_FRAMES[self.level];
        if self.timing {
            self.t += 1;
        }
        if self.t >= limit {
            self.timing = false;
        }
        if self.speed <= 0 && !self.hit {
            self.snd(0xd, sounds);
            self.state = State::Over(40);
        }
        if self.timing {
            self.paddle_input(pressed, held);
        }
        let s = (limit - self.t) / 20;
        if s < 10 {
            self.clock = Some(s);
        }
    }

    /// `labo04_paddle_input` 0x10027c14.
    fn paddle_input(&mut self, pressed: u8, held: u8) {
        let (a, b) = (held & A != 0, held & B != 0);
        if a {
            self.paddle_v = self.paddle_v.min(0) - ONE;
        }
        if b {
            self.paddle_v = self.paddle_v.max(0) + ONE;
        }
        if a == b {
            self.paddle_v = 0;
        }
        self.paddle_v = self.paddle_v.clamp(-PADDLE_CAP, PADDLE_CAP);
        let dial = (pressed & UP != 0) as i32 - (pressed & DOWN != 0) as i32;
        let d = if dial != 0 { dial * DIAL_STEP } else { self.paddle_v };
        self.paddle = (self.paddle + d).clamp(PADDLE_X.0 * ONE, PADDLE_X.1 * ONE);
        if d != 0 {
            self.paddle_flip = d > 0;
        }
    }

    /// `labo04_paddle_cb` 0x10080d20: runs before the ball moves, on last frame's motion.
    fn paddle_hit(&mut self, m: &MiniData, sounds: &mut Vec<(u16, u8)>) {
        let (c, s) = (cos16(m, self.angle), sin16(m, self.angle));
        let (py, line) = (PADDLE_Y * ONE, (PADDLE_Y + 2) * ONE);
        if s >= 0 || self.prev.1 > py + 12 * ONE || self.ball.1 < line {
            return;
        }
        let dy = (self.ball.1 - self.prev.1) as i64;
        let t = if dy == 0 { 0 } else { (((line - self.prev.1) as i64) << 16) / dy };
        // The firmware steps along the unit vector, not Δx, so hx stays near prev x.
        let hx = self.prev.0 + ((t * c as i64) >> 16) as i32;
        let (left, right) = (self.paddle - PADDLE_HALF * ONE, self.paddle + PADDLE_HALF * ONE);
        let near = hx.clamp(left, right);
        let (mut ns, mut nc) = (s, c);
        if ((hx - near) as i64).pow(2) >> 16 <= 36 * ONE as i64 {
            self.ball.0 = hx;
            nc = if near == left {
                -c.abs()
            } else if near == right {
                c.abs()
            } else {
                c + ((((hx - self.paddle) as i64) << 16) / (28 * ONE as i64)) as i32
            };
            ns = s.abs();
            self.snd(0, sounds);
        }
        self.angle = atan2_16(m, ns, nc);
    }

    /// `labo04_ball_cb` 0x1006bd24.
    fn ball_move(&mut self, m: &MiniData, sounds: &mut Vec<(u16, u8)>) {
        if self.hit || !self.timing {
            self.speed = (self.speed - DECEL).max(0);
        }
        let (mut c, mut s) = (cos16(m, self.angle), sin16(m, self.angle));
        self.prev = self.ball;
        let step = |v: i32| ((v as i64 * self.speed as i64) >> 16) as i32;
        let nx = (self.ball.0 + step(c)).clamp(WALL.0 * ONE, WALL.1 * ONE);
        let ny = (self.ball.1 - step(s)).max(WALL.2 * ONE);
        let quiet = self.revealed.is_some();
        if nx <= WALL.0 * ONE || nx >= WALL.1 * ONE {
            c = -c;
            if !quiet {
                self.snd(0, sounds);
            }
        }
        if ny <= WALL.2 * ONE {
            if !quiet {
                self.snd(0, sounds);
            }
            s = -s;
        }
        if ny >= LOST_Y * ONE {
            self.timing = false;
            self.speed = 0;
        }
        if c != 0 {
            self.ball_flip = c > 0;
        }
        self.angle = atan2_16(m, s, c);
        self.ball = (nx, ny);
    }

    /// `labo04_brick_cb` 0x1006c4d4 for every standing block, in order.
    fn bricks_hit(&mut self, m: &MiniData, sounds: &mut Vec<(u16, u8)>) {
        let prize = self.prize.get(self.round as usize).copied().unwrap_or(0) as usize;
        for i in 0..15 {
            if self.bricks[i].appear != Some(0) {
                continue;
            }
            let (mut c, mut s) = (cos16(m, self.angle), sin16(m, self.angle));
            let (bx, by) = brick_xy(i);
            let bits = circle_rect(bx - 10, by - 10, 20, 20, self.ball.0 >> 16, (self.ball.1 >> 16) + 12, 12);
            if bits != 0 {
                self.snd(10, sounds);
                if i + 1 == prize {
                    self.shown = Some(0);
                    self.hit = true;
                }
                self.bricks[i] = Brick { appear: None, fade: Some(0) };
                let mut fx = Node::new(BREAK);
                fx.place(bx, by);
                self.effects.push(fx);
            }
            if bits & 1 != 0 {
                c = -c.abs();
            }
            if bits & 2 != 0 {
                c = c.abs();
            }
            if bits & 4 != 0 {
                s = s.abs();
            }
            if bits & 8 != 0 {
                s = -s.abs();
            }
            self.angle = atan2_16(m, s, c);
        }
    }

    /// Prize callbacks 0x10007b84 / 0x10066440.
    fn prize_tick(&mut self, sounds: &mut Vec<(u16, u8)>) {
        if let Some(n) = self.shown {
            self.shown = Some(n + 1);
            if n + 1 > PRIZE_SHOW {
                self.snd(0x34, sounds);
                self.shown = None;
                self.revealed = Some(0);
                self.timing = false;
            }
        }
        if let Some(n) = self.revealed {
            self.revealed = Some(n + 1);
            if n + 1 == REVEAL {
                self.state = State::WaitStop;
            }
        }
    }
}

/// `FUN_1007c100`: nearest rectangle point to the circle centre; edge bits 1 left, 2 right,
/// 4 top, 8 bottom when it is closer than r (0 when the centre is inside).
fn circle_rect(x: i32, y: i32, w: i32, h: i32, cx: i32, cy: i32, r: i32) -> u8 {
    let (nx, ny) = (cx.clamp(x, x + w), cy.clamp(y, y + h));
    if (cx - nx).pow(2) + (cy - ny).pow(2) >= r * r {
        return 0;
    }
    (nx == x) as u8 | ((nx == x + w) as u8) << 1 | ((ny == y) as u8) << 2 | ((ny == y + h) as u8) << 3
}

impl MiniGame for Lab04 {
    fn update(&mut self, pressed: u8, held: u8, data: &Data, sounds: &mut Vec<(u16, u8)>) -> Option<u8> {
        match self.state {
            State::Setup(n) if n + 1 >= SETUP => {
                self.speed = SPEED;
                self.snd(9, sounds);
                self.state = State::Play;
            }
            State::Setup(n) => self.state = State::Setup(n + 1),
            State::Play => self.play(pressed, held, sounds),
            State::WaitStop if self.speed == 0 => {
                self.round += 1;
                self.t = 0;
                self.clock = None;
                self.timing = true;
                self.state = State::Next(10);
            }
            State::WaitStop => {}
            State::Next(0) if self.round < ROUNDS => self.new_round(),
            State::Next(0) => self.state = State::Over(20),
            State::Next(n) => self.state = State::Next(n - 1),
            State::Over(0) => return Some(self.result()),
            State::Over(n) => self.state = State::Over(n - 1),
        }
        // Layer update: object callbacks in creation order.
        let m = &data.mini;
        self.paddle_hit(m, sounds);
        self.ball_move(m, sounds);
        self.bricks_hit(m, sounds);
        self.prize_tick(sounds);
        for b in &mut self.bricks {
            b.appear = b.appear.map(|a| a.saturating_sub(1));
            b.fade = b.fade.map(|f| f + 1).filter(|&f| f < 6);
        }
        for fx in &mut self.effects {
            fx.tick(&data.motions);
            fx.take_sounds(sounds);
        }
        self.effects.retain(|fx| !fx.done);
        None
    }

    fn draws(&self, out: &mut Vec<Draw>) {
        let sprite = |id, x, y, flip, alpha| Draw { look: Look::Sprite { id, frame: 0 }, x, y, flip, alpha, palette: 0 };
        out.push(sprite(BG, 64, 64, false, 64));
        for (i, b) in self.bricks.iter().enumerate() {
            let (x, y) = brick_xy(i);
            let alpha = match (b.appear, b.fade) {
                (Some(a), _) if a > 2 => continue,
                (Some(a), _) => (64 - 32 * a) as u8,
                (None, Some(f)) => (64 - 64 * f / 6) as u8,
                _ => continue,
            };
            out.push(sprite(BRICK.0, x, y + BRICK.1, false, alpha));
        }
        for fx in &self.effects {
            fx.draws(0, 0, out);
        }
        let prize = self.prize[self.round.min(ROUNDS - 1) as usize] as usize;
        if let (Some(i), true) = (prize.checked_sub(1).filter(|&i| i < 15), self.shown.is_some() || self.revealed.is_some()) {
            let (x, y) = brick_xy(i);
            let anim = PRIZE_ANIM[self.revealed.is_some() as usize];
            out.push(Draw { look: Look::Chr { chr: self.chr, anim, eyes: KEEP, mouth: KEEP }, x, y, flip: false, alpha: 64, palette: 0 });
        }
        out.push(sprite(BALL.0, self.ball.0 >> 16, (self.ball.1 >> 16) + BALL.1, self.ball_flip, 64));
        out.push(sprite(PADDLE.0, self.paddle >> 16, PADDLE_Y + PADDLE.1, self.paddle_flip, 64));
    }

    fn hud(&self) -> Vec<Hud> {
        match self.clock {
            Some(s) => vec![Hud::Sprite { id: CLOCK, frame: 0, x: 120, y: 11 }, Hud::Number { n: s as u32, x: 120, y: 11 }],
            None => Vec::new(),
        }
    }

    fn result_script(&self, result: u8) -> u16 {
        RESULTS[result as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mini::tests::data;

    fn run(g: &mut Lab04, frames: usize, pressed: u8, held: u8) -> Option<u8> {
        let d = data();
        let mut r = None;
        for i in 0..frames {
            r = r.or(g.update(if i == 0 { pressed } else { 0 }, held, &d, &mut Vec::new()));
        }
        r
    }

    fn game(prize: u8) -> Lab04 {
        let mut g = Lab04::new(0, [1; 5], [prize; 5], 115);
        run(&mut g, SETUP as usize, 0, 0);
        assert_eq!((g.state, g.speed), (State::Play, SPEED));
        g
    }

    #[test]
    fn paddle_accelerates_to_the_cap() {
        let mut g = game(1);
        run(&mut g, 3, 0, B);
        assert_eq!(g.paddle, (88 + 1 + 2 + 3) * ONE);
        run(&mut g, 10, 0, B);
        assert_eq!(g.paddle, 112 * ONE, "clamped at the right edge");
        run(&mut g, 1, 0, A | B);
        assert_eq!(g.paddle_v, 0);
        run(&mut g, 1, DOWN, 0);
        assert_eq!(g.paddle, 97 * ONE);
    }

    #[test]
    fn walls_and_paddle_bounce() {
        let d = data();
        let mut g = game(1);
        // Straight down onto the paddle centre.
        (g.ball, g.prev, g.angle) = ((88 * ONE, 118 * ONE), (88 * ONE, 115 * ONE), 0xc000);
        run(&mut g, 4, 0, 0);
        assert!(sin16(&d.mini, g.angle) > 0, "bounced up");
        // Into the left wall.
        (g.ball, g.angle) = ((14 * ONE, 100 * ONE), 0x8000);
        run(&mut g, 1, 0, 0);
        assert!(cos16(&d.mini, g.angle) > 0);
    }

    #[test]
    fn missing_the_ball_ends_the_game() {
        let mut g = game(1);
        (g.ball, g.prev, g.angle) = ((20 * ONE, 150 * ONE), (20 * ONE, 147 * ONE), 0xc000);
        g.paddle = 100 * ONE;
        let r = run(&mut g, 60, 0, 0);
        assert_eq!(r, Some(0));
    }

    #[test]
    fn hitting_the_prize_clears_the_round() {
        // Block 13 = bottom row, middle; the ball goes straight up into it.
        let mut g = game(13);
        (g.ball, g.angle) = ((64 * ONE, 110 * ONE), 0x4000);
        run(&mut g, 10, 0, 0);
        assert!(g.hit && g.bricks[12].appear.is_none());
        assert!(g.bricks.iter().filter(|b| b.appear.is_some()).count() == 14);
        run(&mut g, 60, 0, 0);
        assert_eq!((g.round, g.t), (1, 0));
        assert!(matches!(g.state, State::Setup(_)));
        g.round = 5;
        assert_eq!(g.result(), 3);
    }

    #[test]
    fn time_up_stops_the_paddle() {
        let mut g = game(1);
        g.t = ROUND_FRAMES[0] - 1;
        run(&mut g, 1, 0, 0);
        assert!(!g.timing);
        assert_eq!(g.clock, Some(0));
        let x = g.paddle;
        run(&mut g, 2, 0, B);
        assert_eq!(g.paddle, x);
    }
}
