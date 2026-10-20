import SwiftUI

/// The 128×128 screen filling the whole display, the A / B / C buttons floating over it. On the screen: swipe up / down turn
/// the dial (the Digital Crown does too), a tap selects, swipe left goes back, a long press is
/// the dial's long press.
struct GameView: View {
    /// Display points per game pixel (filling the width would be about 1.6 on a 46 mm watch).
    static let uiScale: CGFloat = 1.5
    /// Crown turn (in its value units, medium sensitivity) for one dial step. Smaller = faster.
    static let crownPerStep = 0.8

    @ObservedObject var engine: Engine
    @State private var crown = 0.0
    @State private var crownStep = 0.0
    @State private var downAt: Date?
    @State private var holdTask: Task<Void, Never>?

    var body: some View {
        GeometryReader { geo in
            // The game renders on a canvas shaped like the display, its own screen centred at
            // `uiScale` points per pixel (smaller than filling the width), the world and
            // backgrounds around it; spots nothing covers show the blurred screen behind.
            let d = min(geo.size.width, geo.size.height) / 4.6
            ZStack(alignment: .bottom) {
                Color.black
                backdrop
                    .frame(width: geo.size.width, height: geo.size.height)
                    .clipped()
                screen
                    .frame(width: geo.size.width, height: geo.size.height)
                    .clipped()
                    .contentShape(Rectangle())
                    .gesture(touch)
                HStack(spacing: d * 0.5) {
                    key("A", TAMA_A, d)
                    key("B", TAMA_B, d)
                    key("C", TAMA_C, d)
                }
                .padding(.bottom, d * 0.25)
            }
            // The canvas follows the whole display (this reader ignores the safe area).
            .onChange(of: geo.size, initial: true) { _, size in
                engine.fit(size, scale: Self.uiScale)
            }
        }
        .ignoresSafeArea()
        .persistentSystemOverlays(.hidden)
        .focusable()
        .digitalCrownRotation($crown, from: -1000, through: 1000, sensitivity: .medium,
                              isContinuous: true, isHapticFeedbackEnabled: true)
        .onChange(of: crown) { _, new in
            // One dial step per `crownStep` of turn (the range wraps: follow the shortest way).
            var d = new - crownStep
            if d > 1000 { d -= 2000 } else if d < -1000 { d += 2000 }
            guard abs(d) >= Self.crownPerStep else { return }
            crownStep = new
            engine.send(d > 0 ? TAMA_GESTURE_SWIPE_UP : TAMA_GESTURE_SWIPE_DOWN)
        }
    }

    /// The screen scaled to fill the display, blurred and dimmed, behind the real one.
    @ViewBuilder private var backdrop: some View {
        if let image = engine.image {
            Image(decorative: image, scale: 1)
                .resizable()
                .aspectRatio(contentMode: .fill)
                .blur(radius: 10)
                .opacity(0.55)
        }
    }

    @ViewBuilder private var screen: some View {
        if let image = engine.image {
            // Never stretched: a canvas a pixel off the display's shape is cropped, not distorted.
            Image(decorative: image, scale: 1)
                .interpolation(.none)
                .resizable()
                .aspectRatio(contentMode: .fill)
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
            .foregroundStyle(.white.opacity(0.4))
            .frame(width: d, height: d)
            .background(Circle().fill(.black.opacity(0.08)))
            .overlay(Circle().stroke(.white.opacity(0.2), lineWidth: 1.5))
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
