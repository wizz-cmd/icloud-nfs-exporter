import SwiftUI
import HydrationCore

@main
struct MenuBarApp: App {
    @State private var appState = AppState()

    var body: some Scene {
        MenuBarExtra {
            StatusPanelView()
                .environment(appState)
        } label: {
            Image(systemName: appState.statusIcon)
                .accessibilityLabel(appState.statusDescription)
        }
        .menuBarExtraStyle(.window)

        Settings {
            SettingsView()
                .environment(appState)
        }
    }
}
