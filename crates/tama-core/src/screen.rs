//! 128×128 RGBA framebuffer plus the sprites the platform has supplied.

use crate::data::Font;
use crate::sprite::Sprite;
use std::collections::{BTreeSet, HashMap};

pub const WIDTH: usize = 128;
pub const HEIGHT: usize = 128;
/// Decoded sprite bytes kept before unused ones are dropped (the platform re-supplies them).
// ponytail: flat 8 MB budget sized for Apple Watch; make it a Game setting if a platform needs another.
pub const SPRITE_BUDGET: usize = 8 << 20;
/// Frames a sprite must go undrawn before it can be dropped (5 s at 20 fps).
const SPRITE_GRACE: u32 = 100;

pub struct Screen {
    /// Decoded sprites by sprite-pack index.
    pub sprites: HashMap<u16, Sprite>,
    /// Render count when each sprite was last drawn (see `evict`).
    pub used: HashMap<u16, u32>,
    /// Renders so far.
    pub tick: u32,
    /// RGBA8888, row-major, `w` × `h` (128 × 128 unless the platform asks for a larger canvas).
    pub fb: Vec<u8>,
    pub w: usize,
    pub h: usize,
    /// Where the game's 128 × 128 screen sits on the canvas (its top-left corner).
    pub org: (i32, i32),
    /// Sprites a draw asked for that are not loaded yet (the platform fetches them).
    pub wanted: BTreeSet<u16>,
    /// Added to every blit position (camera scroll).
    pub off: (i32, i32),
    /// Scale about the screen centre applied to every blit (1.0 = none).
    pub zoom: f32,
    /// Turn of the next sprites about their centre, in turns (0..1, clockwise).
    pub rot: f32,
    /// A further scale about `pivot` (screen pixels), after `off` and `zoom`: the HOME zoom
    /// between layers, centred on the planet, the pet or its mouth.
    pub rel: f32,
    pub pivot: (f32, f32),
}

impl Default for Screen {
    fn default() -> Self {
        Screen {
            sprites: HashMap::new(),
            used: HashMap::new(),
            tick: 0,
            fb: vec![0; WIDTH * HEIGHT * 4],
            w: WIDTH,
            h: HEIGHT,
            org: (0, 0),
            wanted: BTreeSet::new(),
            off: (0, 0),
            zoom: 1.0,
            rot: 0.0,
            rel: 1.0,
            pivot: (64.0, 64.0),
        }
    }
}

impl Screen {
    /// Once per render: drop the longest-unused sprites while the decoded total is over
    /// `SPRITE_BUDGET`, sparing any drawn in the last `SPRITE_GRACE` renders.
    pub fn evict(&mut self) {
        let size = |s: &Sprite| s.pixels.len() + s.palettes.iter().map(|p| p.len() * 2).sum::<usize>();
        let mut total: usize = self.sprites.values().map(size).sum();
        if total > SPRITE_BUDGET {
            let mut old: Vec<(u32, u16)> = (self.sprites.keys())
                .map(|&id| (self.used.get(&id).copied().unwrap_or(0), id))
                .filter(|&(t, _)| self.tick.wrapping_sub(t) > SPRITE_GRACE)
                .collect();
            old.sort_unstable();
            for (_, id) in old {
                if total <= SPRITE_BUDGET {
                    break;
                }
                total -= self.sprites.remove(&id).map_or(0, |s| size(&s));
                self.used.remove(&id);
            }
        }
        self.tick = self.tick.wrapping_add(1);
    }

    /// Where (x, y) lands on screen with `off` and `zoom` (before `rel`).
    pub fn project(&self, x: i32, y: i32) -> (f32, f32) {
        let z = |p: i32, o: i32| 64.0 + (p + o - 64) as f32 * self.zoom;
        (z(x, self.off.0), z(y, self.off.1))
    }

    /// A canvas of `w` × `h` (at least 128 × 128) with the game's screen centred: the remake's
    /// larger screens (the watch) show more around it. Full-screen backgrounds repeat into the
    /// margins; whatever nothing covers stays transparent for the platform to fill.
    pub fn resize(&mut self, w: usize, h: usize) {
        let (w, h) = (w.max(WIDTH), h.max(HEIGHT));
        (self.w, self.h) = (w, h);
        self.org = (((w - WIDTH) / 2) as i32, ((h - HEIGHT) / 2) as i32);
        self.fb = vec![0; w * h * 4];
    }

    /// Clear the canvas: the game's screen black, the margins transparent.
    pub fn clear(&mut self) {
        self.fb.fill(0);
        let (ox, oy) = (self.org.0 as usize, self.org.1 as usize);
        for y in oy..oy + HEIGHT {
            for px in self.fb[(y * self.w + ox) * 4..(y * self.w + ox + WIDTH) * 4].as_chunks_mut::<4>().0 {
                px[3] = 255;
            }
        }
    }

    /// A filled rectangle in screen coordinates; one covering the whole screen covers the canvas.
    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: [u8; 3]) {
        let (x, y, w, h) = if x <= 0 && y <= 0 && x + w >= WIDTH as i32 && y + h >= HEIGHT as i32 {
            (-self.org.0, -self.org.1, self.w as i32, self.h as i32)
        } else {
            (x, y, w, h)
        };
        let (cw, ch) = (self.w as i32, self.h as i32);
        for yy in (y + self.org.1).max(0)..(y + h + self.org.1).min(ch) {
            for xx in (x + self.org.0).max(0)..(x + w + self.org.0).min(cw) {
                let i = (yy as usize * self.w + xx as usize) * 4;
                self.fb[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
    }

    /// Draw `frame` of sprite `id` centred on (cx, cy) with alpha 0–64 and optional flip.
    /// `palette` = (sprite, set) whose palette replaces the sprite's own set 0.
    /// Returns false if the sprite is not loaded yet.
    #[allow(clippy::too_many_arguments)] // mirrors the firmware's sprite draw call
    pub fn blit(
        &mut self,
        id: u16,
        frame: u16,
        cx: i32,
        cy: i32,
        alpha: u8,
        flip: bool,
        palette: Option<(u16, usize)>,
    ) -> bool {
        let (cx, cy) = (cx + self.off.0, cy + self.off.1);
        self.used.insert(id, self.tick);
        if let Some((pid, _)) = palette {
            self.used.insert(pid, self.tick);
        }
        let Some(s) = self.sprites.get(&id) else {
            self.wanted.insert(id);
            return false;
        };
        if let Some((pid, _)) = palette {
            if !self.sprites.contains_key(&pid) {
                self.wanted.insert(pid);
            }
        }
        let (w, h) = (s.w as i32, s.h as i32);
        // The sprite's own origin (record bytes 10/11), mirrored with the sprite.
        let (cx, cy) = (cx + if flip { -s.origin.0 } else { s.origin.0 } as i32, cy + s.origin.1 as i32);
        let f = (frame % s.frames.max(1)) as usize * (w * h) as usize;
        // A palette borrowed from another sprite (pet colours, sprite 206) only defines entries
        // 0–7; the body keeps its own 8–15 (blush, shading).
        let borrowed = palette.is_some_and(|(pid, _)| pid != id);
        let pal = palette.and_then(|(pid, set)| self.sprites.get(&pid).map(|p| (p, set)));
        let a = alpha.min(64) as u32;
        // Zoom scales positions and sizes about the screen centre (HOME zoom between layers).
        let k = self.zoom * self.rel;
        let (dw, dh) = ((w as f32 * k).round() as i32, (h as f32 * k).round() as i32);
        let (px, py) = self.project(cx - self.off.0, cy - self.off.1);
        let (px, py) = (self.pivot.0 + (px - self.pivot.0) * self.rel, self.pivot.1 + (py - self.pivot.1) * self.rel);
        let (x0, y0) = (px.round() as i32 - dw / 2, py.round() as i32 - dh / 2);
        // Turned: walk a square covering the turned sprite and map each pixel back.
        let (sin, cos) = (std::f32::consts::TAU * self.rot).sin_cos();
        let turned = self.rot != 0.0;
        let (cxf, cyf) = (x0 as f32 + dw as f32 / 2.0, y0 as f32 + dh as f32 / 2.0);
        let r = if turned { ((dw * dw + dh * dh) as f32).sqrt() / 2.0 + 1.0 } else { 0.0 };
        let (bx0, by0, bx1, by1) = if turned {
            ((cxf - r) as i32, (cyf - r) as i32, (cxf + r) as i32 + 1, (cyf + r) as i32 + 1)
        } else {
            (x0, y0, x0 + dw, y0 + dh)
        };
        // A screen-sized background drawn over the whole screen repeats into the canvas margins.
        let fills = !turned && (x0, y0, dw, dh) == (0, 0, WIDTH as i32, HEIGHT as i32) && self.org != (0, 0);
        let tiles: &[(i32, i32)] = if fills {
            &[(0, 0), (-128, 0), (128, 0), (0, -128), (0, 128), (-128, -128), (128, -128), (-128, 128), (128, 128)]
        } else {
            &[(0, 0)]
        };
        let (cw, ch) = (self.w as i32, self.h as i32);
        for &(tx, ty) in tiles {
            let (bx0, by0, bx1, by1) = (bx0 + tx, by0 + ty, bx1 + tx, by1 + ty);
            let (x0, y0, cxf, cyf) = (x0 + tx, y0 + ty, cxf + tx as f32, cyf + ty as f32);
            for py in by0.max(-self.org.1)..by1.min(ch - self.org.1) {
                for px in bx0.max(-self.org.0)..bx1.min(cw - self.org.0) {
                    let (x, y) = if turned {
                        let (dx, dy) = (px as f32 + 0.5 - cxf, py as f32 + 0.5 - cyf);
                        let (ux, uy) = (cos * dx + sin * dy, -sin * dx + cos * dy);
                        let (x, y) =
                            (((ux + dw as f32 / 2.0) / k).floor() as i32, ((uy + dh as f32 / 2.0) / k).floor() as i32);
                        if x < 0 || y < 0 || x >= w || y >= h {
                            continue;
                        }
                        (x, y)
                    } else {
                        ((((px - x0) as f32 / k) as i32).min(w - 1), (((py - y0) as f32 / k) as i32).min(h - 1))
                    };
                    let sx = if flip { w - 1 - x } else { x };
                    let idx = s.pixels[f + (y * w + sx) as usize];
                    if s.transparent && idx == s.key {
                        continue;
                    }
                    let c = match pal {
                        Some(_) if borrowed && idx >= BORROWED_COLOURS => s.rgba(0, idx),
                        Some((p, set)) => p.rgba(set, idx),
                        None => s.rgba(0, idx),
                    };
                    let i = ((py + self.org.1) as usize * self.w + (px + self.org.0) as usize) * 4;
                    // On a transparent pixel (the canvas margins) a see-through sprite blends with black.
                    for (dst, &src) in self.fb[i..i + 3].iter_mut().zip(&c[..3]) {
                        *dst = ((src as u32 * a + *dst as u32 * (64 - a)) / 64) as u8;
                    }
                    self.fb[i + 3] = 255;
                }
            }
        }
        true
    }
}

/// Entries a borrowed palette replaces (sprite 206's colour sets have 8 colours).
const BORROWED_COLOURS: u8 = 8;

/// Glyph codes 0xF000.. are control codes; 0xF000 is a newline.
pub const NEWLINE: u16 = 0xF000;
/// Text code that ends a page.
pub const END: u16 = 0xF001;

impl Screen {
    /// Width in pixels of one line of glyph codes. `fonts[1]` covers kana/Latin, `fonts[2]` adds CJK.
    pub fn text_width(fonts: &[Font], glyphs: &[u16]) -> i32 {
        glyphs.iter().take_while(|&&c| c != NEWLINE).map(|&c| glyph(fonts, c).map_or(0, |(f, _)| advance(f, c))).sum()
    }

    /// Draw text with each line centred on `cx` (or starting at `cx` when `center` is false);
    /// `cy` is the middle of the first line. `palette` picks the font sprite's palette set.
    pub fn text(&mut self, fonts: &[Font], glyphs: &[u16], cx: i32, cy: i32, center: bool, palette: usize) {
        let mut y = cy;
        for line in glyphs.split(|&c| c == NEWLINE) {
            let mut x = if center { cx - Self::text_width(fonts, line) / 2 } else { cx };
            for &c in line {
                if c == SPACE {
                    x += fonts.get(1).or(fonts.first()).map_or(0, |f| advance(f, c));
                    continue;
                }
                let Some((f, sprite)) = glyph(fonts, c) else { continue };
                let w = self.sprites.get(&sprite).map_or(16, |s| s.w as i32);
                let pal = (palette > 0).then_some((sprite, palette));
                // Glyph codes start at 1; frame 0 of the font sprite is glyph 1. Narrow glyphs
                // (negative width adjust, e.g. "I") sit mid-cell, so shift them by half the adjust
                // to keep the gaps on both sides even.
                let shift = f.adj.get(c as usize).copied().unwrap_or(0) as i32 / 2;
                self.blit(sprite, c - 1, x + w / 2 + shift, y, 64, false, pal);
                x += advance(f, c);
            }
            y += fonts.get(1).map_or(12, |f| f.line_height as i32 + 2);
        }
    }
}

/// Glyph code of a space in the built-in fonts.
pub const SPACE: u16 = 305;

/// Break lines at spaces so none is wider than `max` pixels.
pub fn wrap(fonts: &[Font], glyphs: &[u16], max: i32) -> Vec<u16> {
    let mut out: Vec<u16> = Vec::with_capacity(glyphs.len());
    let mut line_start = 0;
    for &c in glyphs {
        out.push(c);
        if c == NEWLINE {
            line_start = out.len();
            continue;
        }
        if Screen::text_width(fonts, &out[line_start..]) > max {
            if let Some(sp) = out[line_start..].iter().rposition(|&g| g == SPACE) {
                out[line_start + sp] = NEWLINE;
                line_start += sp + 1;
            }
        }
    }
    out
}

/// How a UI text element lays out its text (`textbox_init` 0x10064d90, from the element's p[]).
#[derive(Clone, Copy, Debug)]
pub struct TextBox {
    /// Font number (p3).
    pub font: usize,
    /// 0 left, 1 centre, 2 right; 0 top, 1 middle, 2 bottom (p8 bits 4–5, 6–7).
    pub h: u8,
    pub v: u8,
    /// Extra advance per glyph (p4) and extra line spacing (p5).
    pub space: i32,
    pub leading: i32,
    /// Palette set (p7).
    pub palette: usize,
}

impl Screen {
    /// Draw text as the firmware's text box does (`FUN_100644d4`): glyphs centred on the pen,
    /// `x`/`y` the centre of the first line (left: first glyph, right: last glyph, centre: the
    /// middle of both), codes 0xF00C–0xF015 switch the palette, 0xF000 starts a line.
    pub fn text_box(&mut self, fonts: &[Font], glyphs: &[u16], x: i32, y: i32, b: TextBox) {
        let Some(font) = fonts.get(b.font) else { return };
        let text: Vec<u16> = glyphs.split(|&c| c == END).next().unwrap_or(&[]).to_vec();
        let lines: Vec<&[u16]> = text.split(|&c| c == NEWLINE).collect();
        let pitch = font.line_height as i32 + b.leading;
        let lines_up = (1 - lines.len() as i32) * pitch;
        let mut ly = y + match b.v {
            1 => lines_up.div_euclid(2),
            2 => lines_up,
            _ => 0,
        };
        let mut palette = b.palette;
        for line in lines {
            let shown: Vec<u16> = line.iter().copied().filter(|&c| c < 0xF000).collect();
            let adv = |c: u16| advance(font, c) + b.space;
            let w = match shown.as_slice() {
                [] | [_] => 0,
                [first, mid @ .., last] => {
                    adv(*first) / 2 + mid.iter().map(|&c| adv(c)).sum::<i32>() + adv(*last) - adv(*last) / 2
                }
            };
            let mut pen = x + match b.h {
                1 => -(w / 2),
                2 => -w,
                _ => 0,
            };
            let mut first = true;
            for &c in line {
                match c {
                    0xF00C..=0xF015 => palette = (c - 0xF00C) as usize,
                    0xF000.. => {}
                    _ => {
                        let a = adv(c);
                        if !first {
                            pen += a / 2;
                        }
                        first = false;
                        if c != SPACE && (c as usize) < font.adj.len() {
                            let pal = (palette > 0).then_some((font.sprite, palette));
                            self.blit(font.sprite, c - 1, pen, ly, 64, false, pal);
                        }
                        pen += (a + 1) / 2;
                    }
                }
            }
            ly += pitch;
        }
    }
}

fn glyph(fonts: &[Font], c: u16) -> Option<(&Font, u16)> {
    if c >= 0xF000 || c == 0 {
        return None;
    }
    let f = fonts.get(1).filter(|f| (c as usize) < f.adj.len()).or_else(|| fonts.get(2))?;
    Some((f, f.sprite))
}

fn advance(f: &Font, c: u16) -> i32 {
    f.advance as i32 + f.adj.get(c as usize).copied().unwrap_or(0) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_box_aligns_like_the_firmware() {
        // Font 2: digits (codes 2..11) advance 10. Right aligned at 72: "123" centres 52, 62, 72.
        let mut font = Font { sprite: 900, advance: 12, line_height: 12, adj: vec![0; 20] };
        for c in 1..12 {
            font.adj[c] = -2;
        }
        let fonts = vec![font.clone(), font.clone(), font];
        let mut s = Screen::default();
        let sprite = Sprite {
            w: 2,
            h: 2,
            frames: 12,
            bits: 2,
            transparent: false,
            key: 0,
            palettes: vec![vec![0xffff; 4]],
            pixels: vec![0; 2 * 2 * 12],
            origin: (0, 0),
        };
        s.sprites.insert(900, sprite);
        let b = TextBox { font: 2, h: 2, v: 1, space: 0, leading: 0, palette: 0 };
        s.text_box(&fonts, &[3, 4, 5], 72, 50, b);
        let lit = |x: usize| s.fb[(50 * WIDTH + x) * 4 + 3] != 0;
        assert!(lit(51) && lit(52) && lit(61) && lit(71) && lit(72));
        assert!(!lit(73) && !lit(50), "glyphs centred on 52..72");
    }

    #[test]
    fn wraps_at_spaces() {
        let font = Font { sprite: 17, advance: 10, line_height: 11, adj: vec![0; 400] };
        let fonts = vec![font.clone(), font];
        // "aaaa aaaa aaaa" with 10 px glyphs, max 95 px → two lines
        let a = 318;
        let g = [a, a, a, a, SPACE, a, a, a, a, SPACE, a, a, a, a];
        let w = wrap(&fonts, &g, 95);
        assert_eq!(w.iter().filter(|&&c| c == NEWLINE).count(), 1);
        assert!(w.iter().position(|&c| c == NEWLINE) == Some(9));
    }

    #[test]
    fn evicts_unused_sprites_over_budget() {
        // 1 MB sprites: 12 of them overflow the 8 MB budget.
        let big = || Sprite {
            w: 1024,
            h: 1024,
            frames: 1,
            bits: 8,
            palettes: vec![vec![0; 1]],
            pixels: vec![0; 1 << 20],
            ..Default::default()
        };
        let mut sc = Screen::default();
        for id in 0..12 {
            sc.sprites.insert(id, big());
        }
        // Sprites 0 and 1 drawn every render; the rest only before the grace period.
        for _ in 0..=SPRITE_GRACE + 1 {
            sc.blit(0, 0, 0, 0, 64, false, None);
            sc.blit(1, 0, 0, 0, 64, false, Some((0, 0)));
            sc.evict();
        }
        assert!(sc.sprites.contains_key(&0) && sc.sprites.contains_key(&1), "in-use sprites kept");
        assert!(sc.sprites.values().map(|s| s.pixels.len()).sum::<usize>() <= SPRITE_BUDGET);
        assert_eq!(sc.sprites.len(), 7, "only as many dropped as needed");
    }

    #[test]
    fn borrowed_colour_palette_keeps_the_bodys_upper_entries() {
        use crate::sprite::Sprite;
        let sprite = |pal: Vec<u16>, pixels: Vec<u8>| Sprite {
            w: 2,
            h: 1,
            frames: 1,
            bits: 4,
            transparent: false,
            key: 0,
            palettes: vec![pal],
            pixels,
            origin: (0, 0),
        };
        let mut body_pal = vec![0; 16];
        body_pal[2] = 0x001f; // blue body
        body_pal[13] = 0xf800; // red blush
        let mut colours = vec![0; 16];
        colours[2] = 0x07e0; // green body colour; entry 13 is black (unused)
        let mut sc = Screen::default();
        sc.sprites.insert(1, sprite(body_pal, vec![2, 13]));
        sc.sprites.insert(206, sprite(colours, vec![0, 0]));
        sc.blit(1, 0, 1, 0, 64, false, Some((206, 0)));
        assert_eq!(&sc.fb[0..3], &[0, 255, 0], "body takes the colour");
        assert_eq!(&sc.fb[4..7], &[255, 0, 0], "blush stays");
    }
}
