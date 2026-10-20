//! Lab menu, shop (buy / cook / sell / shop code / expansion) and the tama sitter screens.

use super::{Action, CardKind, Scene};
use crate::input::{A, B, C, DOWN, PUSH, UP};
use crate::shop::ShopError;
use crate::Game;

mod t {
    pub const INFORMATION: u16 = 0x00;
    pub const LEVELS: [u16; 3] = [0xbf, 0xc0, 0xc1];
    pub const TAMA_SITTER: u16 = 0x16;
    pub const NEW_EGG: u16 = 0x18;
    pub const BREED: u16 = 0x19;
    pub const BRED: u16 = 0x77;
    pub const CAUSE: u16 = 0x79;
    pub const CAUSES: [u16; 3] = [0x7a, 0x7c, 0x7d];
    pub const RELEASE: u16 = 0xda;
    pub const RELEASED: u16 = 0xdd;
    pub const NO_MORE_TODAY: u16 = 0xde;
    pub const SEE_YOU_AGAIN: u16 = 0xe2;
    pub const COLLECTION: u16 = 0x01;
    pub const MINI_GAMES: u16 = 0x02;
    pub const SHOP: u16 = 0x03;
    pub const HELP: u16 = 0x05;
    pub const SETTINGS: u16 = 0x06;
    pub const FIELD_NAMES: [u16; 3] = [0x0d, 0x0e, 0x0f];
    pub const TABS: [u16; 4] = [0x45, 0x46, 0x35, 0x36];
    pub const EXPANSION: u16 = 0x4d;
    pub const BUY: u16 = 0xc2;
    pub const PURCHASE_MADE: u16 = 0xc4;
    pub const COOK: u16 = 0xc6;
    pub const NOT_ENOUGH_FOOD: u16 = 0xc9;
    pub const SELL: u16 = 0xca;
    pub const SALE_MADE: u16 = 0xcb;
    pub const SHOP_CODE: u16 = 0xcc;
    pub const UNLOCK_ITEM: u16 = 0xcd;
    pub const INPUT_CODE: u16 = 0xcf;
    pub const WRONG_CODE: u16 = 0xd0;
    pub const CODE_USED: u16 = 0xd1;
    pub const UNAVAILABLE: u16 = 0xd2;
    pub const CALL_SITTER: u16 = 0xd3;
    pub const CANT_CALL_SITTER: u16 = 0xd6;
    pub const NOT_ENOUGH_POINTS: u16 = 0xd8;
    pub const TAMA_FILE: u16 = 0x09;
    pub const MICROSCOPE: u16 = 0x0a;
    pub const LAB_CODE: u16 = 0x10;
    pub const COLLECTION_CELLS: u16 = 0x11;
    pub const INPUT_LAB_CODE: u16 = 0xe3;
    pub const WRONG_LAB_CODE: u16 = 0xe5;
    pub const LAB_CODE_USED: u16 = 0xe6;
    pub const NO_DATA: u16 = 0xe7;
}

/// LAB CODE letters: font glyphs for 0–9 and A–Z.
const LAB_CHARS: [u16; 36] = {
    let mut c = [0; 36];
    let mut i = 0;
    while i < 36 {
        c[i] = if i < 10 { 306 + i as u16 } else { 318 + i as u16 - 10 };
        i += 1;
    }
    c
};
/// SEARCHING… shows for 40 frames (`FUN_1005e61c` state 1).
const SEARCH_FRAMES: u16 = 40;
/// Found character: background, the character, NEW!, name bar (UI 0x272–0x276).
const FOUND_BG: u16 = 0x272;
/// Card list sprites shared with the hunt team picker (`FUN_1002e120`): card frame, cursor.
const COLLECTION_CARD: u16 = 0x83;
const COLLECTION_CURSOR: u16 = 0x80;
const FOUND_NEW: u16 = 0x274;
const FOUND_NAME_BAR: u16 = 0x276;

const COOK_ICON_MATERIAL_Y: i32 = 112;

/// Lab game menu (index = menu position): title string and game number (scenes 0x5d–0x64).
const LAB_GAMES: [(u16, u8); 7] = [(0x323, 0), (0x324, 1), (0x325, 2), (0x326, 3), (0x32e, 6), (0x328, 5), (0x327, 4)];
/// Play-together menu (09-minigames-together §2.1): title and game number.
/// ARROW, 3 UFOS and WALKING (games 0, 2, 3) are reachable here although the original menu has no entry.
const TOGETHER_GAMES: [(u16, u8); 7] =
    [(0x32f, 4), (0x32a, 1), (0x333, 6), (0x334, 7), (0x329, 0), (0x32b, 2), (0x32c, 3)];
/// Play-together games playable so far.
const TOGETHER_IMPLEMENTED: &[u8] = &[4, 1, 6, 7, 0, 2, 3];
/// Games playable so far.
const IMPLEMENTED: &[u8] = &[0, 1, 2, 3, 4, 5, 6];

impl Game {
    /// Run a menu action and return the scene it opens.
    pub(crate) fn open(&mut self, a: Action) -> Scene {
        let now = self.pet.last.unwrap_or(0);
        match a {
            Action::Feed => Scene::FoodCategory { idx: 0 },
            Action::Groom => self.play(
                &[super::script::GROOM_INTRO, super::script::GROOM],
                (4, super::OnPress::Groom),
                None,
                Scene::Home,
            ),
            Action::CleanPoop => self.start_poop_clean(),
            Action::Settings
            | Action::SettingsTime
            | Action::SettingsTimeDisplay
            | Action::Volume
            | Action::Brightness
            | Action::DateTime
            | Action::DateOrder
            | Action::SetDateOrder(_)
            | Action::ToggleTouch
            | Action::Delete
            | Action::Continue
            | Action::ResetAllAsk
            | Action::ResetAll => self.open_system(a),
            Action::SettingsAtTime => self.settings_at_time(),
            Action::Lab => Scene::Menu {
                items: vec![
                    (t::INFORMATION, Some(Action::Info)),
                    (t::MINI_GAMES, Some(Action::MiniGames)),
                    (t::SHOP, Some(Action::Shop)),
                    (t::HELP, Some(Action::Help)),
                    (t::COLLECTION, Some(Action::Book)),
                    (t::SETTINGS, Some(Action::Settings)),
                ],
                idx: 2,
                back: None,
            },
            Action::Shop => Scene::Menu {
                items: vec![
                    (t::BUY, Some(Action::BuyMenu)),
                    (t::COOK, Some(Action::Cook)),
                    (t::SELL, Some(Action::Sell)),
                    (t::SHOP_CODE, Some(Action::Code)),
                    (t::EXPANSION, Some(Action::Expand)),
                ],
                idx: 0,
                back: Some(Action::Lab),
            },
            Action::BuyMenu => Scene::Menu {
                items: (0..4).map(|i| (t::TABS[i], self.pet.tab_open(i).then_some(Action::BuyTab(i as u8)))).collect(),
                idx: 0,
                back: Some(Action::Shop),
            },
            Action::BuyTab(tab) => {
                let items = self.pet.shop_items(&self.data, tab as usize, now);
                if items.is_empty() {
                    return self.message(t::UNAVAILABLE, Action::BuyMenu);
                }
                Scene::Card { kind: CardKind::Buy(tab), items, idx: 0 }
            }
            Action::Sell => {
                let items = self.sellable();
                if items.is_empty() {
                    return self.message(t::UNAVAILABLE, Action::Shop);
                }
                Scene::Card { kind: CardKind::Sell, items, idx: 0 }
            }
            Action::Cook => {
                let items = self.pet.recipes(&self.data).into_iter().map(|r| r.0).collect();
                Scene::Card { kind: CardKind::Cook, items, idx: 0 }
            }
            Action::Code => Scene::Code { digits: [0; 8], pos: 0 },
            Action::Expand => Scene::Menu {
                items: (0..3u8)
                    .map(|f| {
                        (t::FIELD_NAMES[f as usize], (self.pet.areas & 1 << f == 0).then_some(Action::ExpandField(f)))
                    })
                    .collect(),
                idx: self.pet.field as usize,
                back: Some(Action::Shop),
            },
            Action::ExpandField(f) => match self.pet.unlock_area(f) {
                Ok(()) => self.message(t::PURCHASE_MADE, Action::Shop),
                Err(_) => self.message(t::NOT_ENOUGH_POINTS, Action::Expand),
            },
            Action::Missions => Scene::Missions { idx: 0 },
            Action::Fields => {
                let fields = self.pet.visible_fields(&self.data);
                Scene::Menu {
                    idx: fields.iter().position(|&f| f == self.pet.field).unwrap_or(0),
                    items: fields
                        .into_iter()
                        .map(|f| (t::FIELD_NAMES[f as usize], Some(Action::SwitchField(f))))
                        .collect(),
                    back: None,
                }
            }
            Action::SwitchField(f) => {
                self.pet.switch_field(f);
                self.level = crate::Level::Field;
                Scene::Home
            }
            Action::Help => {
                let adult = self.pet.def.stage == 5;
                Scene::Menu {
                    items: vec![
                        (t::TAMA_SITTER, Some(Action::Sitter)),
                        (t::NEW_EGG, adult.then_some(Action::Release)),
                        (t::BREED, adult.then_some(Action::Breed)),
                    ],
                    idx: 0,
                    back: Some(Action::Lab),
                }
            }
            Action::Sitter => {
                Scene::Ask { text: t::CALL_SITTER, yes: true, on_yes: Action::CallSitter, back: Action::Help }
            }
            Action::Release => {
                Scene::Ask { text: t::RELEASE, yes: true, on_yes: Action::ReleaseNow, back: Action::Help }
            }
            Action::ReleaseNow => {
                self.pet.release_to_field(&self.data);
                Scene::Message { text: t::RELEASED, then: Box::new(Scene::Home) }
            }
            Action::Breed => {
                if self.pet.breeds_today >= crate::family::BREEDS_PER_DAY {
                    return self.message(t::NO_MORE_TODAY, Action::Help);
                }
                let cands = self.pet.breed_candidates(&self.data);
                if cands.is_empty() {
                    return self.message(t::UNAVAILABLE, Action::Help);
                }
                Scene::Breed { cands, idx: 0 }
            }
            Action::BreedWith(id) => match self.pet.breed(&self.data, id) {
                Some(true) => Scene::Message { text: t::BRED, then: Box::new(Scene::Home) },
                Some(false) => Scene::Message { text: t::SEE_YOU_AGAIN, then: Box::new(Scene::Home) },
                None => self.message(t::NO_MORE_TODAY, Action::Help),
            },
            Action::Graves => Scene::Graves { idx: 0, back: Action::Book },
            // LABOMENU_BOOK (`FUN_100464c4`): TAMA FILE (here the graves) and MICROSCOPE.
            Action::Book => Scene::Menu {
                items: vec![(t::TAMA_FILE, Some(Action::Graves)), (t::MICROSCOPE, Some(Action::Microscope))],
                idx: 0,
                back: Some(Action::Lab),
            },
            // MICROSCOPE (`FUN_1005ce28`): ponytail: its COLLECTION viewer is not built yet, so it
            // stays greyed out (the original greys it out until a character is found).
            Action::Microscope => Scene::Menu {
                items: vec![
                    (t::LAB_CODE, Some(Action::LabCode)),
                    (t::COLLECTION_CELLS, (self.pet.lab_cells != 0).then_some(Action::LabCollection)),
                ],
                idx: 0,
                back: Some(Action::Book),
            },
            Action::LabCollection => Scene::Collection { idx: 0 },
            Action::LabCode => {
                let entry = Scene::LabCode { chars: [LAB_CHARS[0]; 8], pos: 0 };
                Scene::Message { text: t::INPUT_LAB_CODE, then: Box::new(entry) }
            }
            Action::SitterView | Action::EndSitter | Action::Tomb | Action::NewEgg => self.open_event(a),
            Action::MiniGames => Scene::Menu {
                items: LAB_GAMES
                    .iter()
                    .enumerate()
                    .map(|(i, &(title, game))| {
                        (
                            title,
                            (IMPLEMENTED.contains(&game) && self.pet.lab_game_open(i)).then_some(Action::LabGame(game)),
                        )
                    })
                    .collect(),
                idx: 0,
                back: Some(Action::Lab),
            },
            Action::LabGame(g) => Scene::Menu {
                items: (0..3)
                    .map(|l| (t::LEVELS[l], self.pet.lab_level_open(l).then_some(Action::PlayLab(g, l as u8))))
                    .collect(),
                idx: 0,
                back: Some(Action::MiniGames),
            },
            Action::PlayLab(g, level) => {
                // Layout = plays % 3. Game 05 only moves on after a finished game (state 3004), the others at start.
                let n = self.pet.mini_plays[g as usize];
                if g != 5 {
                    self.pet.mini_plays[g as usize] = n.wrapping_add(1);
                }
                let (mini, l, n) = (&self.data.mini, level as usize, n as usize);
                let game: Box<dyn crate::mini::MiniGame> = match g {
                    1 => Box::new(crate::mini::lab01::Lab01::new(
                        mini.lab01_maps.get(l * 3 + n % 3).cloned().unwrap_or_default(),
                        l,
                    )),
                    2 => Box::new(crate::mini::lab02::Lab02::new(
                        mini.lab02_boards.get(l * 5 + n % 5).copied().unwrap_or_default(),
                        l,
                    )),
                    3 => Box::new(crate::mini::lab03::Lab03::new(
                        mini.lab03_layouts.get(l * 2 + n % 2).cloned().unwrap_or_default(),
                    )),
                    4 => Box::new(crate::mini::lab04::Lab04::new(
                        l,
                        mini.lab04_launch.get(l * 3 + n % 3).copied().unwrap_or([1; 5]),
                        mini.lab04_prize_brick.get(l * 5 + n % 5).copied().unwrap_or([1; 5]),
                        mini.lab04_prize_chr.get(n % 49).copied().unwrap_or(0),
                    )),
                    5 => Box::new(crate::mini::lab05::Lab05::new(
                        mini.lab05_layouts.get(l * 3 + n % 3).cloned().unwrap_or_default(),
                        l,
                    )),
                    6 => Box::new(crate::mini::lab06::Lab06::new(
                        mini.lab06_courses.get(l * 3 + n % 3).cloned().unwrap_or_default(),
                    )),
                    _ => Box::new(crate::mini::lab00::Lab00::new(
                        mini.lab00_boards.get(l * 3 + n % 3).cloned().unwrap_or_default(),
                    )),
                };
                let session = crate::mini::Session::new(game, Some(l));
                let session = match g {
                    5 => session.with_intro(crate::mini::lab05::OPENING),
                    6 => session.with_intro(crate::mini::lab06::OPENING),
                    _ => session,
                };
                Scene::Mini { session, lab: Some((g, level)) }
            }
            Action::Setsubi
            | Action::ToyList
            | Action::SetsubiAt { .. }
            | Action::PlaceToy { .. }
            | Action::RemoveToy { .. }
            | Action::SpaceMenu
            | Action::Stars
            | Action::Deco
            | Action::Wear(_)
            | Action::Travel
            | Action::GoTravel(_)
            | Action::PlanetLv => self.open_decor(a),
            Action::Hunt | Action::HuntKind(_) | Action::HuntGo { .. } => self.open_hunt(a),
            Action::IntroLanguage
            | Action::IntroLang(_)
            | Action::Language
            | Action::SetLang(_)
            | Action::UserEdit
            | Action::Info
            | Action::InfoFromMissions
            | Action::UserInfo => self.open_intro(a),
            Action::EditPlanetName => {
                let mut chars = [crate::screen::SPACE; 8];
                for (c, &g) in chars.iter_mut().zip(&self.pet.planet_name) {
                    *c = g;
                }
                Scene::PlanetName { chars, pos: 0, intro: false }
            }
            Action::EditBirthday => {
                let (m, d) = self.pet.birthday.unwrap_or((1, 1));
                Scene::Birthday { m, d, pos: 0, intro: false }
            }
            Action::Connect
            | Action::ConnectPlaydate
            | Action::ConnectGifts
            | Action::GiftItem(_)
            | Action::Planets
            | Action::Arcade
            | Action::PlayArcade(_) => self.open_connect(a),
            Action::Together => {
                if self.pet.last.is_some_and(|t| self.pet.is_sleeping_at(t)) {
                    return Scene::Message { text: 0x69, then: Box::new(Scene::Home) };
                }
                Scene::Menu {
                    items: TOGETHER_GAMES
                        .iter()
                        .map(|&(title, g)| {
                            (title, TOGETHER_IMPLEMENTED.contains(&g).then_some(Action::PlayTogether(g)))
                        })
                        .collect(),
                    idx: 0,
                    back: None,
                }
            }
            Action::PlayTogether(g) => {
                let n = self.pet.together_plays[g as usize] as usize;
                self.pet.together_plays[g as usize] = (n as u8).wrapping_add(1);
                let mini = &self.data.mini;
                let game: Box<dyn crate::mini::MiniGame> = match g {
                    // Each together game registers here: game number → constructor (n = play count).
                    4 => Box::new(crate::mini::together04::Together04::new(
                        mini.together04_cards.clone(),
                        mini.together04_pet_slot.clone(),
                        n,
                    )),
                    1 => Box::new(crate::mini::together01::Together01::new(
                        mini.together01_commands.get(n % 3).copied().unwrap_or([[1, 4]; 10]),
                    )),
                    6 => Box::new(crate::mini::together06::Together06::new()),
                    7 => Box::new(crate::mini::together07::Together07::new(
                        mini.together07_maps.get(n % 3).cloned().unwrap_or_default(),
                        self.pet.field,
                    )),
                    // Game 00 keeps an answer index, not a play count: saved as if all 5 rounds are played.
                    // ponytail: the firmware saves the index reached when the game exits, so quitting early differs.
                    0 => {
                        self.pet.together_plays[0] = ((n % 18 + 5) % 18) as u8;
                        Box::new(crate::mini::together00::Together00::new(mini.together00_answers.clone(), n % 18))
                    }
                    2 => Box::new(crate::mini::together02::Together02::new(
                        mini.together02_layouts.get(n % 5).cloned().unwrap_or_default(),
                        mini.together02_swaps.get(n % 3).copied().unwrap_or([10, 16, 20]),
                    )),
                    // ponytail: the firmware shares this counter with game 01 (C10); each game counts its own plays here.
                    3 => Box::new(crate::mini::together03::Together03::new(
                        mini.together03_maps.get(n % 3).cloned().unwrap_or_default(),
                        self.pet.field,
                    )),
                    _ => return self.open(Action::Together),
                };
                Scene::Mini { session: crate::mini::Session::together(game), lab: None }
            }
            Action::CallSitter => match self.pet.call_sitter(now) {
                Ok(()) => self.sitter_arrives(),
                Err(ShopError::NotEnoughPoints) => self.message(t::NOT_ENOUGH_POINTS, Action::Lab),
                Err(_) => self.message(t::CANT_CALL_SITTER, Action::Lab),
            },
        }
    }

    /// After a mini-game: lab games pay points (`FUN_100228c4`), then back to the game menu.
    pub(crate) fn after_mini(&mut self, result: Option<u8>, lab: Option<(u8, u8)>) -> Scene {
        match (result, lab) {
            (Some(r), Some((g, level))) => {
                if g == 5 {
                    self.pet.mini_plays[5] = self.pet.mini_plays[5].wrapping_add(1);
                }
                let points = self.pet.lab_reward(level as usize, r as usize);
                Scene::LabResult { stars: r.min(3), points, nodes: Vec::new(), t: 0 }
            }
            (Some(r), None) => {
                self.pet.play_reward(r as usize);
                self.open(Action::Together)
            }
            (None, Some(_)) => self.open(Action::MiniGames),
            (None, None) => self.open(Action::Together),
        }
    }

    pub(super) fn message(&mut self, text: u16, then: Action) -> Scene {
        let then = self.open(then);
        Scene::Message { text, then: Box::new(then) }
    }

    /// Inventory items that can be sold (`FUN_1004af0c`).
    fn sellable(&self) -> Vec<u16> {
        let mut ids: Vec<u16> = self
            .pet
            .inventory
            .iter()
            .filter(|(id, n)| *n > 0 && self.data.item(*id).is_some_and(|i| i.sell != 0xffff && i.flags & 8 == 0))
            .map(|(id, _)| *id)
            .collect();
        ids.sort_unstable();
        ids
    }

    pub(super) fn update_card(&mut self, kind: CardKind, items: Vec<u16>, mut idx: usize, pressed: u8) -> Scene {
        let n = items.len().max(1);
        if pressed & (A | UP) != 0 {
            idx = (idx + 1) % n;
        }
        if pressed & DOWN != 0 {
            idx = (idx + n - 1) % n;
        }
        let back = match kind {
            CardKind::Buy(_) => Action::BuyMenu,
            CardKind::Toy => Action::Setsubi,
            CardKind::Gift => Action::Connect,
            CardKind::Wear | CardKind::Travel => Action::SpaceMenu,
            _ => Action::Shop,
        };
        if pressed & C != 0 {
            return self.open(back);
        }
        if pressed & (B | PUSH) == 0 {
            return Scene::Card { kind, items, idx };
        }
        let Some(item) = items.get(idx).and_then(|&id| self.data.item(id)) else { return self.open(back) };
        if matches!(kind, CardKind::Toy | CardKind::Wear | CardKind::Travel) {
            return self.pick_decor_card(kind, &items, idx);
        }
        if kind == CardKind::Gift {
            return self.pick_gift(&items, idx);
        }
        let now = self.pet.last.unwrap_or(0);
        let stay = |g: &mut Game| match kind {
            CardKind::Sell => g.open(Action::Sell),
            _ => Scene::Card { kind, items: items.clone(), idx },
        };
        let (text, ok) = match kind {
            CardKind::Buy(_) => match self.pet.buy(&item, now) {
                Ok(()) => (t::PURCHASE_MADE, true),
                Err(ShopError::NotEnoughPoints) => (t::NOT_ENOUGH_POINTS, false),
                Err(_) => (t::UNAVAILABLE, false),
            },
            CardKind::Sell => match self.pet.sell(&item) {
                Ok(()) => (t::SALE_MADE, true),
                Err(_) => (t::UNAVAILABLE, false),
            },
            CardKind::Toy | CardKind::Wear | CardKind::Travel | CardKind::Gift => unreachable!(),
            CardKind::Cook => match self.pet.cook(&self.data, item.id) {
                Ok(()) => {
                    // Cooking serves the dish at once, like feeding.
                    let then = Scene::Card { kind, items, idx };
                    return self.play(
                        &[super::script::SERVE[0], super::script::EAT[0][0]],
                        (0, super::OnPress::None),
                        Some(item.eat_sprite),
                        then,
                    );
                }
                Err(ShopError::NotOwned) => (t::NOT_ENOUGH_FOOD, false),
                Err(_) => (t::UNAVAILABLE, false),
            },
        };
        self.sfx(if ok { 1 } else { 0x0b }, 100);
        let then = stay(self);
        Scene::Message { text, then: Box::new(then) }
    }

    pub(super) fn update_code(&mut self, mut digits: [u8; 8], mut pos: usize, pressed: u8) -> Scene {
        if pressed & (A | UP) != 0 {
            digits[pos] = (digits[pos] + 1) % 10;
        }
        if pressed & DOWN != 0 {
            digits[pos] = (digits[pos] + 9) % 10;
        }
        if pressed & C != 0 {
            if pos == 0 {
                return self.open(Action::Shop);
            }
            pos -= 1;
        }
        if pressed & (B | PUSH) != 0 {
            pos += 1;
            if pos == 8 {
                let code: String = digits.iter().map(|d| char::from(b'0' + d)).collect();
                let text = match self.pet.enter_code(&self.data, &code) {
                    Ok(_) => {
                        self.sfx(super::SND_CODE_UNLOCKED, 100);
                        t::UNLOCK_ITEM
                    }
                    Err(ShopError::Full) => t::CODE_USED,
                    Err(_) => t::WRONG_CODE,
                };
                return self.message(text, Action::Shop);
            }
        }
        Scene::Code { digits, pos }
    }

    pub(super) fn draw_card(&mut self, kind: CardKind, items: &[u16], idx: usize) {
        self.draw_food_card(items, idx);
        let Some(item) = items.get(idx).and_then(|&id| self.data.item(id)) else { return };
        let now = self.pet.last.unwrap_or(0);
        match kind {
            CardKind::Buy(_) => {
                let price = crate::pet::Pet::price(&item, now);
                self.draw_number(price, 64, 115, 1);
            }
            CardKind::Sell => self.draw_number(item.sell as u32, 64, 115, 1),
            // How many are free to place; a mark on the outfit being worn.
            CardKind::Toy => self.draw_number(self.pet.free(item.id) as u32, 64, 115, 1),
            CardKind::Wear => {
                if self.pet.equip.contains(&item.id) {
                    self.draw_text(0xa5, 64, 115, true, 1);
                }
            }
            CardKind::Travel => self.draw_number(self.pet.unchi_meter as u32, 64, 115, 1),
            CardKind::Gift => self.draw_number(self.pet.free(item.id) as u32, 64, 115, 1),
            CardKind::Cook => {
                let material = self.data.recipes.iter().find(|r| r.0 == item.id).map_or(0, |r| r.1);
                if let Some(m) = self.data.item(material) {
                    self.screen.blit(m.icon_sprite, 0, 40, COOK_ICON_MATERIAL_Y, 64, false, None);
                }
                self.draw_number(self.pet.count(material) as u32, 72, 115, 1);
            }
        }
        // Points balance, top right.
        self.draw_number_right(self.pet.points, 124, 8, 1);
    }

    /// One grave: the tama, cause of death (illness / hunger / old age) and age.
    pub(super) fn draw_grave(&mut self, i: usize) {
        self.draw_list(&[], 0, Some(t::CAUSE));
        let Some(g) = self.pet.graves.get(i).copied() else {
            self.draw_text(0xe7, 64, 70, true, 0);
            return;
        };
        self.draw_character(g.ghost.id, Some(g.ghost.color), 64, 92);
        let cause = if g.days >= 30 {
            t::CAUSES[2]
        } else if g.cause == 1 {
            t::CAUSES[0]
        } else {
            t::CAUSES[1]
        };
        self.draw_text(cause, 64, 106, true, 0);
        self.draw_number(g.days as u32, 64, 120, 0);
    }

    /// One mission: description, progress / target, "CLEARED" when paid out.
    pub(super) fn draw_mission(&mut self, i: usize) {
        use crate::progress::{TARGETS, TEXT_FIRST};
        self.draw_list(&[], 0, Some(TEXT_FIRST + i as u16));
        let (have, need) = (self.pet.mission_progress(i), TARGETS[i]);
        self.draw_number(i as u32 + 1, 12, 120, 0);
        self.draw_number(have.min(need), 46, 102, 0);
        self.draw_number(need, 94, 102, 0);
        self.screen.rect(68, 101, 4, 2, [255, 255, 255]);
        if self.pet.claimed & 1 << i != 0 {
            self.draw_text(0xa5, 80, 120, true, 0);
        }
    }

    /// LAB CODE entry: like the shop code, with letters (the original uses its text keyboard).
    pub(super) fn update_lab_code(&mut self, mut chars: [u16; 8], mut pos: usize, pressed: u8) -> Scene {
        let at = LAB_CHARS.iter().position(|&c| c == chars[pos]).unwrap_or(0);
        if pressed & (A | UP) != 0 {
            chars[pos] = LAB_CHARS[(at + 1) % LAB_CHARS.len()];
        }
        if pressed & DOWN != 0 {
            chars[pos] = LAB_CHARS[(at + LAB_CHARS.len() - 1) % LAB_CHARS.len()];
        }
        if pressed & C != 0 {
            if pos == 0 {
                return self.open(Action::Microscope);
            }
            pos -= 1;
        }
        if pressed & (B | PUSH) != 0 {
            pos += 1;
            if pos == 8 {
                let code = chars
                    .iter()
                    .map(|&c| {
                        LAB_CHARS
                            .iter()
                            .position(|&l| l == c)
                            .map_or('0', |i| b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ"[i] as char)
                    })
                    .collect();
                return Scene::Searching { code, t: 0 };
            }
        }
        Scene::LabCode { chars, pos }
    }

    /// SEARCHING…, then the result (`FUN_1005e61c`): a character, or a message; a NO DATA code
    /// is followed by the daily +50 points screen.
    pub(super) fn update_searching(&mut self, code: String, t: u16) -> Scene {
        if t < SEARCH_FRAMES {
            return Scene::Searching { code, t: t + 1 };
        }
        let text = match self.pet.lab_code(&code) {
            crate::shop::LabCode::Found(chr) => return Scene::Found { chr, zoom: false, from: None },
            crate::shop::LabCode::NoData { bonus: true } => {
                return Scene::Message { text: t::NO_DATA, then: Box::new(Scene::Bonus) };
            }
            crate::shop::LabCode::NoData { bonus: false } => t::NO_DATA,
            crate::shop::LabCode::Used => t::LAB_CODE_USED,
            crate::shop::LabCode::Wrong => t::WRONG_LAB_CODE,
        };
        self.message(text, Action::Microscope)
    }

    pub(super) fn draw_lab_code(&mut self, chars: &[u16; 8], pos: usize) {
        self.draw_list(&[], 0, Some(t::INPUT_LAB_CODE));
        for (i, &c) in chars.iter().enumerate() {
            let x = 18 + i as i32 * 13;
            self.screen.text(&self.data.fonts, &[c], x, 86, true, 1);
            if i == pos {
                self.screen.rect(x - 5, 96, 10, 2, [40, 40, 120]);
            }
        }
    }

    /// A microscope character: dial up zooms in (×2), dial down back out; any button returns.
    pub(super) fn update_found(&mut self, chr: u16, mut zoom: bool, from: Option<usize>, pressed: u8) -> Scene {
        if pressed & UP != 0 {
            zoom = true;
        }
        if pressed & DOWN != 0 {
            zoom = false;
        }
        if pressed & (A | B | C | PUSH) != 0 {
            return match from {
                Some(idx) => Scene::Collection { idx },
                None => self.open(Action::Microscope),
            };
        }
        Scene::Found { chr, zoom, from }
    }

    /// A character under the microscope (`FUN_1005c0b0`), with its name, and NEW! when just found.
    pub(super) fn draw_found(&mut self, chr: u16, zoom: bool, new: bool) {
        self.draw_elem(FOUND_BG, None, None);
        let Some(def) = self.data.characters.iter().find(|c| c.chr_id == chr).copied() else { return };
        if new {
            // The NEW! glow fills the screen, so the character and its name go on top.
            let e = self.ui_elem(FOUND_NEW);
            let mut node = crate::motion::Node::new(e.p[3] as u16);
            node.tick(&self.data.motions);
            let mut draws = Vec::new();
            node.draws(e.x as i32, e.y as i32, &mut draws);
            self.draw_list_of(&draws, None);
        }
        if zoom {
            // ×2 about the screen centre: y 72 lands at 80, the character's feet above the name.
            self.screen.zoom = 2.0;
            self.draw_character(def.id, None, 64, 72);
            self.screen.zoom = 1.0;
        } else {
            self.draw_character(def.id, None, 64, 91);
        }
        self.draw_elem(FOUND_NAME_BAR, None, None);
        self.draw_text(def.name_id, 64, 104, true, 1);
    }

    /// Lab-code characters in table order (the 29 entries with a character).
    fn collection(&self) -> Vec<(usize, u16)> {
        crate::shop::LAB_CODES.iter().enumerate().filter(|(_, c)| c.1 != 0).map(|(i, c)| (i, c.1)).collect()
    }

    /// COLLECTION: 2×2 cards a page; B opens a collected one, C goes back.
    pub(super) fn update_collection(&mut self, idx: usize, pressed: u8) -> Scene {
        let cards = self.collection();
        let idx = super::cycle(idx, cards.len(), pressed);
        if pressed & C != 0 {
            return self.open(Action::Microscope);
        }
        if pressed & (B | PUSH) != 0 {
            let (i, chr) = cards[idx];
            if self.pet.lab_cells & 1 << i != 0 {
                return Scene::Found { chr, zoom: false, from: Some(idx) };
            }
            self.sfx(0x0b, 100);
        }
        Scene::Collection { idx }
    }

    pub(super) fn draw_collection(&mut self, idx: usize) {
        const CARD_POS: [(i32, i32); 4] = [(30, 34), (87, 34), (30, 92), (87, 92)];
        self.draw_elem(FOUND_BG, None, None);
        let cards = self.collection();
        let page = idx / 4;
        for (k, &(x, y)) in CARD_POS.iter().enumerate() {
            let Some(&(i, chr)) = cards.get(page * 4 + k) else { break };
            let found = self.pet.lab_cells & 1 << i != 0;
            self.screen.blit(COLLECTION_CARD, 0, x, y, 64, false, (!found).then_some((COLLECTION_CARD, 1)));
            if let Some(def) = self.data.characters.iter().find(|c| c.chr_id == chr).copied().filter(|_| found) {
                self.draw_character_pose(def.id, None, 30, x + 1, y + 19);
            }
        }
        let (cx, cy) = CARD_POS[idx % 4];
        self.screen.blit(COLLECTION_CURSOR, 0, cx, cy, 64, false, None);
    }

    pub(super) fn draw_code(&mut self, digits: &[u8; 8], pos: usize) {
        self.draw_list(&[], 0, Some(t::INPUT_CODE));
        for (i, &d) in digits.iter().enumerate() {
            let x = 16 + i as i32 * 13;
            if i == pos {
                self.screen.rect(x - 6, 96, 12, 2, [40, 40, 120]);
            }
            self.draw_number(d as u32, x, 86, 1);
        }
    }
}

impl Game {
    pub(super) fn update_breed(&mut self, cands: Vec<u16>, idx: usize, pressed: u8) -> Scene {
        let idx = super::cycle(idx, cands.len(), pressed);
        if pressed & C != 0 {
            self.open(Action::Help)
        } else if pressed & (B | PUSH) != 0 {
            Scene::Ask { text: 0xdb, yes: true, on_yes: Action::BreedWith(cands[idx]), back: Action::Breed }
        } else {
            Scene::Breed { cands, idx }
        }
    }

    pub(super) fn update_graves(&mut self, idx: usize, back: Action, pressed: u8) -> Scene {
        let idx = super::cycle(idx, self.pet.graves.len(), pressed);
        if pressed & C != 0 {
            self.open(back)
        } else {
            Scene::Graves { idx, back }
        }
    }

    pub(super) fn update_missions(&mut self, idx: usize, pressed: u8) -> Scene {
        let idx = super::cycle(idx, crate::progress::TARGETS.len(), pressed);
        if pressed & C != 0 {
            self.open(Action::InfoFromMissions)
        } else {
            Scene::Missions { idx }
        }
    }

    /// A running mini-game; C quits where the game allows it.
    pub(super) fn update_mini(
        &mut self,
        mut session: crate::mini::Session,
        lab: Option<(u8, u8)>,
        pressed: u8,
    ) -> Scene {
        if pressed & C != 0 && session.can_quit() {
            self.sfx(2, 100);
            return self.after_mini(None, lab);
        }
        match session.update(pressed, self.held, &self.data, &mut self.sounds) {
            Some(r) if session.event == Some(crate::pet::req::METEOR) => {
                // Back to HOME on the field layer (`labo05_tick` 0xdae).
                self.pet.finish_meteor(r > 0);
                self.level = crate::Level::Field;
                Scene::Home
            }
            Some(r) => match session.arcade {
                Some(g) => self.after_arcade(g, r),
                None => self.after_mini(Some(r), lab),
            },
            None => Scene::Mini { session, lab },
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::data::{Item, KIND_MEAL};
    use crate::input::{B, PUSH};
    use crate::Game;

    const NOON: u64 = 86_400 * 3 + 12 * 3600;

    fn game() -> Game {
        let mut d = crate::evolve::tests::data();
        d.items =
            vec![Item { id: 5010, kind: KIND_MEAL, buy: 100, sell: 50, flags: 2, onaka: 1, ..Default::default() }];
        let mut g = Game::new(d, 1);
        g.pet.def = g.data.character(2113).unwrap();
        g.pet.rank = 1;
        g.pet.rank_seen = 1;
        g.pet.points = 500;
        g.pet.onaka = 6;
        g.pet.gokigen = 20;
        g.frame(NOON, 0);
        g
    }

    fn press(g: &mut Game, bits: u8) {
        g.frame(NOON, bits);
        g.frame(NOON, 0);
    }

    #[test]
    fn breeding_through_the_help_menu() {
        let mut g = game();
        let mut adult = crate::pet::tests::def(4119, 5, 60, 30, 0);
        adult.rank = 2;
        g.data.characters.push(adult);
        for i in 0..16u16 {
            let mut c = crate::pet::tests::def(0x1016 + i, 5, 60, 30, 0);
            c.rank = 2;
            g.data.characters.push(c);
        }
        g.pet.def = adult;
        g.pet.field = 1;
        g.pet.status = crate::pet::status::LOVE;
        g.scene = g.open(super::super::Action::Help);
        press(&mut g, crate::input::A); // NEW EGG
        press(&mut g, crate::input::A); // BREED
        press(&mut g, B);
        assert!(matches!(g.scene, super::Scene::Breed { .. }));
        press(&mut g, B); // THIS TAMA?
        press(&mut g, B); // YES
        assert!(matches!(g.scene, super::Scene::Message { text: 0x77, .. }), "LOVE makes breeding certain");
        assert_eq!((g.pet.def.id, g.pet.generation), (1000, 2));
        assert_eq!(g.pet.grazing_in(1), 1);
    }

    #[test]
    fn long_press_opens_lab_and_buys_food() {
        let mut g = game();
        for _ in 0..14 {
            g.frame(NOON, PUSH);
        }
        g.frame(NOON, 0);
        assert!(matches!(g.scene, super::Scene::Menu { idx: 2, .. }), "lab menu with SHOP selected");
        press(&mut g, B); // SHOP
        press(&mut g, B); // BUY
        press(&mut g, B); // FOOD tab
        assert!(matches!(g.scene, super::Scene::Card { .. }));
        press(&mut g, B); // buy
        assert_eq!((g.pet.points, g.pet.count(5010)), (400, 1));
        assert!(matches!(g.scene, super::Scene::Message { .. }));
    }

    #[test]
    fn meteor_event_starts_at_midday_and_rewards_survival() {
        use crate::pet::req::METEOR;
        let mut g = game();
        g.pet.gokigen = 5;
        g.pet.raise(METEOR);
        g.scene = super::super::Scene::Home;
        g.check_events(NOON, 30);
        assert!(matches!(g.scene, super::super::Scene::Home), "30% roll");
        g.check_events(NOON - 3 * 3600, 0);
        assert!(matches!(g.scene, super::super::Scene::Home), "10–15 h only");
        g.check_events(NOON, 29);
        let super::super::Scene::Mini { session, .. } = &g.scene else { panic!("event game") };
        assert!(session.event == Some(METEOR) && !session.can_quit());
        press(&mut g, crate::input::C); // lane 2, not quit
        assert!(matches!(g.scene, super::super::Scene::Mini { .. }));
        for _ in 0..1200 {
            g.frame(NOON, 0); // empty test table: nothing falls, the pet survives
        }
        assert!(matches!(g.scene, super::super::Scene::Home));
        assert_eq!(g.pet.gokigen, 15);
        assert!(!g.pet.is_pending(METEOR));
        assert_eq!(g.level, crate::Level::Field);
    }

    #[test]
    fn game_05_layout_moves_on_only_after_a_finished_game() {
        let mut g = game();
        g.scene = g.open(super::super::Action::PlayLab(5, 0));
        press(&mut g, crate::input::C); // quit during the opening
        assert_eq!(g.pet.mini_plays[5], 0);
        g.scene = g.open(super::super::Action::PlayLab(6, 0));
        assert_eq!(g.pet.mini_plays[6], 1, "game 06 moves on at start");
        assert!(matches!(g.after_mini(Some(1), Some((5, 0))), super::super::Scene::LabResult { stars: 1, .. }));
        assert_eq!(g.pet.mini_plays[5], 1);
    }

    #[test]
    fn edit_birthday_from_user_info() {
        let mut g = game();
        g.scene = g.open(super::super::Action::UserEdit);
        press(&mut g, B); // EDIT BIRTHDAY
        for _ in 0..3 {
            press(&mut g, crate::input::A); // month 4
        }
        press(&mut g, B);
        press(&mut g, crate::input::DOWN); // day 31 → clamped to 30 for April
        press(&mut g, B);
        assert_eq!(g.pet.birthday, Some((4, 30)));
    }

    #[test]
    fn rank_up_shows_the_old_and_new_planet() {
        let mut g = game();
        g.pet.rank = 4;
        g.frame(NOON, 0);
        assert!(matches!(g.scene, super::Scene::Play(_)), "rank-up animation");
        assert_eq!(g.planet_ranks, Some([1, 4]));
    }

    #[test]
    fn rank_ten_rolls_the_credits() {
        let mut g = game();
        g.pet.rank = 10;
        g.frame(NOON, 0);
        for _ in 0..400 {
            if !matches!(g.scene, super::Scene::Play(_)) {
                break;
            }
            g.frame(NOON, 0);
        }
        g.frame(NOON, B); // CONGRATS! LV. 10 NOW!
        assert!(matches!(g.scene, super::Scene::Credits { .. }));
        g.frame(NOON, 0);
        press(&mut g, B);
        assert!(matches!(g.scene, super::Scene::Home));
    }

    #[test]
    fn name_the_planet_in_settings() {
        let mut g = game();
        g.scene = g.open(super::super::Action::EditPlanetName);
        press(&mut g, crate::input::A); // A
        press(&mut g, B);
        press(&mut g, crate::input::A);
        press(&mut g, crate::input::A); // B
        for _ in 0..7 {
            press(&mut g, B);
        }
        assert_eq!(g.pet.planet_name, vec![318, 319]);
    }

    #[test]
    fn microscope_collection_opens_collected_characters() {
        let mut g = game();
        g.scene = g.open(super::super::Action::Microscope);
        assert!(matches!(&g.scene, super::Scene::Menu { items, .. } if items[1].1.is_none()), "nothing collected yet");
        assert!(matches!(g.pet.lab_code("XKGDC0FX"), crate::shop::LabCode::Found(_)));
        g.scene = g.open(super::super::Action::LabCollection);
        press(&mut g, B); // card 0 (4RG89EWK) not collected
        assert!(matches!(g.scene, super::Scene::Collection { idx: 0 }));
        press(&mut g, crate::input::A);
        press(&mut g, B); // card 1: collected
        assert!(matches!(g.scene, super::Scene::Found { zoom: false, from: Some(1), .. }));
        press(&mut g, crate::input::UP);
        assert!(matches!(g.scene, super::Scene::Found { zoom: true, .. }));
        press(&mut g, crate::input::A);
        assert!(matches!(g.scene, super::Scene::Collection { idx: 1 }));
    }
}
