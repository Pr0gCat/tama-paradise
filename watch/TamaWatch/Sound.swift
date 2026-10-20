import AVFoundation

/// The device's melodies (game.json `melodies`: steps × ms_per_step, period µs, duty) as a 10%
/// pulse wave, like the web build's WebAudio voice.
final class Sound: @unchecked Sendable {
    private struct Melody: Decodable { let ms_per_step: Int; let notes: [[Int]] }
    private struct File: Decodable { let melodies: [Melody] }
    private var melodies: [Melody] = []
    private let engine = AVAudioEngine()
    private var node: AVAudioSourceNode?
    private let lock = NSLock()
    private var notes: [(freq: Double, frames: Int, amp: Float)] = []
    private var phase = 0.0
    private var left = 0

    func load(json: String) {
        let m = (try? JSONDecoder().decode(File.self, from: Data(json.utf8)))?.melodies ?? []
        lock.lock()
        melodies = m
        lock.unlock()
    }

    func play(_ id: Int, volume: Int) {
        lock.lock()
        let all = melodies
        lock.unlock()
        guard id < all.count else { return }
        start()
        let rate = engine.outputNode.outputFormat(forBus: 0).sampleRate
        let m = all[id]
        let seq = m.notes.compactMap { n -> (Double, Int, Float)? in
            guard n.count == 3 else { return nil }
            let frames = Int(Double(n[0] * m.ms_per_step) / 1000 * rate)
            let on = n[1] > 0 && n[2] > 0
            return (on ? 1e6 / Double(n[1]) : 0, frames, on ? 0.25 * Float(volume) / 100 : 0)
        }
        lock.lock()
        notes = seq
        left = 0
        lock.unlock()
    }

    private func start() {
        guard node == nil else { return }
        try? AVAudioSession.sharedInstance().setCategory(.playback, mode: .default)
        try? AVAudioSession.sharedInstance().setActive(true)
        let format = engine.outputNode.inputFormat(forBus: 0)
        let rate = format.sampleRate
        let source = AVAudioSourceNode { [unowned self] _, _, count, list in
            let buffers = UnsafeMutableAudioBufferListPointer(list)
            self.lock.lock()
            defer { self.lock.unlock() }
            for i in 0..<Int(count) {
                while self.left == 0, !self.notes.isEmpty {
                    self.left = self.notes[0].frames
                    if self.left == 0 { self.notes.removeFirst() }
                }
                var v: Float = 0
                if let n = self.notes.first {
                    self.phase = (self.phase + n.freq / rate).truncatingRemainder(dividingBy: 1)
                    v = self.phase < 0.1 ? n.amp : -n.amp * 0.11
                    self.left -= 1
                    if self.left == 0 { self.notes.removeFirst() }
                }
                for b in buffers { b.mData?.assumingMemoryBound(to: Float.self)[i] = v }
            }
            return noErr
        }
        node = source
        engine.attach(source)
        engine.connect(source, to: engine.mainMixerNode, format: AVAudioFormat(standardFormatWithSampleRate: rate, channels: 1))
        try? engine.start()
    }
}
