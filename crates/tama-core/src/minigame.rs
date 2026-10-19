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
