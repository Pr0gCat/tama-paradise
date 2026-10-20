import SwiftUI
import WatchKit

@main
struct TamaWatchApp: App {
    @StateObject private var engine = Engine()
    @StateObject private var awake = Awake()
    @Environment(\.scenePhase) private var phase

    var body: some Scene {
        WindowGroup {
            GameView(engine: engine)
        }
        .onChange(of: phase, initial: true) { _, now in
            switch now {
            case .active:
                engine.resume()
                awake.start()
            // Wrist down with Always On: the game keeps running on the dimmed screen.
            case .inactive:
                engine.save()
            default:
                engine.pause()
            }
        }
    }
}

/// Keeps the game in front while it is open: a mindfulness extended runtime session (up to an
/// hour) stops the watch from returning to the clock when the wrist drops; the screen stays on
/// in Always On. Restarted each time the app comes back to the front.
@MainActor
final class Awake: NSObject, ObservableObject, WKExtendedRuntimeSessionDelegate {
    private var session: WKExtendedRuntimeSession?

    func start() {
        if let session, session.state == .running || session.state == .scheduled { return }
        let s = WKExtendedRuntimeSession()
        s.delegate = self
        s.start()
        session = s
    }

    nonisolated func extendedRuntimeSessionDidStart(_ session: WKExtendedRuntimeSession) {}
    nonisolated func extendedRuntimeSessionWillExpire(_ session: WKExtendedRuntimeSession) {}
    nonisolated func extendedRuntimeSession(_ session: WKExtendedRuntimeSession,
                                            didInvalidateWith reason: WKExtendedRuntimeSessionInvalidationReason,
                                            error: Error?) {
        Task { @MainActor in self.session = nil }
    }
}
