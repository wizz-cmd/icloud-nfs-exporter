import SwiftUI

struct AdvancedSettingsTab: View {
    @Environment(AppState.self) private var appState

    var body: some View {
        Form {
            Section("Hydration Daemon") {
                LabeledContent("Status") {
                    HStack(spacing: 6) {
                        Circle()
                            .fill(appState.daemonRunning ? .green : .red)
                            .frame(width: 8, height: 8)
                            .accessibilityHidden(true)
                        Text(appState.daemonRunning ? "Running" : "Stopped")
                            .foregroundStyle(
                                appState.daemonRunning ? .primary : .secondary)
                    }
                }
                .accessibilityElement(children: .combine)
                .accessibilityLabel(
                    "Daemon status: \(appState.daemonRunning ? "running" : "stopped")")

                LabeledContent("Socket") {
                    Text(appState.config.socketPath)
                        .foregroundStyle(.secondary)
                        .textSelection(.enabled)
                }

                Button {
                    appState.refresh()
                } label: {
                    Label("Refresh Status", systemImage: "arrow.clockwise")
                }
                .accessibilityHint("Reload configuration and check daemon status")
            }
        }
        .formStyle(.grouped)
    }
}
