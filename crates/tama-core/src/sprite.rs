//! Sprite format shared by tama-extract (writer) and the renderer (reader).
//!
//! "TSP1" little-endian: u16 width, u16 height, u16 frames, u8 palette sets, u8 bits per index (colours = 1 << bits),
//! u8 flags (bit0 = transparency on), u8 transparent index, then sets × colours × u16 RGB565,
//! then frames × width × height u8 palette indices. Files are gzipped on disk.

pub const MAGIC: &[u8; 4] = b"TSP1";
pub const TRANSPARENT: u8 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sprite {
    pub w: u16,
    pub h: u16,
    pub frames: u16,
    /// Bits per palette index: 2, 4 or 8.
    pub bits: u8,
    pub transparent: bool,
    /// Palette index drawn as transparent when `transparent` is set (usually 0).
    pub key: u8,
    /// `palettes[set][index]`, RGB565.
    pub palettes: Vec<Vec<u16>>,
    /// frames × h × w indices.
    pub pixels: Vec<u8>,
}

impl Sprite {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = MAGIC.to_vec();
        for v in [self.w, self.h, self.frames] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(&[self.palettes.len() as u8, self.bits, self.transparent as u8 * TRANSPARENT, self.key]);
        for c in self.palettes.iter().flatten() {
            b.extend_from_slice(&c.to_le_bytes());
        }
        b.extend_from_slice(&self.pixels);
        b
    }

    pub fn from_bytes(b: &[u8]) -> Option<Sprite> {
        if b.get(..4)? != MAGIC {
            return None;
        }
        let u16_at = |o: usize| Some(u16::from_le_bytes([*b.get(o)?, *b.get(o + 1)?]));
        let (w, h, frames) = (u16_at(4)?, u16_at(6)?, u16_at(8)?);
        let (sets, bits, flags, key) = (*b.get(10)? as usize, *b.get(11)?, *b.get(12)?, *b.get(13)?);
        let colors = 1usize << bits;
        let mut o = 14;
        let mut palettes = Vec::with_capacity(sets);
        for _ in 0..sets {
            palettes.push((0..colors).map(|i| u16_at(o + i * 2)).collect::<Option<Vec<_>>>()?);
            o += colors * 2;
        }
        let n = frames as usize * w as usize * h as usize;
        let pixels = b.get(o..o + n)?.to_vec();
        Some(Sprite { w, h, frames, bits, transparent: flags & TRANSPARENT != 0, key, palettes, pixels })
    }

    /// RGBA of index `i` in palette `set` (falls back to set 0).
    pub fn rgba(&self, set: usize, i: u8) -> [u8; 4] {
        if self.transparent && i == self.key {
            return [0; 4];
        }
        let c = self.palettes.get(set).unwrap_or(&self.palettes[0]).get(i as usize).copied().unwrap_or(0);
        let (r, g, b) = (c >> 11, (c >> 5) & 63, c & 31);
        [(r * 255 / 31) as u8, (g * 255 / 63) as u8, (b * 255 / 31) as u8, 255]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let s = Sprite {
            w: 2,
            h: 1,
            frames: 2,
            bits: 2,
            transparent: true,
            key: 0,
            palettes: vec![vec![0x07e0, 0, 0xffff, 0xf800]],
            pixels: vec![0, 1, 2, 3],
        };
        assert_eq!(Sprite::from_bytes(&s.to_bytes()), Some(s.clone()));
        assert_eq!(s.rgba(0, 0), [0; 4]);
        assert_eq!(s.rgba(0, 3), [255, 0, 0, 255]);
    }
}
