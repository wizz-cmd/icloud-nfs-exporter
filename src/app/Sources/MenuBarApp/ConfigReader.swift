import Foundation
import HydrationCore

/// Reads and writes the TOML configuration file used by the icne CLI.
struct ConfigReader {

    var socketPath: String
    var mountBase: String
    var allowedNetwork: String
    var folders: [FolderEntry]

    struct FolderEntry: Equatable {
        var source: String
        var label: String
    }

    static let configDir: URL = FileManager.default
        .homeDirectoryForCurrentUser
        .appendingPathComponent(".config/icloud-nfs-exporter")

    static let configPath: URL = configDir
        .appendingPathComponent("config.toml")

    /// Load config from disk; returns defaults if the file is missing.
    static func load() -> ConfigReader {
        guard let data = try? Data(contentsOf: configPath),
              let content = String(data: data, encoding: .utf8)
        else {
            return defaults()
        }
        return parse(content)
    }

    /// Default configuration.
    static func defaults() -> ConfigReader {
        ConfigReader(
            socketPath: HydrationCore.defaultSocketPath,
            mountBase: "/tmp/icne-mnt",
            allowedNetwork: "192.168.0.0/24",
            folders: [])
    }

    /// Write config to disk as TOML.
    func save() throws {
        try FileManager.default.createDirectory(
            at: Self.configDir, withIntermediateDirectories: true)

        var lines: [String] = []
        lines.append("[general]")
        lines.append("socket_path = \"\(socketPath)\"")
        lines.append("mount_base = \"\(mountBase)\"")
        lines.append("")
        lines.append("[nfs]")
        lines.append("server = \"nfsd\"")
        lines.append("allowed_network = \"\(allowedNetwork)\"")
        lines.append("")
        for folder in folders {
            lines.append("[[folders]]")
            lines.append("source = \"\(folder.source)\"")
            lines.append("label = \"\(folder.label)\"")
            lines.append("")
        }

        let content = lines.joined(separator: "\n")
        try content.write(to: Self.configPath, atomically: true, encoding: .utf8)

        // Ensure mount directories exist
        let base = URL(fileURLWithPath: mountBase)
        try FileManager.default.createDirectory(
            at: base, withIntermediateDirectories: true)
        for folder in folders {
            let mp = URL(fileURLWithPath: Self.mountPointFor(
                source: folder.source, mountBase: mountBase))
            try? FileManager.default.createDirectory(
                at: mp, withIntermediateDirectories: true)
        }
    }

    /// Derive the FUSE mount point for a source directory.
    static func mountPointFor(source: String, mountBase: String) -> String {
        let name = URL(fileURLWithPath: source).lastPathComponent
        return URL(fileURLWithPath: mountBase)
            .appendingPathComponent(name).path
    }

    /// Discover iCloud containers in ~/Library/Mobile Documents.
    static func discoverContainers() -> [FolderEntry] {
        let base = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/Mobile Documents")
        guard let children = try? FileManager.default.contentsOfDirectory(
            at: base, includingPropertiesForKeys: [.isDirectoryKey])
        else { return [] }

        var containers: [FolderEntry] = []
        for child in children {
            let vals = try? child.resourceValues(forKeys: [.isDirectoryKey])
            guard vals?.isDirectory == true else { continue }
            let name = child.lastPathComponent
            if name.hasPrefix(".") { continue }
            containers.append(FolderEntry(
                source: child.path,
                label: Self.labelForContainer(name)))
        }

        // iCloud Drive first, then alphabetical
        containers.sort { a, b in
            let aFirst = a.source.hasSuffix("com~apple~CloudDocs")
            let bFirst = b.source.hasSuffix("com~apple~CloudDocs")
            if aFirst != bFirst { return aFirst }
            return a.label.localizedCaseInsensitiveCompare(b.label) == .orderedAscending
        }
        return containers
    }

    private static let appleLabels: [String: String] = [
        "com~apple~CloudDocs": "iCloud Drive",
        "com~apple~Numbers": "Numbers",
        "com~apple~Pages": "Pages",
        "com~apple~Keynote": "Keynote",
        "com~apple~Preview": "Preview",
        "com~apple~TextEdit": "TextEdit",
    ]

    private static func labelForContainer(_ name: String) -> String {
        if let label = appleLabels[name] { return label }
        if name.hasPrefix("iCloud~") {
            return String(name.split(separator: "~").last ?? Substring(name))
        }
        return name
    }

    // MARK: - Parsing

    private static func parse(_ content: String) -> ConfigReader {
        var socketPath = HydrationCore.defaultSocketPath
        var mountBase = "/tmp/icne-mnt"
        var allowedNetwork = "192.168.0.0/24"
        var folders: [FolderEntry] = []
        var currentFolder: [String: String]?

        for rawLine in content.components(separatedBy: "\n") {
            let line = rawLine.trimmingCharacters(in: .whitespaces)
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

            guard let eq = line.firstIndex(of: "=") else { continue }
            let key = line[..<eq].trimmingCharacters(in: .whitespaces)
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
                case "allowed_network": allowedNetwork = val
                default: break
                }
            }
        }

        if let f = currentFolder {
            folders.append(FolderEntry(
                source: f["source", default: ""],
                label: f["label", default: ""]))
        }

        return ConfigReader(
            socketPath: socketPath,
            mountBase: mountBase,
            allowedNetwork: allowedNetwork,
            folders: folders)
    }
}
