//! Mini-game rewards and unlocks shared by every game (05-content §2.11).

use crate::pet::Pet;
use crate::progress::m;

/// Lab game points by `[difficulty][result 0..3]` (XIP 0x10091bec).
pub const LAB_POINTS: [[u32; 4]; 3] = [[10, 100, 150, 250], [10, 200, 300, 500], [10, 300, 450, 800]];
/// Planet rank needed for each lab-menu game (XIP 0x10093976) and for each difficulty (0x10093968).
pub const LAB_GAME_RANK: [u8; 7] = [1, 4, 4, 1, 4, 1, 1];
pub const LAB_LEVEL_RANK: [u8; 3] = [1, 6, 9];
/// Mood for a "play together" result tier 0..3.
pub const PLAY_MOOD: [i32; 4] = [0, 1, 5, 10];

impl Pet {
    pub fn lab_game_open(&self, game: usize) -> bool {
        self.rank >= LAB_GAME_RANK[game]
    }

    pub fn lab_level_open(&self, level: usize) -> bool {
        self.rank >= LAB_LEVEL_RANK[level]
    }

    /// Result screen of a lab game (`FUN_100228c4`): points, and mission 13 for the best result.
    pub fn lab_reward(&mut self, level: usize, result: usize) -> u32 {
        let pts = LAB_POINTS[level.min(2)][result.min(3)];
        self.earn(pts);
        if result >= 3 {
            self.count_mission(m::LAB_PERFECT, 1);
        }
        pts
    }

    /// End of a "play together" game: mood only; a perfect game counts for mission 4.
    pub fn play_reward(&mut self, tier: usize) {
        self.happy(PLAY_MOOD[tier.min(3)]);
        if tier >= 3 {
            self.count_mission(m::PLAY_PERFECT, 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pet::tests::def;

    #[test]
    fn rewards() {
        let mut p = Pet::new(def(2113, 3, 0, 0, 0), 1);
        assert_eq!(p.lab_reward(1, 3), 500);
        assert_eq!(p.mission_progress(m::LAB_PERFECT), 1);
        p.play_reward(2);
        assert_eq!(p.gokigen, 5);
        p.rank = 4;
        assert!(p.lab_game_open(1) && !p.lab_level_open(1));
    }
}

pub mod lab00;
pub mod lab05;
pub mod lab06;

use crate::data::Data;
use crate::motion::{Draw, Node};

/// Shared opening (READY / GO), script 0x952 at (64,64).
pub const READY_GO: u16 = 0x952;

/// A plain sprite draw (opaque, default palette).
pub(crate) fn sprite(id: u16, frame: u16, x: i32, y: i32) -> Draw {
    Draw { look: crate::motion::Look::Sprite { id, frame }, x, y, flip: false, alpha: 64, palette: 0 }
}

/// Something a game draws besides its motion nodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hud {
    Sprite { id: u16, frame: u16, x: i32, y: i32 },
    /// A number in the 16×16 digit font (sprite 19), centred on (x, y).
    Number { n: u32, x: i32, y: i32 },
}

/// One mini-game. The session runs READY/GO, then `update` each frame until it returns a result.
pub trait MiniGame {
    /// One 20 fps frame (`held` = buttons down). Returns the result (0–3) when the game is over.
    fn update(&mut self, pressed: u8, held: u8, data: &Data, sounds: &mut Vec<(u16, u8)>) -> Option<u8>;
    fn draws(&self, out: &mut Vec<Draw>);
    fn hud(&self) -> Vec<Hud>;
    /// Script shown after the game for a result.
    fn result_script(&self, result: u8) -> u16;
    /// C is a game button, so it does not quit during play.
    fn uses_c(&self) -> bool {
        false
    }
}

pub enum Phase {
    /// The game's own opening script, before READY/GO.
    Intro(Node),
    Ready(Node),
    Play,
    Result(Node, u8),
}

/// A running mini-game with its shared opening and result animation.
pub struct Session {
    pub game: Box<dyn MiniGame>,
    pub phase: Phase,
    /// Lab game difficulty, or None for "play together" games.
    pub level: Option<usize>,
    /// Calendar event (req id) this game is played for: no skipping, no quitting, no points.
    pub event: Option<u8>,
}

impl Session {
    pub fn new(game: Box<dyn MiniGame>, level: Option<usize>) -> Self {
        Session { game, phase: Phase::Ready(Node::new(READY_GO)), level, event: None }
    }

    /// Play `script` (at (64,64)) before READY/GO.
    pub fn with_intro(mut self, script: u16) -> Self {
        self.phase = Phase::Intro(Node::new(script));
        self
    }

    /// C leaves the game now (opening, or play when C is not a game button).
    pub fn can_quit(&self) -> bool {
        self.event.is_none()
            && match self.phase {
                Phase::Intro(_) => true,
                Phase::Play => !self.game.uses_c(),
                _ => false,
            }
    }

    /// Returns the result once the result animation has finished.
    pub fn update(&mut self, pressed: u8, held: u8, data: &Data, sounds: &mut Vec<(u16, u8)>) -> Option<u8> {
        let free = self.event.is_none();
        let skip = free && pressed & (crate::input::A | crate::input::B | crate::input::PUSH) != 0;
        match &mut self.phase {
            Phase::Intro(node) => {
                node.tick(&data.motions);
                node.take_sounds(sounds);
                if skip {
                    sounds.push((1, 100));
                }
                if skip || node.is_finished() {
                    self.phase = Phase::Ready(Node::new(READY_GO));
                }
                None
            }
            Phase::Ready(node) => {
                node.tick(&data.motions);
                node.take_sounds(sounds);
                if node.is_finished() {
                    self.phase = Phase::Play;
                }
                None
            }
            Phase::Play => {
                if let Some(r) = self.game.update(pressed, held, data, sounds) {
                    self.phase = Phase::Result(Node::new(self.game.result_script(r)), r);
                }
                None
            }
            Phase::Result(node, r) => {
                node.tick(&data.motions);
                node.take_sounds(sounds);
                (node.is_finished() || node.done || free && pressed & 3 != 0).then_some(*r)
            }
        }
    }

    pub fn draws(&self, out: &mut Vec<Draw>) {
        match &self.phase {
            Phase::Result(node, _) | Phase::Intro(node) => node.draws(64, 64, out),
            Phase::Ready(node) => {
                self.game.draws(out);
                node.draws(64, 64, out);
            }
            Phase::Play => self.game.draws(out),
        }
    }

    pub fn hud(&self) -> Vec<Hud> {
        if matches!(self.phase, Phase::Play) { self.game.hud() } else { Vec::new() }
    }
}
