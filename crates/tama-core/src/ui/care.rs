//! Care screens: HOME input, feeding, care animations, prompts, status, calendar events and
//! notices (docs/spec/08-care-ui.md).

use super::*;

impl Game {
    /// Foods offered for a category (`inv_list_for_eat` 0x1001e250): a free item first, then the
    /// inventory. Babies only get milk / jelly sticks.
    pub(super) fn food_items(&self, cat: u8) -> Vec<u16> {
        let baby = self.pet.def.stage == 2;
        let free = match (cat, baby) {
            (KIND_MEAL, true) => MILK,
            (KIND_MEAL, false) => PELLETS,
            _ => JELLY_STICK,
        };
        let mut items = vec![free];
        if !baby {
            let kinds = self
                .pet
                .inventory
                .iter()
                .filter(|(id, n)| *n > 0 && self.data.item(*id).is_some_and(|i| i.kind == cat));
            items.extend(kinds.map(|(id, _)| *id));
        }
        items
    }

    /// End credits at planet rank 10, with their song (melody 0x54).
    pub(crate) fn credits(&mut self, then: Scene) -> Scene {
        self.sfx(0x54, 100);
        let script = if self.pet.lang == 1 { script::CREDITS_EN } else { script::CREDITS };
        Scene::Credits { node: Node::new(script), then: Box::new(then) }
    }

    /// Rank-up and mission-clear notices when the pet is back at HOME.
    pub(crate) fn check_notices(&mut self) {
        if let Some((old, r)) = self.pet.take_rank_up() {
            // The planet hops from its old look to the new one (anim 0xc4), then CONGRATS.
            let text = if r < 10 { 0x8e + r as u16 - 1 } else { 0x97 };
            self.planet_ranks = Some([old, r]);
            let after = if r >= 10 { self.credits(Scene::Home) } else { Scene::Home };
            let then = Scene::Message { text, then: Box::new(after) };
            self.scene = self.play(&[script::RANK_UP], (0, OnPress::None), None, then);
        } else if !self.pet.claim_missions().is_empty() {
            // Mission clear jingle (PSID_INFO_MISSION `FUN_10048bd0`).
            self.sfx(SND_MISSION_CLEAR, 100);
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
        if p.is_pending(req::BIRTHDAY) && !sleeping && p.cond == Cond::Normal {
            let mate = self.pet.grazing[self.pet.field as usize].last().copied();
            self.roles[1] = mate.and_then(|g| self.data.character(g.id)).map(|c| c.chr_id);
            let cake = self.play(&[script::BIRTHDAY], (0, OnPress::Event(req::BIRTHDAY)), None, Scene::Home);
            self.scene = Scene::Message { text: 0x102, then: Box::new(cake) };
            return;
        }
        // DODODO only lasts 15 minutes and does not wait for care calls.
        if p.is_pending(req::DODODO) && !sleeping && p.cond == Cond::Normal {
            self.scene = self.play(&[script::DODODO], (0, OnPress::Event(req::DODODO)), None, Scene::Home);
            return;
        }
        if p.def.stage < 3 || p.cond != Cond::Normal || busy {
            return;
        }
        let (r, period) = (r % 100, crate::pet::Pet::period(now));
        if !sleeping && p.is_pending(req::METEOR) && period == 1 && r < 30 {
            let game = crate::mini::lab05::Lab05::event(self.data.mini.lab05_event.clone());
            let session =
                crate::mini::Session { event: Some(req::METEOR), ..crate::mini::Session::new(Box::new(game), None) };
            self.scene = Scene::Mini { session: session.with_intro(crate::mini::lab05::EVENT_OPENING), lab: None };
            return;
        }
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
            items: vec![
                (text::FEED, Some(Action::Feed)),
                (text::PLAY, Some(Action::Together)),
                (text::CLEAN_TAMA, Some(Action::Groom)),
            ],
            idx: 0,
            back: None,
        }
    }

    pub(super) fn field_menu(&self) -> Scene {
        Scene::Menu {
            items: vec![
                (text::OUTSIDE_TOY, (self.pet.rank >= 2).then_some(Action::Setsubi)),
                (text::EGG_HUNT, Some(Action::Hunt)),
                (text::CLEAN_POOP, Some(Action::CleanPoop)),
                (text::FIELDS, (self.pet.rank >= 4).then_some(Action::Fields)),
            ],
            idx: 2,
            back: None,
        }
    }

    /// HOME input (`home_field_tama_state` 0x10039eb8, `home_cell_state` 0x100398c8).
    pub(super) fn update_home_input(&mut self, pressed: u8) -> Scene {
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
                Level::Field if self.pet.def.stage > 1 => {
                    self.sfx(0x2b, 100);
                    Level::Space
                }
                _ => Level::Field,
            };
        }
        if !pressed_any(pressed) || self.pet.def.stage == 1 {
            return Scene::Home;
        }
        // C on the field: go to the field's expansion area and back (HOME_FIELD2).
        if self.level == Level::Field && pressed & C != 0 {
            self.field_c();
            return Scene::Home;
        }
        let p = &self.pet;
        let sleeping = p.last.is_some_and(|t| p.is_sleeping_at(t));
        let state = self.home_state.map(|s| s.0);
        // First aid (HOME state 0x19, pet layer) and the baby's support call (0x1a, cell layer).
        if self.level == Level::Pet && state == Some(0x19) {
            return self.rescue();
        }
        if self.level == Level::Cell && state == Some(0x1a) {
            return self.support();
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
            Level::Pet if sleeping && p.is_pending(req::WAKE) && !p.no_wake_today => {
                Scene::Prompt { wake: true, yes: true }
            }
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
            Level::Cell | Level::Space => Scene::Home,
        }
    }

    /// Eat the chosen food (`gohan_state` 0x1003ff80 state 0x4b0/3000).
    pub(super) fn eat(&mut self, cat: u8, items: Vec<u16>, idx: usize) -> Scene {
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
        let mut scripts =
            vec![script::SERVE[pref], if item.drink { script::DRINK[pref] } else { script::EAT[pref][thin] }];
        if cat == KIND_MEAL && self.pet.onaka >= 6 {
            scripts.push(script::FULL);
        }
        self.play(&scripts, (0, OnPress::None), sprite, Scene::Effects { cat, idx })
    }

    pub(super) fn start_poop_clean(&mut self) -> Scene {
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

    pub(super) fn update_play(&mut self, mut p: Play, pressed: u8) -> Scene {
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
            if let Some((_, snd)) = p.cue.filter(|c| c.0 == next) {
                self.sfx(snd, 100);
            }
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
            OnPress::Rescue => {
                self.pet.rescue();
            }
            OnPress::Dying(saved) => self.pet.end_dying(saved),
            OnPress::NewEgg => self.pet.new_egg(&self.data),
            OnPress::EndSitter => self.pet.end_sitter(),
            _ => {}
        }
        *p.then
    }

    // ---- drawing ----------------------------------------------------------

    pub(super) fn update_food_category(&mut self, mut idx: usize, pressed: u8) -> Scene {
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

    pub(super) fn update_food_list(&mut self, cat: u8, items: Vec<u16>, idx: usize, pressed: u8) -> Scene {
        let idx = cycle(idx, items.len(), pressed);
        if pressed & C != 0 {
            Scene::FoodCategory { idx: (cat == KIND_SNACK) as usize }
        } else if pressed & (B | PUSH) != 0 {
            Scene::Confirm { cat, items, idx, yes: true }
        } else {
            Scene::FoodList { cat, items, idx }
        }
    }

    /// EAT THIS? YES / NO.
    pub(super) fn update_confirm(&mut self, cat: u8, items: Vec<u16>, idx: usize, mut yes: bool, pressed: u8) -> Scene {
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

    /// Play the pending eating effects one by one, then back to the food list.
    pub(super) fn update_effects(&mut self, cat: u8, idx: usize) -> Scene {
        match self.pet.complete_effect() {
            Some(id) => {
                let k = req::EFFECTS.iter().position(|&r| r == id).unwrap_or(0);
                self.play(&[script::EFFECTS[k]], (0, OnPress::None), None, Scene::Effects { cat, idx })
            }
            None => {
                let items = self.food_items(cat);
                let idx = idx.min(items.len() - 1);
                Scene::FoodList { cat, items, idx }
            }
        }
    }

    /// WAKE UP? / TIME TO SLEEP?: NO (or C) skips the call for today.
    pub(super) fn update_prompt(&mut self, wake: bool, mut yes: bool, pressed: u8) -> Scene {
        if pressed & (A | UP | DOWN) != 0 {
            yes = !yes;
        }
        if pressed & (B | PUSH) != 0 && yes {
            if wake {
                self.pet.wake()
            } else {
                self.pet.put_to_bed()
            };
            Scene::Home
        } else if pressed & (B | PUSH | C) != 0 {
            if wake {
                self.pet.no_wake_today = true
            } else {
                self.pet.no_sleep_today = true
            }
            Scene::Home
        } else {
            Scene::Prompt { wake, yes }
        }
    }

    /// CELL_STATUS: two pages; C or 200 idle frames close it.
    pub(super) fn update_status(&mut self, mut page: u8, idle: u16, pressed: u8) -> Scene {
        if pressed & (A | UP | DOWN) != 0 {
            page ^= 1;
        }
        if pressed & C != 0 || idle >= 200 {
            Scene::Home
        } else {
            Scene::Status { page, idle: if pressed != 0 { 0 } else { idle + 1 } }
        }
    }

    pub(super) fn update_credits(&mut self, mut node: Node, then: Box<Scene>, pressed: u8) -> Scene {
        node.tick(&self.data.motions);
        node.take_sounds(&mut self.sounds);
        if node.is_finished() || pressed_any(pressed) {
            *then
        } else {
            Scene::Credits { node, then }
        }
    }
}
