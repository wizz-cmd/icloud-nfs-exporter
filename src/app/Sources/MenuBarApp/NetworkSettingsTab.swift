import SwiftUI

struct NetworkSettingsTab: View {
    @Binding var allowedNetwork: String

    var body: some View {
        Form {
            Section("NFS Access") {
                TextField("Allowed Network (CIDR)", text: $allowedNetwork)
                    .accessibilityLabel("Allowed network")
                    .accessibilityHint("CIDR notation, for example 192.168.1.0/24")
                Text("Clients on this network can mount the NFS exports. Use CIDR notation, e.g. 192.168.1.0/24 or 10.0.0.0/8.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
        .formStyle(.grouped)
    }
}
