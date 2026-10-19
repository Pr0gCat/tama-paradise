//! Outside toys on the field (SETSUBI), the planet level (HOME_SPACE) and its menu:
//! TAMA STARS, DECO PLANET, TRAVEL, PLANET LV. (docs/spec/10-decor-planet.md).

use super::{Action, CardKind, Level, Scene};
use crate::input::{A, B, C, DOWN, PUSH, UP};
use crate::motion::{Look, Node, PLANET};
use crate::pet::Pet;
use crate::Game;

mod t {
    pub const CONNECTION: u16 = 34;
    pub const TAMA_STARS: u16 = 35;
    pub const DECO_PLANET: u16 = 36;
    pub const TRAVEL: u16 = 37;
    pub const PLANET_LV: u16 = 38;
    pub const PLACE: u16 = 59;
    pub const REMOVE: u16 = 60;
    pub const IS_THIS_OKAY: u16 = 0x7e;
    pub const REMOVE_Q: u16 = 0x7f;
    pub const GO_TO_PLANET: u16 = 0x81;
    pub const COLLECT_POOPS: u16 = 0x82;
    pub const GOAL: u16 = 131;
    pub const PLACE_HERE: u16 = 0x98;
    pub const PUT_AWAY: u16 = 0x99;
    pub const WAIT_UNTIL_AWAKE: u16 = 0x69;
    pub const STILL_CANT_USE: u16 = 0x6b;
    pub const FROM_LEVEL_3: u16 = 0xf4;
}

/// World point the camera centres on when the pet is focused (keeps the classic field view).
pub(crate) const PET_FOCUS: (i32, i32) = (64, 64);
/// Placement markers: small 32×32, large 64×64.
const MARKER: [u16; 2] = [0x455, 0x456];
const STARFIELD: u16 = 0x450;
const STAR_DOTS: u16 = 0x451;
const DEAD_STAR: u16 = 0x453;
const STAR_POS: [(i32, i32); 5] = [(30, 28), (58, 56), (92, 26), (22, 68), (44, 104)];
const PLANET_POS: (i32, i32) = (90, 92);
/// TAMA STARS: star cursor (UI element 0x2cc), ghost page background (0x13f) and ghost
/// animations for a young / old (> 29 years) tama.
const STAR_CURSOR: u16 = 193;
const GHOST_BG: u16 = 109;
const GHOST: u16 = 0xa9;
const GHOST_OLD: u16 = 0xaa;
const REMOVE_ORDER: [usize; 6] = [0, 4, 1, 2, 5, 3];
/// DECO PLANET sparkle (anim 199).
const SPARKLE: u16 = 0xc7;
/// An outfit-set visitor on the field (`home_scene_build` 0x10084df8), at the area's visitor anchor.
const VISITOR: u16 = 0x2f1;
const VISITOR_ANCHOR: usize = 8;

impl Game {
    /// World position of a toy slot on a field area.
    fn slot_pos(&self, field: u8, area: u8, slot: usize) -> (i32, i32) {
        let p = self.data.decor.home_pos.get(field as usize).map_or((64, 64, 0), |f| f[area as usize][slot]);
        (p.0 as i32, p.1 as i32)
    }

    pub(crate) fn planet_chr_at(&self, rank: u8) -> u16 {
        let class = crate::decor::romtype_class(self.data.rom_type);
        self.data.decor.planet_chr.get(class).map_or(0, |r| r[(rank as usize).min(10)])
    }

    /// Field camera objects: the pet, then the toys placed in this area.
    fn focus_points(&self) -> Vec<(i32, i32)> {
        let (f, a) = (self.pet.field, self.pet.in_area2 as u8);
        let toys = &self.pet.toys[f as usize][a as usize];
        let mut pts = vec![PET_FOCUS];
        pts.extend((0..6).filter(|&s| toys[s] != 0).map(|s| self.slot_pos(f, a, s)).take(4));
        pts
    }

    /// C on the field (`home_field_tama_state`): next focus; after the last one, the expansion area.
    pub(crate) fn field_c(&mut self) {
        let n = self.focus_points().len();
        let unlocked = self.pet.areas & 1 << self.pet.field != 0;
        if n >= 2 {
            self.focus = (self.focus + 1) % n;
            self.sfx(0, 100);
        }
        if (n < 2 || self.focus == 0) && unlocked {
            self.focus = 0;
            self.pet.in_area2 = !self.pet.in_area2;
            self.sfx(9, 100);
        }
    }

    /// Rebuild the field's toy animations when the layout changes; ease the camera to the focus.
    pub(crate) fn update_toys(&mut self) {
        let (f, a) = (self.pet.field, self.pet.in_area2);
        let toys = self.pet.toys[f as usize][a as usize];
        if self.toy_key != Some((f, a, toys)) {
            if self.toy_key.is_some_and(|k| (k.0, k.1) != (f, a)) {
                self.focus = 0;
            }
            self.toy_key = Some((f, a, toys));
            self.toy_nodes =
                (0..6).filter_map(|s| self.data.item(toys[s]).map(|i| (s, Node::new(i.show_anim)))).collect();
            self.toy_nodes
                .sort_by_key(|(s, _)| self.data.decor.home_pos.get(f as usize).map_or(0, |p| p[a as usize][*s].2));
        }
        // A matching outfit set brings one of its two visitors, with a 1-in-3 chance a day.
        let day = self.pet.last.unwrap_or(0) / 86_400;
        if self.visitor_day != Some(day) {
            self.visitor_day = Some(day);
            let (r1, r2) = (self.rand(), self.rand());
            self.visitor = self.pet.outfit_visitors(&self.data).filter(|_| r1 % 3 == 0).and_then(|v| {
                let chr = self.data.character(v[(r2 % 2) as usize])?.chr_id;
                Some((chr, Node::new(VISITOR)))
            });
        }
        if let Some((_, n)) = &mut self.visitor {
            if n.is_finished() {
                *n = Node::new(VISITOR);
            }
            n.tick(&self.data.motions);
        }
        for (_, n) in &mut self.toy_nodes {
            if n.is_finished() {
                *n = Node::new(n.script());
            }
            n.tick(&self.data.motions);
        }
        let pts = self.focus_points();
        let target = pts.get(self.focus).copied().unwrap_or(PET_FOCUS);
        let step = |c: i32, t: i32| c + ((t - c) / 2).clamp(-16, 16) + (t - c).signum() * ((t - c).abs() == 1) as i32;
        self.cam = (step(self.cam.0, target.0), step(self.cam.1, target.1));
    }

    pub(crate) fn draw_toys(&mut self) {
        let (f, a) = (self.pet.field, self.pet.in_area2 as u8);
        let mut draws = Vec::new();
        for (s, n) in &self.toy_nodes {
            let (x, y) = self.slot_pos(f, a, *s);
            n.draws(x, y, &mut draws);
        }
        self.draw_all(&draws);
    }

    /// The outfit-set visitor, drawn over the grazing tamas so it is not hidden.
    pub(crate) fn draw_visitor(&mut self) {
        let Some((chr, n)) = &self.visitor else { return };
        let (x, y) = self.slot_pos(self.pet.field, self.pet.in_area2 as u8, VISITOR_ANCHOR);
        let mut draws = Vec::new();
        n.draws(x, y, &mut draws);
        for d in &mut draws {
            if let Look::Chr { chr: c @ crate::motion::PET, .. } = &mut d.look {
                *c = *chr;
            }
        }
        self.draw_all(&draws);
    }

    /// One frame of a toy's display animation (SETSUBI preview).
    fn draw_toy(&mut self, id: u16, x: i32, y: i32) {
        let Some(item) = self.data.item(id) else { return };
        let mut n = Node::new(item.show_anim);
        n.tick(&self.data.motions);
        let mut draws = Vec::new();
        n.draws(x, y, &mut draws);
        self.draw_all(&draws);
    }

    /// (field, area) pages SETSUBI can show; REMOVE skips empty ones.
    fn setsubi_pages(&self, remove: bool) -> Vec<(u8, u8)> {
        let mut pages = Vec::new();
        for f in self.pet.visible_fields(&self.data) {
            for a in 0..=(self.pet.areas >> f & 1) {
                if !remove || self.pet.toys[f as usize][a as usize].iter().any(|&t| t != 0) {
                    pages.push((f, a));
                }
            }
        }
        pages
    }

    /// Slots the cursor visits: free spots for the toy's size, or placed toys when removing (toy 0).
    fn setsubi_slots(&self, toy: u16, field: u8, area: u8) -> Vec<usize> {
        match self.data.item(toy) {
            Some(item) => Pet::toy_slots(item.kind).to_vec(),
            None => {
                REMOVE_ORDER.into_iter().filter(|&s| self.pet.toys[field as usize][area as usize][s] != 0).collect()
            }
        }
    }

    pub(super) fn open_decor(&mut self, a: Action) -> Scene {
        match a {
            Action::Setsubi => Scene::Menu {
                items: vec![
                    (t::PLACE, self.toy_items().iter().any(|_| true).then_some(Action::ToyList)),
                    (
                        t::REMOVE,
                        (!self.setsubi_pages(true).is_empty()).then_some(Action::SetsubiAt {
                            toy: 0,
                            field: 0xff,
                            area: 0,
                            slot: 0,
                        }),
                    ),
                ],
                idx: 0,
                back: None,
            },
            Action::ToyList => {
                let items = self.toy_items();
                if items.is_empty() {
                    return self.open(Action::Setsubi);
                }
                Scene::Card { kind: CardKind::Toy, items, idx: 0 }
            }
            Action::SetsubiAt { toy, field, area, slot } => {
                let pages = self.setsubi_pages(toy == 0);
                let Some(&(f, a)) = pages
                    .iter()
                    .find(|p| **p == (field, area))
                    .or_else(|| pages.iter().find(|p| p.0 == self.pet.field).or(pages.first()))
                else {
                    return self.open(Action::Setsubi);
                };
                let slots = self.setsubi_slots(toy, f, a);
                let idx = slots.iter().position(|&s| s == slot as usize).unwrap_or(0);
                Scene::Setsubi { toy, field: f, area: a, idx }
            }
            Action::PlaceToy { toy, field, area, slot } => {
                if self.pet.place_toy(&self.data, field, area, slot as usize, toy).is_some() {
                    self.sfx(0x21, 100);
                }
                self.open(Action::ToyList)
            }
            Action::RemoveToy { field, area, slot } => {
                self.pet.remove_toy(field, area, slot as usize);
                if self.setsubi_pages(true).is_empty() {
                    self.open(Action::Setsubi)
                } else {
                    self.open(Action::SetsubiAt { toy: 0, field, area, slot: 0 })
                }
            }
            Action::SpaceMenu => Scene::Menu {
                items: vec![
                    (t::CONNECTION, Some(Action::Connect)),
                    (t::TAMA_STARS, (!self.pet.graves.is_empty()).then_some(Action::Stars)),
                    (t::DECO_PLANET, (!self.worn_items().is_empty()).then_some(Action::Deco)),
                    (t::TRAVEL, Some(Action::Travel)),
                    (t::PLANET_LV, Some(Action::PlanetLv)),
                ],
                idx: 0,
                back: None,
            },
            Action::Stars => self.stars_page(0, 0),
            Action::Deco => {
                if self.pet.rank < 3 {
                    return self.message(t::FROM_LEVEL_3, Action::SpaceMenu);
                }
                Scene::Card { kind: CardKind::Wear, items: self.worn_items(), idx: 0 }
            }
            Action::Wear(id) => {
                let slot = self.data.item(id).map_or(0, |i| i.kind.saturating_sub(5) as usize).min(2);
                let new = if self.pet.equip[slot] == id { 0 } else { id };
                self.pet.wear(&self.data, slot, new);
                self.play(
                    &[SPARKLE],
                    (0, super::OnPress::None),
                    None,
                    Scene::Card { kind: CardKind::Wear, items: self.worn_items(), idx: 0 },
                )
            }
            Action::Travel => {
                let now = self.pet.last.unwrap_or(0);
                if self.pet.is_sleeping_at(now) {
                    self.message(t::WAIT_UNTIL_AWAKE, Action::SpaceMenu)
                } else if self.pet.def.stage < 3 {
                    self.message(t::STILL_CANT_USE, Action::SpaceMenu)
                } else if self.pet.rank < 3 {
                    self.message(t::FROM_LEVEL_3, Action::SpaceMenu)
                } else if self.pet.unchi_meter < crate::decor::TRAVEL_COST {
                    self.message(t::COLLECT_POOPS, Action::SpaceMenu)
                } else {
                    let items =
                        self.pet.destinations(&self.data).into_iter().map(|d| self.data.decor.travel[d].item).collect();
                    Scene::Card { kind: CardKind::Travel, items, idx: 0 }
                }
            }
            Action::GoTravel(d) => {
                // Roles: 1/2 = the field's first two grazing tamas, 3 = the local NPC (0x1006f810).
                let toggle = (self.pet.travel_toggle >> d & 1) as usize;
                let grazing = &self.pet.grazing[self.pet.field as usize];
                for (r, g) in grazing.iter().take(2).enumerate() {
                    self.roles[1 + r] = self.data.character(g.id).map(|c| c.chr_id);
                }
                self.roles[3] = self.data.decor.travel.get(d as usize).map(|t| t.npcs[toggle]);
                self.go_travel(d)
            }
            Action::PlanetLv => Scene::PlanetLv { page: 0 },
            _ => Scene::Home,
        }
    }

    fn go_travel(&mut self, d: u8) -> Scene {
        match self.pet.travel(&self.data, d as usize) {
            Some(souvenir) => {
                let anim = self.data.decor.travel[d as usize].anim;
                let name = self.data.item(souvenir).map_or(0, |i| i.name);
                let then = Scene::Message { text: name, then: Box::new(Scene::Home) };
                self.play(&[anim], (0, super::OnPress::None), None, then)
            }
            None => self.open(Action::Travel),
        }
    }

    /// Toys the player owns (types 3/4), by id.
    fn toy_items(&self) -> Vec<u16> {
        self.owned_of(&[3, 4])
    }

    /// Planet outfits the player owns (types 5/6/7), by id.
    fn worn_items(&self) -> Vec<u16> {
        self.owned_of(&[5, 6, 7])
    }

    fn owned_of(&self, kinds: &[u8]) -> Vec<u16> {
        let mut ids: Vec<u16> = self
            .pet
            .inventory
            .iter()
            .filter(|(id, n)| *n > 0 && self.data.item(*id).is_some_and(|i| kinds.contains(&i.kind)))
            .map(|(id, _)| *id)
            .collect();
        ids.sort_unstable();
        ids
    }

    /// B on a toy / outfit / destination card.
    pub(super) fn pick_decor_card(&mut self, kind: CardKind, items: &[u16], idx: usize) -> Scene {
        let id = items[idx];
        let back = Scene::Card { kind, items: items.to_vec(), idx };
        match kind {
            CardKind::Toy if self.pet.free(id) > 0 => {
                let area = self.pet.in_area2 as u8;
                self.open(Action::SetsubiAt { toy: id, field: self.pet.field, area, slot: 0 })
            }
            CardKind::Wear => {
                let worn = self.pet.equip.contains(&id);
                let text = if worn { t::REMOVE_Q } else { t::IS_THIS_OKAY };
                Scene::Ask { text, yes: true, on_yes: Action::Wear(id), back: Action::Deco }
            }
            CardKind::Travel => {
                let d = self.data.decor.travel.iter().position(|d| d.item == id).unwrap_or(0) as u8;
                Scene::Ask { text: t::GO_TO_PLANET, yes: true, on_yes: Action::GoTravel(d), back: Action::Travel }
            }
            _ => {
                self.sfx(0x0b, 100);
                back
            }
        }
    }

    pub(super) fn update_setsubi(
        &mut self,
        toy: u16,
        mut field: u8,
        mut area: u8,
        mut idx: usize,
        pressed: u8,
    ) -> Scene {
        let n = self.setsubi_slots(toy, field, area).len().max(1);
        let step: i32 = if pressed & (A | UP) != 0 {
            1
        } else if pressed & DOWN != 0 {
            -1
        } else {
            0
        };
        if step != 0 {
            let next = idx as i32 + step;
            if (0..n as i32).contains(&next) {
                idx = next as usize;
            } else {
                // Past either end: the next / previous field area.
                let pages = self.setsubi_pages(toy == 0);
                let cur = pages.iter().position(|p| *p == (field, area)).unwrap_or(0);
                (field, area) = pages[(cur as i32 + step).rem_euclid(pages.len().max(1) as i32) as usize];
                let m = self.setsubi_slots(toy, field, area).len().max(1);
                idx = if step > 0 { 0 } else { m - 1 };
            }
        }
        if pressed & C != 0 {
            return self.open(if toy == 0 { Action::Setsubi } else { Action::ToyList });
        }
        if pressed & (B | PUSH) != 0 {
            let slots = self.setsubi_slots(toy, field, area);
            let Some(&slot) = slots.get(idx) else { return self.open(Action::Setsubi) };
            let slot = slot as u8;
            let back = Action::SetsubiAt { toy, field, area, slot };
            return if toy == 0 {
                Scene::Ask { text: t::PUT_AWAY, yes: true, on_yes: Action::RemoveToy { field, area, slot }, back }
            } else {
                Scene::Ask { text: t::PLACE_HERE, yes: true, on_yes: Action::PlaceToy { toy, field, area, slot }, back }
            };
        }
        Scene::Setsubi { toy, field, area, idx }
    }

    /// SETSUBI view (`setsubi_draw_area` 0x10070300): the camera centres on the cursor slot.
    pub(super) fn draw_setsubi(&mut self, toy: u16, field: u8, area: u8, idx: usize) {
        let slots = self.setsubi_slots(toy, field, area);
        let Some(&cur) = slots.get(idx) else { return };
        let (cx, cy) = self.slot_pos(field, area, cur);
        self.screen.off = (64 - cx, 64 - cy);
        if let Some(bg) = self.field_bg_at(field, area == 1) {
            self.screen.blit(bg, 0, 64, 64, 64, false, None);
        }
        let placed = self.pet.toys[field as usize][area as usize];
        let blink = (self.frame_count / 6).is_multiple_of(2);
        for (s, &id) in placed.iter().enumerate() {
            let (x, y) = self.slot_pos(field, area, s);
            if id != 0 && !(toy == 0 && s == cur && !blink) {
                self.draw_toy(id, x, y);
            }
        }
        if toy != 0 {
            for &s in &slots {
                let (x, y) = self.slot_pos(field, area, s);
                if s != cur || blink {
                    self.screen.blit(MARKER[(s >= 4) as usize], 0, x, y, 64, false, None);
                }
            }
        }
        self.screen.off = (0, 0);
        self.screen.blit(super::ARROWS, 2, 10, 116, 64, false, None);
        self.screen.blit(super::ARROWS, 3, 118, 116, 64, false, None);
    }

    /// The planet with its outfits (CHARA placeholder 2), awake pose 1 or asleep 15.
    fn draw_planet(&mut self, x: i32, y: i32) {
        let asleep = self.pet.last.is_some_and(|t| self.pet.is_sleeping_at(t));
        let d = crate::motion::Draw { look: Look::None, x, y, flip: false, alpha: 64, palette: 0 };
        self.draw_chr(&d, PLANET, if asleep { 15 } else { 1 }, crate::motion::KEEP, crate::motion::KEEP);
    }

    /// HOME_SPACE (`space_view_build` 0x10088850): starfield, dead tamas' stars, the planet.
    pub(crate) fn draw_space(&mut self) {
        self.draw_starfield();
        let bob = (self.frame_count / 20 % 2) as i32;
        self.draw_planet(PLANET_POS.0, PLANET_POS.1 + bob);
    }

    /// Starfield with the dead tamas' stars (shared by HOME_SPACE and TAMA STARS).
    fn draw_starfield(&mut self) {
        let pal = (self.frame_count / 10 % 4) as usize;
        self.screen.blit(STARFIELD, 0, 64, 64, 64, false, Some((STARFIELD, pal)));
        self.screen.blit(STAR_DOTS, 0, 64, 64, 64, false, Some((STAR_DOTS, pal)));
        let graves: Vec<_> = self.pet.graves.iter().rev().take(5).map(|g| g.days).collect();
        for (days, &(x, y)) in graves.into_iter().zip(STAR_POS.iter()) {
            self.screen.blit(DEAD_STAR, (days > 29) as u16, x, y, 64, false, None);
        }
    }

    /// Graves shown as stars: the newest five, star k = `graves[len − 1 − k]`.
    fn star_grave(&self, k: usize) -> Option<crate::family::Grave> {
        self.pet.graves.iter().rev().nth(k).copied()
    }

    fn stars_page(&mut self, idx: usize, page: u8) -> Scene {
        let node = match (page, self.star_grave(idx)) {
            (1, Some(g)) => {
                // The ghost stands in for the pet placeholder (role 0).
                self.roles[0] = self.data.character(g.ghost.id).map(|c| c.chr_id);
                Node::new(if g.days > 29 { GHOST_OLD } else { GHOST })
            }
            _ => Node::new(STAR_CURSOR),
        };
        Scene::Stars { idx, page, node }
    }

    pub(super) fn update_stars(&mut self, idx: usize, page: u8, mut node: Node, pressed: u8) -> Scene {
        node.tick(&self.data.motions);
        let n = self.pet.graves.len().min(STAR_POS.len()).max(1);
        match page {
            0 => {
                if pressed & C != 0 {
                    return self.open(Action::SpaceMenu);
                }
                if pressed & (B | PUSH) != 0 {
                    self.sfx(40, 100);
                    return self.stars_page(idx, 1);
                }
                let step = if pressed & (A | UP) != 0 {
                    1
                } else if pressed & DOWN != 0 {
                    n - 1
                } else {
                    0
                };
                if step != 0 && n > 1 {
                    return Scene::Stars { idx: (idx + step) % n, page, node };
                }
            }
            _ => {
                if pressed & C != 0 {
                    return self.stars_page(idx, 0);
                }
                if pressed & A != 0 {
                    return self.stars_page(idx, if page == 1 { 2 } else { 1 });
                }
            }
        }
        Scene::Stars { idx, page, node }
    }

    pub(super) fn draw_stars(&mut self, idx: usize, page: u8, node: &Node) {
        match page {
            0 => {
                self.draw_starfield();
                let (x, y) = STAR_POS[idx % STAR_POS.len()];
                let mut draws = Vec::new();
                node.draws(x, y, &mut draws);
                self.draw_all(&draws);
            }
            1 => {
                self.screen.blit(GHOST_BG, 0, 64, 64, 64, false, None);
                let mut draws = Vec::new();
                node.draws(64, 96, &mut draws);
                self.draw_all(&draws);
            }
            _ => {
                let i = self.pet.graves.len().saturating_sub(1 + idx);
                self.draw_grave(i);
            }
        }
    }

    /// PLANET LV. (`scene_tamaspace_rank`): page 0 the rank, page 1 the next goal.
    pub(super) fn draw_planet_lv(&mut self, page: u8) {
        let rank = self.pet.rank as usize;
        if page == 0 {
            self.draw_list(&[], 0, Some(t::PLANET_LV));
            self.draw_digits(rank as u32, 64, 60);
            if rank < 10 {
                self.draw_text(t::GOAL, 64, 96, true, 0);
            }
        } else if let Some(&goal) = self.data.decor.rank_goals.get(rank) {
            self.draw_list(&[], 0, Some(t::GOAL));
            self.draw_text(goal, 64, 40, true, 0);
        }
    }

    pub(super) fn update_space_input(&mut self, pressed: u8) -> Scene {
        if pressed & UP != 0 {
            self.sfx(0x28, 40);
            self.level = Level::Field;
            return Scene::Home;
        }
        if pressed & A == 0 || self.pet.def.stage == 1 {
            return Scene::Home;
        }
        let asleep = self.pet.last.is_some_and(|t| self.pet.is_sleeping_at(t));
        if asleep || self.pet.cond != crate::pet::Cond::Normal {
            self.sfx(0x0b, 100);
            return Scene::Home;
        }
        self.sfx(1, 100);
        self.open(Action::SpaceMenu)
    }
}

impl Game {
    /// PLANET LV.: B turns to the goal page (or the credits at rank 10), C or any key goes back.
    pub(super) fn update_planet_lv(&mut self, page: u8, pressed: u8) -> Scene {
        if pressed & C != 0 && page == 1 {
            Scene::PlanetLv { page: 0 }
        } else if pressed & (B | PUSH) != 0 && page == 0 && self.pet.rank < 10 {
            Scene::PlanetLv { page: 1 }
        } else if pressed & (B | PUSH) != 0 && page == 0 {
            let menu = self.open(Action::SpaceMenu);
            self.credits(menu)
        } else if pressed & (A | B | C | PUSH) != 0 {
            self.open(Action::SpaceMenu)
        } else {
            Scene::PlanetLv { page }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{Destination, Item};

    const NOON: u64 = 86_400 * 3 + 12 * 3600;

    fn game() -> Game {
        let mut d = crate::evolve::tests::data();
        d.items = vec![
            Item { id: 7001, kind: 3, ..Default::default() },
            Item { id: 7002, kind: 4, ..Default::default() },
            Item { id: 8001, kind: 5, ..Default::default() },
            Item { id: 8002, kind: 7, ..Default::default() },
            Item { id: 9901, kind: 9, ..Default::default() },
        ];
        d.decor.home_pos = vec![[[(10, 20, 100); 9]; 2]; 3];
        d.decor.travel = vec![Destination { item: 9901, rank: 3, anim: 0, souvenirs: [8001, 8002], npcs: [167, 168] }];
        let mut g = Game::new(d, 1);
        g.pet.def = g.data.character(2113).unwrap();
        g.pet.rank = 3;
        g.pet.rank_seen = 3;
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
    fn place_a_toy_from_the_field_menu() {
        let mut g = game();
        g.pet.give(7001, 1);
        g.level = Level::Field;
        press(&mut g, A); // field menu
        press(&mut g, DOWN);
        press(&mut g, DOWN);
        assert!(matches!(&g.scene, Scene::Menu { idx: 0, .. }));
        press(&mut g, B); // OUTSIDE TOY
        press(&mut g, B); // PLACE
        assert!(matches!(g.scene, Scene::Card { kind: CardKind::Toy, .. }));
        press(&mut g, B); // the toy
        assert!(matches!(g.scene, Scene::Setsubi { idx: 0, .. }));
        press(&mut g, A); // slot 1
        press(&mut g, B); // PLACE HERE?
        press(&mut g, B); // YES
        assert_eq!(g.pet.toys[g.pet.field as usize][0][1], 7001);
        assert_eq!(g.pet.free(7001), 0);
        // The field now has a second camera focus.
        g.scene = Scene::Home;
        press(&mut g, C);
        assert_eq!(g.focus, 1);
    }

    #[test]
    fn planet_menu_travel_and_outfits() {
        let mut g = game();
        g.level = Level::Field;
        press(&mut g, DOWN);
        assert_eq!(g.level, Level::Space);
        g.pet.unchi_meter = 60;
        press(&mut g, A);
        for _ in 0..3 {
            press(&mut g, A); // to TRAVEL
        }
        press(&mut g, B);
        assert!(matches!(g.scene, Scene::Card { kind: CardKind::Travel, .. }));
        press(&mut g, B); // GO TO THIS PLANET?
        press(&mut g, B); // YES
        assert_eq!((g.pet.unchi_meter, g.pet.count(8001)), (0, 1));
        assert_eq!(g.roles[3], Some(167), "the local NPC stands in for role 3");
        assert_eq!(g.resolve_chr(crate::motion::ROLE + 3).0, 167);
        g.scene = g.open(Action::Deco);
        press(&mut g, B); // CREPE HAT: IS THIS OKAY?
        press(&mut g, B);
        assert_eq!(g.pet.equip[0], 8001);
    }

    #[test]
    fn matching_outfits_bring_a_visitor_some_days() {
        let mut g = game();
        g.data.decor.equip_sets = vec![[8001, 0, 8002, 9000, 9001]];
        for id in [9000, 9001] {
            let mut c = crate::pet::tests::def(id, 5, 0, 0, 0);
            c.chr_id = 150 + (id - 9000);
            g.data.characters.push(c);
        }
        g.pet.give(8001, 1);
        g.pet.give(8002, 1);
        assert!(g.pet.wear(&g.data, 0, 8001) && g.pet.wear(&g.data, 2, 8002));
        g.level = Level::Field;
        let mut seen = 0;
        for day in 0..30 {
            let t = NOON + day * 86_400;
            g.pet.last = Some(t);
            g.frame(t, 0);
            if let Some((chr, _)) = &g.visitor {
                assert!([150, 151].contains(chr));
                seen += 1;
            }
        }
        assert!((3..=20).contains(&seen), "about one day in three: {seen}");
    }

    #[test]
    fn tama_stars_pages() {
        let mut g = game();
        for days in [10, 40] {
            let ghost = crate::family::Ghost { id: 2113, color: 0, generation: 1 };
            g.pet.graves.push(crate::family::Grave { ghost, cause: 3, days, spirit: false });
        }
        g.scene = g.open(Action::Stars);
        press(&mut g, A); // the older grave's star
        assert!(matches!(g.scene, Scene::Stars { idx: 1, page: 0, .. }));
        press(&mut g, B);
        assert!(matches!(g.scene, Scene::Stars { idx: 1, page: 1, .. }));
        assert!(g.roles[0].is_some(), "the ghost plays the pet's part");
        press(&mut g, A);
        assert!(matches!(g.scene, Scene::Stars { page: 2, .. }));
        press(&mut g, C);
        press(&mut g, C);
        assert!(matches!(g.scene, Scene::Menu { .. }), "back to the planet menu");
    }
}
