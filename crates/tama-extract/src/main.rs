//! Extract game data and sprites from a Tamagotchi Paradise flash dump.
//!
//! Usage: tama-extract <dump.bin> <data_dir>
//! Writes <data_dir>/game.json and <data_dir>/sprites/<index>.tsp.gz (format: tama_core::sprite).
use std::io::Write;
use tama_core::data::{CharDef, Data, Evolution, Font, Item, Melody, Model, Part, UiElem};
use tama_core::sprite::Sprite;

const ASSET_ARC: usize = 0x111000;
const XIP_FLASH: usize = 0x11000;
const XIP_BASE: usize = 0x1000_0000;
const VERSION_INFO: usize = 0xd29000;

fn u16_at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}

fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

fn xip(addr: usize) -> usize {
    addr - XIP_BASE + XIP_FLASH
}

/// Records of `n` u16 words from data-pack table `t` (ARC2 entry 0, 23 u16-unit offsets).
fn table(dump: &[u8], t: usize, n: usize) -> Vec<Vec<u16>> {
    let pack = arc_entry(dump, 0).0;
    let start = pack + u32_at(dump, pack + t * 4) as usize * 2;
    let end = pack + u32_at(dump, pack + (t + 1) * 4) as usize * 2;
    dump[start..end]
        .chunks_exact(n * 2)
        .map(|r| (0..n).map(|i| u16_at(r, i * 2)).collect())
        .collect()
}

/// Start of data-pack table `t` (u16-unit offsets at the pack start).
fn table_start(dump: &[u8], t: usize) -> usize {
    let pack = arc_entry(dump, 0).0;
    pack + u32_at(dump, pack + t * 4) as usize * 2
}

/// Keyframe `idx` (`FUN_10023af4` 0x10023af4): table 16 holds u32 u16-unit offsets into table 15.
/// Layout: u16 flags, x, y, then optional fields chosen by flag bits.
fn keyframe(dump: &[u8], idx: usize) -> Part {
    let at = table_start(dump, 15) + u32_at(dump, table_start(dump, 16) + idx * 4) as usize * 2;
    let flags = u16_at(dump, at);
    let mut k = [0u16; 11];
    k[1] = u16_at(dump, at + 2);
    k[2] = u16_at(dump, at + 4);
    k[3] = 0x1000;
    k[5] = 0x40;
    let mut p = at + 6;
    for (bit, slot) in [(2, 6), (3, 7), (4, 4), (5, 3), (6, 5), (7, 8), (8, 9), (9, 10), (10, 0)] {
        if flags >> bit & 1 != 0 {
            k[slot] = u16_at(dump, p);
            p += 2;
        }
    }
    Part { sprite: k[0], frame: k[6], x: k[1] as i16, y: k[2] as i16, kind: k[10], alpha: k[5] as u8, flip: k[9] & 1 != 0 }
}

/// Character models (table 18, 0xd8 bytes = 54 × {i16 first keyframe, i16 part count}).
fn models(dump: &[u8], chr_ids: impl Iterator<Item = u16>) -> Vec<Model> {
    let t18 = table_start(dump, 18);
    let pose = |chr: u16, anim: usize| -> Vec<Part> {
        let rec = t18 + chr as usize * 0xd8 + anim * 4;
        let (first, count) = (u16_at(dump, rec) as i16, u16_at(dump, rec + 2) as i16);
        (0..count.max(0)).map(|i| keyframe(dump, (first + i) as usize)).collect()
    };
    chr_ids
        .map(|chr| Model { chr_id: chr, anims: (0..54).map(|a| pose(chr, a)).collect() })
        .filter(|m| m.anims.iter().any(|a| !a.is_empty()))
        .collect()
}

/// Motion scripts: table 13 = u16 offsets in u32 units into table 14 (one extra end entry).
fn motions(dump: &[u8]) -> Vec<Vec<u16>> {
    let (t13, t14) = (table_start(dump, 13), table_start(dump, 14));
    let n = (t14 - t13) / 2 - 1;
    (0..n)
        .map(|i| {
            let (a, b) = (u16_at(dump, t13 + i * 2) as usize, u16_at(dump, t13 + i * 2 + 2) as usize);
            (0..(b - a) * 2).map(|w| u16_at(dump, t14 + a * 4 + w * 2)).collect()
        })
        .collect()
}

/// UI elements: table 4 = u16 offsets (u16 units) into table 5. Record: u16 header (kind in the
/// top 4 bits, optional-field bits below), i16 x, i16 y, then the present optional fields.
fn ui_elems(dump: &[u8]) -> Vec<UiElem> {
    const SPEC: [&[(u16, usize, i16)]; 9] = [
        &[(0x800, 0, 0)],
        &[(0x1, 3, 0), (0x8, 4, 0), (0x10, 5, 0x1000), (0x20, 6, 0x40), (0x40, 7, 0), (0x80, 8, 0), (0x100, 9, 0), (0x400, 10, 0), (0x800, 0, 0)],
        &[(0x1, 3, 0), (0x8, 4, 0), (0x10, 5, 0), (0x40, 6, 0x40), (0x80, 7, 0), (0x100, 8, 0), (0x400, 9, 0), (0x800, 0, 0)],
        &[(0x8, 3, 1), (0x10, 4, 1), (0x20, 5, 0), (0x40, 6, 0), (0x100, 7, 0), (0x400, 8, 0), (0x800, 0, 0)],
        &[(0x1, 3, 0), (0x8, 4, 0), (0x10, 5, 0), (0x20, 6, 0), (0x100, 7, 0), (0x200, 8, 0), (0x800, 0, 0)],
        &[(0x1, 3, 0), (0x100, 4, 0), (0x200, 5, 0), (0x800, 0, 0)],
        &[(0x1, 3, 0)],
        &[(0x1, 8, 0), (0x8, 3, 0), (0x10, 4, 0), (0x20, 5, 0), (0x40, 7, 0), (0x80, 6, 0), (0x800, 0, 0)],
        &[(0x1, 0, 0), (0x8, 4, 0), (0x10, 5, 0), (0x40, 6, 0x40), (0x80, 7, 0), (0x100, 8, 0), (0x400, 9, 0), (0x800, 3, 0)],
    ];
    let (t4, t5) = (table_start(dump, 4), table_start(dump, 5));
    (0..(t5 - t4) / 2)
        .map(|i| {
            let a = t5 + u16_at(dump, t4 + i * 2) as usize * 2;
            let h = u16_at(dump, a);
            let kind = (h >> 12) as usize;
            let mut p = [0i16; 11];
            let mut q = a + 6;
            for &(bit, slot, default) in SPEC.get(kind).copied().unwrap_or(&[]) {
                p[slot] = if h & bit != 0 {
                    q += 2;
                    u16_at(dump, q - 2) as i16
                } else {
                    default
                };
            }
            UiElem { kind: kind as u8, x: u16_at(dump, a + 2) as i16, y: u16_at(dump, a + 4) as i16, p }
        })
        .collect()
}

/// The 5 fonts: table 0 = u16 offsets (u16 units) into table 1.
/// Record: u16 sprite, u16 advance, u16 line height, i16 glyph count − 1, then per glyph c
/// at +4 + c*4: {u16 char code, i16 advance adjust}.
fn fonts(dump: &[u8]) -> Vec<Font> {
    let (t0, t1) = (table_start(dump, 0), table_start(dump, 1));
    (0..5)
        .map(|i| {
            let f = t1 + u16_at(dump, t0 + i * 2) as usize * 2;
            let count = u16_at(dump, f + 6) as i16 as usize + 1;
            Font {
                sprite: u16_at(dump, f),
                advance: u16_at(dump, f + 2) as i16,
                line_height: u16_at(dump, f + 4) as i16,
                adj: (0..count).map(|c| if c == 0 { 0 } else { u16_at(dump, f + 6 + c * 4) as i16 }).collect(),
            }
        })
        .collect()
}

/// String tables: ARC2 entry 2 holds 9 files (languages); each = u16 count, u16 offsets[count+1]
/// (u16 units from the file start), strings of glyph codes.
fn strings(dump: &[u8]) -> Vec<Vec<Vec<u16>>> {
    let arc = arc_entry(dump, 2).0;
    let n = u32_at(dump, arc + 12) as usize;
    (0..n)
        .map(|i| {
            let file = arc + u32_at(dump, arc + 0x10 + i * 16 + 4) as usize;
            let w = |k: usize| u16_at(dump, file + k * 2) as usize;
            (0..w(0)).map(|j| (w(j + 1)..w(j + 2)).map(|k| w(k) as u16).take_while(|&c| c != 0).collect()).collect()
        })
        .collect()
}

/// ARC2 entry `i` of the root asset archive: (start, len) in the dump.
fn arc_entry(dump: &[u8], i: usize) -> (usize, usize) {
    assert_eq!(&dump[ASSET_ARC..ASSET_ARC + 4], b"ARC2", "asset archive not found");
    let e = ASSET_ARC + 0x10 + i * 16;
    (ASSET_ARC + u32_at(dump, e + 4) as usize, u32_at(dump, e + 12) as usize)
}

/// Bytewise RLE (GMMan sprites.md): 0 ends; bit7 set = that many literals; else repeat next byte.
fn rle(src: &[u8]) -> Vec<u8> {
    let (mut out, mut i) = (Vec::new(), 0);
    while i < src.len() {
        let c = src[i] as usize;
        i += 1;
        if c == 0 {
            break;
        }
        if c & 0x80 != 0 {
            let n = (c & 0x7f).min(src.len() - i);
            out.extend_from_slice(&src[i..i + n]);
            i += n;
        } else if i < src.len() {
            out.extend(std::iter::repeat(src[i]).take(c));
            i += 1;
        }
    }
    out
}

/// Decode one original sprite record (24-byte header, see docs/spec/04-assets.md §2.3).
/// Returns None for formats this dump never uses (1/16/24 bpp, wordwise RLE).
fn decode_sprite(rec: &[u8]) -> Option<Sprite> {
    let flags = rec[4];
    let bits = match rec[5] {
        1 => 2,
        2 => 4,
        3 => 8,
        _ => return None,
    };
    if flags & 0x40 != 0 {
        return None;
    }
    let n = u16_at(rec, 6) as usize;
    let (sw, sh, iw, ih) = (rec[8] as usize, rec[9] as usize, rec[12] as usize, rec[13] as usize);
    let (sets, poff, doff) = (rec[15] as usize, u16_at(rec, 18) as usize, u16_at(rec, 20) as usize);
    let colors = 1usize << bits;
    let palettes = (0..sets).map(|s| (0..colors).map(|i| u16_at(rec, poff + (s * colors + i) * 2)).collect()).collect();

    let per = sw * sh * bits / 8;
    let cell = |k: usize| -> Vec<u8> {
        let mut d = if flags & 0x20 != 0 {
            let o = u32_at(rec, doff + 8 * k) as usize;
            let len = u32_at(rec, doff + 8 * k + 4) as usize;
            let start = doff + (o & 0x7fff_ffff);
            let raw: Vec<u8> = rec[start..start + len].to_vec();
            let raw = if flags & 0x80 != 0 { raw.iter().map(|b| b ^ 0x53).collect() } else { raw };
            if o & 0x8000_0000 != 0 { raw } else { rle(&raw) }
        } else {
            let raw = rec[doff + k * per..doff + (k + 1) * per].to_vec();
            if flags & 0x80 != 0 { raw.iter().map(|b| b ^ 0x53).collect() } else { raw }
        };
        d.resize(per.max(d.len()), 0);
        d
    };

    let (w, h) = (sw * iw, sh * ih);
    let frames = n / (iw * ih);
    let mut pixels = vec![0u8; frames * w * h];
    for k in 0..frames * iw * ih {
        let d = cell(k);
        let (f, c) = (k / (iw * ih), k % (iw * ih));
        let (cy, cx) = (c / iw, c % iw);
        for y in 0..sh {
            for x in 0..sw {
                let bi = (y * sw + x) * bits;
                let idx = (d[bi >> 3] >> (bi & 7)) & ((1 << bits) - 1) as u8;
                pixels[f * w * h + (cy * sh + y) * w + cx * sw + x] = idx;
            }
        }
    }
    Some(Sprite {
        w: w as u16,
        h: h as u16,
        frames: frames as u16,
        bits: bits as u8,
        transparent: flags & 4 != 0,
        key: u16_at(rec, 16) as u8,
        palettes,
        pixels,
    })
}

fn write_sprites(dump: &[u8], dir: &std::path::Path) -> (usize, usize) {
    std::fs::create_dir_all(dir).expect("create sprites dir");
    let (pack, len) = arc_entry(dump, 1);
    let pack = &dump[pack..pack + len];
    let count = u32_at(pack, 0) as usize / 4;
    let mut skipped = 0;
    for i in 0..count {
        let rec = &pack[u32_at(pack, i * 4) as usize..];
        let Some(s) = decode_sprite(rec) else {
            skipped += 1;
            continue;
        };
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
        gz.write_all(&s.to_bytes()).unwrap();
        std::fs::write(dir.join(format!("{i}.tsp.gz")), gz.finish().unwrap()).expect("write sprite");
    }
    (count - skipped, skipped)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let [_, dump_path, out_dir] = &args[..] else {
        eprintln!("usage: tama-extract <dump.bin> <data_dir>");
        std::process::exit(2);
    };
    let dump = std::fs::read(dump_path).expect("read dump");
    let out_dir = std::path::Path::new(out_dir);

    let characters: Vec<CharDef> = table(&dump, 11, 15)
        .into_iter()
        .filter(|r| r[0] != 0xffff)
        .map(|r| CharDef {
            id: r[0],
            chr_id: r[1],
            name_id: r[2],
            stage: r[3] as u8,
            species: r[4],
            rank: r[5] as u8,
            likes: r[6] as u8,
            attribute: r[7] as u8,
            gender: r[8] as u8,
            hungry_min: r[9],
            happy_min: r[10],
            // r[11] "Evolution" is never read by the firmware.
            disease: r[12] as u8,
            dislikes: r[13] as u8,
            color: r[14] as u8,
        })
        .collect();

    let items: Vec<Item> = table(&dump, 10, 23)
        .into_iter()
        .map(|r| Item {
            id: r[0],
            name: r[1],
            eat_sprite: r[2],
            icon_sprite: r[3],
            drink: r[10] & 1 != 0,
            kind: r[11] as u8,
            buy: r[12],
            sell: r[13],
            category: r[14] as u8,
            rank: r[16] as u8,
            group: r[17] as u8,
            snack_flag: r[18] as u8,
            onaka: r[19] as u8,
            gokigen: r[20] as u8,
            day_group: r[21] as u8,
            flags: r[22],
        })
        .collect();

    let words = |addr: usize, n: usize| -> Vec<u16> { (0..n).map(|i| u16_at(&dump, xip(addr) + i * 2)).collect() };
    let teen = words(0x1009_1314, 12);
    let adult = words(0x1009_132c, 48);
    let evolution = Evolution {
        // Code constants in tama_evolve 0x1000fc98 (MOVW immediates, not a table).
        kids: [2091, 2113, 2069],
        rare: [4112, 4134, 4090],
        failed: 4017,
        teens: std::array::from_fn(|f| std::array::from_fn(|b| teen[f * 4 + b])),
        adults: std::array::from_fn(|f| std::array::from_fn(|b| std::array::from_fn(|q| adult[f * 16 + b * 4 + q]))),
    };

    let mut chr_ids: Vec<u16> = characters.iter().map(|c| c.chr_id).collect();
    chr_ids.sort_unstable();
    chr_ids.dedup();
    let models = models(&dump, chr_ids.into_iter());
    let home = words(0x1009_dbd8, 96);
    let home_motions = home.chunks(3).map(|c| [c[0], c[1], c[2]]).collect();
    let data = Data {
        rom_type: u32_at(&dump, VERSION_INFO) as u8,
        characters,
        items,
        evolution,
        models,
        motions: motions(&dump),
        home_motions,
        fonts: fonts(&dump),
        strings: strings(&dump),
        ui: ui_elems(&dump),
        melodies: {
            // 87 pointers; one step = (divider + 1) ms, dividers at 0x10091b00 as {id, div} × 5.
            let tempo = words(0x1009_1b00, 10);
            (0..0x57u16)
                .map(|i| {
                    let div = tempo.chunks(2).find(|c| c[0] == i).map_or(8, |c| c[1]);
                    let mut at = xip(u32_at(&dump, xip(0x100a_0a28) + i as usize * 4) as usize);
                    let mut notes = Vec::new();
                    while u16_at(&dump, at) != 0 {
                        notes.push([u16_at(&dump, at), u16_at(&dump, at + 2), u16_at(&dump, at + 4)]);
                        at += 6;
                    }
                    Melody { ms_per_step: div + 1, notes }
                })
                .collect()
        },
        // 109 × {u16 item, u16 pad, u32 char* code}. Six more codes live in encrypted PRAM (not read).
        shop_codes: (0..109)
            .map(|i| {
                let e = xip(0x100a_1a20) + i * 8;
                let at = xip(u32_at(&dump, e + 4) as usize);
                let code: String = dump[at..at + 8].iter().map(|&b| b as char).collect();
                (u16_at(&dump, e), code)
            })
            .collect(),
        recipes: words(0x1009_1fbc, 24).chunks(2).map(|c| (c[0], c[1])).collect(),
        weather_months: words(0x1009_1b5c, 72).chunks(6).map(|c| std::array::from_fn(|i| c[i])).collect(),
        mini: tama_core::data::MiniData {
            // Game 00 board pointers live in RW data (packed in encrypted PRAM at 0xe6a0);
            // the boards themselves are in XIP. Rows per level at 0x1009de92 (31, 26, 21);
            // the game reads 24, so short boards are padded with empty rows.
            lab00_boards: [0x100a_013e, 0x100a_02b2, 0x100a_0426, 0x100a_0c78, 0x100a_0db0, 0x100a_0ee8, 0x100a_0696, 0x100a_0792, 0x100a_088e]
                .iter()
                .enumerate()
                .map(|(i, &a)| {
                    let rows = words(0x1009_de92, 3)[i / 3] as usize;
                    let w = words(a, rows * 6);
                    let mut b: Vec<[i16; 6]> = w.chunks(6).map(|r| std::array::from_fn(|k| r[k] as i16)).collect();
                    b.resize(b.len().max(24), [0; 6]);
                    b
                })
                .collect(),
            // Game 01 maps (`FUN_10020a30`): 10 × {u16 segment, u16 style}. Segment table
            // 0x1009b168 = 32 × {u32 n, u32 ptr → {i16 x, i16 y}[n]}, style table 0x10093574 =
            // 22 × {u32 n, u32 ptr → u16 type[n]}; platform j of map entry i is at height
            // y + 640 i with type style[j % n] (`FUN_10009d98`), then the goal at (64, 6400).
            lab01_maps: [0x1009_1fec, 0x1009_2160, 0x1009_22d4, 0x1009_2448, 0x1009_25bc, 0x1009_2730, 0x1009_28a4, 0x1009_2a3c, 0x1009_2bd4]
                .iter()
                .map(|&m| {
                    let list = |table: usize, i: u16| {
                        let e = xip(table) + i as usize * 8;
                        (u32_at(&dump, e) as usize, u32_at(&dump, e + 4) as usize)
                    };
                    let mut p: Vec<[i16; 3]> = Vec::new();
                    for (i, e) in words(m, 20).chunks(2).enumerate() {
                        let ((n, seg), (sn, sty)) = (list(0x1009_b168, e[0]), list(0x1009_3574, e[1]));
                        let (seg, sty) = (words(seg, n * 2), words(sty, sn));
                        p.extend((0..n).map(|j| [seg[2 * j] as i16, seg[2 * j + 1] as i16 + 640 * i as i16, sty[j % sn] as i16]));
                    }
                    p.push([64, 6400, 0]);
                    p
                })
                .collect(),
            // Game 02 boards: 15 × 15 × u32 card type at 0x1009436c.
            lab02_boards: (0..15).map(|b| std::array::from_fn(|i| u32_at(&dump, xip(0x1009_436c) + b * 60 + i * 4) as u8)).collect(),
        },
        hunt_finds: words(0x100a_0600, 75).chunks(3).map(|c| [c[0], c[1], c[2]]).collect(),
        weather_patterns: (0..6).map(|p| std::array::from_fn(|i| dump[xip(0x1009_138c) + p * 5 + i])).collect(),
        field_bg: {
            let bg = words(0x1009_2e06, 96);
            (0..3).map(|f| std::array::from_fn(|v| std::array::from_fn(|k| bg[f * 32 + v * 16 + k]))).collect()
        },
    };
    for id in data.evolution.kids.iter().chain(data.evolution.teens.iter().flatten()) {
        assert!(data.character(*id).is_some(), "evolution target {id} missing");
    }
    std::fs::write(out_dir.join("game.json"), serde_json::to_string(&data).unwrap()).expect("write");
    println!(
        "{} characters, {} models, {} motions, {} items, rom_type {}",
        data.characters.len(),
        data.models.len(),
        data.motions.len(),
        data.items.len(),
        data.rom_type
    );
    let (ok, skipped) = write_sprites(&dump, &out_dir.join("sprites"));
    println!("{ok} sprites written, {skipped} skipped (unsupported format)");
}
