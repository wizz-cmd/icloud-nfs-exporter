import Foundation
import HydrationCore

/// Reads the TOML configuration file used by the icne CLI.
struct ConfigReader {

    var socketPath: String
    var mountBase: String
    var folders: [FolderEntry]

    struct FolderEntry {
        var source: String
        var label: String
    }

    static let configPath: URL = FileManager.default
        .homeDirectoryForCurrentUser
        .appendingPathComponent(".config/icloud-nfs-exporter/config.toml")

    /// Load config from disk; returns defaults if the file is missing.
    static func load() -> ConfigReader {
        guard let data = try? Data(contentsOf: configPath),
              let content = String(data: data, encoding: .utf8)
        else {
            return ConfigReader(
                socketPath: HydrationCore.defaultSocketPath,
                mountBase: "/tmp/icne-mnt",
                folders: [])
        }
        return parse(content)
    }

    /// Derive the FUSE mount point for a source directory.
    static func mountPointFor(source: String, mountBase: String) -> String {
        let name = URL(fileURLWithPath: source).lastPathComponent
        return URL(fileURLWithPath: mountBase)
            .appendingPathComponent(name).path
    }

    // MARK: - Private

    private static func parse(_ content: String) -> ConfigReader {
        var socketPath = HydrationCore.defaultSocketPath
        var mountBase = "/tmp/icne-mnt"
        var folders: [FolderEntry] = []

        var currentFolder: [String: String]?

        for rawLine in content.components(separatedBy: "\n") {
            let line = rawLine.trimmingCharacters(in: .whitespaces)

            // Skip comments and section headers (except [[folders]])
            if line.hasPrefix("#") { continue }
            if line == "[[folders]]" {
                if let f = currentFolder {
                    folders.append(FolderEntry(
                        source: f["source", default: ""],
                        label: f["label", default: ""]))
                }
                currentFolder = [:]
                continue
            }
            if line.hasPrefix("[") { continue }

            // Key = value
            guard let eq = line.firstIndex(of: "=") else { continue }
            let key = line[..<eq]
                .trimmingCharacters(in: .whitespaces)
            var val = line[line.index(after: eq)...]
                .trimmingCharacters(in: .whitespaces)
            if val.hasPrefix("\"") && val.hasSuffix("\"") && val.count >= 2 {
                val = String(val.dropFirst().dropLast())
            }

            if currentFolder != nil {
                currentFolder?[key] = val
            } else {
                switch key {
                case "socket_path": socketPath = val
                case "mount_base": mountBase = val
                default: break
                }
            }
        }

        // Flush last folder
        if let f = currentFolder {
            folders.append(FolderEntry(
                source: f["source", default: ""],
                label: f["label", default: ""]))
        }

        return ConfigReader(
            socketPath: socketPath,
            mountBase: mountBase,
            folders: folders)
    }
}
