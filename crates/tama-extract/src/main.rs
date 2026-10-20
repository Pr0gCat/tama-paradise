//! Extract game data and sprites from a Tamagotchi Paradise flash dump.
//!
//! Usage: tama-extract <dump.bin> <data_dir>
//! Writes <data_dir>/game.json and <data_dir>/sprites/<index>.tsp.gz (format: tama_core::sprite).
use std::io::Write;

fn write_sprites(dump: &[u8], dir: &std::path::Path) -> (usize, usize) {
    std::fs::create_dir_all(dir).expect("create sprites dir");
    let count = tama_extract::sprite_count(dump);
    let mut skipped = 0;
    for i in 0..count {
        let Some(s) = tama_extract::sprite(dump, i) else {
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

    let data = tama_extract::extract(&dump);
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
