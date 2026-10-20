//! Traditional Chinese (zh-Hant), a language the remake adds: the zh strings converted glyph by
//! glyph, with the Traditional characters the firmware's font lacks drawn from Cubic 11.

use crate::sprite::Sprite;

include!("hant_table.rs");

/// The added glyphs: 16×16, 1 bit per pixel, 2 bytes a row with the left pixel in the top bit.
const BITMAPS: &[u8; GLYPHS * 32] = include_bytes!("hant.bin");

/// Simplified glyph codes to Traditional ones, longest rule first.
pub fn convert(s: &[u16]) -> Vec<u16> {
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        match RULES.iter().find(|(from, _)| s[i..].starts_with(from)) {
            Some((from, to)) => {
                out.extend_from_slice(to);
                i += from.len();
            }
            None => {
                out.push(s[i]);
                i += 1;
            }
        }
    }
    out
}

/// Append the added glyphs to font 2's sprite (frame `FIRST - 1` on), inked with palette index 2
/// like the firmware's glyphs.
pub fn extend_font(s: &mut Sprite) {
    if s.frames as usize + 1 != FIRST as usize || (s.w, s.h) != (16, 16) {
        return;
    }
    for row in BITMAPS.chunks(2) {
        let row = u16::from_be_bytes([row[0], row[1]]);
        s.pixels.extend((0..16).map(|x| if row & (0x8000 >> x) != 0 { 2 } else { s.key }));
    }
    s.frames += GLYPHS as u16;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_rules_beat_character_rules() {
        // A word whose first character converts differently on its own (e.g. 斗笠 stays 斗笠 though 斗 alone becomes 鬥).
        let single = |c: u16| RULES.iter().find(|r| r.0 == [c]).map(|r| r.1[0]);
        let (word, to) = RULES
            .iter()
            .find(|(w, t)| w.len() == 2 && single(w[0]).is_some_and(|c| c != t[0]))
            .unwrap();
        assert_eq!(convert(word), *to);
        assert_eq!(convert(&[1, 2]), [1, 2]);
        assert!(NAME.iter().any(|&c| c >= FIRST));
    }
}
