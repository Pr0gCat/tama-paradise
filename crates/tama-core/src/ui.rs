//! Screens: HOME zoom levels, ring menu, feeding, care animations, prompts, status
//! (docs/spec/08-care-ui.md). Layout coordinates come from the original UI elements.

use crate::data::{UiElem, KIND_MEAL, KIND_SNACK, UI_ANIM, UI_SPRITE};
use crate::input::{A, B, C, DOWN, PUSH, UP};
use crate::motion::Node;
use crate::pet::{req, Cond};
use crate::Game;

mod shop;

/// String ids (English table; same ids in every language).
mod text {
    pub const FOOD: u16 = 0x33;
    pub const SNACKS: u16 = 0x34;
    pub const OUTSIDE_TOY: u16 = 0x37;
    pub const EGG_HUNT: u16 = 0x38;
    pub const CLEAN_POOP: u16 = 0x39;
    pub const FIELDS: u16 = 0x3a;
    pub const FEED: u16 = 0x44;
    pub const PLAY: u16 = 0x47;
    pub const CLEAN_TAMA: u16 = 0x48;
    pub const WAKE_UP: u16 = 0x66;
    pub const TIME_TO_SLEEP: u16 = 0x67;
    pub const EAT_THIS: u16 = 0x9d;
    pub const HUNGER: u16 = 0x9e;
    pub const HAPPY: u16 = 0x9f;
    pub const YEARS: u16 = 0xa0;
    pub const YES: u16 = 0xff;
    pub const NO: u16 = 0x100;
}

/// Motion scripts used by the care screens (08-care-ui §2.4–2.10).
mod script {
    pub const SERVE: [u16; 3] = [0x60c, 0x60f, 0x611];
    pub const EAT: [[u16; 2]; 3] = [[0x5fa, 0x600], [0x5fd, 0x603], [0x606, 0x609]];
    pub const DRINK: [u16; 3] = [0x619, 0x61c, 0x61f];
    pub const FULL: u16 = 0x613;
    pub const REFUSE_FULL: u16 = 0x615;
    pub const REFUSE_SICK: u16 = 0x617;
    pub const GROOM_INTRO: u16 = 0x64c;
    pub const GROOM: u16 = 0x64e;
    pub const TOILET: u16 = 0x575;
    pub const MEDICINE: u16 = 0x5ae;
    pub const CURED: u16 = 0x5bb;
    pub const INJURY: u16 = 0x5c0;
    pub const INJURY_DONE: u16 = 0x5c8;
    pub const HAPPY_JUMP: u16 = 0x576;
    /// After eating: COLOR, LOVE, IMMUNITY, HAPPY, REDU_CM (same order as `req::EFFECTS`).
    pub const EFFECTS: [u16; 5] = [0x622, 0x623, 0x624, 0x625, 0x626];
    pub const SHOOTINGSTAR: u16 = 0x6ac;
    pub const UFO: u16 = 0x6ae;
    pub const SANTA: u16 = 0x6af;
    pub const EVE: u16 = 0x6b0;
    pub const XMAS: u16 = 0x6b1;
}

/// UI elements (08-care-ui §2.3–2.11, §3.4).
mod elem {
    pub const MENU_BG: u16 = 0x2ad;
    pub const MENU_LABEL: u16 = 0xa4;
    pub const LIST_BG: u16 = 0x2af;
    pub const CARD_PANEL: u16 = 0xdd;
    pub const CARD_NAME_BAR: u16 = 0xdf;
    pub const CARD_CAPSULE: u16 = 0xe6;
    pub const STATUS_BG: u16 = 0x296;
    pub const STATUS_FRAME: u16 = 0x297;
    pub const STATUS_HUNGER: [u16; 6] = [0x299, 0x29a, 0x29b, 0x29c, 0x29d, 0x29e];
    pub const STATUS_HAPPY_BAR: u16 = 0x2a0;
    pub const NAME_BG: u16 = 0x2a5;
    pub const NAME_BAR: u16 = 0x2a7;
    pub const POOP: [u16; 6] = [0x2e8, 0x2e9, 0x2ea, 0x2eb, 0x2ec, 0x2ed];
    pub const POOP_CLEAN: u16 = 0x2da;
}

const ARROWS: u16 = 89;
const TABLE: u16 = 0x786;
/// Placeholder sprite in eating scripts, replaced by the food's sprite.
const FOOD_PLACEHOLDER: u16 = 0x682;
const PELLETS: u16 = 5002;
const MILK: u16 = 5001;
const JELLY_STICK: u16 = 6001;
/// Glyph code of '0' (digits are consecutive).
const DIGIT_0: u16 = 306;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Field,
    Pet,
    Cell,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    Feed,
    Groom,
    CleanPoop,
    Lab,
    Shop,
    Help,
    BuyMenu,
    BuyTab(u8),
    Sell,
    Cook,
    Code,
    Expand,
    ExpandField(u8),
    CallSitter,
    Missions,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CardKind {
    Buy(u8),
    Sell,
    Cook,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OnPress {
    None,
    Groom,
    Medicine,
    Injury,
    /// Calendar event: reward applied when the animation ends.
    Event(u8),
}

pub(crate) struct Play {
    node: Node,
    queue: Vec<u16>,
    presses: u8,
    /// Care effect applied on the `at`-th button press at a pause point.
    on_press: (u8, OnPress),
    food: Option<u16>,
    then: Box<Scene>,
}

pub(crate) enum Scene {
    Home,
    /// Ring menu; C goes back to `back` (or HOME).
    Menu { items: Vec<(u16, Option<Action>)>, idx: usize, back: Option<Action> },
    /// Item carousel for the shop (buy / sell / cook).
    Card { kind: CardKind, items: Vec<u16>, idx: usize },
    /// A notice; any button continues.
    Message { text: u16, then: Box<Scene> },
    /// YES/NO question.
    Ask { text: u16, yes: bool, on_yes: Action, back: Action },
    /// 8-digit code entry.
    Code { digits: [u8; 8], pos: usize },
    /// Mission list (lab menu INFORMATION).
    Missions { idx: usize },
    FoodCategory { idx: usize },
    FoodList { cat: u8, items: Vec<u16>, idx: usize },
    Confirm { cat: u8, items: Vec<u16>, idx: usize, yes: bool },
    Play(Play),
    Prompt { wake: bool, yes: bool },
    Status { page: u8, idle: u16 },
    /// Play the pending eating effects one by one, then show the food list.
    Effects { cat: u8, idx: usize },
}

fn pressed_any(p: u8) -> bool {
    p & (A | B | C | PUSH) != 0
}

impl Game {
    fn ui_elem(&self, id: u16) -> UiElem {
        self.data.ui(id)
    }

    pub(crate) fn string(&self, id: u16) -> Vec<u16> {
        self.data
            .strings
            .get(self.lang as usize)
            .or_else(|| self.data.strings.get(1))
            .and_then(|t| t.get(id as usize))
            .cloned()
            .unwrap_or_default()
    }

    pub(crate) fn play(&self, scripts: &[u16], on_press: (u8, OnPress), food: Option<u16>, then: Scene) -> Scene {
        let mut queue: Vec<u16> = scripts.iter().rev().copied().collect();
        let first = queue.pop().unwrap_or(0);
        Scene::Play(Play { node: Node::new(first), queue, presses: 0, on_press, food, then: Box::new(then) })
    }

    /// Foods offered for a category (`inv_list_for_eat` 0x1001e250): a free item first, then the
    /// inventory. Babies only get milk / jelly sticks.
    fn food_items(&self, cat: u8) -> Vec<u16> {
        let baby = self.pet.def.stage == 2;
        let free = match (cat, baby) {
            (KIND_MEAL, true) => MILK,
            (KIND_MEAL, false) => PELLETS,
            _ => JELLY_STICK,
        };
        let mut items = vec![free];
        if !baby {
            let kinds = self.pet.inventory.iter().filter(|(id, n)| *n > 0 && self.data.item(*id).is_some_and(|i| i.kind == cat));
            items.extend(kinds.map(|(id, _)| *id));
        }
        items
    }

    /// Sound ids (04-assets SND8, 08-care-ui U6): 0 cursor, 1 confirm, 2 cancel, 0xb not allowed, 0x28 zoom.
    pub(crate) fn sfx(&mut self, id: u16, vol: u8) {
        self.sounds.push((id, vol));
    }

    /// Handle one frame of input for the current scene.
    pub(crate) fn update_scene(&mut self, pressed: u8) {
        if matches!(
            self.scene,
            Scene::Menu { .. }
                | Scene::FoodCategory { .. }
                | Scene::FoodList { .. }
                | Scene::Confirm { .. }
                | Scene::Prompt { .. }
                | Scene::Status { .. }
                | Scene::Card { .. }
                | Scene::Ask { .. }
                | Scene::Code { .. }
                | Scene::Missions { .. }
        ) {
            let disabled = matches!(&self.scene, Scene::Menu { items, idx, .. } if items[*idx].1.is_none());
            if pressed & C != 0 {
                self.sfx(2, 100);
            } else if pressed & (B | PUSH) != 0 {
                self.sfx(if disabled { 0x0b } else { 1 }, 100);
            } else if pressed & (A | UP | DOWN) != 0 {
                self.sfx(0, 100);
            }
        }
        let scene = std::mem::replace(&mut self.scene, Scene::Home);
        self.scene = match scene {
            Scene::Home => self.update_home_input(pressed),
            Scene::Menu { items, mut idx, back } => {
                let n = items.len();
                if pressed & (A | UP) != 0 {
                    idx = (idx + 1) % n;
                }
                if pressed & DOWN != 0 {
                    idx = (idx + n - 1) % n;
                }
                if pressed & C != 0 {
                    back.map_or(Scene::Home, |a| self.open(a))
                } else if pressed & (B | PUSH) != 0 {
                    match items[idx].1 {
                        Some(a) => self.open(a),
                        None => Scene::Menu { items, idx, back },
                    }
                } else {
                    Scene::Menu { items, idx, back }
                }
            }
            Scene::Card { kind, items, idx } => self.update_card(kind, items, idx, pressed),
            Scene::Message { text, then } => {
                if pressed_any(pressed) { *then } else { Scene::Message { text, then } }
            }
            Scene::Ask { text, mut yes, on_yes, back } => {
                if pressed & (A | UP | DOWN) != 0 {
                    yes = !yes;
                }
                if pressed & (B | PUSH) != 0 && yes {
                    self.open(on_yes)
                } else if pressed & (B | PUSH | C) != 0 {
                    self.open(back)
                } else {
                    Scene::Ask { text, yes, on_yes, back }
                }
            }
            Scene::Code { digits, pos } => self.update_code(digits, pos, pressed),
            Scene::Missions { mut idx } => {
                if pressed & (A | UP) != 0 {
                    idx = (idx + 1) % 22;
                }
                if pressed & DOWN != 0 {
                    idx = (idx + 21) % 22;
                }
                if pressed & C != 0 { self.open(Action::Lab) } else { Scene::Missions { idx } }
            }
            Scene::FoodCategory { mut idx } => {
                if pressed & (A | UP | DOWN) != 0 {
                    idx ^= 1;
                }
                if pressed & C != 0 {
                    self.pet_menu()
                } else if pressed & (B | PUSH) != 0 {
                    let cat = if idx == 0 { KIND_MEAL } else { KIND_SNACK };
                    Scene::FoodList { cat, items: self.food_items(cat), idx: 0 }
                } else {
                    Scene::FoodCategory { idx }
                }
            }
            Scene::FoodList { cat, items, mut idx } => {
                let n = items.len();
                if pressed & (A | UP) != 0 {
                    idx = (idx + 1) % n;
                }
                if pressed & DOWN != 0 {
                    idx = (idx + n - 1) % n;
                }
                if pressed & C != 0 {
                    Scene::FoodCategory { idx: (cat == KIND_SNACK) as usize }
                } else if pressed & (B | PUSH) != 0 {
                    Scene::Confirm { cat, items, idx, yes: true }
                } else {
                    Scene::FoodList { cat, items, idx }
                }
            }
            Scene::Confirm { cat, items, idx, mut yes } => {
                if pressed & (A | UP | DOWN) != 0 {
                    yes = !yes;
                }
                if pressed & C != 0 || (pressed & (B | PUSH) != 0 && !yes) {
                    Scene::FoodList { cat, items, idx }
                } else if pressed & (B | PUSH) != 0 {
                    self.eat(cat, items, idx)
                } else {
                    Scene::Confirm { cat, items, idx, yes }
                }
            }
            Scene::Play(p) => self.update_play(p, pressed),
            Scene::Effects { cat, idx } => match self.pet.complete_effect() {
                Some(id) => {
                    let k = req::EFFECTS.iter().position(|&r| r == id).unwrap_or(0);
                    self.play(&[script::EFFECTS[k]], (0, OnPress::None), None, Scene::Effects { cat, idx })
                }
                None => {
                    let items = self.food_items(cat);
                    let idx = idx.min(items.len() - 1);
                    Scene::FoodList { cat, items, idx }
                }
            },
            Scene::Prompt { wake, mut yes } => {
                if pressed & (A | UP | DOWN) != 0 {
                    yes = !yes;
                }
                if pressed & (B | PUSH) != 0 && yes {
                    if wake { self.pet.wake() } else { self.pet.put_to_bed() };
                    Scene::Home
                } else if pressed & C != 0 || pressed & (B | PUSH) != 0 {
                    if wake { self.pet.no_wake_today = true } else { self.pet.no_sleep_today = true }
                    Scene::Home
                } else {
                    Scene::Prompt { wake, yes }
                }
            }
            Scene::Status { mut page, idle } => {
                if pressed & (A | UP | DOWN) != 0 {
                    page ^= 1;
                }
                if pressed & C != 0 || idle >= 200 {
                    Scene::Home
                } else {
                    Scene::Status { page, idle: if pressed != 0 { 0 } else { idle + 1 } }
                }
            }
        };
    }

    /// Rank-up and mission-clear notices when the pet is back at HOME.
    pub(crate) fn check_notices(&mut self) {
        if let Some(r) = self.pet.take_rank_up() {
            self.sfx(1, 100);
            let text = if r < 10 { 0x8e + r as u16 - 1 } else { 0x96 };
            self.scene = Scene::Message { text, then: Box::new(Scene::Home) };
        } else if !self.pet.claim_missions().is_empty() {
            self.sfx(1, 100);
            self.scene = Scene::Message { text: 0xa8, then: Box::new(Scene::Home) };
        }
    }

    /// Start a pending calendar event when HOME allows it (`FUN_1001abd0`): checked once a minute
    /// with a fresh roll, as the original rolls each time HOME opens.
    pub(crate) fn check_events(&mut self, now: u64, r: u32) {
        let p = &self.pet;
        let sleeping = p.is_sleeping_at(now);
        let hour = now % 86_400 / 3600;
        let busy = [req::ONAKA, req::GOKIGEN, req::SOWASOWA].iter().any(|&q| p.is_pending(q));
        if p.def.stage < 3 || p.cond != Cond::Normal || busy {
            return;
        }
        let (r, period) = (r % 100, crate::pet::Pet::period(now));
        let ev = if p.is_pending(req::EVE) && sleeping && hour > 18 {
            Some((req::EVE, script::EVE))
        } else if sleeping {
            None
        } else if p.is_pending(req::NEWYEAR) {
            Some((req::NEWYEAR, script::HAPPY_JUMP))
        } else if p.is_pending(req::SHOOTINGSTAR) && period == 3 && r < 10 {
            Some((req::SHOOTINGSTAR, script::SHOOTINGSTAR))
        } else if p.is_pending(req::UFO) {
            Some((req::UFO, script::UFO))
        } else if p.is_pending(req::SANTA) && period >= 3 && r < 10 {
            Some((req::SANTA, script::SANTA))
        } else if p.is_pending(req::XMAS) {
            Some((req::XMAS, script::XMAS))
        } else {
            None
        };
        if let Some((id, anim)) = ev {
            self.scene = self.play(&[anim], (0, OnPress::Event(id)), None, Scene::Home);
        }
    }

    pub(crate) fn pet_menu(&self) -> Scene {
        Scene::Menu {
            items: vec![(text::FEED, Some(Action::Feed)), (text::PLAY, None), (text::CLEAN_TAMA, Some(Action::Groom))],
            idx: 0,
            back: None,
        }
    }

    fn field_menu(&self) -> Scene {
        Scene::Menu {
            items: vec![(text::OUTSIDE_TOY, None), (text::EGG_HUNT, None), (text::CLEAN_POOP, Some(Action::CleanPoop)), (text::FIELDS, None)],
            idx: 2,
            back: None,
        }
    }

    /// HOME input (`home_field_tama_state` 0x10039eb8, `home_cell_state` 0x100398c8).
    fn update_home_input(&mut self, pressed: u8) -> Scene {
        if pressed & (UP | DOWN) != 0 {
            self.sfx(0x28, 40);
        }
        if pressed & UP != 0 {
            self.level = match self.level {
                Level::Field => Level::Pet,
                _ if self.pet.def.stage > 1 => Level::Cell,
                l => l,
            };
        }
        if pressed & DOWN != 0 {
            self.level = match self.level {
                Level::Cell => Level::Pet,
                _ => Level::Field,
            };
        }
        if !pressed_any(pressed) || self.pet.def.stage == 1 {
            return Scene::Home;
        }
        let p = &self.pet;
        let sleeping = p.last.is_some_and(|t| p.is_sleeping_at(t));
        if p.cond == Cond::Dead {
            self.pet.new_egg(&self.data);
            return Scene::Home;
        }
        if p.cond == Cond::Death {
            self.pet.rescue();
            return Scene::Home;
        }
        if self.level != Level::Cell && p.is_pending(req::SOWASOWA) {
            self.pet.toilet();
            return self.play(&[script::TOILET], (0, OnPress::None), None, Scene::Home);
        }
        match self.level {
            Level::Cell if p.cond == Cond::Cold => {
                self.play(&[script::MEDICINE, script::CURED], (5, OnPress::Medicine), None, Scene::Home)
            }
            Level::Cell if pressed & A != 0 => Scene::Status { page: 0, idle: 0 },
            Level::Pet if p.cond == Cond::Injury => {
                self.play(&[script::INJURY, script::INJURY_DONE], (4, OnPress::Injury), None, Scene::Home)
            }
            Level::Pet if sleeping && p.is_pending(req::WAKE) && !p.no_wake_today => Scene::Prompt { wake: true, yes: true },
            Level::Pet if p.is_pending(req::SLEEP) && !p.no_sleep_today => Scene::Prompt { wake: false, yes: true },
            _ if pressed & A == 0 => Scene::Home,
            // Sick, injured or asleep pets cannot open the menu.
            _ if sleeping || p.cond != Cond::Normal => {
                self.sfx(0x0b, 100);
                Scene::Home
            }
            Level::Pet => {
                self.sfx(1, 100);
                self.pet_menu()
            }
            Level::Field => {
                self.sfx(1, 100);
                self.field_menu()
            }
            Level::Cell => Scene::Home,
        }
    }

    /// Eat the chosen food (`gohan_state` 0x1003ff80 state 0x4b0/3000).
    fn eat(&mut self, cat: u8, items: Vec<u16>, idx: usize) -> Scene {
        let Some(item) = self.data.item(items[idx]) else { return Scene::FoodList { cat, items, idx } };
        let sprite = Some(item.eat_sprite);
        let back = Scene::FoodList { cat, items: items.clone(), idx };
        let p = &self.pet;
        if !matches!(p.cond, Cond::Normal) {
            return self.play(&[script::REFUSE_SICK], (0, OnPress::None), sprite, back);
        }
        if cat == KIND_MEAL && p.onaka >= 6 {
            return self.play(&[script::REFUSE_FULL], (0, OnPress::None), sprite, back);
        }
        let pref = match p.food_pref(&item) {
            1.. => 1,
            0 => 0,
            _ => 2,
        };
        let thin = (cat == KIND_SNACK || p.onaka >= 3) as usize;
        if !self.pet.feed(&item) {
            return back;
        }
        self.pet.give(item.id, -1);
        let mut scripts = vec![script::SERVE[pref], if item.drink { script::DRINK[pref] } else { script::EAT[pref][thin] }];
        if cat == KIND_MEAL && self.pet.onaka >= 6 {
            scripts.push(script::FULL);
        }
        self.play(&scripts, (0, OnPress::None), sprite, Scene::Effects { cat, idx })
    }

    fn start_poop_clean(&mut self) -> Scene {
        let anim = self.ui_elem(elem::POOP_CLEAN);
        let clean = if anim.kind == UI_ANIM { anim.p[3] as u16 } else { 0 };
        let had = self.pet.clean();
        let mut scripts = vec![clean];
        if had > 0 {
            self.pet.happy(1);
            scripts.push(script::HAPPY_JUMP);
        }
        self.play(&scripts, (0, OnPress::None), None, Scene::Home)
    }

    fn update_play(&mut self, mut p: Play, pressed: u8) -> Scene {
        if p.node.is_paused() && pressed_any(pressed) {
            p.presses += 1;
            if p.presses == p.on_press.0 && p.on_press.1 == OnPress::Groom {
                self.pet.groom();
            }
            p.node.resume();
        }
        p.node.tick(&self.data.motions);
        p.node.take_sounds(&mut self.sounds);
        if !p.node.is_finished() {
            return Scene::Play(p);
        }
        if let Some(next) = p.queue.pop() {
            p.node = Node::new(next);
            return Scene::Play(p);
        }
        // Treatments complete when their animations end.
        match p.on_press.1 {
            OnPress::Medicine => {
                self.pet.medicine();
            }
            OnPress::Injury => {
                self.pet.treat_injury();
            }
            OnPress::Event(id) => self.pet.finish_event(id),
            _ => {}
        }
        *p.then
    }

    // ---- drawing ----------------------------------------------------------

    pub(crate) fn draw_elem(&mut self, id: u16, frame: Option<u16>, palette: Option<usize>) {
        let e = self.ui_elem(id);
        if e.kind != UI_SPRITE {
            return;
        }
        let pal = palette.or((e.p[8] > 0).then_some(e.p[8] as usize));
        let sprite = e.p[3] as u16;
        self.screen.blit(sprite, frame.unwrap_or(e.p[7] as u16), e.x as i32, e.y as i32, e.p[6].clamp(0, 64) as u8, false, pal.map(|s| (sprite, s)));
    }

    /// Font palette sets (font sprite 17): 0 = white, 1 = dark navy.
    pub(crate) fn draw_text(&mut self, id: u16, x: i32, y: i32, center: bool, palette: usize) {
        let glyphs = self.string(id);
        self.screen.text(&self.data.fonts, &glyphs, x, y, center, palette);
    }

    pub(crate) fn draw_number(&mut self, n: u32, x: i32, y: i32, palette: usize) {
        let glyphs: Vec<u16> = n.to_string().bytes().map(|b| DIGIT_0 + (b - b'0') as u16).collect();
        self.screen.text(&self.data.fonts, &glyphs, x, y, true, palette);
    }

    /// Field background sprite for the current field and time of day (`field_bg_create` 0x10015d90).
    pub(crate) fn field_bg(&self) -> Option<u16> {
        let period = self.pet.last.map_or(1, crate::pet::Pet::period) as usize;
        let id = self.data.field_bg.get(self.pet.field as usize)?[0][1 + period * 3];
        let e = self.ui_elem(id);
        (e.kind == UI_SPRITE).then_some(e.p[3] as u16)
    }

    pub(crate) fn draw_field_bg(&mut self) {
        match self.field_bg() {
            Some(bg) => {
                self.screen.blit(bg, 0, 64, 64, 64, false, None);
            }
            None => self.screen.rect(0, 0, 128, 128, [200, 230, 240]),
        }
    }

    pub(crate) fn draw_poop(&mut self) {
        let old = self.pet.unchi.iter().filter(|&&a| a >= crate::pet::UNCHI_OLD).count();
        let frame = (self.frame_count / 15 % 2) as u16;
        for i in 0..self.pet.unchi.len() {
            self.draw_elem(elem::POOP[i], Some(frame), Some((i < old) as usize));
        }
    }

    /// Draw the current non-HOME scene. Returns false for HOME (drawn by the caller).
    pub(crate) fn render_scene(&mut self) -> bool {
        let scene = std::mem::replace(&mut self.scene, Scene::Home);
        let drawn = match &scene {
            Scene::Home => false,
            Scene::Menu { items, idx, .. } => {
                self.draw_elem(elem::MENU_BG, None, None);
                // Vertical wheel: item k from the selection at y = 64 − 88·sin(25°·k).
                let n = items.len() as i32;
                let mut order: Vec<i32> = (-(n / 2)..=(n - 1) / 2).collect();
                order.sort_by_key(|k| -k.abs());
                for k in order {
                    let i = (*idx as i32 + k).rem_euclid(n) as usize;
                    let a = (25.0f32 * k as f32).to_radians();
                    let (x, y) = (64 + (10.0 * (1.0 - (45.0f32 * k as f32).to_radians().cos())) as i32, 64 - (88.0 * a.sin()) as i32);
                    let label = self.ui_elem(elem::MENU_LABEL).p[3] as u16;
                    let gray = items[i].1.is_none() as usize;
                    self.screen.blit(label, 0, x, y, 64, false, (gray > 0).then_some((label, gray)));
                    self.draw_text(items[i].0, x, y, true, 1);
                }
                true
            }
            Scene::FoodCategory { idx } => {
                self.draw_list(&[text::FOOD, text::SNACKS], *idx, None);
                true
            }
            Scene::FoodList { items, idx, .. } => {
                self.draw_food_card(items, *idx);
                true
            }
            Scene::Confirm { yes, .. } => {
                self.draw_list(&[text::YES, text::NO], !*yes as usize, Some(text::EAT_THIS));
                true
            }
            Scene::Prompt { wake, yes } => {
                self.draw_list(&[text::YES, text::NO], !*yes as usize, Some(if *wake { text::WAKE_UP } else { text::TIME_TO_SLEEP }));
                true
            }
            Scene::Play(p) => {
                self.draw_field_bg();
                if p.food.is_some() {
                    self.screen.blit(TABLE, 0, 64, 106, 64, false, None);
                }
                let mut draws = Vec::new();
                p.node.draws(64, 64, &mut draws);
                self.draw_list_of(&draws, p.food);
                true
            }
            Scene::Status { page, .. } => {
                self.draw_status(*page);
                true
            }
            Scene::Effects { .. } => {
                self.draw_field_bg();
                true
            }
            Scene::Card { kind, items, idx } => {
                self.draw_card(*kind, items, *idx);
                true
            }
            Scene::Message { text, .. } => {
                self.draw_list(&[], 0, Some(*text));
                true
            }
            Scene::Ask { text, yes, .. } => {
                self.draw_list(&[text::YES, text::NO], !*yes as usize, Some(*text));
                true
            }
            Scene::Code { digits, pos } => {
                self.draw_code(digits, *pos);
                true
            }
            Scene::Missions { idx } => {
                self.draw_mission(*idx);
                true
            }
        };
        self.scene = scene;
        drawn
    }

    /// Simple vertical list (category, YES/NO): labels on the menu sprite, selection in colour.
    pub(crate) fn draw_list(&mut self, labels: &[u16], sel: usize, title: Option<u16>) {
        self.draw_elem(elem::LIST_BG, None, None);
        let label = self.ui_elem(elem::MENU_LABEL).p[3] as u16;
        let mut top = 40;
        if let Some(t) = title {
            let glyphs = crate::screen::wrap(&self.data.fonts, &self.string(t), 104);
            let lines = glyphs.iter().filter(|&&c| c == crate::screen::NEWLINE).count() as i32 + 1;
            let h = lines * 13 + 8;
            self.screen.rect(8, 10, 112, h, [255, 255, 255]);
            self.screen.text(&self.data.fonts, &glyphs, 64, 10 + 4 + 6, true, 1);
            top = 10 + h + 24;
        }
        for (i, &l) in labels.iter().enumerate() {
            let y = top + i as i32 * 40;
            let gray = (i != sel) as usize;
            self.screen.blit(label, 0, 64, y, 64, false, (gray > 0).then_some((label, gray)));
            self.draw_text(l, 64, y, true, 1);
        }
    }

    /// One food card of the carousel (`itemlist_create` 0x1002cc90, mode 2).
    pub(crate) fn draw_food_card(&mut self, items: &[u16], idx: usize) {
        self.draw_elem(elem::LIST_BG, None, None);
        self.draw_elem(elem::CARD_PANEL, None, None);
        let Some(item) = self.data.item(items[idx]) else { return };
        self.screen.blit(item.icon_sprite, 0, 64, 40, 64, false, None);
        self.draw_elem(elem::CARD_NAME_BAR, None, None);
        self.draw_text(item.name, 64, 92, true, 0);
        self.draw_elem(elem::CARD_CAPSULE, None, None);
        if item.buy != 0xffff {
            let n = self.pet.count(item.id) as u32;
            self.draw_number(n, 12, 8, 1);
        }
        if items.len() > 1 {
            self.screen.blit(ARROWS, 2, 10, 116, 64, false, None);
            self.screen.blit(ARROWS, 3, 118, 116, 64, false, None);
        }
    }

    /// Draw motion-script output with the food placeholder replaced.
    fn draw_list_of(&mut self, draws: &[crate::motion::Draw], food: Option<u16>) {
        for d in draws {
            match d.look {
                crate::motion::Look::Sprite { id, frame } => {
                    let id = if id == FOOD_PLACEHOLDER { food.unwrap_or(id) } else { id };
                    self.screen.blit(id, frame, d.x, d.y, d.alpha, d.flip, None);
                }
                crate::motion::Look::Chr { chr, anim, eyes, mouth } => {
                    self.draw_chr(d, chr, anim, eyes, mouth);
                }
                crate::motion::Look::None => {}
            }
        }
    }

    /// CELL_STATUS (`cellstatus_state` 0x100452f0).
    fn draw_status(&mut self, page: u8) {
        if page == 0 {
            self.draw_elem(elem::STATUS_BG, None, None);
            self.draw_elem(elem::STATUS_FRAME, None, None);
            self.draw_text(text::HUNGER, 10, 11, false, 0);
            for i in 0..self.pet.onaka as usize {
                self.draw_elem(elem::STATUS_HUNGER[i], None, None);
            }
            self.draw_text(text::HAPPY, 10, 58, false, 0);
            self.draw_elem(elem::STATUS_HAPPY_BAR, Some(self.pet.gokigen as u16), None);
            self.draw_number(self.pet.days as u32, 52, 110, 1);
            self.draw_text(text::YEARS, 70, 110, false, 1);
        } else {
            self.draw_elem(elem::NAME_BG, None, None);
            self.draw_elem(elem::NAME_BAR, None, None);
            self.draw_text(self.pet.def.name_id, 64, 18, true, 0);
        }
    }
}
