import SwiftUI

struct StatusPanelView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.openSettings) private var openSettings

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            // Status header
            HStack(spacing: 8) {
                Image(systemName: appState.statusIcon)
                    .symbolRenderingMode(.hierarchical)
                    .foregroundStyle(appState.daemonRunning ? .green : .secondary)
                    .font(.title3)
                Text(appState.daemonRunning ? "Running" : "Stopped")
                    .font(.headline)
                    .foregroundStyle(appState.daemonRunning ? .primary : .secondary)
            }
            .accessibilityElement(children: .combine)
            .accessibilityLabel(appState.statusDescription)

            Divider()

            // Folder list
            if appState.config.folders.isEmpty {
                Text("No folders configured")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                    .padding(.vertical, 4)
            } else {
                ForEach(appState.config.folders, id: \.source) { folder in
                    VStack(alignment: .leading, spacing: 2) {
                        Text(folder.label.isEmpty
                             ? URL(fileURLWithPath: folder.source).lastPathComponent
                             : folder.label)
                            .font(.subheadline)
                        Text(ConfigReader.mountPointFor(
                            source: folder.source,
                            mountBase: appState.config.mountBase))
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                            .truncationMode(.middle)
                    }
                    .accessibilityElement(children: .combine)
                }
            }

            Divider()

            Button {
                appState.refresh()
            } label: {
                Label("Refresh", systemImage: "arrow.clockwise")
            }
            .keyboardShortcut("r")
            .accessibilityHint("Reload configuration and check daemon status")

            Button {
                openSettings()
            } label: {
                Label("Settings\u{2026}", systemImage: "gear")
            }
            .keyboardShortcut(",")

            Divider()

            Button {
                NSApp.terminate(nil)
            } label: {
                Label("Quit", systemImage: "xmark.circle")
            }
            .keyboardShortcut("q")
        }
        .padding(16)
        .frame(width: 280)
        .onAppear {
            if appState.needsOnboarding {
                openSettings()
            }
        }
    }
}
