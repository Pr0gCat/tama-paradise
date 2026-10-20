//! Life events at HOME: the tama sitter, the baby's evolution support, first aid, dying, the
//! graves and the tombstone (docs/spec/01-care.md §2.9, 05-content §2.2, 08-care-ui §2.12).

use super::{pressed_any, Action, Level, OnPress, Scene};
use crate::input::{A, B, C};
use crate::motion::Node;
use crate::pet::{req, Cond};
use crate::Game;

/// Motion scripts.
mod script {
    /// HELP_SITTER (`FUN_1004d304`): arrives, stays (80% / 20%), leaves.
    pub const SITTER_ARRIVE: u16 = 0x831;
    pub const SITTER_STAY: [u16; 2] = [0x832, 0x833];
    pub const SITTER_LEAVE: u16 = 0x834;
    /// EVENT_EVOL_SUPPORT (`FUN_1003bd20`): the baby waiting, and taking in an icon.
    pub const SUPPORT_WAIT: u16 = 0x54a;
    pub const SUPPORT_TAKE: u16 = 0x547;
    /// EVENT_EVOLUTION (`FUN_1003dab8`): hatching (alone / with a parent), each in two parts,
    /// and the evolution of the other stages (old look role 0, new look role 1).
    pub const HATCH: [[u16; 2]; 2] = [[0x53c, 0x53e], [0x541, 0x543]];
    pub const EVOLVE: u16 = 0x549;
    /// LABOGAME_RESULT: a star dropping into its slot.
    pub const RESULT_STAR: u16 = 0x95e;
    /// EVENT_RESCUE (`FUN_1003eae4`): first aid, then recovered.
    pub const RESCUE: [u16; 2] = [0x5ce, 0x5d4];
    /// EVENT_DYING (`FUN_1003d474`): dying (young / old age), saved by an ancestor, death
    /// (young), old age (then the soul rising).
    pub const DYING: [u16; 2] = [0x5db, 0x5ed];
    pub const SAVED: u16 = 0x5e0;
    pub const DEATH: u16 = 0x5e7;
    pub const OLD_AGE: [u16; 2] = [0x5ef, 0x5f4];
    /// Graves screen: two background layers, a new grave (+1 for old age), the closing scene.
    pub const GRAVES_BG: [u16; 2] = [191, 192];
    pub const NEW_GRAVE: u16 = 200;
    pub const GRAVES: u16 = 194;
    /// EVENT_DEAD (`FUN_1003ccb0`): tombstone (+1 for old age), flowers, the new egg arriving.
    pub const TOMB: u16 = 169;
    pub const FLOWERS: u16 = 21;
    pub const NEW_EGG: u16 = 0x672;
}

/// UI elements.
mod elem {
    pub const SUPPORT_ICONS: [u16; 4] = [0x35a, 0x377, 0x374, 0x364];
    /// Memory icon slots 0x35a + i (`FUN_1003bd20` state 3000).
    pub const MEMORY_SLOTS: u16 = 0x35a;
    /// Evolution points screen: background, balance bar, bonus bar, banner, then (z 10) the
    /// points icon and the "G" mark.
    pub const EVOLVE_POINTS: [u16; 6] = [0x223, 0x226, 0x229, 0x22a, 0x224, 0x228];
    pub const EVOLVE_BALANCE: u16 = 0x225;
    pub const EVOLVE_BONUS: u16 = 0x227;
    /// Lab game result: background, balance bar, points icon; balance; star slots; panel and "G".
    pub const RESULT: [u16; 3] = [0x3da, 0x3db, 0x3dd];
    pub const RESULT_BALANCE: u16 = 0x3dc;
    pub const RESULT_STAR_SLOTS: [u16; 3] = [0x3df, 0x3e0, 0x3e1];
    pub const RESULT_PANEL: [u16; 2] = [0x3e4, 0x3e3];
    pub const RESULT_POINTS: u16 = 0x3e2;
    pub const GRAVE_SLOTS: u16 = 0x2d0;
    pub const TOMB_BG: u16 = 0x13f;
    pub const FLOWERS: [u16; 2] = [0x142, 0x143];
    pub const TOMB: u16 = 0x140;
    pub const RECORD_BG: u16 = 0x194;
    pub const RECORD_BARS: [u16; 3] = [0x147, 0x14a, 0x14d];
    pub const NAME: u16 = 0x146;
    pub const DAYS: u16 = 0x148;
    pub const YEARS: u16 = 0x149;
    pub const CAUSE: u16 = 0x14b;
    pub const CAUSE_TEXT: u16 = 0x14c;
}

/// Area icon sprites for the support scene (`FUN_1000ae44`, XIP 0x10092ddc).
const AREA_ICONS: [u16; 3] = [0x4a0, 0x4a1, 0x49c];
/// Food category icons 0–17 (`FUN_1000ae78`, XIP 0x10092de2).
const FOOD_ICONS: [u16; 18] = [
    0x490, 0x491, 0x492, 0x493, 0x494, 0x495, 0x496, 0x497, 0x498, 0x49d, 0x49e, 0x49f, 0x4a2, 0x4a3, 0x4a4, 0x4a5,
    0x4a6, 0x4a7,
];
/// Memory icons for care mistakes, full-hunger and full-mood top-ups.
const MISS_ICON: u16 = 0x499;
const ONAKA_ICON: u16 = 0x49a;
const GOKIGEN_ICON: u16 = 0x49b;
/// A memory icon starts flying every 2 frames; 20 frames' pause after the last one.
const MEMORY_GAP: u16 = 2;
const MEMORY_PAUSE: u16 = 20;
/// Babies older than this (4 h) evolve with memories, not support (`FUN_1003bd20`).
const SUPPORT_LIMIT: u32 = 14_400;
/// Evolution points screen: keys work after 20 frames, it closes itself after 200.
const POINTS_WAIT: u16 = 20;
const POINTS_TIMEOUT: u16 = 200;
const SND_EVOLVE_POINTS: u16 = 0x21;
/// Lab game result: stars drop in after 11 frames and land 40 frames later with the sound for
/// the result (XIP 0x100a13b8); keys work 11 frames after that, and it closes itself after 100.
const RESULT_STARS_AT: u16 = 11;
const RESULT_STAR_LAND: u16 = 40;
const RESULT_WAIT: u16 = 11;
const RESULT_TIMEOUT: u16 = 100;
const RESULT_SOUNDS: [u16; 4] = [36, 41, 41, 24];
/// Icons are taken in this order (context slots 10, 13, 12, 11).
const SUPPORT_ORDER: [usize; 4] = [0, 3, 2, 1];
/// Where the icons fly to (`FUN_1006f290(.., 0x40, 0x36)`).
const SUPPORT_TARGET: (i32, i32) = (64, 54);
/// An icon flies in over 1 s on curve 5 (strong ease-in), position and fade alike (`FUN_10066b34`).
const SUPPORT_FLY: u16 = 20;
const FLY_CURVE: u8 = 5;
const END_SITTING: u16 = 0xd4;
const NEW_EGG: u16 = 0x18;
const YEARS: u16 = 0xa0;
const CAUSE: u16 = 0x79;
const SFX_OK: u16 = 1;
const FLOWER_DROP: i32 = 256;
const SFX_BACK: u16 = 2;

/// "%d"/"%02d" output as text codes (`FUN_1008df10`): digit d → d + 2, ' ' → 1, '-' → 0xd.
pub(crate) fn printf_digits(s: &str) -> Vec<u16> {
    s.bytes()
        .map(|b| match b {
            b'0'..=b'9' => (b - b'0') as u16 + 2,
            b'-' => 0xd,
            _ => 1,
        })
        .collect()
}

impl Game {
    /// HOME's routing to event scenes (`FUN_1001abd0`): dead → EVENT_DEAD, dying → EVENT_DYING,
    /// with the sitter → HELP_SITTER.
    pub(crate) fn route_home(&mut self) {
        self.scene = match self.pet.cond {
            Cond::Dead => self.tomb(),
            Cond::Dying => {
                let old = (self.pet.days >= 30) as usize;
                Scene::Dying { node: Node::new(script::DYING[old]) }
            }
            Cond::Sitter => Scene::Sitter { node: Node::new(script::SITTER_STAY[0]) },
            _ => return,
        };
    }

    pub(crate) fn open_event(&mut self, a: Action) -> Scene {
        match a {
            Action::SitterView => Scene::Sitter { node: Node::new(script::SITTER_STAY[0]) },
            Action::EndSitter => self.play(&[script::SITTER_LEAVE], (0, OnPress::EndSitter), None, Scene::Home),
            Action::Tomb => self.tomb(),
            Action::NewEgg => {
                self.level = Level::Field;
                self.play(&[script::NEW_EGG], (0, OnPress::NewEgg), None, Scene::Home)
            }
            _ => Scene::Home,
        }
    }

    pub(crate) fn sitter_arrives(&self) -> Scene {
        Scene::Sitter { node: Node::new(script::SITTER_ARRIVE) }
    }

    /// Pet layer, first-aid call (HOME state 0x19): any key starts EVENT_RESCUE.
    pub(crate) fn rescue(&mut self) -> Scene {
        self.sfx(SFX_OK, 100);
        if !self.pet.is_pending(req::DEATH) {
            return Scene::Home;
        }
        self.play(&script::RESCUE, (0, OnPress::Rescue), None, Scene::Home)
    }

    /// Cell layer, support call (HOME state 0x1a): any key starts EVENT_EVOL_SUPPORT.
    pub(crate) fn support(&mut self) -> Scene {
        self.sfx(SFX_OK, 100);
        Scene::Support { pet: Node::new(script::SUPPORT_WAIT), round: 0, t: 0, busy: false }
    }

    pub(crate) fn update_sitter(&mut self, mut node: Node, pressed: u8) -> Scene {
        if self.pet.cond != Cond::Sitter {
            // 19:00: the sitter brings the pet back to the field.
            self.level = Level::Field;
            return self.play(&[script::SITTER_LEAVE], (0, OnPress::None), None, Scene::Home);
        }
        if pressed_any(pressed) {
            return Scene::Ask { text: END_SITTING, yes: false, on_yes: Action::EndSitter, back: Action::SitterView };
        }
        node.tick(&self.data.motions);
        node.take_sounds(&mut self.sounds);
        if node.is_finished() {
            let stay = (self.rand() % 100 >= 80) as usize;
            node = Node::new(script::SITTER_STAY[stay]);
        }
        Scene::Sitter { node }
    }

    /// Four icons of the dominant area fly into the baby, one per key press (or after 10 s),
    /// then it evolves (EVENT_EVOLUTION, `req_done(EVOL_SUPPORT)`).
    pub(crate) fn update_support(
        &mut self,
        mut pet: Node,
        mut round: u8,
        mut t: u16,
        mut busy: bool,
        pressed: u8,
    ) -> Scene {
        pet.tick(&self.data.motions);
        pet.take_sounds(&mut self.sounds);
        t += 1;
        if busy {
            if t >= 40 {
                (pet, busy, round, t) = (Node::new(script::SUPPORT_WAIT), false, round + 1, 0);
            }
        } else if round == 4 {
            if t >= 20 {
                return self.evolution();
            }
        } else if pressed & (A | B | C | crate::input::PUSH) != 0 || t >= 200 {
            self.sfx(SFX_OK, 100);
            (pet, busy, t) = (Node::new(script::SUPPORT_TAKE), true, 0);
        }
        Scene::Support { pet, round, t, busy }
    }

    /// A pending EVOLUTION at HOME (`FUN_10019b48` → EVENT_EVOL_SUPPORT `FUN_1003bd20`): the egg
    /// hatches at once, a young baby waiting for support evolves at once, everyone else first
    /// takes in the icons of what it lived through.
    pub(crate) fn begin_evolution(&mut self) -> Scene {
        let p = &self.pet;
        if p.def.stage == 2 && p.age < SUPPORT_LIMIT && p.is_pending(req::EVOL_SUPPORT) {
            return Scene::Support { pet: Node::new(script::SUPPORT_WAIT), round: 0, t: 0, busy: false };
        }
        if p.def.stage < 2 || (p.def.stage == 2 && p.age < SUPPORT_LIMIT) {
            return self.evolution();
        }
        let icons = self.memory_icons();
        // They fly in in random order.
        let mut order: Vec<u8> = (0..icons.len() as u8).collect();
        for i in (1..order.len()).rev() {
            let j = self.rand() as usize % (i + 1);
            order.swap(i, j);
        }
        let icons = icons.into_iter().zip(order).collect();
        Scene::Memories { pet: Node::new(script::SUPPORT_WAIT), icons, t: 0 }
    }

    /// What the pet lived through, as icons (also shown in the cell): areas (≤ 10), native foods
    /// (≤ 4), care mistakes, full hunger and full mood (≤ 6 each).
    pub(crate) fn memory_icons(&self) -> Vec<u16> {
        let p = &self.pet;
        let mut icons: Vec<u16> = p.field_log.iter().take(10).map(|&f| AREA_ICONS[(f as usize).min(2)]).collect();
        icons.extend(p.food_log.iter().take(4).map(|&c| FOOD_ICONS.get(c as usize).copied().unwrap_or(FOOD_ICONS[0])));
        for (n, icon) in [(p.miss_count, MISS_ICON), (p.onaka_max, ONAKA_ICON), (p.gokigen_max, GOKIGEN_ICON)] {
            icons.extend(std::iter::repeat_n(icon, n.min(6) as usize));
        }
        icons
    }

    pub(crate) fn update_memories(&mut self, mut pet: Node, icons: Vec<(u16, u8)>, t: u16) -> Scene {
        pet.tick(&self.data.motions);
        pet.take_sounds(&mut self.sounds);
        if t >= icons.len() as u16 * MEMORY_GAP + SUPPORT_FLY + MEMORY_PAUSE {
            return self.evolution();
        }
        Scene::Memories { pet, icons, t: t + 1 }
    }

    pub(crate) fn draw_memories(&mut self, pet: &Node, icons: &[(u16, u8)], t: u16) {
        self.draw_field_bg();
        let mut draws = Vec::new();
        pet.draws(64, 64, &mut draws);
        self.draw_list_of(&draws, None);
        for (slot, &(icon, k)) in icons.iter().enumerate() {
            let flown = t.saturating_sub(k as u16 * MEMORY_GAP).min(SUPPORT_FLY) as i32;
            if flown == SUPPORT_FLY as i32 {
                continue;
            }
            let e = self.ui_elem(elem::MEMORY_SLOTS + slot as u16);
            let fly = |a: i32, b: i32| crate::ease::ease(FLY_CURVE, a, b, flown as u32, SUPPORT_FLY as u32);
            let (x, y) = (fly(e.x as i32, SUPPORT_TARGET.0), fly(e.y as i32, SUPPORT_TARGET.1));
            self.screen.blit(icon, 0, x, y, fly(64, 0) as u8, false, None);
        }
    }

    /// EVENT_EVOLUTION (`FUN_1003dab8`): evolve, then hatch (with the parent who stayed on the
    /// field from the second generation on), or show the old look turning into the new one and
    /// the points it brought. Back at the pet level.
    pub(crate) fn evolution(&mut self) -> Scene {
        let old = self.pet.def.chr_id;
        if !self.pet.evolve(&self.data) {
            return Scene::Home;
        }
        self.level = Level::Pet;
        if self.pet.def.stage == 2 {
            let pair = self.pet.generation >= 2;
            if pair {
                // ponytail: the parent is the latest tama left on this field (`FUN_1001887c`
                // reads the last grazing field); fine while the family stays on one field.
                let parent = self.pet.grazing[self.pet.field as usize].last().map(|g| g.id);
                self.roles[1] = parent.and_then(|id| self.data.character(id)).map(|c| c.chr_id);
            }
            return self.play(&script::HATCH[pair as usize], (0, OnPress::None), None, Scene::Home);
        }
        self.old_chr = Some(old);
        self.play(&[script::EVOLVE], (0, OnPress::None), None, Scene::EvolvePoints { t: 0 })
    }

    pub(crate) fn update_evolve_points(&mut self, t: u16, pressed: u8) -> Scene {
        if t == 0 {
            self.sfx(SND_EVOLVE_POINTS, 100);
        }
        if (t >= POINTS_WAIT && pressed_any(pressed)) || t >= POINTS_TIMEOUT {
            return Scene::Home;
        }
        Scene::EvolvePoints { t: t + 1 }
    }

    /// The balance after the evolution and the bonus for the new stage (300 / 500 / 800).
    pub(crate) fn draw_evolve_points(&mut self) {
        for id in elem::EVOLVE_POINTS {
            self.draw_elem(id, None, None);
        }
        let bonus = match self.pet.def.stage {
            3 => 300,
            4 => 500,
            5 => 800,
            _ => 0,
        };
        for (id, n) in [(elem::EVOLVE_BALANCE, self.pet.points), (elem::EVOLVE_BONUS, bonus)] {
            self.draw_number_elem(id, n);
        }
    }

    /// A number in a text element, printed with "%d" (`text_printf` 0x10064f1c: digit d is
    /// code d + 2 in the element's font).
    pub(crate) fn draw_number_elem(&mut self, id: u16, n: u32) {
        self.draw_text_elem(id, &printf_digits(&n.to_string()));
    }

    /// Text in a UI text element, laid out as its p[] says (font, alignment, spacing, palette).
    pub(crate) fn draw_text_elem(&mut self, id: u16, glyphs: &[u16]) {
        let e = self.ui_elem(id);
        let b = crate::screen::TextBox {
            font: e.p[3].max(0) as usize,
            h: (e.p[8] >> 4 & 3) as u8,
            v: (e.p[8] >> 6 & 3) as u8,
            space: e.p[4] as i32,
            leading: e.p[5] as i32,
            palette: e.p[7].max(0) as usize,
        };
        self.screen.text_box(&self.data.fonts, glyphs, e.x as i32, e.y as i32, b);
    }

    pub(crate) fn update_lab_result(
        &mut self,
        stars: u8,
        points: u32,
        mut nodes: Vec<Node>,
        t: u16,
        pressed: u8,
    ) -> Scene {
        for n in &mut nodes {
            n.tick(&self.data.motions);
        }
        if t == RESULT_STARS_AT {
            nodes = (0..stars).map(|_| Node::new(script::RESULT_STAR)).collect();
        }
        if t == RESULT_STARS_AT + if stars > 0 { RESULT_STAR_LAND } else { 0 } {
            self.sfx(RESULT_SOUNDS[stars as usize], 100);
        }
        let open = RESULT_STARS_AT + if stars > 0 { RESULT_STAR_LAND } else { 0 } + RESULT_WAIT;
        if t >= open && (pressed_any(pressed) || t >= open + RESULT_TIMEOUT) {
            if pressed_any(pressed) {
                self.sfx(SFX_OK, 100);
            }
            return self.open(Action::MiniGames);
        }
        Scene::LabResult { stars, points, nodes, t: t + 1 }
    }

    pub(crate) fn draw_lab_result(&mut self, stars: u8, points: u32, nodes: &[Node], t: u16) {
        for id in elem::RESULT {
            self.draw_elem(id, None, None);
        }
        self.draw_number_elem(elem::RESULT_BALANCE, self.pet.points);
        let landed = t >= RESULT_STARS_AT + RESULT_STAR_LAND;
        for (i, &id) in elem::RESULT_STAR_SLOTS.iter().enumerate() {
            let frame = (landed && (i as u8) < stars) as u16;
            self.draw_elem(id, Some(frame), None);
        }
        for id in elem::RESULT_PANEL {
            self.draw_elem(id, None, None);
        }
        self.draw_number_elem(elem::RESULT_POINTS, points);
        let mut draws = Vec::new();
        for (n, &slot) in nodes.iter().zip(&elem::RESULT_STAR_SLOTS) {
            let e = self.ui_elem(slot);
            n.draws(e.x as i32, e.y as i32, &mut draws);
        }
        self.draw_list_of(&draws, None);
    }

    /// DeathRemain (60 s) runs out, then: an old-age ancestor saves the pet, or it dies (young,
    /// or of old age) and the graves are shown.
    pub(crate) fn update_dying(&mut self, mut node: Node) -> Scene {
        node.tick(&self.data.motions);
        node.take_sounds(&mut self.sounds);
        if self.pet.cond != Cond::Dying || self.pet.death_remain > 0 {
            return Scene::Dying { node };
        }
        if let Some(i) = self.pet.dying_ancestor() {
            let ghost = self.pet.graves[i].ghost.id;
            self.roles[1] = self.data.character(ghost).map(|c| c.chr_id);
            return self.play(&[script::SAVED], (0, OnPress::Dying(Some(i))), None, Scene::Home);
        }
        let old = self.pet.days >= 30;
        let slot = self.pet.graves.len().min(4);
        let at = |id| {
            let e = self.ui_elem(id);
            (e.x as i32, e.y as i32)
        };
        let (gx, gy) = at(elem::GRAVE_SLOTS + slot as u16);
        let mut nodes: Vec<(Node, i32, i32)> = script::GRAVES_BG.iter().map(|&s| (Node::new(s), 64, 64)).collect();
        nodes.push((Node::new(script::NEW_GRAVE + old as u16), gx, gy));
        nodes.push((Node::new(script::GRAVES), 90, 92));
        let graves = Scene::Graveyard { nodes };
        let scripts: &[u16] = if old { &script::OLD_AGE } else { &[script::DEATH] };
        if old {
            // ponytail: the firmware clears the dirt between the two old-age scripts; done up front.
            self.pet.status &= !crate::pet::status::DIRTY;
        }
        self.play(scripts, (0, OnPress::Dying(None)), None, graves)
    }

    pub(crate) fn update_graveyard(&mut self, mut nodes: Vec<(Node, i32, i32)>) -> Scene {
        for (n, _, _) in &mut nodes {
            n.tick(&self.data.motions);
            n.take_sounds(&mut self.sounds);
        }
        // The new grave's script signals the end (the planet beside it loops forever).
        if nodes.get(2).is_some_and(|(n, _, _)| n.is_finished()) {
            return Scene::Home;
        }
        Scene::Graveyard { nodes }
    }

    fn tomb(&self) -> Scene {
        let old = self.pet.daiojo() as u16;
        let at = |id| {
            let e = self.ui_elem(id);
            (e.x as i32, e.y as i32)
        };
        let mut nodes: Vec<(Node, i32, i32)> = elem::FLOWERS
            .iter()
            .map(|&f| {
                // The flowers drop in from 256 px above (`FUN_10066968`).
                let (x, y) = at(f);
                (Node::new(script::FLOWERS), x, y - FLOWER_DROP)
            })
            .collect();
        let (x, y) = at(elem::TOMB);
        nodes.push((Node::new(script::TOMB + old), x, y));
        Scene::Dead { page: 0, nodes }
    }

    /// Tombstone: B shows the record (C back); holding C and pressing A asks for a NEW EGG.
    pub(crate) fn update_dead(&mut self, page: u8, mut nodes: Vec<(Node, i32, i32)>, pressed: u8) -> Scene {
        if page == 1 {
            if pressed & C != 0 {
                self.sfx(SFX_BACK, 100);
                return self.tomb();
            }
            return Scene::Dead { page, nodes };
        }
        if pressed & B != 0 {
            self.sfx(SFX_OK, 100);
            return Scene::Dead { page: 1, nodes: Vec::new() };
        }
        if self.held & C != 0 && pressed & A != 0 {
            self.sfx(SFX_OK, 100);
            return Scene::Menu { items: vec![(NEW_EGG, Some(Action::NewEgg))], idx: 0, back: Some(Action::Tomb) };
        }
        for (k, (n, _, y)) in nodes.iter_mut().enumerate() {
            n.tick(&self.data.motions);
            n.take_sounds(&mut self.sounds);
            if let Some(&f) = elem::FLOWERS.get(k) {
                // ponytail: the firmware's tween curve is not decoded; a straight 16-frame drop.
                *y = (*y + FLOWER_DROP / 16).min(self.ui_elem(f).y as i32);
            }
        }
        Scene::Dead { page, nodes }
    }

    // ---- drawing ----------------------------------------------------------

    pub(crate) fn draw_nodes(&mut self, nodes: &[(Node, i32, i32)]) {
        let mut draws = Vec::new();
        for (n, x, y) in nodes {
            n.draws(*x, *y, &mut draws);
        }
        self.draw_list_of(&draws, None);
    }

    pub(crate) fn draw_support(&mut self, pet: &Node, round: u8, t: u16, busy: bool) {
        self.draw_field_bg();
        // The baby's script draws the cell background, so the icons go on top of it.
        let mut draws = Vec::new();
        pet.draws(64, 64, &mut draws);
        self.draw_list_of(&draws, None);
        let icon = AREA_ICONS[(self.pet.dominant_field() as usize).min(2)];
        for (r, &k) in SUPPORT_ORDER.iter().enumerate() {
            let e = self.ui_elem(elem::SUPPORT_ICONS[k]);
            let p = match r as u8 {
                r if r < round => continue,
                r if r == round && busy => (t.min(SUPPORT_FLY) as i32, SUPPORT_FLY as i32),
                _ => (0, 1),
            };
            let fly = |a: i32, b: i32| crate::ease::ease(FLY_CURVE, a, b, p.0 as u32, p.1 as u32);
            let (x, y, alpha) =
                (fly(e.x as i32, SUPPORT_TARGET.0), fly(e.y as i32, SUPPORT_TARGET.1), fly(64, 0) as u8);
            self.screen.blit(icon, 0, x, y, alpha, false, None);
        }
    }

    /// Graves after a death: the earlier graves stand still (frame 1 = old age).
    pub(crate) fn draw_graveyard(&mut self, nodes: &[(Node, i32, i32)]) {
        self.draw_nodes(&nodes[..2]);
        let n = self.pet.graves.len();
        for (i, g) in self.pet.graves.clone().iter().enumerate().take(n.saturating_sub(1)) {
            let e = self.ui_elem(elem::GRAVE_SLOTS + i as u16);
            self.screen.blit(e.p[3] as u16, (g.days >= 30) as u16, e.x as i32, e.y as i32, 64, false, None);
        }
        self.draw_nodes(&nodes[2..]);
    }

    pub(crate) fn draw_dead(&mut self, page: u8, nodes: &[(Node, i32, i32)]) {
        if page == 0 {
            self.draw_elem(elem::TOMB_BG, None, None);
            self.draw_nodes(nodes);
            return;
        }
        self.draw_elem(elem::RECORD_BG, None, None);
        for b in elem::RECORD_BARS {
            self.draw_elem(b, None, None);
        }
        let at = |g: &Game, id| {
            let e = g.ui_elem(id);
            (e.x as i32, e.y as i32)
        };
        let (x, y) = at(self, elem::NAME);
        self.draw_text(self.pet.def.name_id, x, y, true, 0);
        let grave = self.pet.graves.last().copied();
        let (x, y) = at(self, elem::DAYS);
        self.draw_number(grave.map_or(0, |g| g.days as u32), x, y, 1);
        let (x, y) = at(self, elem::YEARS);
        self.draw_text(YEARS, x, y, false, 1);
        let (x, y) = at(self, elem::CAUSE);
        self.draw_text(CAUSE, x, y, true, 1);
        let cause = match grave.map(|g| g.cause) {
            Some(1) => Some(0x7a),
            Some(3) => Some(0x7c),
            _ => None,
        };
        if let Some(c) = cause {
            let (x, y) = at(self, elem::CAUSE_TEXT);
            self.draw_text(c, x, y, true, 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::Scene;
    use super::{Action, Level, RESULT_STARS_AT, RESULT_STAR_LAND, RESULT_WAIT};
    use crate::input::{A, B, C};
    use crate::pet::{req, Cond};
    use crate::Game;

    const NOON: u64 = 86_400 * 3 + 12 * 3600;

    fn game(stage_id: u16) -> Game {
        let mut g = Game::new(crate::evolve::tests::data(), 1);
        g.pet.def = g.data.character(stage_id).unwrap();
        g.pet.onaka = 6;
        g.pet.gokigen = 20;
        g.pet.rank_seen = g.pet.rank;
        g.frame(NOON, 0);
        g
    }

    fn press(g: &mut Game, bits: u8) {
        g.frame(NOON, bits);
        g.frame(NOON, 0);
    }

    fn run_until_home(g: &mut Game) {
        for _ in 0..2000 {
            if matches!(g.scene, Scene::Home) {
                return;
            }
            g.frame(NOON, 0);
        }
        panic!("scene never ended");
    }

    #[test]
    fn support_call_is_answered_from_the_cell_layer() {
        let mut g = game(1001);
        g.pet.field_log = vec![1; 4];
        g.pet.raise(req::EVOL_SUPPORT);
        g.level = crate::Level::Cell;
        g.frame(NOON, 0);
        press(&mut g, A);
        assert!(matches!(g.scene, Scene::Support { .. }));
        for _ in 0..4 {
            press(&mut g, B);
            for _ in 0..40 {
                g.frame(NOON, 0);
            }
        }
        for _ in 0..20 {
            g.frame(NOON, 0);
        }
        assert!(!matches!(g.scene, Scene::Support { .. }));
        assert_eq!(g.pet.def.stage, 3, "evolved with the dominant area");
        assert_eq!(g.pet.def.id, g.data.evolution.kids[1]);
    }

    #[test]
    fn first_aid_from_the_pet_layer() {
        let mut g = game(2113);
        g.pet.raise(req::DEATH);
        g.frame(NOON, 0);
        press(&mut g, A);
        assert!(matches!(g.scene, Scene::Play(_)));
        run_until_home(&mut g);
        assert_eq!(g.pet.cond, Cond::Normal);
    }

    #[test]
    fn dying_then_graves_then_tombstone_then_new_egg() {
        let mut g = game(2113);
        g.pet.generation = 3;
        g.pet.raise(req::DYING);
        g.frame(NOON, 0);
        assert!(matches!(g.scene, Scene::Dying { .. }));
        g.frame(NOON + 61, 0);
        assert!(matches!(g.scene, Scene::Play(_)), "death animation");
        assert_eq!(g.pet.cond, Cond::Dying);
        for _ in 0..2000 {
            if matches!(g.scene, Scene::Graveyard { .. }) {
                break;
            }
            g.frame(NOON + 61, 0);
        }
        assert_eq!(g.pet.cond, Cond::Dead);
        for _ in 0..2000 {
            if matches!(g.scene, Scene::Dead { .. }) {
                break;
            }
            g.frame(NOON + 61, 0);
        }
        assert!(matches!(g.scene, Scene::Dead { page: 0, .. }));
        press(&mut g, A);
        assert!(matches!(g.scene, Scene::Dead { page: 0, .. }), "A alone does nothing");
        press(&mut g, B);
        assert!(matches!(g.scene, Scene::Dead { page: 1, .. }));
        press(&mut g, C);
        g.frame(NOON + 61, C);
        g.frame(NOON + 61, C | A);
        g.frame(NOON + 61, 0);
        assert!(matches!(g.scene, Scene::Menu { .. }), "hold C + A: NEW EGG");
        press(&mut g, B);
        run_until_home(&mut g);
        assert_eq!((g.pet.def.id, g.pet.cond, g.level), (1000, Cond::Normal, crate::Level::Field));
        assert_eq!(g.pet.generation, 1, "an egg without a partner starts over at generation 1");
    }

    #[test]
    fn sitter_screen_until_taken_back() {
        let mut g = game(2113);
        g.pet.points = 300;
        let now = NOON;
        g.pet.call_sitter(now).unwrap();
        g.frame(now, 0);
        assert!(matches!(g.scene, Scene::Sitter { .. }));
        press(&mut g, A);
        assert!(matches!(g.scene, Scene::Ask { yes: false, .. }), "END TAMA SITTING? defaults to NO");
        press(&mut g, B);
        assert!(matches!(g.scene, Scene::Sitter { .. }));
        press(&mut g, A);
        press(&mut g, A);
        press(&mut g, B);
        run_until_home(&mut g);
        assert_eq!(g.pet.cond, Cond::Normal);
    }

    #[test]
    fn sitter_hands_back_at_seven_on_the_field() {
        let mut g = game(2113);
        g.pet.points = 300;
        g.pet.call_sitter(NOON).unwrap();
        g.frame(NOON, 0);
        let mut t = NOON;
        while t < NOON + 7 * 3600 + 60 {
            t += 60;
            g.frame(t, 0);
        }
        assert_eq!((g.pet.cond, g.level), (Cond::Normal, crate::Level::Field));
        assert!(!matches!(g.scene, Scene::Sitter { .. }));
    }

    #[test]
    fn microscope_no_data_code_pays_fifty_once_a_day() {
        let mut g = game(2113);
        g.pet.points = 0;
        for round in 0..2 {
            g.scene = Scene::Searching { code: "CCU056KH".into(), t: 0 };
            for _ in 0..41 {
                g.frame(NOON, 0);
            }
            assert!(matches!(g.scene, Scene::Message { text: 0xe7, .. }), "NO DATA");
            press(&mut g, B);
            assert_eq!(matches!(g.scene, Scene::Bonus), round == 0);
        }
        assert_eq!(g.pet.points, 50);
    }

    #[test]
    fn the_egg_hatches_in_front_of_the_player() {
        let mut g = game(1000);
        g.pet.raise(req::EVOLUTION);
        g.frame(NOON, 0);
        assert!(matches!(g.scene, Scene::Play(_)), "hatching");
        assert_eq!(g.pet.def.stage, 2);
        run_until_home(&mut g);
        assert_eq!(g.level, Level::Pet);
    }

    #[test]
    fn a_child_takes_in_its_memories_then_shows_the_bonus() {
        let mut g = game(2113);
        g.pet.field_log = vec![1; 4];
        g.pet.miss_count = 2;
        g.pet.raise(req::EVOLUTION);
        g.frame(NOON, 0);
        let Scene::Memories { icons, .. } = &g.scene else { panic!("memories") };
        assert_eq!(icons.len(), 6, "4 areas + 2 mistakes");
        let old = g.pet.def.chr_id;
        while matches!(g.scene, Scene::Memories { .. }) {
            g.frame(NOON, 0);
        }
        assert_eq!(g.pet.def.stage, 4);
        assert_eq!(g.old_chr, Some(old), "the old look plays role 0");
        while matches!(g.scene, Scene::Play(_)) {
            g.frame(NOON, 0);
        }
        assert!(matches!(g.scene, Scene::EvolvePoints { .. }));
        press(&mut g, B);
        assert!(matches!(g.scene, Scene::EvolvePoints { .. }), "keys wait 20 frames");
        for _ in 0..20 {
            g.frame(NOON, 0);
        }
        press(&mut g, B);
        assert!(matches!(g.scene, Scene::Home));
        g.frame(NOON, 0);
        assert_eq!(g.old_chr, None);
    }

    #[test]
    fn poop_plays_at_home_but_not_after_a_menu() {
        let mut g = game(2113);
        g.pet.raise(req::UNCHI);
        g.frame(NOON, 0);
        assert!(!g.pet.is_pending(req::UNCHI), "pooped at HOME");
        run_until_home(&mut g);
        g.scene = g.open(Action::Lab);
        g.pet.raise(req::UNCHI);
        g.frame(NOON, 0);
        assert!(!g.pet.is_pending(req::UNCHI), "settled when HOME opens again");
    }

    #[test]
    fn lab_result_drops_the_stars_then_waits_for_a_key() {
        let mut g = game(2113);
        g.scene = g.after_mini(Some(3), Some((0, 0)));
        let mut sounds = Vec::new();
        for _ in 0..RESULT_STARS_AT + RESULT_STAR_LAND + 1 {
            g.frame(NOON, 0);
            sounds.extend(g.take_sounds());
        }
        assert!(sounds.iter().any(|s| s.0 == 24), "three stars");
        assert!(matches!(&g.scene, Scene::LabResult { nodes, .. } if nodes.len() == 3));
        for _ in 0..RESULT_WAIT {
            g.frame(NOON, 0);
        }
        press(&mut g, B);
        assert!(matches!(g.scene, Scene::Menu { .. }), "back to the game menu");
    }
}
