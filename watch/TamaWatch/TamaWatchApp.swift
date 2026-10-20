import SwiftUI

@main
struct TamaWatchApp: App {
    @StateObject private var engine = Engine()
    @Environment(\.scenePhase) private var phase

    var body: some Scene {
        WindowGroup {
            GameView(engine: engine)
        }
        .onChange(of: phase) { _, now in
            if now == .active { engine.resume() } else { engine.pause() }
        }
    }
}
