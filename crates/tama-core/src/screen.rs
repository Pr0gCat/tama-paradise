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
}

impl Default for Screen {
    fn default() -> Self {
        Screen { sprites: HashMap::new(), fb: vec![0; WIDTH * HEIGHT * 4], wanted: BTreeSet::new(), off: (0, 0), zoom: 1.0 }
    }
}

impl Screen {
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
    pub fn blit(&mut self, id: u16, frame: u16, cx: i32, cy: i32, alpha: u8, flip: bool, palette: Option<(u16, usize)>) -> bool {
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
        let pal = palette.and_then(|(pid, set)| self.sprites.get(&pid).map(|p| (p, set)));
        let a = alpha.min(64) as u32;
        // Zoom scales positions and sizes about the screen centre (HOME zoom between layers).
        let k = self.zoom;
        let (dw, dh) = ((w as f32 * k).round() as i32, (h as f32 * k).round() as i32);
        let (x0, y0) = (64 + ((cx - 64) as f32 * k).round() as i32 - dw / 2, 64 + ((cy - 64) as f32 * k).round() as i32 - dh / 2);
        for py in y0.max(0)..(y0 + dh).min(HEIGHT as i32) {
            let y = (((py - y0) as f32 / k) as i32).min(h - 1);
            for px in x0.max(0)..(x0 + dw).min(WIDTH as i32) {
                let x = (((px - x0) as f32 / k) as i32).min(w - 1);
                let sx = if flip { w - 1 - x } else { x };
                let idx = s.pixels[f + (y * w + sx) as usize];
                if s.transparent && idx == s.key {
                    continue;
                }
                let c = match pal {
                    Some((p, set)) => p.rgba(set, idx),
                    None => s.rgba(0, idx),
                };
                let i = (py as usize * WIDTH + px as usize) * 4;
                for ch in 0..3 {
                    let d = self.fb[i + ch] as u32;
                    self.fb[i + ch] = ((c[ch] as u32 * a + d * (64 - a)) / 64) as u8;
                }
                self.fb[i + 3] = 255;
            }
        }
        true
    }
}

/// Glyph codes 0xF000.. are control codes; 0xF000 is a newline.
pub const NEWLINE: u16 = 0xF000;

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
}
