//! Play-together game 06: CATCH (09-minigames-together §2.4). Three throws; a marker sweeps the
//! gauge and A/B stops it, the zone it stops in decides the catch.

use super::{Hud, MiniGame};
use crate::data::Data;
use crate::input::{A, B};
use crate::motion::{Draw, Look, Node};

/// Scene time per frame (16.16 seconds).
const FRAME: i32 = 0xccc;
/// Time of one marker pass by throw (XIP 0x10092dc0): 0.65 / 0.5 / 0.4 s.
const SWEEP: [i32; 3] = [0xa666, 0x8000, 0x6666];
/// Marker path (elems 0x5a1 / 0x5a5) and the left edges of zones 1–3 (elems 0x5a2–0x5a4).
const LEFT: i32 = 9;
const RIGHT: i32 = 119;
const ZONES: [i32; 3] = [57, 89, 109];
/// Pet reaction by zone (XIP 0x10093e92), then the catch / fetch-the-ball follow-up.
const ZONE_ANIMS: [u16; 4] = [0xb18, 0xb14, 0xb10, 0xb0b];
const CAUGHT: u16 = 0xb1b;
const FETCH: u16 = 0xb1d;
const IDLE: u16 = 0xb0a;
const OPENING: u16 = 0xb04;
const RESULTS: [u16; 4] = [0xb24, 0xb23, 0xb22, 0xb21];
/// Pet actor (elems 0x59e…) and the gauge (elem 0x5a0) / marker (elem 0x5a1) sprites.
const PET: (i32, i32) = (64, 88);
const GAUGE: (u16, i32, i32) = (0x8c3, 64, 112);
const MARKER: (u16, i32) = (0x8c2, 104);
const BLINK: u8 = 16;
/// `idle_exceeds(10)`: 10 s without input puts the marker back at the start.
const IDLE_FRAMES: u32 = 200;
/// The gauge fades in over 0.5 s.
const SHOW: u8 = 10;

/// `gauge_update_cb` 0x1007a65c: a linear tween of the marker, one pass per `SWEEP`.
#[derive(Clone, Copy, Debug)]
struct Gauge {
    /// Marker x (16.16) at the start / end of the pass, and the pass time so far.
    from: i32,
    to: i32,
    t: i32,
    len: i32,
    frozen: Option<i32>,
    /// Next pass goes right-to-left.
    back: bool,
    running: bool,
    blink: u8,
    /// Frames since the gauge was shown (None = hidden).
    shown: Option<u8>,
}

impl Gauge {
    fn new() -> Self {
        Gauge {
            from: LEFT << 16,
            to: LEFT << 16,
            t: 0,
            len: 0,
            frozen: None,
            back: false,
            running: false,
            blink: 0,
            shown: None,
        }
    }

    fn x(&self) -> i32 {
        self.frozen.unwrap_or_else(|| {
            if self.t >= self.len {
                self.to
            } else {
                self.from + ((self.to - self.from) as i64 * self.t as i64 / self.len as i64) as i32
            }
        })
    }

    /// `gauge_reset` 0x10017d84 / `gauge_start` 0x10017dd8.
    fn reset(&mut self, run: bool) {
        *self = Gauge { running: run, shown: self.shown, ..Gauge::new() };
    }

    /// `gauge_zone` 0x10017cf8.
    fn zone(&self) -> usize {
        let x = self.x();
        ZONES.iter().filter(|&&z| x >= z << 16).count()
    }

    fn update(&mut self, pressed: u8, idle: u32, throw: usize, sounds: &mut Vec<(u16, u8)>) {
        if self.running {
            if self.blink > 0 {
                self.blink -= 1;
                self.running = self.blink > 0;
            } else if pressed & (A | B) != 0 {
                sounds.push((1, 100));
                self.frozen = Some(self.x());
                self.blink = BLINK;
            } else if idle > IDLE_FRAMES {
                sounds.push((1, 100));
                self.reset(false);
            } else if self.t >= self.len {
                let (l, r) = (LEFT << 16, RIGHT << 16);
                (self.from, self.to) = if self.back { (r, l) } else { (l, r) };
                (self.t, self.len) = (0, SWEEP[throw.min(2)]);
                self.back = !self.back;
            }
        }
        self.t += FRAME;
        if let Some(s) = &mut self.shown {
            *s = s.saturating_add(1);
        }
    }

    fn palette(&self) -> u8 {
        (self.blink & 7 > 3) as u8
    }
}

pub struct Together06 {
    /// `osewagame06_throws` 0x100804e0 sub-state and its wait.
    state: u16,
    wait: u16,
    throw: usize,
    zone: usize,
    /// Catches by zone.
    count: [u8; 4],
    gauge: Gauge,
    idle: u32,
    pet: Node,
}

impl Default for Together06 {
    fn default() -> Self {
        Self::new()
    }
}

impl Together06 {
    pub fn new() -> Self {
        Together06 {
            state: 0,
            wait: 0,
            throw: 0,
            zone: 0,
            count: [0; 4],
            gauge: Gauge::new(),
            idle: 0,
            pet: Self::play(OPENING),
        }
    }

    fn play(script: u16) -> Node {
        let mut n = Node::new(script);
        n.place(PET.0, PET.1);
        n
    }

    /// `osewagame06_grade` 0x1007aee8.
    pub fn grade(&self) -> u8 {
        let c = self.count;
        if c[3] >= 3 {
            3
        } else if c[3] > 0 && c[0] == 0 {
            2
        } else if c[0] < 2 {
            1
        } else {
            0
        }
    }
}

impl MiniGame for Together06 {
    fn update(&mut self, pressed: u8, _held: u8, data: &Data, sounds: &mut Vec<(u16, u8)>) -> Option<u8> {
        self.idle = if pressed != 0 { 0 } else { self.idle + 1 };
        if self.wait > 0 {
            self.wait -= 1;
        } else {
            match self.state {
                // Opening (state 1000.0–1): the pet shows a throw.
                // ponytail: the fades between the opening, the throws and the result are not drawn.
                0 if self.pet.is_finished() => self.state = 1000,
                1000 => {
                    self.pet = Self::play(IDLE);
                    self.gauge.reset(false);
                    self.gauge.shown = None;
                    self.wait = 10;
                    self.state = 1001;
                }
                1001 => {
                    // ponytail: the gauge also shrinks from 2× while it fades in; only the fade is drawn.
                    self.gauge.shown = Some(0);
                    self.wait = 20;
                    self.state = 2000;
                }
                2000 => {
                    self.gauge.reset(true);
                    self.idle = 0;
                    self.state = 2001;
                }
                2001 if !self.gauge.running => self.state = 3000,
                3000 => {
                    self.zone = self.gauge.zone();
                    self.count[self.zone] += 1;
                    self.gauge.shown = None;
                    self.pet = Self::play(ZONE_ANIMS[self.zone]);
                    self.state = 3001;
                }
                3001 if self.pet.is_finished() => {
                    self.pet = Self::play(if self.zone == 0 { FETCH } else { CAUGHT });
                    self.state = 3002;
                }
                3002 if self.pet.is_finished() => {
                    self.throw += 1;
                    if self.throw < 3 {
                        self.state = 1000;
                    } else {
                        self.wait = 10;
                        self.state = 4000;
                    }
                }
                4000 => return Some(self.grade()),
                _ => {}
            }
        }
        self.gauge.update(pressed, self.idle, self.throw, sounds);
        self.pet.tick(&data.motions);
        self.pet.take_sounds(sounds);
        None
    }

    fn draws(&self, out: &mut Vec<Draw>) {
        self.pet.draws(0, 0, out);
        if let Some(t) = self.gauge.shown {
            let alpha = (64 * t.min(SHOW) as u32 / SHOW as u32) as u8;
            out.push(Draw {
                look: Look::Sprite { id: GAUGE.0, frame: 0 },
                x: GAUGE.1,
                y: GAUGE.2,
                flip: false,
                alpha,
                palette: 0,
                scale: 256,
                rot: 0,
            });
            let look = Look::Sprite { id: MARKER.0, frame: 0 };
            out.push(Draw {
                look,
                x: self.gauge.x() >> 16,
                y: MARKER.1,
                flip: false,
                alpha,
                palette: self.gauge.palette(),
                scale: 256,
                rot: 0,
            });
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
        PET
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(g: &mut Together06, frames: usize, pressed: u8) -> Option<u8> {
        let d = crate::evolve::tests::data();
        let mut r = None;
        for _ in 0..frames {
            r = r.or(g.update(pressed, 0, &d, &mut Vec::new()));
        }
        r
    }

    /// Run until the marker sweeps (state 2001).
    fn sweeping(g: &mut Together06) {
        while g.state != 2001 {
            run(g, 1, 0);
        }
    }

    #[test]
    fn zones_by_marker_x() {
        let mut g = Gauge::new();
        for (x, z) in [(9, 0), (56, 0), (57, 1), (88, 1), (89, 2), (108, 2), (109, 3), (119, 3)] {
            g.frozen = Some(x << 16);
            assert_eq!(g.zone(), z, "x {x}");
        }
    }

    #[test]
    fn marker_sweeps_back_and_forth() {
        let mut g = Gauge::new();
        g.reset(true);
        let mut xs = Vec::new();
        for _ in 0..30 {
            g.update(0, 0, 0, &mut Vec::new());
            xs.push(g.x() >> 16);
        }
        // 0.65 s is a hair over 13 frames, so a pass takes 14.
        assert_eq!(xs[13], RIGHT);
        assert!(xs[12] < RIGHT && xs[14] < RIGHT && xs[27] == LEFT);
        assert!(xs.windows(2).take(13).all(|w| w[0] < w[1]));
    }

    #[test]
    fn a_stops_the_marker_and_it_blinks() {
        let mut g = Together06::new();
        sweeping(&mut g);
        run(&mut g, 7, 0);
        let x = g.gauge.x();
        let mut s = Vec::new();
        g.update(A, 0, &crate::evolve::tests::data(), &mut s);
        assert_eq!(s, vec![(1, 100)]);
        assert_eq!(g.gauge.x(), x);
        run(&mut g, BLINK as usize - 1, 0);
        assert_eq!((g.state, g.gauge.x()), (2001, x), "blinking for 16 frames");
        run(&mut g, 3, 0);
        assert_eq!(g.count.iter().sum::<u8>(), 1);
        assert_eq!(g.state, 3001);
    }

    #[test]
    fn ten_idle_seconds_drop_the_ball() {
        let mut g = Together06::new();
        sweeping(&mut g);
        run(&mut g, IDLE_FRAMES as usize + 3, 0);
        assert_eq!(g.count, [1, 0, 0, 0]);
        assert_eq!(g.gauge.x(), LEFT << 16);
    }

    #[test]
    fn three_throws_then_the_grade() {
        let mut g = Together06::new();
        let mut r = None;
        for _ in 0..3 {
            sweeping(&mut g);
            // Stop the marker on the first pass at its right end (zone 3).
            run(&mut g, 13, 0);
            while g.gauge.x() < 109 << 16 {
                run(&mut g, 1, 0);
            }
            g.update(B, 0, &crate::evolve::tests::data(), &mut Vec::new());
            r = run(&mut g, 60, 0);
        }
        assert_eq!(g.count, [0, 0, 0, 3]);
        assert_eq!(r, Some(3));
        for (c, r) in [([0, 0, 1, 2], 2), ([1, 0, 0, 2], 1), ([0, 3, 0, 0], 1), ([2, 0, 0, 1], 0), ([1, 1, 1, 0], 1)] {
            g.count = c;
            assert_eq!(g.grade(), r, "{c:?}");
        }
    }
}
