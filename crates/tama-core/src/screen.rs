//! 128×128 RGBA framebuffer plus the sprites the platform has supplied.

use crate::sprite::Sprite;
use std::collections::HashMap;

pub const WIDTH: usize = 128;
pub const HEIGHT: usize = 128;

pub struct Screen {
    /// Decoded sprites by sprite-pack index.
    pub sprites: HashMap<u16, Sprite>,
    /// RGBA8888, row-major.
    pub fb: Vec<u8>,
}

impl Default for Screen {
    fn default() -> Self {
        Screen { sprites: HashMap::new(), fb: vec![0; WIDTH * HEIGHT * 4] }
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
        let Some(s) = self.sprites.get(&id) else { return false };
        let (w, h) = (s.w as i32, s.h as i32);
        let f = (frame % s.frames.max(1)) as usize * (w * h) as usize;
        let pal = palette.and_then(|(pid, set)| self.sprites.get(&pid).map(|p| (p, set)));
        let a = alpha.min(64) as u32;
        for y in 0..h {
            for x in 0..w {
                let (px, py) = (cx - w / 2 + x, cy - h / 2 + y);
                if !(0..WIDTH as i32).contains(&px) || !(0..HEIGHT as i32).contains(&py) {
                    continue;
                }
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
            }
        }
        true
    }
}
