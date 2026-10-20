import SwiftUI

/// The 128×128 screen with the watch's controls: swipe up / down turn the dial (the Digital
/// Crown does too), a tap selects, swipe left goes back, a long press is the dial's long press.
struct GameView: View {
    @ObservedObject var engine: Engine
    @State private var crown = 0.0
    @State private var crownStep = 0.0
    @State private var downAt: Date?
    @State private var holdTask: Task<Void, Never>?

    var body: some View {
        GeometryReader { geo in
            ZStack {
                Color.black
                if let image = engine.image {
                    Image(decorative: image, scale: 1)
                        .interpolation(.none)
                        .resizable()
                        .aspectRatio(1, contentMode: .fit)
                        .frame(width: geo.size.width, height: geo.size.height)
                } else if let error = engine.error {
                    Text(error).font(.footnote).multilineTextAlignment(.center).padding()
                }
            }
            .contentShape(Rectangle())
            .gesture(touch)
        }
        .ignoresSafeArea()
        .focusable()
        .digitalCrownRotation($crown, from: -1000, through: 1000, sensitivity: .medium,
                              isContinuous: true, isHapticFeedbackEnabled: true)
        .onChange(of: crown) { _, new in
            // One dial step per unit turned (the range wraps, so follow the shortest way round).
            var d = new - crownStep
            if d > 1000 { d -= 2000 } else if d < -1000 { d += 2000 }
            guard abs(d) >= 1 else { return }
            crownStep = new
            engine.send(d > 0 ? TAMA_GESTURE_SWIPE_UP : TAMA_GESTURE_SWIPE_DOWN)
        }
    }

    private var touch: some Gesture {
        DragGesture(minimumDistance: 0)
            .onChanged { _ in
                guard downAt == nil else { return }
                downAt = Date()
                holdTask = Task { @MainActor in
                    try? await Task.sleep(for: .milliseconds(400))
                    if !Task.isCancelled { engine.holding = true }
                }
            }
            .onEnded { v in
                holdTask?.cancel()
                defer { downAt = nil; engine.holding = false }
                if engine.holding { return }
                let (dx, dy) = (v.translation.width, v.translation.height)
                if max(abs(dx), abs(dy)) < 12 {
                    engine.send(TAMA_GESTURE_TAP)
                } else if abs(dy) >= abs(dx) {
                    engine.send(dy < 0 ? TAMA_GESTURE_SWIPE_UP : TAMA_GESTURE_SWIPE_DOWN)
                } else if dx < 0 {
                    engine.send(TAMA_GESTURE_SWIPE_LEFT)
                }
            }
    }
}
