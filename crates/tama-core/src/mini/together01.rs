//! Play-together game 01 "FLAGS" (scene 0x66, tick `FUN_100539dc`; 09-minigames-together §2.3).

use super::{Hud, MiniGame};
use crate::data::Data;
use crate::input::{A, B};
use crate::motion::{Draw, Node};

const INTRO: u16 = 0xa95;
/// Feint and answer scripts by command value 1..4 (XIP 0x100a00a0 / 0x100a00a8).
const FEINT: [u16; 4] = [0xa96, 0xa97, 0xa98, 0xa99];
const ANSWER: [u16; 4] = [0xa9a, 0xa9b, 0xa9c, 0xa9d];
const OK: u16 = 0xa9e;
const NG: u16 = 0xa9f;
/// Pose shown silently before the result (state 0x3f2).
const OUTRO: u16 = 0xaa2;
/// Result scripts by tier 0..3 (0x100a13c0 reversed: perfect is first there).
const RESULTS: [u16; 4] = [0xaa3, 0xaa2, 0xaa1, 0xaa0];
/// Answer value when no button is pressed.
const NONE: u8 = 4;
const WINDOW: u16 = 20;
/// The pet (UI element 0x53b).
const PET: (i32, i32) = (64, 88);

pub struct Together01 {
    cmds: [[u8; 2]; 10],
    state: u16,
    wait: u16,
    pet: Node,
    cmd: usize,
    score: u8,
    input: u8,
    a: bool,
    b: bool,
    count: u16,
}

impl Together01 {
    /// `cmds` = together01_commands[play count % 3] (`[feint, answer]`).
    pub fn new(cmds: [[u8; 2]; 10]) -> Self {
        Together01 {
            cmds,
            state: 0x65,
            wait: 0,
            pet: Node::new(INTRO),
            cmd: 0,
            score: 0,
            input: NONE,
            a: false,
            b: false,
            count: 0,
        }
    }

    /// Thresholds 10/8/1 (0x100a13d8).
    pub fn result(&self) -> u8 {
        match self.score {
            10.. => 3,
            8.. => 2,
            1.. => 1,
            _ => 0,
        }
    }

    fn play(&mut self, script: u16) {
        self.pet = Node::new(script);
    }

    fn anim(table: &[u16; 4], v: u8) -> u16 {
        table[(v as usize).clamp(1, 4) - 1]
    }
}

impl MiniGame for Together01 {
    /// Touch: A flag, B flag, both together.
    fn touch_map(&self) -> crate::touch::Map {
        crate::touch::Map { up: crate::input::A, down: crate::input::B, tap: crate::input::A | crate::input::B, ..crate::touch::Map::DEFAULT }
    }
    fn update(&mut self, pressed: u8, _held: u8, data: &Data, sounds: &mut Vec<(u16, u8)>) -> Option<u8> {
        self.pet.tick(&data.motions);
        let mut s = Vec::new();
        self.pet.take_sounds(&mut s);
        // State 0x3f2 mutes the volume until the result (`FUN_10030bb4(0)`).
        if !matches!(self.state, 2000) {
            sounds.append(&mut s);
        }
        if self.wait > 0 {
            self.wait -= 1;
            return None;
        }
        match self.state {
            0x65 if self.pet.is_finished() => self.state = 1000,
            1000 | 0x3e9 => {
                (self.input, self.a, self.b) = (NONE, false, false);
                self.play(Self::anim(&FEINT, self.cmds[self.cmd][0]));
                self.state = 0x3ea;
            }
            // Jumping the gun during the feint ends the game.
            0x3ea if pressed & (A | B) != 0 => {
                self.play(NG);
                (self.wait, self.state) = (30, 0x7d1);
            }
            0x3ea if self.pet.is_finished() => {
                sounds.push((10, 100));
                self.play(Self::anim(&ANSWER, self.cmds[self.cmd][1]));
                (self.count, self.state) = (0, 0x3ec);
            }
            0x3ec => {
                if pressed & A != 0 && !self.a {
                    self.a = true;
                    self.input = if self.b { 3 } else { 1 };
                    sounds.push((1, 100));
                }
                if pressed & B != 0 && !self.b {
                    self.b = true;
                    self.input = if self.a { 3 } else { 2 };
                    sounds.push((1, 100));
                }
                self.count += 1;
                if self.count >= WINDOW {
                    self.state = 0x3ed;
                }
            }
            0x3ed => {
                if self.input == self.cmds[self.cmd][1] {
                    self.play(OK);
                    sounds.push((52, 100));
                    self.cmd += 1;
                    self.score += 1;
                    self.state = if self.cmd < 10 { 0x3e9 } else { 0x3f2 };
                } else {
                    self.play(NG);
                    sounds.push((13, 100));
                    self.state = 0x3f2;
                }
                self.wait = 20;
            }
            0x3f2 => {
                self.play(OUTRO);
                (self.wait, self.state) = (20, 2000);
            }
            2000 | 0x7d1 => return Some(self.result()),
            _ => {}
        }
        None
    }

    fn draws(&self, out: &mut Vec<Draw>) {
        self.pet.draws(PET.0, PET.1, out);
    }

    fn hud(&self) -> Vec<Hud> {
        if self.state == 0x65 {
            return Vec::new();
        }
        // ponytail: the "\x9a" icon glyph before the number is not drawn (no text in Hud).
        vec![Hud::Sprite { id: 0x80e, frame: 0, x: 15, y: 11 }, Hud::Number { n: self.score as u32, x: 14, y: 11 }]
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

    /// Tests have no motion data, so every script (feint, intro) finishes at once.
    fn game(answers: [u8; 10]) -> Together01 {
        Together01::new(std::array::from_fn(|i| [1, answers[i]]))
    }

    /// Run until the input window (state 0x3ec) opens, then press `keys` (one per frame) and
    /// let the window close and the judgement run.
    fn answer(g: &mut Together01, keys: &[u8]) -> Option<u8> {
        let d = crate::evolve::tests::data();
        let mut s = Vec::new();
        for _ in 0..100 {
            if g.state == 0x3ec {
                break;
            }
            if let Some(r) = g.update(0, 0, &d, &mut s) {
                return Some(r);
            }
        }
        for i in 0..WINDOW as usize + 1 + 21 {
            if let Some(r) = g.update(keys.get(i).copied().unwrap_or(0), 0, &d, &mut s) {
                return Some(r);
            }
        }
        None
    }

    fn finish(g: &mut Together01) -> u8 {
        let d = crate::evolve::tests::data();
        (0..200).find_map(|_| g.update(0, 0, &d, &mut Vec::new())).expect("game ends")
    }

    #[test]
    fn copying_the_flags_scores() {
        let mut g = game([1, 2, 3, 4, 3, 3, 1, 2, 4, 4]);
        for keys in [&[A][..], &[B], &[A, B], &[], &[B, A], &[B, 0, A], &[A, A, A], &[0, B], &[], &[]] {
            assert_eq!(answer(&mut g, keys), None);
        }
        assert_eq!(g.score, 10);
        assert_eq!(finish(&mut g), 3);
    }

    #[test]
    fn a_miss_ends_the_game() {
        let mut g = game([1, 2, 3, 4, 3, 3, 1, 2, 4, 4]);
        answer(&mut g, &[A]);
        answer(&mut g, &[A]);
        assert_eq!(g.score, 1);
        assert_eq!(finish(&mut g), 1);
        // A press after the 20-frame window does not count: both flags were needed.
        let mut g = game([3; 10]);
        let mut keys = vec![0; WINDOW as usize];
        keys.extend([A, B]);
        answer(&mut g, &keys);
        assert_eq!(finish(&mut g), 0);
    }

    #[test]
    fn pressing_during_the_feint_ends_the_game() {
        let d = crate::evolve::tests::data();
        let mut g = game([4; 10]);
        let mut s = Vec::new();
        g.update(0, 0, &d, &mut s);
        g.update(0, 0, &d, &mut s);
        assert_eq!(g.state, 0x3ea);
        g.update(B, 0, &d, &mut s);
        assert_eq!((g.state, g.wait), (0x7d1, 30));
        assert_eq!(finish(&mut g), 0);
    }

    #[test]
    fn result_tiers() {
        let mut g = game([4; 10]);
        for (score, r) in [(0, 0), (1, 1), (7, 1), (8, 2), (9, 2), (10, 3)] {
            g.score = score;
            assert_eq!(g.result(), r);
        }
    }
}
