import SwiftUI

struct SettingsView: View {
    @Environment(AppState.self) private var appState

    @State private var socketPath = ""
    @State private var mountBase = ""
    @State private var allowedNetwork = ""
    @State private var folders: [ConfigReader.FolderEntry] = []
    @State private var hasChanges = false

    var body: some View {
        TabView {
            GeneralSettingsTab(socketPath: $socketPath, mountBase: $mountBase)
                .tabItem { Label("General", systemImage: "gear") }

            FoldersSettingsTab(folders: $folders, mountBase: mountBase)
                .tabItem { Label("Folders", systemImage: "folder") }

            NetworkSettingsTab(allowedNetwork: $allowedNetwork)
                .tabItem { Label("Network", systemImage: "network") }

            AdvancedSettingsTab()
                .tabItem { Label("Advanced", systemImage: "gearshape.2") }
        }
        .frame(width: 500)
        .onAppear { loadFromConfig() }
        .onChange(of: socketPath) { hasChanges = true }
        .onChange(of: mountBase) { hasChanges = true }
        .onChange(of: allowedNetwork) { hasChanges = true }
        .onChange(of: folders) { hasChanges = true }
        .safeAreaInset(edge: .bottom) {
            HStack {
                Button("Revert") { loadFromConfig(); hasChanges = false }
                    .disabled(!hasChanges)
                Spacer()
                Button("Save") { save() }
                    .keyboardShortcut(.defaultAction)
                    .disabled(!hasChanges)
            }
            .padding(16)
        }
    }

    private func loadFromConfig() {
        socketPath = appState.config.socketPath
        mountBase = appState.config.mountBase
        allowedNetwork = appState.config.allowedNetwork
        folders = appState.config.folders
        hasChanges = false
    }

    private func save() {
        var updated = appState.config
        updated.socketPath = socketPath
        updated.mountBase = mountBase
        updated.allowedNetwork = allowedNetwork
        updated.folders = folders
        do {
            try appState.saveConfig(updated)
            hasChanges = false
        } catch {
            // Error is visible in the Advanced tab via daemon status
        }
    }
}
