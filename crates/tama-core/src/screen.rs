//! 128×128 RGBA framebuffer plus the sprites the platform has supplied.

use crate::data::Font;
use crate::sprite::Sprite;
use std::collections::{BTreeSet, HashMap};

pub const WIDTH: usize = 128;
pub const HEIGHT: usize = 128;

pub struct Screen {
    /// Decoded sprites by sprite-pack index.
    pub sprites: HashMap<u16, Sprite>,
    /// RGBA8888, row-major.
    pub fb: Vec<u8>,
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
            fb: vec![0; WIDTH * HEIGHT * 4],
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
    /// Where (x, y) lands on screen with `off` and `zoom` (before `rel`).
    pub fn project(&self, x: i32, y: i32) -> (f32, f32) {
        let z = |p: i32, o: i32| 64.0 + (p + o - 64) as f32 * self.zoom;
        (z(x, self.off.0), z(y, self.off.1))
    }

    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: [u8; 3]) {
        for yy in y.max(0)..(y + h).min(HEIGHT as i32) {
            for xx in x.max(0)..(x + w).min(WIDTH as i32) {
                let i = (yy as usize * WIDTH + xx as usize) * 4;
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
        for py in by0.max(0)..by1.min(HEIGHT as i32) {
            for px in bx0.max(0)..bx1.min(WIDTH as i32) {
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
                let i = (py as usize * WIDTH + px as usize) * 4;
                for (dst, &src) in self.fb[i..i + 3].iter_mut().zip(&c[..3]) {
                    *dst = ((src as u32 * a + *dst as u32 * (64 - a)) / 64) as u8;
                }
                self.fb[i + 3] = 255;
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
