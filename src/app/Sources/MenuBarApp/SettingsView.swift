import SwiftUI

struct SettingsView: View {
    @State private var folders: [ConfigReader.FolderEntry]
    @State private var network: String
    @State private var available: [ConfigReader.FolderEntry] = []

    var onSave: ((ConfigReader) -> Void)?
    var onCancel: (() -> Void)?

    private let config: ConfigReader

    init(config: ConfigReader,
         onSave: ((ConfigReader) -> Void)? = nil,
         onCancel: (() -> Void)? = nil) {
        self.config = config
        self.onSave = onSave
        self.onCancel = onCancel
        _folders = State(initialValue: config.folders)
        _network = State(initialValue: config.allowedNetwork)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            // Header
            Text("iCloud NFS Exporter")
                .font(.title2.bold())

            // Folders
            GroupBox {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Exported Folders")
                        .font(.headline)

                    if folders.isEmpty {
                        Text("No folders selected. Add iCloud folders to export via NFS.")
                            .foregroundColor(.secondary)
                            .padding(.vertical, 4)
                    } else {
                        ForEach(Array(folders.enumerated()), id: \.offset) { i, folder in
                            HStack {
                                VStack(alignment: .leading) {
                                    Text(folder.label)
                                        .fontWeight(.medium)
                                    Text(folder.source)
                                        .font(.caption)
                                        .foregroundColor(.secondary)
                                        .lineLimit(1)
                                        .truncationMode(.middle)
                                }
                                Spacer()
                                Button(role: .destructive) {
                                    folders.remove(at: i)
                                } label: {
                                    Image(systemName: "minus.circle.fill")
                                }
                                .buttonStyle(.plain)
                                .foregroundColor(.red)
                            }
                            if i < folders.count - 1 { Divider() }
                        }
                    }

                    Divider()

                    Menu {
                        ForEach(
                            Array(available.enumerated()), id: \.offset
                        ) { _, container in
                            let alreadyAdded = folders.contains {
                                $0.source == container.source
                            }
                            Button {
                                if !alreadyAdded {
                                    folders.append(container)
                                }
                            } label: {
                                HStack {
                                    Text(container.label)
                                    if alreadyAdded {
                                        Image(systemName: "checkmark")
                                    }
                                }
                            }
                            .disabled(alreadyAdded)
                        }
                    } label: {
                        Label("Add iCloud Folder", systemImage: "plus")
                    }
                }
                .padding(4)
            }

            // Network
            GroupBox {
                VStack(alignment: .leading, spacing: 8) {
                    Text("NFS Network Access")
                        .font(.headline)
                    HStack {
                        Text("Allow connections from:")
                        TextField("CIDR", text: $network)
                            .textFieldStyle(.roundedBorder)
                            .frame(width: 180)
                    }
                    Text("Use CIDR notation, e.g. 192.168.1.0/24 or 10.0.0.0/8")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
                .padding(4)
            }

            Spacer()

            // Buttons
            HStack {
                Spacer()
                Button("Cancel") { onCancel?() }
                    .keyboardShortcut(.cancelAction)
                Button("Save") { save() }
                    .keyboardShortcut(.defaultAction)
            }
        }
        .padding(20)
        .frame(width: 520, height: 460)
        .onAppear {
            available = ConfigReader.discoverContainers()
        }
    }

    private func save() {
        var updated = config
        updated.folders = folders
        updated.allowedNetwork = network
        onSave?(updated)
    }
}
