import SwiftUI

/// The 128×128 screen filling the display, the A / B / C buttons floating over it. On the screen: swipe up / down turn
/// the dial (the Digital Crown does too), a tap selects, swipe left goes back, a long press is
/// the dial's long press.
struct GameView: View {
    @ObservedObject var engine: Engine
    @State private var crown = 0.0
    @State private var crownStep = 0.0
    @State private var downAt: Date?
    @State private var holdTask: Task<Void, Never>?

    var body: some View {
        GeometryReader { geo in
            // The screen fills the display (square, centred); the buttons float over its
            // bottom edge, half see-through.
            let side = min(geo.size.width, geo.size.height)
            let d = side / 4.6
            ZStack(alignment: .bottom) {
                Color.black
                screen
                    .frame(width: side, height: side)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    .contentShape(Rectangle())
                    .gesture(touch)
                HStack(spacing: d * 0.5) {
                    key("A", TAMA_A, d)
                    key("B", TAMA_B, d)
                    key("C", TAMA_C, d)
                }
                .padding(.bottom, d * 0.25)
            }
        }
        .ignoresSafeArea()
        .persistentSystemOverlays(.hidden)
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

    @ViewBuilder private var screen: some View {
        if let image = engine.image {
            Image(decorative: image, scale: 1)
                .interpolation(.none)
                .resizable()
        } else if let error = engine.error {
            Text(error).font(.footnote).multilineTextAlignment(.center)
        } else {
            Color.black
        }
    }

    /// One of the device's buttons: held while the finger is on it.
    private func key(_ label: String, _ bit: UInt32, _ d: CGFloat) -> some View {
        Text(label)
            .font(.system(size: d * 0.42, weight: .semibold))
            .foregroundStyle(.white.opacity(0.9))
            .frame(width: d, height: d)
            .background(Circle().fill(.black.opacity(0.35)))
            .overlay(Circle().stroke(.white.opacity(0.45), lineWidth: 1.5))
            .contentShape(Circle())
            .onLongPressGesture(minimumDuration: 0, maximumDistance: 40) {
                // A tap too short to be seen pressing still counts, once.
                engine.tap(bit)
            } onPressingChanged: { pressing in
                engine.button(bit, down: pressing)
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
