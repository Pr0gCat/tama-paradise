// One-shot asset extractor. Filled in at M2, once a flash dump exists.
fn main() {
    let path = std::env::args().nth(1).expect("usage: tama-extract <dump.bin>");
    let dump = std::fs::read(&path).expect("read dump");
    println!("{path}: {} bytes", dump.len());
}
