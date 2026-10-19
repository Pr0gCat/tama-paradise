//! Offline social play: playdates and gifts with "friend codes" (05-content §2.10).
//!
//! The original swaps a 0x14-byte playdate packet over the link cable. Here the same fields are
//! written as a code the players exchange any way they like; both sides apply the original
//! rules to the two packets, so they reach the same outcome.

use crate::data::Data;
use crate::pet::{req, status, Cond, Pet};
use crate::progress::m;
use serde::{Deserialize, Serialize};

pub const FRIENDS_MAX: usize = 12;

/// The playdate packet (GMMan `playdate.md`), plus what the remake needs to show the friend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Card {
    pub rom_type: u8,
    pub device_uid: u32,
    pub chara_uid: u32,
    pub tama: u16,
    pub color: u8,
    pub onaka: u8,
    pub gokigen: u8,
    pub love: bool,
    /// Friendship this player recorded for the other one (0 = never met).
    pub friendship: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Friend {
    pub device_uid: u32,
    pub chara_uid: u32,
    pub tama: u16,
    pub color: u8,
    /// 1–4.
    pub friendship: u8,
    /// Last result (`Outcome as u8`; 5 = arcade).
    pub result: u8,
}

/// Playdate result (0 play, 1 fight, 2 ate, 3 eaten, 4 married).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Play(u8),
    Fight,
    Married,
}

impl Outcome {
    fn code(self) -> u8 {
        match self {
            Outcome::Play(_) => 0,
            Outcome::Fight => 1,
            Outcome::Married => 4,
        }
    }
}

const CODE_MAGIC: &str = "TP1";

impl Card {
    /// Friend code: "TP1-" + hex of the fields + 2-digit checksum.
    pub fn encode(&self) -> String {
        let body = format!(
            "{:02x}{:08x}{:08x}{:04x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            self.rom_type,
            self.device_uid,
            self.chara_uid,
            self.tama,
            self.color,
            self.onaka,
            self.gokigen,
            self.love as u8,
            self.friendship
        );
        let sum = body.bytes().fold(0u8, |a, b| a.wrapping_add(b));
        format!("{CODE_MAGIC}-{body}{sum:02x}").to_uppercase()
    }

    pub fn decode(code: &str) -> Option<Card> {
        let code = code.trim().to_lowercase();
        let body = code.strip_prefix(&format!("{}-", CODE_MAGIC.to_lowercase()))?;
        if body.len() != 34 || !body.is_ascii() {
            return None;
        }
        let (fields, sum) = body.split_at(32);
        if fields.bytes().fold(0u8, |a, b| a.wrapping_add(b)) != u8::from_str_radix(sum, 16).ok()? {
            return None;
        }
        let h = |a: usize, b: usize| u32::from_str_radix(&fields[a..b], 16).ok();
        Some(Card {
            rom_type: h(0, 2)? as u8,
            device_uid: h(2, 10)?,
            chara_uid: h(10, 18)?,
            tama: h(18, 22)? as u16,
            color: h(22, 24)? as u8,
            onaka: h(24, 26)? as u8,
            gokigen: h(26, 28)? as u8,
            love: h(28, 30)? != 0,
            friendship: h(30, 32)? as u8,
        })
    }
}

/// Base friendship from species rank (`FUN_1006cbf4`).
fn base_friendship(a: u8, b: u8) -> u8 {
    if a == 5 || b == 5 {
        4
    } else if a == 0 || b == 0 {
        1
    } else {
        4 - (a as i32 - b as i32).unsigned_abs().min(3) as u8
    }
}

impl Pet {
    fn friend(&self, uid: u32) -> Option<&Friend> {
        self.friends.iter().find(|f| f.device_uid == uid)
    }

    /// My card for `other_uid` (records the friendship I have stored for that player).
    pub fn card(&self, rom_type: u8, other_uid: Option<u32>) -> Card {
        Card {
            rom_type,
            device_uid: self.device_uid,
            chara_uid: self.chara_uid,
            tama: self.def.id,
            color: self.body_color(),
            onaka: self.onaka,
            gokigen: self.gokigen,
            love: self.status & status::LOVE != 0,
            friendship: other_uid.and_then(|u| self.friend(u)).map_or(0, |f| f.friendship),
        }
    }

    /// Play with a friend's card (`FUN_1008ada4` / `FUN_1008c430`). `marry` = both players chose to
    /// marry when it is offered (both adults, friendship 4, a friendly playdate).
    pub fn playdate(&mut self, data: &Data, other: &Card, marry: bool) -> Option<Outcome> {
        if other.device_uid == self.device_uid || self.cond != Cond::Normal || self.def.stage < 3 {
            return None;
        }
        let them = data.character(other.tama)?;
        let mine = self.friend(other.device_uid).map_or(0, |f| f.friendship);
        let recorded = if mine == 0 || other.friendship == 0 { 0 } else { mine.max(other.friendship) };
        let friendship = recorded.max(base_friendship(self.def.rank, them.rank));
        // HOW PLAY: eating needs character flags the data does not carry yet, so it never happens.
        let fight = (self.onaka < 3 || self.gokigen < 6) && (other.onaka < 3 || other.gokigen < 6);
        let outcome = if fight {
            Outcome::Fight
        } else if marry && self.def.stage == 5 && them.stage == 5 && recorded == 4 {
            Outcome::Married
        } else {
            Outcome::Play(friendship.saturating_sub(1))
        };
        // Friendship grows only after a friendly playdate with someone met before.
        let love = (self.status & status::LOVE != 0) as u8 + other.love as u8;
        let grown = if mine == 0 || outcome.code() != 0 { friendship } else { (friendship + 1 + love).min(4) };
        self.count_mission(m::LINKS, 1);
        self.remember_friend(other, grown, outcome.code());
        match outcome {
            Outcome::Fight if self.cond == Cond::Normal => self.raise(req::INJURY),
            Outcome::Married => self.marry(data, other),
            _ => {}
        }
        Some(outcome)
    }

    fn remember_friend(&mut self, c: &Card, friendship: u8, result: u8) {
        let rec = Friend {
            device_uid: c.device_uid,
            chara_uid: c.chara_uid,
            tama: c.tama,
            color: c.color,
            friendship,
            result,
        };
        self.friends.retain(|f| f.device_uid != c.device_uid);
        if self.friends.len() == FRIENDS_MAX {
            self.friends.remove(0);
        }
        self.friends.push(rec);
    }

    /// Linked marriage: the adult goes to the field, the egg takes the partner's looks and a 50/50 colour.
    fn marry(&mut self, data: &Data, other: &Card) {
        let field = crate::data::attr_to_field(self.def.attribute).unwrap_or(self.field);
        let generation = (self.generation + 1).min(99);
        let color = if self.rand100() < 50 { self.body_color() } else { other.color };
        self.release_to(field);
        if self.rank == 3 || (5..=9).contains(&self.rank) {
            self.rank_up();
        }
        self.new_egg(data);
        self.generation = generation;
        self.color = color;
        self.looks = Some(other.tama);
    }

    /// Gift code for one item (`FUN_100899c0`): removes it from the inventory.
    pub fn gift(&mut self, item: u16) -> Option<String> {
        // Placed toys and worn outfits cannot be given away.
        if self.free(item) == 0 {
            return None;
        }
        self.give(item, -1);
        self.gifts_sent = self.gifts_sent.wrapping_add(1);
        self.count_mission(m::GIFTS, 1);
        let body = format!("{:08x}{:04x}{:04x}", self.device_uid, self.gifts_sent, item);
        let sum = body.bytes().fold(0u8, |a, b| a.wrapping_add(b));
        Some(format!("TG1-{body}{sum:02x}").to_uppercase())
    }

    /// Receive a gift code once.
    pub fn receive_gift(&mut self, data: &Data, code: &str) -> Option<u16> {
        let code = code.trim().to_lowercase();
        let body = code.strip_prefix("tg1-")?;
        if body.len() != 18 || !body.is_ascii() {
            return None;
        }
        let (fields, sum) = body.split_at(16);
        if fields.bytes().fold(0u8, |a, b| a.wrapping_add(b)) != u8::from_str_radix(sum, 16).ok()? {
            return None;
        }
        let item = u16::from_str_radix(&fields[12..16], 16).ok()?;
        let key = fields[..12].to_string();
        if self.gifts_received.contains(&key) || u32::from_str_radix(&fields[..8], 16).ok()? == self.device_uid {
            return None;
        }
        let it = data.item(item)?;
        self.gifts_received.push(key);
        self.give_capped(&it, 1);
        Some(item)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::Item;
    use crate::pet::tests::def;

    fn data() -> Data {
        let mut d = crate::evolve::tests::data();
        let mut a = def(4119, 5, 60, 30, 0);
        a.rank = 2;
        let mut b = def(4122, 5, 60, 30, 0);
        b.rank = 3;
        d.characters.extend([a, b]);
        d.items = vec![Item { id: 7001, kind: 3, ..Default::default() }];
        d
    }

    fn pet(d: &Data, id: u16, uid: u32) -> Pet {
        let mut p = Pet::new(d.character(id).unwrap(), uid);
        p.device_uid = uid;
        p.chara_uid = uid * 7;
        p.onaka = 6;
        p.gokigen = 20;
        p.rank = 3;
        p
    }

    #[test]
    fn card_roundtrip_and_checksum() {
        let d = data();
        let c = pet(&d, 4119, 42).card(6, None);
        let code = c.encode();
        assert_eq!(Card::decode(&code), Some(c));
        let mut bad = code.clone();
        bad.replace_range(6..7, if &code[6..7] == "0" { "1" } else { "0" });
        assert_eq!(Card::decode(&bad), None);
    }

    #[test]
    fn both_sides_agree_and_friendship_grows() {
        let d = data();
        let (mut a, mut b) = (pet(&d, 4119, 1), pet(&d, 4122, 2));
        for round in 0..4 {
            let (ca, cb) = (a.card(6, Some(2)), b.card(1, Some(1)));
            let ra = a.playdate(&d, &cb, false).unwrap();
            let rb = b.playdate(&d, &ca, false).unwrap();
            assert_eq!(ra, rb, "round {round}");
        }
        // base 3 (ranks 2 vs 3), then +1 per friendly playdate after the first, capped at 4.
        assert_eq!(a.friends[0].friendship, 4);
        assert_eq!(a.mission_progress(m::LINKS), 4);
    }

    #[test]
    fn hungry_pets_fight_and_get_hurt() {
        let d = data();
        let (mut a, mut b) = (pet(&d, 4119, 1), pet(&d, 4122, 2));
        a.onaka = 1;
        b.gokigen = 2;
        let cb = b.card(1, Some(1));
        assert_eq!(a.playdate(&d, &cb, false), Some(Outcome::Fight));
        assert_eq!(a.cond, Cond::Injury);
    }

    #[test]
    fn best_friends_can_marry() {
        let d = data();
        let (mut a, mut b) = (pet(&d, 4119, 1), pet(&d, 4122, 2));
        for _ in 0..3 {
            let (ca, cb) = (a.card(6, Some(2)), b.card(1, Some(1)));
            a.playdate(&d, &cb, false);
            b.playdate(&d, &ca, false);
        }
        let cb = b.card(1, Some(1));
        assert_eq!(a.playdate(&d, &cb, true), Some(Outcome::Married));
        assert_eq!((a.def.id, a.looks, a.generation), (1000, Some(4122), 2));
    }

    #[test]
    fn gifts_arrive_once() {
        let d = data();
        let (mut a, mut b) = (pet(&d, 4119, 1), pet(&d, 4122, 2));
        a.give(7001, 1);
        let code = a.gift(7001).unwrap();
        assert_eq!(a.count(7001), 0);
        assert_eq!(b.receive_gift(&d, &code), Some(7001));
        assert_eq!(b.receive_gift(&d, &code), None);
        assert_eq!(a.receive_gift(&d, &code), None, "not to yourself");
    }
}
