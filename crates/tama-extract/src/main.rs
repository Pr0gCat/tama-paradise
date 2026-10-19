//! Extract game data from a Tamagotchi Paradise flash dump into data/game.json.
//!
//! Usage: tama-extract <dump.bin> <out.json>
use tama_core::data::{CharDef, Data, Evolution, Item};

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
    assert_eq!(&dump[ASSET_ARC..ASSET_ARC + 4], b"ARC2", "asset archive not found");
    let pack = ASSET_ARC + u32_at(dump, ASSET_ARC + 0x10 + 4) as usize;
    let start = pack + u32_at(dump, pack + t * 4) as usize * 2;
    let end = pack + u32_at(dump, pack + (t + 1) * 4) as usize * 2;
    dump[start..end]
        .chunks_exact(n * 2)
        .map(|r| (0..n).map(|i| u16_at(r, i * 2)).collect())
        .collect()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let [_, dump_path, out_path] = &args[..] else {
        eprintln!("usage: tama-extract <dump.bin> <out.json>");
        std::process::exit(2);
    };
    let dump = std::fs::read(dump_path).expect("read dump");

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

    let data = Data { rom_type: u32_at(&dump, VERSION_INFO) as u8, characters, items, evolution };
    for id in data.evolution.kids.iter().chain(data.evolution.teens.iter().flatten()) {
        assert!(data.character(*id).is_some(), "evolution target {id} missing");
    }
    std::fs::write(out_path, serde_json::to_string_pretty(&data).unwrap()).expect("write");
    println!("{} characters, {} items, rom_type {}", data.characters.len(), data.items.len(), data.rom_type);
}
