import Foundation

/// A type that can inspect a file on disk and determine its iCloud hydration state.
///
/// Conform to this protocol to provide custom file-state detection logic,
/// for example in tests where you need deterministic results without
/// querying the real filesystem.
public protocol FileStateDetecting: Sendable {
    /// Detects the current iCloud hydration state of the file at the given URL.
    ///
    /// - Parameter url: The file URL to inspect. Must be a file (not directory) URL.
    /// - Returns: The detected ``FileState`` for the file.
    /// - Throws: An error if the file's resource values cannot be read.
    func detectState(at url: URL) throws -> FileState
}

/// Production implementation of ``FileStateDetecting`` that queries the filesystem.
///
/// Uses a combination of `.icloud` stub filename detection and `URLResourceValues`
/// (specifically `isUbiquitousItemKey`, `ubiquitousItemDownloadingStatusKey`, and
/// `ubiquitousItemIsDownloadingKey`) to determine whether a file is local, evicted,
/// downloading, or in an unknown state.
public struct FileStateDetector: FileStateDetecting, Sendable {
    /// Creates a new file state detector.
    public init() {}

    /// Detects the current iCloud hydration state of the file at the given URL.
    ///
    /// The detection logic follows this order of precedence:
    /// 1. If the filename matches the `.Name.icloud` stub pattern, returns ``FileState/evicted``.
    /// 2. If the file does not exist on disk, returns ``FileState/unknown``.
    /// 3. If the file is not iCloud-managed, returns ``FileState/local``.
    /// 4. Otherwise, inspects `URLResourceValues` to determine the download status.
    ///
    /// - Parameter url: The file URL to inspect.
    /// - Returns: The detected ``FileState``.
    /// - Throws: An error if `URL.resourceValues(forKeys:)` fails.
    public func detectState(at url: URL) throws -> FileState {
        let name = url.lastPathComponent

        // .icloud placeholder stub (e.g. ".Document.pdf.icloud")
        if name.hasPrefix(".") && name.hasSuffix(".icloud") {
            return .evicted
        }

        guard FileManager.default.fileExists(atPath: url.path) else {
            return .unknown
        }

        let keys: Set<URLResourceKey> = [
            .isUbiquitousItemKey,
            .ubiquitousItemDownloadingStatusKey,
            .ubiquitousItemIsDownloadingKey,
        ]
        let values = try url.resourceValues(forKeys: keys)

        // Not iCloud-managed — treat as fully local
        guard values.isUbiquitousItem == true else {
            return .local
        }

        if values.ubiquitousItemIsDownloading == true {
            return .downloading
        }

        if let status = values.ubiquitousItemDownloadingStatus {
            switch status {
            case .current, .downloaded:
                return .local
            case .notDownloaded:
                return .evicted
            default:
                return .unknown
            }
        }

        return .unknown
    }
}
