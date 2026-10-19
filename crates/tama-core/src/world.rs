//! Fields, expansion areas and weather (05-content §2.5).

use crate::data::Data;
use crate::pet::{date, Pet};

impl Pet {
    /// Daily weather pattern (`FUN_1008fdec`): first month-table bucket whose running sum
    /// exceeds a 0–99 roll; none → pattern 5.
    pub fn roll_weather(&mut self, data: &Data, now: u64) {
        let month = date(now).1 as usize - 1;
        let Some(chances) = data.weather_months.get(month) else { return };
        let r = self.rand100() as u16;
        let mut sum = 0;
        self.weather = 5;
        for (i, &c) in chances.iter().take(5).enumerate() {
            sum += c;
            if sum > r {
                self.weather = i as u8;
                break;
            }
        }
    }

    /// Weather now: 0 clear, 1 cloudy, 2 rain or snow.
    pub fn weather_now(&self, data: &Data, now: u64) -> u8 {
        let period = Pet::period(now) as usize;
        data.weather_patterns.get(self.weather as usize).map_or(0, |p| p[period])
    }

    /// Fields the player can visit (`field_visible` 0x1006258c): the home field below rank 4,
    /// two fields at rank 4–5, all three from rank 6.
    pub fn visible_fields(&self, data: &Data) -> Vec<u8> {
        let home = crate::data::attr_to_field(data.rom_type).unwrap_or(0);
        match self.rank {
            0..=3 => vec![home],
            4 | 5 => {
                let mut f = vec![home, (home + 1) % 3];
                f.sort_unstable();
                f
            }
            _ => vec![0, 1, 2],
        }
    }

    /// Switch field (FIELDSWITCH, `set_fieldtype` 0x10010cf4): the first switch at rank 4 → 5.
    pub fn switch_field(&mut self, f: u8) {
        if f == self.field {
            return;
        }
        if self.rank == 4 {
            self.rank_up();
        }
        self.set_field(f);
        self.in_area2 = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pet::tests::def;

    fn data() -> Data {
        let mut d = crate::evolve::tests::data();
        d.weather_months = vec![[30, 0, 10, 60, 0, 0]; 12];
        d.weather_patterns = vec![[0; 5], [2; 5], [1; 5], [0, 0, 0, 2, 2], [2, 2, 0, 0, 0], [0, 0, 2, 0, 0]];
        d
    }

    #[test]
    fn weather_follows_month_table_and_period() {
        let d = data();
        let mut p = Pet::new(def(2113, 3, 45, 20, 0), 1);
        let mut seen = [0; 6];
        for _ in 0..1000 {
            p.roll_weather(&d, 0);
            seen[p.weather as usize] += 1;
        }
        assert_eq!(seen[1] + seen[4] + seen[5], 0, "0% patterns never happen");
        assert!(seen[3] > seen[0] && seen[0] > seen[2], "{seen:?}");
        p.weather = 3;
        assert_eq!(p.weather_now(&d, 12 * 3600), 0);
        assert_eq!(p.weather_now(&d, 20 * 3600), 2, "evening rain in pattern 3");
    }

    #[test]
    fn switching_field_first_time_at_rank_4_ranks_up() {
        let d = data();
        let mut p = Pet::new(def(2113, 3, 45, 20, 0), 1);
        p.field = 1;
        p.rank = 4;
        assert_eq!(p.visible_fields(&d), vec![1, 2]);
        p.switch_field(2);
        assert_eq!((p.field, p.rank), (2, 5));
        p.switch_field(1);
        assert_eq!(p.rank, 5);
        p.rank = 6;
        assert_eq!(p.visible_fields(&d).len(), 3);
    }
}

#[cfg(test)]
mod game_tests {
    use crate::Game;

    #[test]
    fn weather_is_kept_across_save_and_load() {
        let mut d = crate::evolve::tests::data();
        d.weather_months = vec![[100, 0, 0, 0, 0, 0]; 12];
        d.weather_patterns = vec![[0; 5]; 6];
        let now = 20_745 * 86_400 + 22 * 3600;
        let mut g = Game::new(d.clone(), 1);
        g.frame(now, 0);
        g.pet.weather = 1;
        let back = Game::load(d, &g.save());
        let mut back = back.unwrap();
        back.frame(now + 1, 0);
        assert_eq!(back.pet.weather, 1);
    }
}
