import SwiftUI

struct GeneralSettingsTab: View {
    @Binding var socketPath: String
    @Binding var mountBase: String

    var body: some View {
        Form {
            Section("Daemon") {
                TextField("IPC Socket Path", text: $socketPath)
                    .accessibilityLabel("IPC socket path")
                    .accessibilityHint("Unix domain socket for daemon communication")
            }

            Section("Storage") {
                TextField("Mount Base Directory", text: $mountBase)
                    .accessibilityLabel("Mount base directory")
                    .accessibilityHint("Parent directory for FUSE mount points")
            }
        }
        .formStyle(.grouped)
    }
}
