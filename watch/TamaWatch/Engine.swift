import Foundation
import CoreGraphics
import Compression

/// Runs tama-core (through tama-ffi) at 20 fps and turns its framebuffer into an image.
/// Game data comes from the app bundle, copied there at build time from the local, unpublished
/// data/ folder (see the "Copy game data" build phase); it is never committed.
@MainActor
final class Engine: ObservableObject {
    @Published private(set) var image: CGImage?
    @Published private(set) var error: String?

    private var game: OpaquePointer?
    private var timer: Timer?
    private var gesture: UInt8 = UInt8(TAMA_GESTURE_NONE)
    var holding = false
    private let sound = Sound()
    nonisolated private static let saveKey = "tama-save"

    init() {
        // Parsing the data takes a while on a watch: off the main thread, so launch is quick.
        Task.detached(priority: .userInitiated) {
            let loaded = Self.load()
            await MainActor.run { self.start(loaded) }
        }
    }

    private struct Loaded: @unchecked Sendable {
        var game: OpaquePointer?
        var json: String?
        var error: String?
    }

    nonisolated private static func load() -> Loaded {
        let t0 = Date()
        guard let url = Bundle.main.url(forResource: "game", withExtension: "json"),
              let json = try? String(contentsOf: url, encoding: .utf8) else {
            return Loaded(error: "game.json missing: build with the local data/ folder")
        }
        var err: UnsafeMutablePointer<CChar>?
        var game: OpaquePointer?
        if let saved = UserDefaults.standard.string(forKey: saveKey) {
            game = tama_load(json, saved, &err)
            if let e = err { tama_string_free(e); err = nil }
        }
        if game == nil {
            game = tama_new(json, UInt32.random(in: 1...UInt32.max), &err)
        }
        var error: String?
        if let e = err {
            error = String(cString: e)
            tama_string_free(e)
        }
        print("tama: loaded in \(Date().timeIntervalSince(t0)) s")
        return Loaded(game: game, json: json, error: error)
    }

    private func start(_ loaded: Loaded) {
        error = loaded.error
        game = loaded.game
        if let json = loaded.json {
            let sound = self.sound
            Task.detached(priority: .utility) { sound.load(json: json) }
        }
        resume()
    }

    /// Local wall-clock seconds, as the game expects.
    static func now() -> UInt64 {
        UInt64(Date().timeIntervalSince1970 + Double(TimeZone.current.secondsFromGMT()))
    }

    /// One gesture, used by the next frame.
    func send(_ g: Int32) { gesture = UInt8(g) }

    func resume() {
        guard let game, timer == nil else { return }
        tama_advance_to(game, Self.now())
        timer = Timer.scheduledTimer(withTimeInterval: Double(TAMA_TICK_MS) / 1000, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated { self?.tick() }
        }
    }

    func pause() {
        timer?.invalidate()
        timer = nil
        save()
    }

    func save() {
        guard let game, let s = tama_save(game) else { return }
        UserDefaults.standard.set(String(cString: s), forKey: Self.saveKey)
        tama_string_free(s)
    }

    private func tick() {
        guard let game else { return }
        tama_frame_touch(game, Self.now(), gesture, holding)
        gesture = UInt8(TAMA_GESTURE_NONE)
        supplySprites(game)
        playSounds(game)
        drain(tama_take_connect(game))
        image = framebufferImage(game)
    }

    private func supplySprites(_ game: OpaquePointer) {
        var n = 0
        guard let ids = tama_missing_sprites(game, &n) else { return }
        defer { tama_u16_free(ids, n) }
        for id in UnsafeBufferPointer(start: ids, count: n) {
            guard let url = Bundle.main.url(forResource: "\(id).tsp", withExtension: "gz", subdirectory: "sprites"),
                  let gz = try? Data(contentsOf: url), let raw = gunzip(gz) else { continue }
            raw.withUnsafeBytes { b in
                _ = tama_add_sprite(game, id, b.bindMemory(to: UInt8.self).baseAddress, raw.count)
            }
        }
    }

    private func playSounds(_ game: OpaquePointer) {
        var n = 0
        guard let s = tama_take_sounds(game, &n) else { return }
        defer { tama_u32_free(s, n) }
        if let last = UnsafeBufferPointer(start: s, count: n).last {
            sound.play(Int(last >> 8), volume: Int(last & 0xff))
        }
    }

    private func drain(_ s: UnsafeMutablePointer<CChar>?) {
        // ponytail: CONNECTION (playdates, gifts) and save export are not on the watch yet.
        if let s { tama_string_free(s) }
    }

    private func framebufferImage(_ game: OpaquePointer) -> CGImage? {
        var n = 0
        guard let p = tama_framebuffer(game, &n) else { return nil }
        let data = Data(bytes: p, count: n) as CFData
        guard let provider = CGDataProvider(data: data) else { return nil }
        return CGImage(width: Int(TAMA_WIDTH), height: Int(TAMA_HEIGHT), bitsPerComponent: 8, bitsPerPixel: 32,
                       bytesPerRow: Int(TAMA_WIDTH) * 4, space: CGColorSpaceCreateDeviceRGB(),
                       bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.noneSkipLast.rawValue),
                       provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent)
    }
}

/// gzip → raw bytes (tama-extract writes a plain 10-byte header, then a raw deflate stream).
func gunzip(_ gz: Data) -> Data? {
    guard gz.count > 18, gz[0] == 0x1f, gz[1] == 0x8b else { return nil }
    var start = 10
    let flags = gz[3]
    if flags & 4 != 0 { start += 2 + Int(gz[10]) | Int(gz[11]) << 8 }
    if flags & 8 != 0 { while start < gz.count, gz[start] != 0 { start += 1 }; start += 1 }
    if flags & 16 != 0 { while start < gz.count, gz[start] != 0 { start += 1 }; start += 1 }
    if flags & 2 != 0 { start += 2 }
    let size = gz.suffix(4).withUnsafeBytes { Int($0.loadUnaligned(as: UInt32.self).littleEndian) }
    let body = gz.subdata(in: start..<(gz.count - 8))
    var out = Data(count: size)
    let written = out.withUnsafeMutableBytes { dst in
        body.withUnsafeBytes { src in
            compression_decode_buffer(dst.bindMemory(to: UInt8.self).baseAddress!, size,
                                      src.bindMemory(to: UInt8.self).baseAddress!, body.count, nil, COMPRESSION_ZLIB)
        }
    }
    return written == size ? out : nil
}
