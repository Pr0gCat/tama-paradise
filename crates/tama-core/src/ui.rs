//! Screens: HOME zoom levels, ring menu, feeding, care animations, prompts, status
//! (docs/spec/08-care-ui.md). Layout coordinates come from the original UI elements.

use crate::data::{UiElem, KIND_MEAL, KIND_SNACK, UI_ANIM, UI_SPRITE};
use crate::input::{A, B, C, DOWN, PUSH, UP};
use crate::motion::Node;
use crate::pet::{req, Cond};
use crate::Game;

mod care;
mod connect;
mod decor;
mod draw;
mod event;
mod hunt;
mod lab;
pub(crate) use lab::{FADE_FULL, FADE_STEP};
mod settings;
mod shop;
mod system;
mod zoom;
pub(crate) use system::{CLOCK_IDLE, MASTER_VOLUME};
pub(crate) use zoom::Change;

pub(crate) use connect::PLAYDATE_ANIM;
pub(crate) use decor::{PET_FOCUS, PLANET_POS};

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
pub(crate) mod script {
    /// EVENT_UNCHI (`FUN_1003e824`): squat, poop, happy.
    pub const POOP: u16 = 0x574;
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
    pub const DODODO: u16 = 0x64a;
    pub const RANK_UP: u16 = 0xc4;
    pub const CREDITS: u16 = 0x16;
    pub const CREDITS_EN: u16 = 0x17;
    /// Birthday cake; role 1 = a grazing tama of this field.
    pub const BIRTHDAY: u16 = 0x6b2;
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
    pub const CARD_COUNT: u16 = 0xd9;
    /// Message box frame (`msgbox_create`).
    pub const MSG_BOX: u16 = 0x9d;
    /// Ring-menu item text (style 1), also the message box text.
    pub const MENU_TEXT: u16 = 0x9e;
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
/// Infinity sign: the count of free items on the food card.
const INFINITY: u16 = 0x67;
/// Melodies (docs/spec/data/sound_usage.csv): a pet call, mission clear, shop code unlocked,
/// first start, egg-hunt trip music.
pub(crate) const SND_CALL: u16 = 3;
const SND_MISSION_CLEAR: u16 = 37;
const SND_CODE_UNLOCKED: u16 = 49;
const SND_FIRST_START: u16 = 60;
const SND_HUNT_TRIP: u16 = 80;
/// String insert code for the planet name.
const PLANET_NAME: u16 = 0xF003;
/// Texts the remake adds (no firmware string), ids from 0xE000 (`hant::REMAKE`, every language):
/// TOUCH ON, TOUCH OFF, SAVE DATA, EXPORT, IMPORT, NEW GAME, the name of zh-Hant, DEVELOPER,
/// GROW UP, +10000, FULL CARE, GAME INFO, UNLOCK ALL, TEST NOTIFY, NOTIFY ON, NOTIFY OFF.
pub(crate) const REMAKE_TEXT: u16 = 0xE000;
/// Rain / snow overlay, 128×128 × 4 frames (UI element 0x2d5).
const PARTICLE: u16 = 0x457;
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
    /// Planet level (HOME_SPACE), below the field.
    Space,
}

/// Zoom stop of each HOME layer (08-care-ui §2.2.1: scale 0.5 planet, 1 field, 2 pet, 4 cell).
pub(crate) fn zoom_stop(l: Level) -> f32 {
    match l {
        Level::Space => -8.0,
        Level::Field => 0.0,
        Level::Pet => 8.0,
        Level::Cell => 16.0,
    }
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
    Fields,
    SwitchField(u8),
    Release,
    ReleaseNow,
    Breed,
    BreedWith(u16),
    Graves,
    Sitter,
    MiniGames,
    LabGame(u8),
    PlayLab(u8, u8),
    Setsubi,
    ToyList,
    /// SETSUBI cursor; toy 0 = REMOVE, field 0xff = start on the current field.
    SetsubiAt {
        toy: u16,
        field: u8,
        area: u8,
        slot: u8,
    },
    PlaceToy {
        toy: u16,
        field: u8,
        area: u8,
        slot: u8,
    },
    RemoveToy {
        field: u8,
        area: u8,
        slot: u8,
    },
    SpaceMenu,
    Stars,
    Deco,
    Wear(u16),
    Travel,
    GoTravel(u8),
    PlanetLv,
    /// Pet menu PLAY: the play-together menu, and one of its games (by game number).
    Together,
    PlayTogether(u8),
    /// EGG HUNT: place menu, team picker for a place, and setting off with three members.
    Hunt,
    HuntKind(u8),
    HuntGo {
        kind: u8,
        team: [u8; 3],
    },
    /// Planet menu CONNECTION and its entries.
    Connect,
    /// Lab menu SETTINGS and its birthday editor.
    Settings,
    /// SETTINGS → TIME submenu (cursor on DATE/TIME, on DISPLAY), back to SETTINGS on TIME.
    SettingsTime,
    SettingsTimeDisplay,
    SettingsAtTime,
    Volume,
    SaveData,
    SettingsAtSaveData,
    /// DEVELOPER (remake): quick progress for testing.
    Developer,
    DevGrow,
    DevPoints,
    DevCare,
    DevUnlock,
    /// ARE YOU SURE? before developer action n (menu order); back to the menu at n.
    DevAsk(u8),
    /// TEST NOTIFY: the page shows a notification (nothing in the save changes).
    DevNotify,
    DeveloperAt(u8),
    SettingsAtDeveloper,
    ExportSave,
    ImportSave,
    NewGameAsk,
    DateTime,
    DateOrder,
    SetDateOrder(u8),
    /// The remake's touch controls on the screen (swipe = dial, tap = dial press).
    ToggleTouch,
    /// SETTINGS → NOTIFY ON/OFF (remake): turning it on asks the page for permission.
    ToggleNotify,
    /// DELETE (cold start with a save): CONTINUE (set the clock), RESET ALL (asks first).
    Delete,
    Continue,
    ResetAllAsk,
    ResetAll,
    /// INFORMATION submenu (cursor on MISSION when coming back from it), USER INFO, and its
    /// edit menu.
    Info,
    InfoFromMissions,
    /// INFORMATION → GAME INFO (remake): this build's version.
    GameInfo,
    InfoFromGameInfo,
    UserInfo,
    UserEdit,
    EditBirthday,
    EditPlanetName,
    /// New-game setup: the language list and a chosen language.
    IntroLanguage,
    IntroLang(u8),
    /// SETTINGS: change the language later.
    Language,
    SetLang(u8),
    ConnectPlaydate,
    ConnectGifts,
    GiftItem(u16),
    Planets,
    /// CONNECTION → MINI GAMES: the Tama Space Arcade menu, and one of its two games.
    Arcade,
    PlayArcade(u8),
    /// HELP_SITTER while sitting, and taking the pet back.
    SitterView,
    EndSitter,
    /// EVENT_DEAD tombstone and its NEW EGG.
    Tomb,
    NewEgg,
    /// Lab menu BOOK (TAMA FILE / MICROSCOPE), MICROSCOPE and its LAB CODE entry.
    Book,
    Microscope,
    LabCode,
    LabCollection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CardKind {
    Buy(u8),
    Sell,
    Cook,
    Toy,
    Wear,
    Travel,
    Gift,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OnPress {
    None,
    Groom,
    Medicine,
    Injury,
    /// Calendar event: reward applied when the animation ends.
    Event(u8),
    /// First aid (`req_done(DEATH)`), end of dying (saved by grave n, or dead), new egg,
    /// sitter leaving.
    Rescue,
    Dying(Option<usize>),
    NewEgg,
    EndSitter,
    /// Any key skips to the end (meals and snacks; the remake's addition).
    Skip,
}

pub(crate) struct Play {
    node: Node,
    queue: Vec<u16>,
    presses: u8,
    /// Care effect applied on the `at`-th button press at a pause point.
    on_press: (u8, OnPress),
    food: Option<u16>,
    /// A melody started together with a later script of the queue: (script, sound).
    cue: Option<(u16, u16)>,
    then: Box<Scene>,
    /// Poop cleaning: the poops swept up the screen (count, how many are old) and frames played.
    sweep: Option<(usize, usize)>,
    frames: u16,
}

pub(crate) enum Scene {
    Home,
    /// Ring menu; C goes back to `back` (or HOME).
    Menu {
        items: Vec<(u16, Option<Action>)>,
        idx: usize,
        back: Option<Action>,
    },
    /// Item carousel for the shop (buy / sell / cook).
    Card {
        kind: CardKind,
        items: Vec<u16>,
        idx: usize,
    },
    /// A notice; any button continues.
    Message {
        text: u16,
        then: Box<Scene>,
    },
    /// YES/NO question.
    Ask {
        text: u16,
        yes: bool,
        on_yes: Action,
        back: Action,
    },
    /// 8-digit code entry.
    Code {
        digits: [u8; 8],
        pos: usize,
    },
    /// Points received (the mission reward page, elements 0x223–0x22a): `amount` G, then `then`
    /// on a key.
    Reward {
        amount: u32,
        then: Box<Scene>,
    },
    /// MISSION COMPLETE from HOME (`FUN_10048bd0` states 4000 / 5000): each new mission's page,
    /// then its reward page (+1000); `t` = frames on the page.
    MissionClear {
        queue: Vec<usize>,
        reward: bool,
        t: u16,
    },
    /// MISSION (PSID_INFO_MISSION 0x39): `view` 0 = overview (cleared / 22), 1 = 2×2 icon list,
    /// 2 = one mission; `idx` = mission, `t` = frames on the page (ticker, idle exit).
    Missions {
        view: u8,
        idx: usize,
        t: u16,
    },
    /// Breeding candidates.
    Breed {
        cands: Vec<u16>,
        idx: usize,
    },
    /// Grave records (lab menu COLLECTION).
    Graves {
        idx: usize,
        back: Action,
    },
    /// SETSUBI: choosing a slot for `toy` (0 = choosing a toy to put away).
    Setsubi {
        toy: u16,
        field: u8,
        area: u8,
        idx: usize,
    },
    /// TAMA STARS (`lookstars_state` 0x10057e54): page 0 the stars with a cursor, 1 the ghost,
    /// 2 its profile. `node` is the cursor (page 0) or the ghost animation (page 1).
    Stars {
        idx: usize,
        page: u8,
        node: Node,
    },
    /// End credits (`FUN_10058a0c`): any key or the end of the script goes on to `then`.
    Credits {
        node: Node,
        then: Box<Scene>,
    },
    /// Page 0: rank; page 1: the next goal.
    PlanetLv {
        page: u8,
    },
    HuntTeam {
        kind: u8,
        idx: usize,
        picked: Vec<usize>,
    },
    /// EDIT PLANET NAME: 8 letters, `pos` = the one being chosen.
    /// `intro` = part of the new-game setup (no way back; it goes on to the next step).
    PlanetName {
        chars: [u16; 8],
        pos: usize,
        intro: bool,
    },
    /// EDIT BIRTHDAY: month and day, `pos` 0 = month.
    Birthday {
        m: u8,
        d: u8,
        pos: u8,
        intro: bool,
    },
    /// CONNECTION → PLANETS: the friend list.
    Friends {
        idx: usize,
    },
    /// Opened eggs; `peach` = the golden-peach page after a great result.
    HuntEggs {
        items: Vec<u16>,
        t: u16,
        peach: bool,
    },
    /// Points earned, then `then` (lab game result screen).
    Points {
        points: u32,
        then: Box<Scene>,
    },
    /// MICROSCOPE's daily +50 for a NO DATA code (`FUN_1005cc78`).
    Bonus,
    /// A mini-game: lab game index and difficulty (None for play-together games).
    Mini {
        session: crate::mini::Session,
        lab: Option<(u8, u8)>,
    },
    FoodCategory {
        idx: usize,
    },
    FoodList {
        cat: u8,
        items: Vec<u16>,
        idx: usize,
    },
    Confirm {
        cat: u8,
        items: Vec<u16>,
        idx: usize,
        yes: bool,
    },
    Play(Play),
    Prompt {
        wake: bool,
        yes: bool,
    },
    Status {
        page: u8,
        idle: u16,
    },
    /// Play the pending eating effects one by one, then show the food list.
    Effects {
        cat: u8,
        idx: usize,
    },
    /// HELP_SITTER while the pet is with the sitter (`FUN_1004d304` state 2000).
    Sitter {
        node: Node,
    },
    /// EVENT_EVOL_SUPPORT (`FUN_1003bd20`): `round` icons taken, `t` frames into this step,
    /// `busy` while the baby takes one in.
    Support {
        pet: Node,
        round: u8,
        t: u16,
        busy: bool,
    },
    /// Before an evolution (`FUN_1003bd20` states 3000–4001): icons of the pet's memories
    /// (sprite, flight order) fly into it, `t` frames in.
    Memories {
        pet: Node,
        icons: Vec<(u16, u8)>,
        t: u16,
    },
    /// After an evolution (`FUN_1003dab8` states 1001–1003): points balance and bonus.
    EvolvePoints {
        t: u16,
    },
    /// LABOGAME_RESULT (`labogame_result_tick` 0x1003298c): the stars earned drop in, then the
    /// balance and the points earned; back to the game menu.
    LabResult {
        stars: u8,
        points: u32,
        nodes: Vec<Node>,
        t: u16,
    },
    /// SETTINGS → SOUND: level 0–3, `idle` frames without A/B.
    Level {
        level: u8,
        idle: u16,
    },
    /// SETTINGS → TIME → DATE/TIME (or CORRECTTIME after DELETE's CONTINUE, `boot`): year,
    /// month, day, hour 1–12, minute, AM/PM; field `pos`.
    Clock {
        vals: [u16; 6],
        pos: u8,
        boot: bool,
    },
    /// LABOMENU's "TAMAGOTCHI Lab" logo, `t` frames in.
    LabSplash {
        t: u16,
    },
    /// LABOMENU: the icon grid, cursor `idx`.
    LabMenu {
        idx: u8,
    },
    /// INFORMATION → USER INFO: planet, days and points; egg-bang day; birthday.
    UserInfo {
        page: u8,
    },
    /// INFORMATION → GAME INFO (remake).
    GameInfo,
    /// EVENT_DYING (`FUN_1003d474`) until DeathRemain runs out.
    Dying {
        node: Node,
    },
    /// The graves after a death (EVENT_DYING state 4000); the last node ends the scene.
    Graveyard {
        nodes: Vec<(Node, i32, i32)>,
    },
    /// EVENT_DEAD (`FUN_1003ccb0`): page 0 the tombstone, 1 the record.
    Dead {
        page: u8,
        nodes: Vec<(Node, i32, i32)>,
    },
    /// MICROSCOPE (`FUN_1004f360`): LAB CODE entry (font glyphs), searching, a character found.
    LabCode {
        chars: [u16; 8],
        pos: usize,
    },
    Searching {
        code: String,
        t: u16,
    },
    /// A microscope character (`FUN_1005c0b0`): just found (`from` None, with NEW!) or opened
    /// from COLLECTION (`from` = its card); the dial zooms in and out.
    Found {
        chr: u16,
        zoom: bool,
        from: Option<usize>,
    },
    /// MICROSCOPE → COLLECTION (`FUN_1005ead0`): cards of the 29 lab-code characters.
    Collection {
        idx: usize,
    },
}

fn pressed_any(p: u8) -> bool {
    p & (A | B | C | PUSH) != 0
}

impl Game {
    fn ui_elem(&self, id: u16) -> UiElem {
        self.data.ui(id)
    }

    pub(crate) fn string(&self, id: u16) -> Vec<u16> {
        if let Some(i) = id.checked_sub(REMAKE_TEXT).map(usize::from).filter(|&i| i < crate::hant::REMAKE[0].len()) {
            let texts = crate::hant::REMAKE.get(self.pet.lang as usize).unwrap_or(&crate::hant::REMAKE[1]);
            return texts[i].to_vec();
        }
        let s = self
            .data
            .strings
            .get(self.pet.lang as usize)
            .or_else(|| self.data.strings.get(1))
            .and_then(|t| t.get(id as usize))
            .cloned()
            .unwrap_or_default();
        // Insert code 0xF003 stands for the planet's name.
        s.into_iter().flat_map(|c| if c == PLANET_NAME { self.pet.planet_name.clone() } else { vec![c] }).collect()
    }

    pub(crate) fn play(&self, scripts: &[u16], on_press: (u8, OnPress), food: Option<u16>, then: Scene) -> Scene {
        let mut queue: Vec<u16> = scripts.iter().rev().copied().collect();
        let first = queue.pop().unwrap_or(0);
        let node = Node::started(first, &self.data.motions);
        Scene::Play(Play {
            node,
            queue,
            presses: 0,
            on_press,
            food,
            cue: None,
            then: Box::new(then),
            sweep: None,
            frames: 0,
        })
    }

    /// Sound ids (04-assets SND8, 08-care-ui U6): 0 cursor, 1 confirm, 2 cancel, 0xb not allowed, 0x28 zoom.
    pub(crate) fn sfx(&mut self, id: u16, vol: u8) {
        self.sounds.push((id, vol));
    }

    /// Handle one frame of input for the current scene.
    pub(crate) fn update_scene(&mut self, pressed: u8) {
        if self.scene.has_menu_sounds() {
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
            Scene::Home if self.level == Level::Space => self.update_space_input(pressed),
            Scene::Home => self.update_home_input(pressed),
            Scene::Menu { items, idx, back } => self.update_menu(items, idx, back, pressed),
            Scene::Ask { text, yes, on_yes, back } => self.update_ask(text, yes, on_yes, back, pressed),
            // Notices: any button goes on.
            Scene::Message { text, then } => {
                if pressed_any(pressed) {
                    *then
                } else {
                    Scene::Message { text, then }
                }
            }
            Scene::Points { points, then } => {
                if pressed_any(pressed) {
                    *then
                } else {
                    Scene::Points { points, then }
                }
            }
            Scene::Bonus => {
                if pressed_any(pressed) {
                    self.open(Action::Microscope)
                } else {
                    Scene::Bonus
                }
            }
            Scene::Found { chr, zoom, from } => self.update_found(chr, zoom, from, pressed),
            Scene::Collection { idx } => self.update_collection(idx, pressed),
            // Care (ui/care.rs).
            Scene::FoodCategory { idx } => self.update_food_category(idx, pressed),
            Scene::FoodList { cat, items, idx } => self.update_food_list(cat, items, idx, pressed),
            Scene::Confirm { cat, items, idx, yes } => self.update_confirm(cat, items, idx, yes, pressed),
            Scene::Effects { cat, idx } => self.update_effects(cat, idx),
            Scene::Play(p) => self.update_play(p, pressed),
            Scene::Prompt { wake, yes } => self.update_prompt(wake, yes, pressed),
            Scene::Status { page, idle } => self.update_status(page, idle, pressed),
            Scene::Credits { node, then } => self.update_credits(node, then, pressed),
            // Life events (ui/event.rs).
            Scene::Sitter { node } => self.update_sitter(node, pressed),
            Scene::Support { pet, round, t, busy } => self.update_support(pet, round, t, busy, pressed),
            Scene::Memories { pet, icons, t } => self.update_memories(pet, icons, t),
            Scene::EvolvePoints { t } => self.update_evolve_points(t, pressed),
            Scene::UserInfo { page } => self.update_user_info(page, pressed),
            Scene::GameInfo if pressed_any(pressed) => {
                self.sfx(2, 100);
                self.open(Action::InfoFromGameInfo)
            }
            Scene::GameInfo => Scene::GameInfo,
            Scene::LabSplash { t } => self.update_lab_splash(t),
            Scene::LabMenu { idx } => self.update_lab_menu(idx, pressed),
            Scene::Level { level, idle } => self.update_level(level, idle, pressed),
            Scene::Clock { vals, pos, boot } => self.update_clock(vals, pos, boot, pressed),
            Scene::LabResult { stars, points, nodes, t } => self.update_lab_result(stars, points, nodes, t, pressed),
            Scene::Dying { node } => self.update_dying(node),
            Scene::Graveyard { nodes } => self.update_graveyard(nodes),
            Scene::Dead { page, nodes } => self.update_dead(page, nodes, pressed),
            Scene::LabCode { chars, pos } => self.update_lab_code(chars, pos, pressed),
            Scene::Searching { code, t } => self.update_searching(code, t),
            // Lab menu, shop, family, mini-games (ui/shop.rs).
            Scene::Card { kind, items, idx } => self.update_card(kind, items, idx, pressed),
            Scene::Code { digits, pos } => self.update_code(digits, pos, pressed),
            Scene::Breed { cands, idx } => self.update_breed(cands, idx, pressed),
            Scene::Graves { idx, back } => self.update_graves(idx, back, pressed),
            Scene::Missions { view, idx, t } => self.update_missions(view, idx, t, pressed),
            Scene::MissionClear { queue, reward, t } => self.update_mission_clear(queue, reward, t, pressed),
            Scene::Reward { then, .. } if pressed_any(pressed) => *then,
            s @ Scene::Reward { .. } => s,
            Scene::Mini { session, lab } => self.update_mini(session, lab, pressed),
            // Settings (ui/settings.rs).
            Scene::PlanetName { chars, pos, intro } => self.update_planet_name(chars, pos, intro, pressed),
            Scene::Birthday { m, d, pos, intro } => self.update_birthday(m, d, pos, intro, pressed),
            // Field, planet and connection (ui/decor.rs, ui/hunt.rs, ui/connect.rs).
            Scene::Setsubi { toy, field, area, idx } => self.update_setsubi(toy, field, area, idx, pressed),
            Scene::HuntTeam { kind, idx, picked } => self.update_hunt_team(kind, idx, picked, pressed),
            Scene::HuntEggs { items, t, peach } => self.update_hunt_eggs(items, t, peach, pressed),
            Scene::PlanetLv { page } => self.update_planet_lv(page, pressed),
            Scene::Stars { idx, page, node } => self.update_stars(idx, page, node, pressed),
            Scene::Friends { idx } => self.update_friends(idx, pressed),
        };
    }

    /// Ring menu: A / dial move, B or the dial button opens the item, C goes back.
    fn update_menu(
        &mut self,
        items: Vec<(u16, Option<Action>)>,
        idx: usize,
        back: Option<Action>,
        pressed: u8,
    ) -> Scene {
        let idx = cycle_list(idx, items.len(), pressed);
        // The wheel turns half the remaining way each frame (`listscroll` 0x10005f30). The
        // firmware ignores B and C until it stops; the remake takes them at once.
        let n = items.len() as f32;
        let key = (items.len(), items.first().map_or(0, |i| i.0), back);
        let pos = self.menu_pos.filter(|m| (m.0, m.1, m.2) == key).map_or(idx as f32, |m| m.3);
        let mut d = (idx as f32 - pos).rem_euclid(n);
        if d > n / 2.0 {
            d -= n;
        }
        let pos = if d.abs() < 0.25 { idx as f32 } else { (pos + d / 2.0).rem_euclid(n) };
        self.menu_pos = Some((key.0, key.1, key.2, pos));
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

    /// YES / NO question.
    fn update_ask(&mut self, text: u16, mut yes: bool, on_yes: Action, back: Action, pressed: u8) -> Scene {
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
}

impl Scene {
    /// Scenes that click on cursor moves, confirm and cancel (U6).
    fn has_menu_sounds(&self) -> bool {
        matches!(
            self,
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
                | Scene::MissionClear { .. }
                | Scene::Reward { .. }
                | Scene::UserInfo { .. }
                | Scene::GameInfo
                | Scene::Breed { .. }
                | Scene::Graves { .. }
                | Scene::Setsubi { .. }
                | Scene::Friends { .. }
                | Scene::Birthday { .. }
                | Scene::Stars { .. }
                | Scene::PlanetName { .. }
                | Scene::PlanetLv { .. }
                | Scene::LabCode { .. }
                | Scene::Collection { .. }
        )
    }
}

/// Wrapping list cursor: A or dial up moves forward, dial down back.
/// Vertical lists (menus, the food list): the remake turns the dial the other way there, so DOWN
/// (the page's left button) moves to the next item and UP (right) to the previous one. A still
/// steps forward.
pub(crate) fn cycle_list(idx: usize, n: usize, pressed: u8) -> usize {
    let swapped =
        pressed & !(UP | DOWN) | if pressed & UP != 0 { DOWN } else { 0 } | if pressed & DOWN != 0 { UP } else { 0 };
    cycle(idx, n, swapped)
}

impl Game {
    /// The number editors' keys (`numspin` 0x1002df08): A while C is held steps back, and only a
    /// second C press 3–10 frames after letting go cancels (`key_double(4)`). Returns the presses
    /// to step with and whether to cancel.
    pub(crate) fn spin_keys(&self, pressed: u8) -> (u8, bool) {
        let step = if pressed & A != 0 && self.held & C != 0 { pressed & !A | DOWN } else { pressed };
        let since = self.frame_count.wrapping_sub(self.c_released);
        (step, pressed & C != 0 && ((3..=10).contains(&since) || self.touch_back))
    }
}

pub(crate) fn cycle(idx: usize, n: usize, pressed: u8) -> usize {
    let n = n.max(1);
    if pressed & (A | UP) != 0 {
        (idx + 1) % n
    } else if pressed & DOWN != 0 {
        (idx + n - 1) % n
    } else {
        idx
    }
}
