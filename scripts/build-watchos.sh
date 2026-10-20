#!/bin/sh
# Build crates/tama-ffi for watchOS and pack target/TamaCore.xcframework (static library +
# include/tama.h) for the watch app.
#
#   scripts/build-watchos.sh            device (arm64) + simulator (arm64)
#   scripts/build-watchos.sh --arm64_32 also Series 4-8 hardware (arm64_32)
#
# aarch64-apple-watchos and aarch64-apple-watchos-sim ship prebuilt std via rustup.
# arm64_32-apple-watchos is tier 3: no prebuilt std, so it needs a nightly toolchain with
# rust-src and -Zbuild-std. Its slice is lipo'd with the arm64 device slice into one library,
# since an XCFramework holds one library per platform variant.
set -eu
cd "$(dirname "$0")/.."

ARM64_32=0
[ "${1:-}" = "--arm64_32" ] && ARM64_32=1

rustup target add aarch64-apple-watchos aarch64-apple-watchos-sim
cargo build -p tama-ffi --release --target aarch64-apple-watchos
cargo build -p tama-ffi --release --target aarch64-apple-watchos-sim

LIB=libtama_ffi.a
DEVICE=target/aarch64-apple-watchos/release/$LIB
if [ "$ARM64_32" = 1 ]; then
    if ! rustup run nightly rustc --version >/dev/null 2>&1; then
        echo "--arm64_32 needs a nightly toolchain: rustup toolchain install nightly --component rust-src" >&2
        exit 1
    fi
    cargo +nightly build -p tama-ffi --release --target arm64_32-apple-watchos -Zbuild-std=std,panic_abort
    mkdir -p target/watchos-device
    lipo -create "$DEVICE" target/arm64_32-apple-watchos/release/$LIB -output target/watchos-device/$LIB
    DEVICE=target/watchos-device/$LIB
fi

OUT=target/TamaCore.xcframework
rm -rf "$OUT"
xcodebuild -create-xcframework \
    -library "$DEVICE" -headers crates/tama-ffi/include \
    -library target/aarch64-apple-watchos-sim/release/$LIB -headers crates/tama-ffi/include \
    -output "$OUT"
echo "$OUT"
