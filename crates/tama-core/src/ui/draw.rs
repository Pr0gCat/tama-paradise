//! Drawing: UI elements, text, lists, cards, the field and characters, and every scene
//! (`render_scene`).

use super::*;

impl Game {
    pub(crate) fn draw_elem(&mut self, id: u16, frame: Option<u16>, palette: Option<usize>) {
        let e = self.ui_elem(id);
        if e.kind != UI_SPRITE {
            return;
        }
        let pal = palette.or((e.p[8] > 0).then_some(e.p[8] as usize));
        let sprite = e.p[3] as u16;
        self.screen.blit(
            sprite,
            frame.unwrap_or(e.p[7] as u16),
            e.x as i32,
            e.y as i32,
            e.p[6].clamp(0, 64) as u8,
            false,
            pal.map(|s| (sprite, s)),
        );
    }

    /// Font palette sets (font sprite 17): 0 = white, 1 = dark navy.
    pub(crate) fn draw_text(&mut self, id: u16, x: i32, y: i32, center: bool, palette: usize) {
        let glyphs = self.string(id);
        self.screen.text(&self.data.fonts, &glyphs, x, y, center, palette);
    }

    pub(crate) fn digits(&self, n: u32) -> Vec<u16> {
        n.to_string().bytes().map(|b| DIGIT_0 + (b - b'0') as u16).collect()
    }

    pub(crate) fn draw_number(&mut self, n: u32, x: i32, y: i32, palette: usize) {
        let glyphs = self.digits(n);
        self.screen.text(&self.data.fonts, &glyphs, x, y, true, palette);
    }

    /// A number whose right edge is at `right` (balances that must stay on screen).
    pub(crate) fn draw_number_right(&mut self, n: u32, right: i32, y: i32, palette: usize) {
        let glyphs = self.digits(n);
        let w = crate::screen::Screen::text_width(&self.data.fonts, &glyphs);
        self.screen.text(&self.data.fonts, &glyphs, right - w, y, false, palette);
    }

    /// Field background sprite for the current field and time of day (`field_bg_create` 0x10015d90).
    pub(crate) fn field_bg(&self) -> Option<u16> {
        self.field_bg_at(self.pet.field, self.pet.in_area2)
    }

    pub(crate) fn field_bg_at(&self, field: u8, area2: bool) -> Option<u16> {
        let now = self.pet.last.unwrap_or(12 * 3600);
        let period = crate::pet::Pet::period(now) as usize;
        let weather = self.pet.weather_now(&self.data, now) as usize;
        let id = self.data.field_bg.get(field as usize)?[area2 as usize][1 + period * 3 + weather];
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
        // Rain / snow when the weather is 2: a full-screen 4-frame overlay (UI element 0x2d5).
        if self.pet.weather_now(&self.data, self.pet.last.unwrap_or(0)) == 2 {
            let frame = (self.frame_count / 4 % 4) as u16;
            self.screen.blit(PARTICLE, frame, 64, 64, 64, false, None);
        }
    }

    /// Draw a character (TamaID) standing at (x, y) in its large idle pose, with `color`.
    pub(crate) fn draw_character(&mut self, id: u16, color: Option<u8>, x: i32, y: i32) {
        self.draw_character_pose(id, color, 1, x, y);
    }

    pub(crate) fn draw_character_pose(&mut self, id: u16, color: Option<u8>, anim: u16, x: i32, y: i32) {
        let Some(def) = self.data.character(id) else { return };
        let Some(parts) = self.data.model(def.chr_id).and_then(|m| m.anims.get(anim as usize)).cloned() else { return };
        let color = color.filter(|&c| c < 16).unwrap_or(def.color) as usize;
        for part in parts {
            if !matches!(part.kind, 0 | 1 | 15) {
                continue;
            }
            let pal = (part.kind == 0 && def.stage > 1 && color < 16).then_some((206, color));
            self.screen.blit(part.sprite, part.frame, x + part.x as i32, y + part.y as i32, part.alpha, part.flip, pal);
        }
    }

    /// Grazing tamas walking around the field (small poses; up to 4).
    pub(crate) fn draw_grazing(&mut self) {
        const SPOTS: [(i32, i32); 4] = [(24, 72), (104, 76), (36, 112), (96, 116)];
        let shown = self.pet.grazing_shown(self.pet.field);
        let bob = (self.frame_count / 15 % 2) as u16;
        for (g, &(x, y)) in shown.iter().zip(SPOTS.iter()) {
            self.draw_character_pose(g.id, Some(g.color), 30 + bob, x, y);
        }
    }

    pub(crate) fn draw_poop(&mut self) {
        let old = self.pet.unchi.iter().filter(|&&a| a >= crate::pet::UNCHI_OLD).count();
        let frame = (self.frame_count / 15 % 2) as u16;
        for i in 0..self.pet.unchi.len() {
            self.draw_elem(elem::POOP[i], Some(frame), Some((i < old) as usize));
        }
    }

    /// Poops swept by the cleaning robot (`poopclean_show_field` 0x10070194): each waits until
    /// the robot's edge reaches it, (128 − x)·11/32 frames, then rises 128 px over 40 frames.
    fn draw_swept_poop(&mut self, n: usize, old: usize, t: u16) {
        let frame = t / 15 % 2;
        for (i, &id) in elem::POOP.iter().enumerate().take(n) {
            let x = self.ui_elem(id).x as i32;
            let wait = (128 - x) * 11 / 32;
            let dy = (128 * (t as i32 - wait)).clamp(0, 128 * 40) / 40;
            self.screen.off = (0, -dy);
            self.draw_elem(id, Some(frame), Some((i < old) as usize));
        }
        self.screen.off = (0, 0);
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
                    let (x, y) = (
                        64 + (10.0 * (1.0 - (45.0f32 * k as f32).to_radians().cos())) as i32,
                        64 - (88.0 * a.sin()) as i32,
                    );
                    let label = self.ui_elem(elem::MENU_LABEL).p[3] as u16;
                    let gray = items[i].1.is_none() as usize;
                    self.screen.blit(label, 0, x, y, 64, false, (gray > 0).then_some((label, gray)));
                    // Text element 0x9e (`ringmenu_create` 0x1002af28): font 2, centred as a
                    // block (two-line names too), palette 4 when greyed.
                    let glyphs = self.string(items[i].0);
                    self.draw_text_elem_at(elem::MENU_TEXT, &glyphs, x, y, (gray > 0).then_some(4));
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
                self.draw_list(
                    &[text::YES, text::NO],
                    !*yes as usize,
                    Some(if *wake { text::WAKE_UP } else { text::TIME_TO_SLEEP }),
                );
                true
            }
            Scene::Stars { idx, page, node } => {
                self.draw_stars(*idx, *page, node);
                true
            }
            Scene::Credits { node, .. } => {
                let mut draws = Vec::new();
                node.draws(64, 64, &mut draws);
                self.draw_list_of(&draws, None);
                true
            }
            Scene::Play(p) => {
                self.draw_field_bg();
                if p.food.is_some() {
                    self.screen.blit(TABLE, 0, 64, 106, 64, false, None);
                }
                if let Some((n, old)) = p.sweep {
                    self.draw_swept_poop(n, old, p.frames);
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
            Scene::Sitter { node } | Scene::Dying { node } => {
                self.draw_field_bg();
                let mut draws = Vec::new();
                node.draws(64, 64, &mut draws);
                self.draw_list_of(&draws, None);
                true
            }
            Scene::Support { pet, round, t, busy } => {
                self.draw_support(pet, *round, *t, *busy);
                true
            }
            Scene::Memories { pet, icons, t } => {
                self.draw_memories(pet, icons, *t);
                true
            }
            Scene::LabResult { stars, points, nodes, t } => {
                self.draw_lab_result(*stars, *points, nodes, *t);
                true
            }
            Scene::Level { level, .. } => {
                self.draw_level(*level);
                true
            }
            Scene::Clock { vals, pos, .. } => {
                self.draw_clock(vals, *pos);
                true
            }
            Scene::LabSplash { .. } => {
                self.draw_lab_splash();
                true
            }
            Scene::LabMenu { idx } => {
                self.draw_lab_menu(*idx);
                true
            }
            Scene::UserInfo { page } => {
                self.draw_user_info(*page);
                true
            }
            Scene::EvolvePoints { .. } => {
                self.draw_evolve_points();
                true
            }
            Scene::Graveyard { nodes } => {
                self.draw_graveyard(nodes);
                true
            }
            Scene::Dead { page, nodes } => {
                self.draw_dead(*page, nodes);
                true
            }
            Scene::LabCode { chars, pos } => {
                self.draw_lab_code(chars, *pos);
                true
            }
            Scene::Searching { .. } => {
                self.draw_list(&[], 0, Some(0xe1));
                true
            }
            Scene::Found { chr, zoom, from } => {
                self.draw_found(*chr, *zoom, from.is_none());
                true
            }
            Scene::Collection { idx } => {
                self.draw_collection(*idx);
                true
            }
            Scene::Card { kind, items, idx } => {
                self.draw_card(*kind, items, *idx);
                true
            }
            Scene::Bonus => {
                self.draw_list(&[], 0, Some(0xe2));
                self.draw_digits(crate::shop::LAB_BONUS, 64, 96);
                true
            }
            Scene::Points { points, .. } => {
                self.draw_list(&[], 0, Some(0xb6));
                self.draw_digits(*points, 64, 80);
                true
            }
            // Message box (`msgbox_create` 0x10025140): the white frame 0x9d, its text centred in
            // element 0x9e.
            Scene::Message { text, .. } => {
                self.draw_elem(elem::LIST_BG, None, None);
                self.draw_elem(elem::MSG_BOX, None, None);
                let glyphs = self.string(*text);
                self.draw_text_elem(elem::MENU_TEXT, &glyphs);
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
            Scene::MissionClear { queue, reward, t } => {
                self.draw_mission_clear(queue[0], *reward, *t);
                true
            }
            Scene::Missions { view, idx, t } => {
                self.draw_mission(*view, *idx, *t);
                true
            }
            Scene::Breed { cands, idx } => {
                self.draw_list(&[], 0, Some(0x19));
                self.draw_character(cands[*idx], None, 64, 112);
                true
            }
            Scene::Setsubi { toy, field, area, idx } => {
                self.draw_setsubi(*toy, *field, *area, *idx);
                true
            }
            Scene::Friends { idx } => {
                self.draw_friend(*idx);
                true
            }
            Scene::PlanetName { chars, pos, .. } => {
                self.draw_planet_name(chars, *pos);
                true
            }
            Scene::Birthday { m, d, pos, .. } => {
                self.draw_birthday(*m, *d, *pos);
                true
            }
            Scene::HuntTeam { idx, picked, .. } => {
                self.draw_hunt_team(*idx, picked);
                true
            }
            Scene::HuntEggs { items, t, peach } => {
                self.draw_hunt_eggs(items, *t, *peach);
                true
            }
            Scene::PlanetLv { page } => {
                self.draw_planet_lv(*page);
                true
            }
            Scene::Graves { idx, .. } => {
                self.draw_grave(*idx);
                true
            }
            Scene::Mini { session, .. } => {
                self.screen.rect(0, 0, 128, 128, [0, 0, 0]);
                if let Some(bg) = self.field_bg().filter(|_| session.game.field_bg()) {
                    self.screen.zoom = 2.0;
                    self.screen.blit(bg, 0, 64, 64, 64, false, None);
                    self.screen.zoom = 1.0;
                }
                let mut draws = Vec::new();
                session.draws(&mut draws);
                self.draw_list_of(&draws, None);
                for h in session.hud() {
                    match h {
                        crate::mini::Hud::Sprite { id, frame, x, y } => {
                            self.screen.blit(id, frame, x, y, 64, false, None);
                        }
                        crate::mini::Hud::Number { n, x, y } => self.draw_digits(n, x, y),
                    }
                }
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
        // The count top left (element 0xd9); free items show the infinity sign instead. The
        // capsule element 0xe6 is not shown on the card.
        if item.buy != 0xffff {
            self.draw_number_elem(elem::CARD_COUNT, self.pet.count(item.id) as u32);
        } else {
            let e = self.ui_elem(elem::CARD_COUNT);
            self.screen.blit(INFINITY, 0, e.x as i32, e.y as i32, 64, false, None);
        }
        if items.len() > 1 {
            self.screen.blit(ARROWS, 2, 10, 116, 64, false, None);
            self.screen.blit(ARROWS, 3, 118, 116, 64, false, None);
        }
    }

    /// Number in the 16×16 digit font (sprite 19: frame 0 blank, digit d = frame d + 1), 10 px apart, centred on (x, y).
    pub(crate) fn draw_digits(&mut self, n: u32, x: i32, y: i32) {
        let s = n.to_string();
        let w = s.len() as i32 * 10;
        for (i, b) in s.bytes().enumerate() {
            self.screen.blit(19, (b - b'0') as u16 + 1, x - w / 2 + 5 + i as i32 * 10, y, 64, false, None);
        }
    }

    /// Draw motion-script output with the food placeholder replaced.
    pub(crate) fn draw_list_of(&mut self, draws: &[crate::motion::Draw], food: Option<u16>) {
        let saved = (self.screen.zoom, self.screen.off);
        for d in draws {
            // ponytail: a turn applies to each sprite about its own centre (character parts too).
            self.screen.rot = d.rot as f32 / 256.0;
            if d.scale == 0 {
                continue;
            }
            // A node's own scale is about its position: fold it into the screen zoom and offset.
            if d.scale != 256 {
                let s = d.scale as f32 / 256.0;
                let o = |p: i32, off: i32| ((p as f32 * (1.0 - s) + off as f32 - 64.0) / s + 64.0).round() as i32;
                self.screen.off = (o(d.x, saved.1 .0), o(d.y, saved.1 .1));
                self.screen.zoom = saved.0 * s;
            }
            match d.look {
                crate::motion::Look::Sprite { id, frame } => {
                    let id = if id == FOOD_PLACEHOLDER { food.unwrap_or(id) } else { id };
                    self.screen.blit(
                        id,
                        frame,
                        d.x,
                        d.y,
                        d.alpha,
                        d.flip,
                        (d.palette > 0).then_some((id, d.palette as usize)),
                    );
                }
                crate::motion::Look::Chr { chr, anim, eyes, mouth } => {
                    self.draw_chr(d, chr, anim, eyes, mouth);
                }
                crate::motion::Look::None => {}
            }
            (self.screen.zoom, self.screen.off) = saved;
            self.screen.rot = 0.0;
        }
    }

    /// CELL_STATUS (`cellstatus_state` 0x100452f0).
    pub(super) fn draw_status(&mut self, page: u8) {
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
