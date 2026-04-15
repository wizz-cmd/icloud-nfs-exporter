import SwiftUI

struct FoldersSettingsTab: View {
    @Binding var folders: [ConfigReader.FolderEntry]
    let mountBase: String

    @State private var available: [ConfigReader.FolderEntry] = []

    var body: some View {
        Form {
            Section("Exported Folders") {
                if folders.isEmpty {
                    Text("No folders selected. Add iCloud folders to export via NFS.")
                        .foregroundStyle(.secondary)
                } else {
                    ForEach(Array(folders.enumerated()), id: \.element.source) { i, folder in
                        LabeledContent {
                            Button(role: .destructive) {
                                folders.remove(at: i)
                            } label: {
                                Image(systemName: "minus.circle.fill")
                                    .foregroundStyle(.red)
                            }
                            .buttonStyle(.plain)
                            .accessibilityLabel("Remove \(folder.label)")
                            .accessibilityHint("Removes this folder from NFS exports")
                        } label: {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(folder.label.isEmpty
                                     ? URL(fileURLWithPath: folder.source).lastPathComponent
                                     : folder.label)
                                Text(ConfigReader.mountPointFor(
                                    source: folder.source,
                                    mountBase: mountBase))
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                        }
                    }
                }

                Menu {
                    ForEach(available, id: \.source) { container in
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
                .accessibilityLabel("Add iCloud folder to export")
            }
        }
        .formStyle(.grouped)
        .onAppear {
            available = ConfigReader.discoverContainers()
        }
    }
}
