//! HOME layer changes (docs/spec/08-care-ui.md §2.22): the zoom-and-fade transitions to and from
//! the cell (pink) and the planet (white) with the warp overlay, the cell layer's own picture,
//! and the layer indicator bar.

use super::*;
use crate::pet::{req, status, Cond};

/// Fade colours: the cell's pink (RGB565 0xF43B) and white.
const PINK: [u8; 3] = [0xF7, 0x87, 0xDE];
const WHITE: [u8; 3] = [255, 255, 255];
/// Warp overlay (sprite 1102 speed lines): fading in when leaving, out when arriving.
const WARP_IN: u16 = 0x2c6;
const WARP_OUT: u16 = 0x2c7;
/// Cell layer: organelles over the tissue (script 0xf8, sprite 1160).
const CELL_SCENE: u16 = 0xf8;
const CELL_BG: u16 = 0x356;
const MEMORY_SLOTS: u16 = 0x35a;
/// Layer indicator: bar (sprite 1100), cursor slots (sprite 1101), alert slots (sprite 1103).
const BAR: u16 = 0x2bd;
const CURSOR_SLOTS: u16 = 0x2be;
const ALERT_SLOTS: u16 = 0x2c2;
/// Indicator timing: 5 frames in, 100 frames shown, 5 frames out.
const BAR_SLIDE: u16 = 5;
const BAR_SHOW: u16 = 100;
/// Sound when arriving on the field or pet layer, and when zooming out of a scene.
const SND_LAYER: u16 = 9;
const SND_ZOOM_OUT: u16 = 0x2b;

/// A layer change in progress: which one, and frames since it started.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Change {
    /// Field / pet → cell (`FUN_10085bb4` then `FUN_10084cfc`).
    ToCell,
    /// Cell → pet (`FUN_10084d70` then `FUN_100859a8`).
    FromCell,
    /// Field → planet (`FUN_10085c14` then `space_zoom_out_enter` 0x10088aac).
    ToSpace,
    /// Planet → field (`space_zoom_in_leave` 0x10088af4 then `FUN_10085aec`).
    FromSpace,
}

/// One half of a change: layer drawn, zoom from → to over `frames`, fade colour, fading out
/// (with a delay and step) or in (step 4), and the warp overlay to start.
struct Half {
    layer: Level,
    zoom: (f32, f32),
    frames: u16,
    colour: [u8; 3],
    out: Option<(u16, u8)>,
    warp: Option<u16>,
}

impl Change {
    fn halves(self, from_zoom: f32) -> [Half; 2] {
        let space = zoom_stop(Level::Space);
        match self {
            Change::ToCell => [
                Half {
                    layer: Level::Pet,
                    zoom: (from_zoom, 36.0),
                    frames: 12,
                    colour: PINK,
                    out: Some((8, 8)),
                    warp: None,
                },
                Half {
                    layer: Level::Cell,
                    zoom: (0.0, 16.0),
                    frames: 20,
                    colour: PINK,
                    out: None,
                    warp: Some(WARP_OUT),
                },
            ],
            Change::FromCell => [
                Half {
                    layer: Level::Cell,
                    zoom: (16.0, 0.0),
                    frames: 12,
                    colour: PINK,
                    out: Some((8, 8)),
                    warp: Some(WARP_IN),
                },
                Half { layer: Level::Pet, zoom: (36.0, 8.0), frames: 16, colour: PINK, out: None, warp: None },
            ],
            Change::ToSpace => [
                Half {
                    layer: Level::Field,
                    zoom: (from_zoom, -20.0),
                    frames: 16,
                    colour: WHITE,
                    out: Some((0, 4)),
                    warp: Some(WARP_IN),
                },
                Half {
                    layer: Level::Space,
                    zoom: (space + 32.0, space),
                    frames: 12,
                    colour: WHITE,
                    out: None,
                    warp: None,
                },
            ],
            Change::FromSpace => [
                Half {
                    layer: Level::Space,
                    zoom: (space, space + 32.0),
                    frames: 12,
                    colour: WHITE,
                    out: Some((0, 4)),
                    warp: None,
                },
                Half {
                    layer: Level::Field,
                    zoom: (-16.0, 0.0),
                    frames: 24,
                    colour: WHITE,
                    out: None,
                    warp: Some(WARP_OUT),
                },
            ],
        }
    }

    /// The layer the change ends on.
    fn target(self) -> Level {
        match self {
            Change::ToCell => Level::Cell,
            Change::FromCell => Level::Pet,
            Change::ToSpace => Level::Space,
            Change::FromSpace => Level::Field,
        }
    }
}

impl Game {
    /// Start a layer change; the dial is ignored until it ends.
    pub(crate) fn begin_change(&mut self, c: Change) {
        self.sfx(if matches!(c, Change::ToCell | Change::FromSpace) { 0x28 } else { SND_ZOOM_OUT }, 100);
        self.change = Some((c, 0, self.zoom));
        self.indicator = None;
        self.level = c.target();
        self.step_change();
    }

    /// Advance the change by one frame: zoom, fade and overlay; the indicator at the end.
    pub(crate) fn step_change(&mut self) {
        let Some((c, t, from)) = self.change else { return };
        let halves = c.halves(from);
        let first = t < halves[0].frames;
        let (h, u) = if first { (&halves[0], t) } else { (&halves[1], t - halves[0].frames) };
        if u == 0 {
            self.warp = h.warp.map(|id| Node::new(self.ui_elem(id).p[3] as u16));
        }
        let k = (u + 1).min(h.frames) as f32 / h.frames as f32;
        self.zoom = h.zoom.0 + (h.zoom.1 - h.zoom.0) * k;
        self.shown = Some(h.layer);
        self.fade_colour = h.colour;
        self.fade = match h.out {
            Some((delay, speed)) => (u + 1).saturating_sub(delay).min(8) as u8 * speed,
            None => FADE_FULL.saturating_sub((u as u8 + 1) * FADE_STEP),
        }
        .min(FADE_FULL);
        if !first && u + 1 >= h.frames {
            self.change = None;
            self.shown = None;
            self.zoom = zoom_stop(c.target());
            self.settled = Some(c.target());
            self.show_indicator();
        } else {
            self.change = Some((c, t + 1, from));
        }
    }

    /// Field ↔ pet needs no change scene: a sound and the indicator when the zoom arrives.
    pub(crate) fn arrive(&mut self) {
        self.sfx(SND_LAYER, 100);
        self.show_indicator();
    }

    pub(crate) fn show_indicator(&mut self) {
        self.indicator = Some(0);
    }

    pub(crate) fn tick_overlays(&mut self) {
        if let Some(w) = &mut self.warp {
            w.tick(&self.data.motions);
            if w.done {
                self.warp = None;
            }
        }
        self.indicator = self.indicator.map(|t| t + 1).filter(|&t| t < BAR_SLIDE * 2 + BAR_SHOW);
        if self.level == Level::Cell {
            self.cell_scene.tick(&self.data.motions);
        }
    }

    /// The warp overlay, fixed to the screen.
    pub(crate) fn draw_warp(&mut self) {
        if let Some(w) = self.warp.take() {
            let mut draws = Vec::new();
            w.draws(64, 64, &mut draws);
            self.draw_list_of(&draws, None);
            self.warp = Some(w);
        }
    }

    /// The cell layer (HOME_CELL 0x20, `FUN_10084974`): tissue with drifting organelles and the
    /// icons of what the pet lived through; the pet is drawn over it as a cell.
    pub(crate) fn draw_cell(&mut self) {
        // Zoomed below 0.5 the half-size tissue (frame 1) is drawn twice as large, so it still
        // covers the screen; the scene's own copy of the tissue is not drawn.
        let bg = self.ui_elem(CELL_BG);
        let zoom = self.screen.zoom;
        let small = zoom < 0.5;
        self.screen.zoom = if small { zoom * 2.0 } else { zoom };
        self.screen.blit(bg.p[3] as u16, small as u16, bg.x as i32, bg.y as i32, 64, false, None);
        self.screen.zoom = zoom;
        self.cell_scene.visible = false;
        let mut draws = Vec::new();
        self.cell_scene.draws(64, 64, &mut draws);
        self.draw_list_of(&draws, None);
        for (i, icon) in self.memory_icons().into_iter().enumerate() {
            let e = self.ui_elem(MEMORY_SLOTS + i as u16);
            self.screen.blit(icon, 0, e.x as i32, e.y as i32, 64, false, None);
        }
    }

    /// The cell's motion for the pet (`FUN_1006d188`, table XIP 0x10091f58).
    pub(crate) fn cell_motion(&self) -> u16 {
        let p = &self.pet;
        let asleep = p.last.is_some_and(|t| p.is_sleeping_at(t));
        let onaka = match p.onaka {
            0..=2 => 0,
            3..=4 => 1,
            _ => 2,
        };
        let gokigen = match p.gokigen {
            0..=5 => 0,
            6..=15 => 1,
            _ => 2,
        };
        match p.cond {
            Cond::Cold if asleep => 0x100,
            Cond::Injury if asleep => 0x101,
            _ if asleep => 0xff,
            Cond::Cold => 0xfc,
            Cond::Injury | Cond::Death | Cond::Dying => 0xfb,
            Cond::Normal if p.is_pending(req::SOWASOWA) => 0xfa,
            Cond::Normal if p.is_pending(req::EVOL_SUPPORT) => 0x548,
            Cond::Normal if p.status & status::LOVE != 0 => 0xfd,
            Cond::Normal if p.status & status::IMMUNITY != 0 => 0xfe,
            Cond::Normal if p.status & status::HAPPY != 0 => 0xf9,
            Cond::Normal if onaka == 0 || gokigen == 0 => 0xfb,
            Cond::Normal if onaka == 2 || gokigen == 2 => 0xf9,
            _ => 0xfa,
        }
    }

    /// Layer indicator (`FUN_1001afa0`): bar, a cursor over this layer, and an alert over the
    /// layer that needs the player; slides up for 5 frames, stays 100, slides away.
    pub(crate) fn draw_indicator(&mut self) {
        let Some(t) = self.indicator else { return };
        let k = if t < BAR_SLIDE {
            (t + 1) as i32
        } else if t < BAR_SLIDE + BAR_SHOW {
            BAR_SLIDE as i32
        } else {
            (BAR_SLIDE * 2 + BAR_SHOW - t - 1) as i32
        };
        let (dy, alpha) = (8 - 8 * k / BAR_SLIDE as i32, (64 * k / BAR_SLIDE as i32) as u8);
        let bar = self.ui_elem(BAR);
        self.screen.blit(bar.p[3] as u16, 0, bar.x as i32, bar.y as i32 + dy, alpha, false, None);
        let layer = match self.level {
            Level::Space => 0,
            Level::Field => 1,
            Level::Pet => 2,
            Level::Cell => 3,
        };
        let cursor = self.ui_elem(CURSOR_SLOTS + layer);
        let y = bar.y as i32 + dy - 8;
        self.screen.blit(cursor.p[3] as u16, 0, cursor.x as i32, y, alpha, false, None);
        let p = &self.pet;
        let asleep = p.last.is_some_and(|t| p.is_sleeping_at(t));
        let alert = if [req::EVOLUTION, req::ONAKA, req::GOKIGEN].iter().any(|&r| p.is_pending(r)) {
            Some(2)
        } else if p.is_pending(req::EVOL_SUPPORT) {
            Some(3)
        } else if p.is_pending(req::DODODO) {
            Some(1)
        } else {
            match p.cond {
                Cond::Cold => Some(3),
                Cond::Injury | Cond::Death | Cond::Dying => Some(2),
                _ => None,
            }
        };
        if let (Some(a), false) = (alert, asleep) {
            let e = self.ui_elem(ALERT_SLOTS + a);
            self.screen.blit(e.p[3] as u16, 0, e.x as i32, y, alpha, false, None);
        }
    }

    pub(crate) fn new_cell_scene() -> Node {
        Node::new(CELL_SCENE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOON: u64 = 86_400 * 3 + 12 * 3600;

    #[test]
    fn into_the_cell_and_back_with_fades_and_the_indicator() {
        let mut g = Game::new(crate::evolve::tests::data(), 1);
        g.pet.def = g.data.character(2113).unwrap();
        (g.pet.onaka, g.pet.gokigen, g.pet.rank_seen) = (6, 20, g.pet.rank);
        g.frame(NOON, 0); // a new game starts on the pet layer
        g.take_sounds();
        assert!(matches!(g.scene, Scene::Home), "scene");
        assert_eq!(g.level, Level::Pet);
        g.frame(NOON, UP); // cell
        assert_eq!(g.change.map(|c| c.0), Some(Change::ToCell));
        let sounds = g.take_sounds();
        assert!(sounds.iter().any(|s| s.0 == 0x28), "{sounds:?}");
        let mut max_fade = 0;
        for i in 0..32 {
            max_fade = max_fade.max(g.fade);
            g.frame(NOON, if i == 3 { DOWN } else { 0 }); // ignored while changing
        }
        assert_eq!(max_fade, FADE_FULL, "pink flash");
        assert_eq!(g.level, Level::Cell);
        g.frame(NOON, 0);
        assert!(g.indicator.is_some(), "the indicator shows on arrival");
        g.frame(NOON, DOWN);
        assert_eq!(g.change.map(|c| c.0), Some(Change::FromCell));
        for _ in 0..40 {
            g.frame(NOON, 0);
        }
        assert_eq!((g.level, g.zoom), (Level::Pet, 8.0));
    }

    #[test]
    fn down_to_the_planet_and_back_up() {
        let mut data = crate::evolve::tests::data();
        // Three columns per behaviour, as in the real table: the planet must never index it.
        data.home_motions = vec![[0, 0, 0]; 0x20];
        let mut g = Game::new(data, 1);
        g.pet.def = g.data.character(2113).unwrap();
        (g.pet.onaka, g.pet.gokigen, g.pet.rank_seen) = (6, 20, g.pet.rank);
        g.frame(NOON, 0);
        g.frame(NOON, DOWN); // field
        for _ in 0..20 {
            g.frame(NOON, 0);
        }
        g.frame(NOON, DOWN);
        assert_eq!(g.change.map(|c| c.0), Some(Change::ToSpace));
        while g.change.is_some() {
            g.frame(NOON, 0);
        }
        assert_eq!(g.level, Level::Space);
        g.frame(NOON, UP);
        assert_eq!(g.change.map(|c| c.0), Some(Change::FromSpace));
        while g.change.is_some() {
            g.frame(NOON, 0);
        }
        assert_eq!((g.level, g.zoom), (Level::Field, 0.0));
    }
}
