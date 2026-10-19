//! Play-together game 00 "ARROW" (scene 0x65, tick `FUN_10052e7c`; 09-minigames-together §2.6).
//! Five rounds: pick left (A) or right (B), then the pet looks one way from a fixed table.

use super::{push_sprite, Hud, MiniGame};
use crate::data::Data;
use crate::input::{A, B};
use crate::motion::{Draw, Look, Node};

const INTRO: u16 = 0xa8b;
const IDLE: u16 = 0xa8c;
/// Look left / right by answer (XIP 0x100a009a), then happy / sad.
const LOOK: [u16; 2] = [0xa8d, 0xa8e];
const OK: u16 = 0xa8f;
const NG: u16 = 0xa90;
/// Result scripts by tier 0..3 (elem 0x531).
const RESULTS: [u16; 4] = [0xa94, 0xa93, 0xa92, 0xa91];
const ROUNDS: u8 = 5;
const AUTO_PICK: u16 = 200;
/// Pet (elems 0x52b / 0x52e / 0x531).
const PET: (i32, i32) = (64, 88);
/// Arrow sprite (elem 0x52f at (24,104); 0x530 is the same flipped at (104,104)).
const ARROW: u16 = 0x89c;
/// Unpicked arrow fade, 0.2 s; result panel fade-in, 0.3 s.
const ARROW_FADE: u8 = 4;
const PANEL_FADE: u8 = 6;
/// Result panel (elem 0x535) with its two icons (0x538 / 0x539).
const PANEL: [(u16, i32, i32); 3] = [(0x89b, 64, 104), (0x899, 48, 96), (0x89a, 80, 96)];

/// Opening jingle.
const OPENING_SND: u16 = 0x4c;

pub struct Together00 {
    started: bool,
    answers: Vec<u8>,
    /// Answer index 0..17, saved back when the game ends (C10).
    pub idx: usize,
    state: u16,
    wait: u16,
    pet: Node,
    round: u8,
    /// Frames waited for a pick (`ctx+0x24`).
    count: u16,
    pick: u8,
    score: u8,
    miss: u8,
    arrows: bool,
    /// Frames since the unpicked arrow started fading.
    fade: Option<u8>,
    /// Frames since the result panel started fading in.
    panel: Option<u8>,
}

impl Together00 {
    /// `answers` = together00 table (0 left, 1 right), `idx` = saved answer index.
    pub fn new(answers: Vec<u8>, idx: usize) -> Self {
        let mut pet = Node::new(INTRO);
        pet.place(PET.0, PET.1);
        Together00 {
            started: false,
            answers,
            idx: idx.min(17),
            state: 0x65,
            wait: 0,
            pet,
            round: 0,
            count: 0,
            pick: 0,
            score: 0,
            miss: 0,
            arrows: false,
            fade: None,
            panel: None,
        }
    }

    fn play(&mut self, script: u16) {
        self.pet = Node::new(script);
        self.pet.place(PET.0, PET.1);
    }

    fn answer(&self) -> u8 {
        self.answers.get(self.idx).copied().unwrap_or(0)
    }

    /// Thresholds 5/3/1 (0x100a13d0).
    pub fn result(&self) -> u8 {
        match self.score {
            5.. => 3,
            3.. => 2,
            1.. => 1,
            _ => 0,
        }
    }

    /// `osewagame00_new_round` 0x1006784c (+ the beep the tick plays after it).
    fn new_round(&mut self, sounds: &mut Vec<(u16, u8)>) {
        (self.arrows, self.fade) = (true, None);
        self.play(IDLE);
        sounds.push((0, 100));
        self.round += 1;
        sounds.push((77, 100));
        self.state = 0x3ea;
    }
}

impl MiniGame for Together00 {
    fn update(&mut self, pressed: u8, _held: u8, data: &Data, sounds: &mut Vec<(u16, u8)>) -> Option<u8> {
        if !self.started {
            // Opening jingle (`FUN_10052e7c` state 100).
            self.started = true;
            sounds.push((OPENING_SND, 100));
        }
        self.pet.tick(&data.motions);
        self.pet.take_sounds(sounds);
        for t in [&mut self.fade, &mut self.panel].into_iter().flatten() {
            *t = t.saturating_add(1);
        }
        if self.wait > 0 {
            self.wait -= 1;
            return None;
        }
        match self.state {
            // ponytail: the opening also plays sound 76 under the fade-in; the fades are not drawn.
            0x65 if self.pet.is_finished() => self.state = 1000,
            1000 | 0x3e9 => self.new_round(sounds),
            // `osewagame00_input` 0x10078a38: 200 frames picks left.
            0x3ea => {
                self.count += 1;
                let pick = if self.count == AUTO_PICK || pressed & A != 0 {
                    Some(0)
                } else if pressed & B != 0 {
                    Some(1)
                } else {
                    None
                };
                if let Some(p) = pick {
                    self.pick = p;
                    sounds.push((1, 100));
                    self.state = 0x3eb;
                }
            }
            0x3eb => {
                self.fade = Some(0);
                self.count = 0;
                self.state = 0x3ec;
            }
            0x3ec => {
                self.play(LOOK[self.answer() as usize & 1]);
                self.state = 0x3ed;
            }
            0x3ed => (self.wait, self.state) = (20, 0x3ee),
            0x3ee => {
                if self.answer() == self.pick {
                    self.play(OK);
                    self.score += 1;
                    sounds.push((52, 100));
                } else {
                    self.play(NG);
                    self.miss += 1;
                    sounds.push((51, 100));
                }
                self.state = 0x3ef;
            }
            0x3ef => {
                self.idx = if self.idx < 17 { self.idx + 1 } else { 0 };
                self.wait = 30;
                if self.round < ROUNDS {
                    self.state = 0x3e9;
                } else {
                    self.arrows = false;
                    self.state = 2000;
                }
            }
            2000 => {
                self.play(IDLE);
                (self.wait, self.state) = (10, 0x7d1);
            }
            0x7d1 => {
                self.panel = Some(0);
                (self.wait, self.state) = (30, 0x7d2);
            }
            0x7d2 => return Some(self.result()),
            _ => {}
        }
        None
    }

    fn draws(&self, out: &mut Vec<Draw>) {
        self.pet.draws(0, 0, out);
        if self.arrows {
            let faded = 64 - (64 * self.fade.unwrap_or(0).min(ARROW_FADE) as u32 / ARROW_FADE as u32) as u8;
            for (side, x) in [(0u8, 24), (1, 104)] {
                let alpha = if self.fade.is_some() && side != self.pick { faded } else { 64 };
                out.push(Draw {
                    look: Look::Sprite { id: ARROW, frame: 0 },
                    x,
                    y: 104,
                    flip: side == 1,
                    alpha,
                    palette: 0,
                    scale: 256,
                });
            }
        }
        if let Some(t) = self.panel {
            let alpha = (64 * t.min(PANEL_FADE) as u32 / PANEL_FADE as u32) as u8;
            for (id, x, y) in PANEL {
                push_sprite(out, id, 0, x, y, alpha);
            }
        }
    }

    fn hud(&self) -> Vec<Hud> {
        match (self.state, self.panel) {
            (0x65, _) => Vec::new(),
            // ponytail: the score box fades out and the panel numbers fade in; Hud has no alpha, so they swap.
            (_, Some(_)) => vec![
                Hud::Number { n: self.score as u32, x: 24, y: 104 },
                Hud::Number { n: self.miss as u32, x: 104, y: 104 },
            ],
            _ => vec![
                Hud::Sprite { id: 0x80e, frame: 0, x: 15, y: 11 },
                Hud::Number { n: self.score as u32, x: 14, y: 11 },
            ],
        }
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

    const ANSWERS: [u8; 18] = [0, 0, 1, 0, 1, 0, 1, 1, 0, 0, 1, 0, 1, 0, 1, 1, 1, 0];

    fn step(g: &mut Together00, pressed: u8) -> Option<u8> {
        g.update(pressed, 0, &crate::evolve::tests::data(), &mut Vec::new())
    }

    /// Wait for the pick window, press `key` (0 = let the 200 frames run out), and finish the round.
    fn round(g: &mut Together00, key: u8) -> Option<u8> {
        while g.state != 0x3ea {
            step(g, 0);
        }
        if key == 0 {
            (0..AUTO_PICK).for_each(|_| assert_eq!(step(g, 0), None));
        } else {
            step(g, key);
        }
        (0..300).find_map(|_| step(g, 0).map(Some).or((g.state == 0x3ea).then_some(None))).flatten()
    }

    #[test]
    fn matching_the_pet_scores() {
        let mut g = Together00::new(ANSWERS.to_vec(), 0);
        // Answers 0,0,1,0,1: four right, one wrong.
        for k in [A, A, B, B, B] {
            round(&mut g, k);
        }
        assert_eq!((g.score, g.miss, g.idx), (4, 1, 5));
        let r = (0..100).find_map(|_| step(&mut g, 0));
        assert_eq!(r, Some(2));
    }

    #[test]
    fn no_pick_is_left_and_the_index_wraps() {
        let mut g = Together00::new(ANSWERS.to_vec(), 15);
        // Answers 1,1,0,0,0 (index 15, 16, 17, then 0, 1); waiting picks left each time.
        for _ in 0..5 {
            round(&mut g, 0);
        }
        assert_eq!((g.score, g.idx), (3, 2));
    }

    #[test]
    fn result_tiers() {
        let mut g = Together00::new(ANSWERS.to_vec(), 0);
        for (score, r) in [(0, 0), (1, 1), (2, 1), (3, 2), (4, 2), (5, 3)] {
            g.score = score;
            assert_eq!(g.result(), r);
        }
    }
}
