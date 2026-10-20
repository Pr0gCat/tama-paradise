# Tama Paradise (web remake)

A fan-made remake of Tamagotchi Paradise in Rust, compiled to WebAssembly and played in the browser
(also as a home-screen app on phones).

**Play:** https://pr0gcat.github.io/tama-paradise/

This repository and the site contain **no game data**. The page asks for a flash dump of your own
Tamagotchi Paradise and reads the graphics, texts and tables from it in your browser; the dump stays
in that browser (IndexedDB) and is never uploaded.

Not affiliated with or endorsed by Bandai. Tamagotchi is a trademark of Bandai.

## Layout

- `crates/tama-core` – the game (care, evolution, menus, mini-games, rendering to a 128×128 screen)
- `crates/tama-extract` – reads game data and sprites from a flash dump (library and CLI)
- `crates/tama-web` – wasm bindings
- `web/` – the page, service worker and icons
- `docs/` – the behaviour spec the remake follows

## Local build

```sh
wasm-pack build crates/tama-web --target web --out-dir ../../web/pkg --release
python3 -m http.server 8080 -d web
```

Optionally extract the data once (`cargo run --release -p tama-extract -- <dump.bin> data`) and link
`web/game.json` → `../data/game.json` and `web/sprites` → `../data/sprites`; the page then uses
them instead of asking for a dump (`?dump` forces the dump path, `?debug` shows the pet's numbers).

## Apple Watch

`watch/TamaWatch.xcodeproj` is a watch-only SwiftUI app over the C ABI in `crates/tama-ffi`.

```sh
scripts/build-watchos.sh   # target/TamaCore.xcframework
open watch/TamaWatch.xcodeproj
```

A build phase copies your local `data/game.json` and `data/sprites` into that build only (they
are never committed). Controls: swipe up / down or the Digital Crown turn the dial, tap selects,
swipe left goes back, a long press is the dial's long press.

## Credits

Traditional Chinese glyphs missing from the original font are drawn from
[Cubic 11](https://github.com/ACh-K/Cubic-11) (licence in `re/hant/LICENSE-Cubic11.txt`).
QR scanning uses [jsQR](https://github.com/cozmo/jsQR) (`web/vendor/jsQR.LICENSE`).

© the author, all rights reserved.
