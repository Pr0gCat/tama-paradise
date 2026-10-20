import SwiftUI

/// The 128×128 screen with the A / B / C buttons under it. On the screen: swipe up / down turn
/// the dial (the Digital Crown does too), a tap selects, swipe left goes back, a long press is
/// the dial's long press.
struct GameView: View {
    @ObservedObject var engine: Engine
    @State private var crown = 0.0
    @State private var crownStep = 0.0
    @State private var downAt: Date?
    @State private var holdTask: Task<Void, Never>?

    var body: some View {
        VStack(spacing: 4) {
            screen
                .aspectRatio(1, contentMode: .fit)
                .contentShape(Rectangle())
                .gesture(touch)
            Spacer(minLength: 0)
            HStack(spacing: 6) {
                key("A", TAMA_A)
                key("B", TAMA_B)
                key("C", TAMA_C)
            }
            .frame(height: 34)
        }
        .padding(.horizontal, 4)
        .padding(.bottom, 6)
        .ignoresSafeArea(edges: .bottom)
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
    private func key(_ label: String, _ bit: UInt32) -> some View {
        Text(label)
            .font(.system(size: 15, weight: .semibold))
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(Capsule().fill(Color(white: 0.25)))
            .contentShape(Capsule())
            .onLongPressGesture(minimumDuration: 0, maximumDistance: 40) {
                // A tap too short to be seen pressing still counts once.
                engine.button(bit, down: true)
                engine.button(bit, down: false)
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
