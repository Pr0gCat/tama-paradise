//! Tama Space Arcade 02 "SHOOTING" (scene 0x86, `arcade_shoot_play` 0x1008764c; 09-arcade §3).
//!
//! Co-op over the link cable: germs pop out of 18 holes, 9 on each screen, and both players
//! share one crosshair — the host steers X, the guest steers Y, either may fire.
//! Coordinates are the host's: my holes are at y 16/48/80, the friend's at −16/−48/−80.

use super::arcade01::{count_hud, rand, Panel, COUNT};
use super::{sprite, Hud, MiniGame};
use crate::data::Data;
use crate::input::{A, B, C, DOWN, PUSH, UP};
use crate::motion::{Draw, Node};

/// Host opening (elem 0x5e8).
pub const INTRO: u16 = 0xb66;
/// ARCADE_RESULT scripts (XIP 0x10095750, host row of game 1) by result 0..2.
pub const RESULTS: [u16; 3] = [0xb2e, 0xb2d, 0xb2d];
/// 1200 ticks of one frame (60 s).
pub const TICKS: u16 = 1200;
const BG: u16 = 0x8df;
const DESK: u16 = 0x8e0;
const CROSS: u16 = 0x8e1;
const GERM: u16 = 0x8e2;
const SCORE_BOX: u16 = 0x8e3;
const KNOB: u16 = 0x8e4;
const LEVER: u16 = 0x8e5;
/// The pet at the lever (elem 0x5fa) and getting off it at the end (elem 0x5fb).
const LEVER_PET: u16 = 0xb68;
const LEVER_OFF: u16 = 0xb69;
/// Shot effect (elem 0x5ef) at the crosshair.
const SHOT: u16 = 0xb6b;
/// Lever knob x by input direction left / none / right (RW 0x18000a08), y 116.
const KNOB_X: [i32; 3] = [86, 91, 96];
/// In-game result panel (RW 0x18000000 game 1) by result.
const PANEL: [u16; 3] = [0x8cf, 0x8ce, 0x8cd];
const PANEL_PET: [u16; 3] = [0xb31, 0xb30, 0xb2f];
/// Germ kinds (XIP 0x1009df32): frames on screen after popping up, and points.
const STAY: [u16; 3] = [60, 50, 40];
const POINTS: [u16; 3] = [1, 2, 3];
/// Hole centres (elems 0x5f1–0x5f9); holes 9–17 are the same on the friend's screen (y negated).
const HOLE_X: [f32; 3] = [24.0, 64.0, 104.0];
const HOLE_Y: [f32; 3] = [16.0, 48.0, 80.0];
/// Crosshair step per frame (±0x400 × 0x100) and range (`FUN_10031a2c`).
const STEP: f32 = 4.0;
/// A shot hits a germ within 16 px of the crosshair (`FUN_10063ce4`).
const HIT_R2: f32 = 256.0;
/// Germ pop-up 0.25 s, retreat 0.15 s, hit: shake 0.6 s then fade 0.6 s (frames).
const POP: u16 = 5;
const HIDE: u16 = 3;
const HIT_SHOW: u16 = 12;
/// CPU partner: frames from lining up to firing.
// ponytail: fixed CPU reaction; tune here.
const CPU_REACT: u8 = 5;

fn hole(i: usize) -> (f32, f32) {
    let y = HOLE_Y[i % 9 / 3];
    (HOLE_X[i % 3], if i < 9 { y } else { -y })
}

#[derive(Clone, Copy, Debug)]
struct Germ {
    kind: u8,
    /// Frames since it started popping up.
    t: u16,
    stay: u16,
    /// Frames since hit / since it started to hide.
    hit: Option<u16>,
    hide: Option<u16>,
}

impl Germ {
    fn hittable(&self) -> bool {
        self.t >= POP && self.hit.is_none() && self.hide.is_none()
    }

    /// `FUN_10063eb0`: count the stay down once up, then hide. Returns false when gone.
    fn tick(&mut self) -> bool {
        self.t += 1;
        if let Some(h) = &mut self.hit {
            *h += 1;
            return *h < 2 * HIT_SHOW;
        }
        if let Some(h) = &mut self.hide {
            *h += 1;
            return *h < HIDE;
        }
        if self.t > POP {
            if self.stay == 0 {
                self.hide = Some(0);
            }
            self.stay = self.stay.saturating_sub(1);
        }
        true
    }

    /// Sprite 0x8e2 frame (palette = kind, `FUN_10063abc`): 0 up, 1 once hit (`FUN_10063ce4`),
    /// 2 when the shake ends and it fades (`FUN_10063eb0`).
    fn frame(&self) -> u16 {
        match self.hit {
            None => 0,
            Some(h) if h <= HIT_SHOW => 1,
            Some(_) => 2,
        }
    }

    /// Pop-up / hide drawn as a fade (the original scales; draws here have no scale).
    fn alpha(&self) -> u8 {
        let a = match (self.hit, self.hide) {
            (Some(h), _) if h > HIT_SHOW => 64 - 64 * (h - HIT_SHOW) / HIT_SHOW,
            (Some(_), _) => 64,
            (_, Some(h)) => 64 - 64 * h / HIDE,
            _ => (64 * self.t / POP).min(64),
        };
        a as u8
    }
}

/// One side's lever (`FUN_1001ee44`): after a shot it is stuck for 4 frames; the partner's shot
/// locks it for 8 (`FUN_1001ee14`).
#[derive(Clone, Copy, Debug, Default)]
struct Lever {
    dir: i8,
    freeze: u8,
    lock: u8,
    fire: bool,
}

impl Lever {
    fn input(&mut self, minus: bool, plus: bool, dial: i8, fire: bool) -> f32 {
        self.fire = false;
        self.dir = 0;
        if self.freeze > 0 {
            self.freeze -= 1;
        } else if self.lock > 0 {
            self.lock -= 1;
        } else if fire {
            (self.fire, self.freeze) = (true, 4);
        } else {
            self.dir = (plus as i8 - minus as i8 + dial).signum();
        }
        self.dir as f32 * STEP
    }

    fn partner_fired(&mut self) {
        self.lock = if self.lock == 0 { 8 } else { 4 };
    }

    fn busy(&self) -> bool {
        self.freeze > 0 || self.lock > 0
    }
}

enum St {
    Intro,
    Count(u8),
    Play,
    /// Time is up: germs hide, the pet gets off the lever.
    End,
    Panel(Panel),
}

pub struct Arcade02 {
    st: St,
    intro: Node,
    pet: Node,
    pattern: Vec<[u16; 2]>,
    next: usize,
    /// Frames until the next spawn row (16.16 accumulator 0x38 in the firmware).
    acc: i32,
    germs: [Option<Germ>; 18],
    cross: (f32, f32),
    me: Lever,
    cpu: Lever,
    /// CPU partner: germ it steers to, frames lined up.
    cpu_target: Option<usize>,
    cpu_wait: u8,
    score: u16,
    t: u16,
    shots: Vec<(Node, i32, i32)>,
    rng: u32,
}

impl Arcade02 {
    /// `pattern` = one of the three spawn tables (XIP 0x1009c4ec): `[side << 14 | kind, delay]`.
    pub fn new(pattern: Vec<[u16; 2]>, seed: u32) -> Self {
        Arcade02 {
            st: St::Intro,
            intro: Node::new(INTRO),
            pet: Node::new(LEVER_PET),
            pattern,
            next: 0,
            acc: 0,
            germs: [None; 18],
            cross: (64.0, 48.0),
            me: Lever::default(),
            cpu: Lever::default(),
            cpu_target: None,
            cpu_wait: 0,
            score: 0,
            t: TICKS,
            shots: Vec::new(),
            rng: seed | 1,
        }
    }

    /// `FUN_1007b00c`: ≤ 10 → 0, ≤ 50 → 1, else 2 (100 / 250 / 500 points).
    pub fn result(&self) -> u8 {
        match self.score {
            0..=10 => 0,
            11..=50 => 1,
            _ => 2,
        }
    }

    /// `FUN_10031f10`: rows due now pop out in a random free hole of their side (1 mine,
    /// 2 the friend's, 3 either); a row with no free hole is dropped.
    fn spawn(&mut self) {
        if self.acc >= 1 {
            self.acc -= 1;
        }
        while self.acc < 1 && self.next < self.pattern.len() {
            let [code, delay] = self.pattern[self.next];
            self.next += 1;
            let holes = match code >> 14 {
                3 => 0..18,
                1 => 0..9,
                _ => 9..18,
            };
            let free: Vec<usize> = holes.filter(|&i| self.germs[i].is_none()).collect();
            if !free.is_empty() {
                let i = free[rand(&mut self.rng) as usize % free.len()];
                let kind = (code & 0x3fff).min(2) as u8;
                self.germs[i] = Some(Germ { kind, t: 0, stay: STAY[kind as usize], hit: None, hide: None });
            }
            self.acc += delay as i32;
        }
    }

    /// The friend offline: steer Y to the germ nearest the crosshair, fire once lined up.
    // ponytail: the player keeps the host's X and the CPU plays the guest's Y (and fires), so the
    // one-axis-each co-op survives; one player on both axes (dial for Y) would leave the friend idle.
    fn cpu_input(&mut self) -> (bool, bool, bool) {
        let alive = |g: &Option<Germ>| g.is_some_and(|g| g.hittable());
        if !self.cpu_target.is_some_and(|i| alive(&self.germs[i])) {
            let c = self.cross;
            self.cpu_target = (0..18).filter(|&i| alive(&self.germs[i])).min_by_key(|&i| {
                let (x, y) = hole(i);
                ((y - c.1).abs() + (x - c.0).abs() / 2.0) as i32
            });
        }
        let Some(i) = self.cpu_target else { return (false, false, false) };
        let (x, y) = hole(i);
        let dy = y - self.cross.1;
        let on = (x - self.cross.0).powi(2) + dy * dy <= HIT_R2;
        self.cpu_wait = if on { self.cpu_wait.saturating_add(1) } else { 0 };
        (dy <= -2.0, dy >= 2.0, on && self.cpu_wait >= CPU_REACT)
    }

    fn play(&mut self, pressed: u8, held: u8, sounds: &mut Vec<(u16, u8)>) {
        let dial = (pressed & UP != 0) as i8 - (pressed & DOWN != 0) as i8;
        let dx = self.me.input(held & A != 0, held & C != 0, dial, pressed & (B | PUSH) != 0);
        let (up, down, fire) = self.cpu_input();
        let dy = self.cpu.input(up, down, 0, fire);
        self.cross = ((self.cross.0 + dx).clamp(16.0, 112.0), (self.cross.1 + dy).clamp(-80.0, 80.0));
        self.spawn();
        for g in &mut self.germs {
            if g.as_mut().is_some_and(|g| !g.tick()) {
                *g = None;
            }
        }
        if !(self.me.fire || self.cpu.fire) {
            return;
        }
        // One shot a frame; whoever did not fire is locked for a moment.
        match (self.me.fire, self.cpu.fire) {
            (true, false) => self.cpu.partner_fired(),
            (false, true) => self.me.partner_fired(),
            _ => {}
        }
        sounds.push((0x14, 100));
        self.shots.push((Node::new(SHOT), self.cross.0 as i32, self.cross.1 as i32));
        let c = self.cross;
        let hit = (0..18).find(|&i| {
            let (x, y) = hole(i);
            self.germs[i].is_some_and(|g| g.hittable()) && (x - c.0).powi(2) + (y - c.1).powi(2) <= HIT_R2
        });
        if let Some(g) = hit.and_then(|i| self.germs[i].as_mut()) {
            g.hit = Some(0);
            self.score += POINTS[g.kind as usize];
            self.acc -= 6;
            sounds.push((52, 100));
        }
    }

    /// Screen offset: the friend's screen is shown while the crosshair is on it, as if the two
    /// devices lay head to head.
    // ponytail: the friend's half is drawn without its desk details and not rotated.
    fn view(&self) -> i32 {
        if self.cross.1 < 0.0 {
            128
        } else {
            0
        }
    }
}

impl MiniGame for Arcade02 {
    /// Touch: C moves right, so swipe left does nothing.
    fn touch_map(&self) -> crate::touch::Map {
        crate::touch::Map { back: 0, ..crate::touch::Map::DEFAULT }
    }
    fn update(&mut self, pressed: u8, held: u8, data: &Data, sounds: &mut Vec<(u16, u8)>) -> Option<u8> {
        for (n, ..) in &mut self.shots {
            n.tick(&data.motions);
        }
        self.shots.retain(|(n, ..)| !n.is_finished());
        self.pet.tick(&data.motions);
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
                    self.st = St::Play;
                }
            }
            St::Play => {
                self.play(pressed, held, sounds);
                self.t = self.t.saturating_sub(1);
                if self.t == 0 {
                    // `FUN_10064068` / `FUN_1001ecb8`.
                    for g in self.germs.iter_mut().flatten() {
                        if g.hit.is_none() && g.hide.is_none() {
                            g.hide = Some(0);
                        }
                    }
                    self.pet = Node::new(LEVER_OFF);
                    self.st = St::End;
                }
            }
            St::End => {
                for g in &mut self.germs {
                    if g.as_mut().is_some_and(|g| !g.tick()) {
                        *g = None;
                    }
                }
                if self.germs.iter().all(Option::is_none) && self.pet.is_finished() {
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
        let v = self.view();
        out.push(sprite(BG, 0, 64, 64));
        for (i, g) in self.germs.iter().enumerate() {
            let Some(g) = g else { continue };
            let (x, y) = hole(i);
            let frame = g.frame();
            let at = (x as i32, y as i32 + v);
            if (0..128).contains(&at.1) {
                out.push(Draw { alpha: g.alpha(), palette: g.kind, ..sprite(GERM, frame, at.0, at.1) });
            }
        }
        let c = (self.cross.0 as i32, self.cross.1 as i32 + v);
        out.push(Draw { alpha: 57, ..sprite(CROSS, 0, c.0, c.1) });
        for (n, x, y) in &self.shots {
            n.draws(*x, *y + v, out);
        }
        if v == 0 {
            out.push(sprite(DESK, 0, 64, 128));
            out.push(sprite(SCORE_BOX, 0, 31, 113));
            out.push(sprite(LEVER, 0, 91, 116));
            out.push(sprite(KNOB, self.me.busy() as u16, KNOB_X[(self.me.dir + 1) as usize], 116));
            self.pet.draws(91, 116, out);
        } else {
            out.push(Draw { flip: true, ..sprite(DESK, 0, 64, 0) });
        }
        match &self.st {
            St::Count(n) => out.extend(count_hud(*n)),
            St::Panel(p) => p.draws(out),
            _ => {}
        }
    }

    fn hud(&self) -> Vec<Hud> {
        match self.st {
            St::Intro => Vec::new(),
            St::Panel(ref p) => vec![
                Hud::Sprite { id: 0x8cb, frame: 0, x: 64 + p.dx(), y: 44 },
                Hud::Number { n: self.score as u32, x: 64 + p.dx(), y: 44 },
            ],
            _ => {
                let mut h = Vec::new();
                if self.view() == 0 {
                    h.push(Hud::Number { n: self.score as u32, x: 40, y: 114 });
                }
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

    fn playing(pattern: Vec<[u16; 2]>) -> Arcade02 {
        let mut g = Arcade02::new(pattern, 3);
        g.st = St::Play;
        g
    }

    #[test]
    fn result_by_score() {
        let mut g = playing(Vec::new());
        for (s, r) in [(0, 0), (10, 0), (11, 1), (50, 1), (51, 2), (90, 2)] {
            g.score = s;
            assert_eq!(g.result(), r);
        }
    }

    #[test]
    fn rows_spawn_on_their_side_after_their_delay() {
        let mut g = playing(vec![[1 << 14, 3], [2 << 14 | 2, 0], [1 << 14 | 1, 5]]);
        g.spawn();
        let side = |g: &Arcade02, r: std::ops::Range<usize>| r.filter(|&i| g.germs[i].is_some()).count();
        assert_eq!((side(&g, 0..9), side(&g, 9..18)), (1, 0));
        g.spawn();
        g.spawn();
        assert_eq!(side(&g, 9..18), 0);
        g.spawn();
        // Delay 3, then the friend's row and (delay 0) my next row at once.
        assert_eq!((side(&g, 0..9), side(&g, 9..18)), (2, 1));
        let kinds: Vec<u8> = g.germs.iter().flatten().map(|g| g.kind).collect();
        assert!(kinds.contains(&2) && kinds.contains(&1));
    }

    #[test]
    fn a_shot_on_a_germ_scores_its_points() {
        let d = crate::evolve::tests::data();
        let mut g = playing(Vec::new());
        g.germs[4] = Some(Germ { kind: 2, t: POP, stay: 40, hit: None, hide: None });
        g.cross = (66.0, 46.0);
        g.update(B, 0, &d, &mut Vec::new());
        assert_eq!(g.score, 3);
        assert!(g.germs[4].unwrap().hit.is_some());
        assert_eq!(g.cpu.lock, 8, "the partner waits after my shot");
        // Stuck for 4 frames after a shot.
        g.update(0, C, &d, &mut Vec::new());
        assert_eq!(g.cross.0, 66.0);
    }

    #[test]
    fn cpu_partner_steers_y_and_fires() {
        let d = crate::evolve::tests::data();
        let mut g = playing(Vec::new());
        // A germ on the friend's screen in the column of the crosshair.
        g.germs[13] = Some(Germ { kind: 0, t: POP, stay: 600, hit: None, hide: None });
        g.cross = (64.0, 48.0);
        for _ in 0..60 {
            g.update(0, 0, &d, &mut Vec::new());
        }
        assert_eq!(g.score, 1, "cross at {:?}", g.cross);
        assert!(g.cross.1 < 0.0 && g.view() == 128);
    }

    #[test]
    fn germ_frame_follows_the_hit() {
        let mut g = Germ { kind: 2, t: POP, stay: 10, hit: None, hide: None };
        assert_eq!(g.frame(), 0);
        g.hit = Some(0);
        assert_eq!(g.frame(), 1, "shaking");
        g.hit = Some(HIT_SHOW + 1);
        assert_eq!(g.frame(), 2, "fading");
    }

    #[test]
    fn time_up_ends_with_the_panel() {
        let d = crate::evolve::tests::data();
        let mut g = Arcade02::new(vec![[3 << 14, 20]; 170], 9);
        let mut r = None;
        let mut frames = 0;
        while r.is_none() && frames < 3000 {
            r = g.update(0, 0, &d, &mut Vec::new());
            frames += 1;
        }
        assert!(frames > TICKS as usize && r.is_some(), "{frames}");
    }
}
