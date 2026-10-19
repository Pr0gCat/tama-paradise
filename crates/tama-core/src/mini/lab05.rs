//! Lab game 05: PUNCHING METEORITE!, and the meteor event that reuses it (09-minigames-lab-b §2.2–2.3).

use super::{sprite, Hud, MiniGame};
use crate::data::Data;
use crate::input::{A, B, C};
use crate::motion::{Draw, Look, Node, KEEP, PET};

const FRAMES: u16 = 880;
const ROW_EVERY: u16 = 20;
/// A glove checks for meteors for 19 frames (hidden when its counter reaches 20).
const GLOVE_FRAMES: u8 = 20;
const HURT_FRAMES: u8 = 20;
/// Frozen frames between the last play frame and the ending (`task_wait(10)`).
const END_WAIT: u16 = 10;
const LIVES: [u8; 3] = [3, 2, 1];
const THRESHOLDS: [u16; 3] = [25, 20, 6];
/// Fall time by speed index (3.0 / 2.5 / 2.0 s) and the punched-away flight (1.5 s), linear tweens.
const FALL: [u16; 3] = [60, 50, 40];
const FLY: u16 = 30;
/// Per lane: glove (0x492–0x494), meteor start (0x497/0x49b/0x49f) and landing (0x498/0x49c/0x4a0),
/// explosion (0x499/0x49d/0x4a1), punched-away target (0x49a/0x49e/0x4a2).
const GLOVE: [(i32, i32); 3] = [(32, 94), (64, 88), (96, 94)];
const START: [(i32, i32); 3] = [(-15, -6), (64, -16), (143, -6)];
const LAND: [(i32, i32); 3] = [(40, 104), (64, 96), (88, 104)];
const BOOM: [(i32, i32); 3] = [(46, 112), (64, 108), (80, 112)];
const AWAY: [(i32, i32); 3] = [(-40, 64), (108, -36), (162, 64)];
/// Glove hit box top is 12 above the glove (0x490); meteor box half height is 12 (0x496).
const GLOVE_TOP: i32 = 12;
const METEOR_HALF: i32 = 12;
/// Hearts 0x4a3–0x4a5: sprite 0x865, palette 1 = lost.
const HEARTS: [(i32, i32); 3] = [(95, 121), (107, 121), (119, 121)];
const HEART: u16 = 0x865;
const BACKGROUND: [u16; 2] = [0x95f, 0x960];
/// Player at element 0x48d (62,122): sprite 230 (origin (4,−1)), frame 2 while hurt.
const PLAYER: u16 = 230;
const PLAYER_AT: (i32, i32) = (62, 122);
/// Event: the pet instead (T7 element 0x48d). Poses 10 / 4 when hit are a guess from the element and `inst+6 = 4`.
const EVENT_POSE: [u16; 2] = [10, 4];
const GLOVE_SPRITE: u16 = 0x864;
const METEOR: u16 = 0x866;
/// Punch effect at glove + (0,−16) (0x491) and explosion.
const PUNCH: u16 = 0xa24;
const EXPLODE: u16 = 0xa25;
pub const OPENING: u16 = 0xa1d;
pub const EVENT_OPENING: u16 = 0x6a9;
const RESULTS: [u16; 4] = [0xa2c, 0xa2a, 0xa28, 0xa26];
/// Event endings by survival: hit (0x6ab), survived (0x6aa).
const EVENT_RESULTS: [u16; 2] = [0x6ab, 0x6aa];

struct Meteor {
    /// None once punched (flying away, no more checks).
    lane: Option<usize>,
    from: (i32, i32),
    to: (i32, i32),
    t: u16,
    len: u16,
}

impl Meteor {
    fn pos(&self) -> (i32, i32) {
        let k = |a: i32, b: i32| a + (b - a) * self.t.min(self.len) as i32 / self.len as i32;
        (k(self.from.0, self.to.0), k(self.from.1, self.to.1))
    }
}

pub struct Lab05 {
    rows: Vec<[[u8; 2]; 3]>,
    event: bool,
    frame: u16,
    score: u16,
    lives: u8,
    hearts: u8,
    /// Shown gloves: (frames shown, punched something).
    gloves: [Option<(u8, bool)>; 3],
    hurt: u8,
    meteors: Vec<Meteor>,
    effects: Vec<Node>,
    bg: [Node; 2],
    /// Frames left before the next ending step, and whether the end sound already played.
    end_wait: Option<u16>,
    end_sound: bool,
}

impl Lab05 {
    /// `rows` = layouts[level*3 + plays % 3].
    pub fn new(rows: Vec<[[u8; 2]; 3]>, level: usize) -> Self {
        Self::with(rows, LIVES[level.min(2)], false)
    }

    /// The meteor event (scene 99): one life, its own table.
    pub fn event(rows: Vec<[[u8; 2]; 3]>) -> Self {
        Self::with(rows, 1, true)
    }

    fn with(rows: Vec<[[u8; 2]; 3]>, lives: u8, event: bool) -> Self {
        let bg = BACKGROUND.map(Node::new);
        Lab05 {
            rows,
            event,
            frame: 0,
            score: 0,
            lives,
            hearts: lives,
            gloves: [None; 3],
            hurt: 0,
            meteors: Vec::new(),
            effects: Vec::new(),
            bg,
            end_wait: None,
            end_sound: false,
        }
    }

    pub fn result(&self) -> u8 {
        if self.event {
            return (self.lives > 0) as u8;
        }
        THRESHOLDS.iter().position(|&t| self.score >= t).map_or(0, |i| 3 - i as u8)
    }

    fn effect(&mut self, script: u16, (x, y): (i32, i32)) {
        let mut n = Node::new(script);
        n.place(x, y);
        self.effects.push(n);
    }

    /// `labo05_meteor_cb` 0x1005f750 for every meteor.
    fn meteors(&mut self, sounds: &mut Vec<(u16, u8)>) {
        let mut i = 0;
        while i < self.meteors.len() {
            let (x, y) = self.meteors[i].pos();
            match self.meteors[i].lane {
                Some(l) if self.gloves[l].is_none() => {
                    if y >= LAND[l].1 {
                        self.meteors.remove(i);
                        self.effect(EXPLODE, BOOM[l]);
                        self.hurt = HURT_FRAMES;
                        self.lives = self.lives.saturating_sub(1);
                        sounds.push((0xe, 100));
                        continue;
                    }
                }
                Some(l) => {
                    if GLOVE[l].1 - GLOVE_TOP <= y + METEOR_HALF {
                        self.effect(PUNCH, (GLOVE[l].0, GLOVE[l].1 - 16));
                        self.meteors[i] = Meteor { lane: None, from: (x, y), to: AWAY[l], t: 0, len: FLY };
                        if let Some(g) = &mut self.gloves[l] {
                            g.1 = true;
                        }
                        self.score += 1;
                        sounds.push((9, 100));
                    }
                }
                None if self.meteors[i].t >= self.meteors[i].len => {
                    self.meteors.remove(i);
                    continue;
                }
                None => {}
            }
            self.meteors[i].t += 1;
            i += 1;
        }
    }
}

impl MiniGame for Lab05 {
    fn update(&mut self, pressed: u8, _held: u8, data: &Data, sounds: &mut Vec<(u16, u8)>) -> Option<u8> {
        for n in self.effects.iter_mut().chain(self.bg.iter_mut()) {
            n.tick(&data.motions);
            n.take_sounds(sounds);
        }
        self.effects.retain(|n| !n.done);
        if let Some(w) = self.end_wait {
            if w > 0 {
                self.end_wait = Some(w - 1);
                return None;
            }
            if self.event || self.end_sound {
                return Some(self.result());
            }
            // State 3000/3001: end jingle, then wait for it to finish.
            sounds.push((0x4a, 100));
            self.end_sound = true;
            let len = data
                .melodies
                .get(0x4a)
                .map_or(0, |m| m.notes.iter().map(|n| n[0] as u32).sum::<u32>() * m.ms_per_step as u32 / 50);
            self.end_wait = Some(len as u16);
            return None;
        }
        // `labo05_tick` state 1001: one punch a frame, A/B/C = lanes 0/1/2. A shown glove ignores presses.
        let lane = [A, B, C].iter().position(|&k| pressed & k != 0);
        if let Some(l) = lane {
            self.gloves[l].get_or_insert((0, false));
        }
        self.frame += 1;
        for g in &mut self.gloves {
            if let Some((n, hit)) = g {
                *n += 1;
                if *n >= GLOVE_FRAMES || *hit {
                    *g = None;
                }
            }
        }
        if self.frame.is_multiple_of(ROW_EVERY) {
            if let Some(row) = self.rows.get((self.frame / ROW_EVERY - 1) as usize) {
                for (l, &[present, speed]) in row.iter().enumerate() {
                    if present != 0 {
                        self.meteors.push(Meteor {
                            lane: Some(l),
                            from: START[l],
                            to: LAND[l],
                            t: 0,
                            len: FALL[(speed as usize).min(2)],
                        });
                        sounds.push((0x32, 100));
                    }
                }
            }
        }
        self.hurt = self.hurt.saturating_sub(1);
        let over = self.frame >= FRAMES || self.lives == 0;
        self.meteors(sounds);
        if over {
            self.end_wait = Some(END_WAIT);
        }
        None
    }

    fn draws(&self, out: &mut Vec<Draw>) {
        for n in &self.bg {
            n.draws(64, 64, out);
        }
        let (x, y) = PLAYER_AT;
        if self.event {
            let anim = EVENT_POSE[(self.hurt > 0) as usize];
            out.push(Draw { look: Look::Chr { chr: PET, anim, eyes: KEEP, mouth: KEEP }, ..sprite(0, 0, x, y) });
        } else {
            // Sprite origin (4,−1) applied here; the blitter centres sprites.
            out.push(sprite(PLAYER, if self.hurt > 0 { 2 } else { 0 }, x + 4, y - 1));
        }
        for m in &self.meteors {
            let (x, y) = m.pos();
            out.push(sprite(METEOR, 0, x, y));
        }
        for (l, g) in self.gloves.iter().enumerate() {
            if g.is_some() {
                out.push(sprite(GLOVE_SPRITE, 0, GLOVE[l].0, GLOVE[l].1));
            }
        }
        for n in &self.effects {
            n.draws(0, 0, out);
        }
        for i in 0..self.hearts {
            let (x, y) = HEARTS[i as usize];
            out.push(Draw { palette: (i >= self.lives) as u8, ..sprite(HEART, 0, x, y) });
        }
    }

    fn hud(&self) -> Vec<Hud> {
        Vec::new()
    }

    fn result_script(&self, result: u8) -> u16 {
        if self.event {
            EVENT_RESULTS[result.min(1) as usize]
        } else {
            RESULTS[result as usize]
        }
    }

    fn uses_c(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lane 1, speed 0 at row 0: spawns at frame 20, enters the glove window (y ≥ 64) 43 frames later,
    /// lands (y ≥ 96) 60 frames later.
    fn one_meteor() -> Vec<[[u8; 2]; 3]> {
        let mut rows = vec![[[0, 0]; 3]; 40];
        rows[0][1] = [1, 0];
        rows
    }

    fn run(g: &mut Lab05, frames: usize, press_at: Option<(usize, u8)>) -> Option<u8> {
        let d = crate::evolve::tests::data();
        let mut r = None;
        for i in 0..frames {
            let p = press_at.filter(|&(f, _)| f == i).map_or(0, |(_, k)| k);
            r = r.or(g.update(p, 0, &d, &mut Vec::new()));
        }
        r
    }

    #[test]
    fn a_glove_in_time_punches_the_meteor() {
        let mut g = Lab05::new(one_meteor(), 0);
        run(&mut g, 63, None); // frame 63: meteor t = 43 → y = 64
        run(&mut g, 1, Some((0, B)));
        assert_eq!(g.score, 1);
        assert_eq!(g.gloves[1], Some((1, true)));
        run(&mut g, 1, None);
        assert_eq!(g.gloves[1], None, "a punching glove is put away the next frame");
        assert_eq!(g.lives, 3);
    }

    #[test]
    fn early_glove_expires_and_the_meteor_lands() {
        let mut g = Lab05::new(one_meteor(), 0);
        run(&mut g, 20, None);
        run(&mut g, 20, Some((0, B))); // shown frames 21–39
        assert_eq!(g.gloves[1], None);
        run(&mut g, 20, Some((0, B))); // shown frames 41–59, meteor still above y 64
        assert_eq!(g.score, 0);
        run(&mut g, 20, None); // lands at frame 80
        assert_eq!((g.score, g.lives, g.hurt > 0), (0, 2, true));
    }

    #[test]
    fn c_is_lane_two_and_a_shown_glove_is_not_extended() {
        let mut g = Lab05::new(vec![[[0, 0]; 3]; 40], 0);
        run(&mut g, 1, Some((0, C)));
        run(&mut g, 10, Some((5, C)));
        assert_eq!(g.gloves[2], Some((11, false)));
        run(&mut g, 8, None);
        assert_eq!(g.gloves[2], Some((19, false)));
        run(&mut g, 1, None);
        assert_eq!(g.gloves[2], None);
        assert!(g.uses_c());
    }

    #[test]
    fn the_game_ends_at_frame_880_with_score_thresholds() {
        let mut g = Lab05::new(vec![[[0, 0]; 3]; 40], 2);
        assert_eq!(g.lives, 1);
        run(&mut g, 879, None);
        assert!(g.end_wait.is_none());
        assert_eq!(run(&mut g, 13, None), Some(0)); // 10 frozen frames, end jingle (empty here)
        for (s, r) in [(25, 3), (24, 2), (20, 2), (19, 1), (6, 1), (5, 0)] {
            g.score = s;
            assert_eq!(g.result(), r);
        }
    }

    #[test]
    fn the_event_ends_on_the_first_hit() {
        let mut g = Lab05::event(one_meteor());
        assert_eq!(run(&mut g, 80, None), None);
        assert_eq!(g.lives, 0);
        // The end check runs before the meteors, so it sees the lost life a frame later.
        assert_eq!(run(&mut g, 12, None), Some(0));
        assert_eq!(g.result_script(0), 0x6ab);
    }
}
