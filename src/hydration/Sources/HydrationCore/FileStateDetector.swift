import Foundation

/// Detects the iCloud hydration state of a file on disk.
public protocol FileStateDetecting: Sendable {
    func detectState(at url: URL) throws -> FileState
}

/// Production detector that uses URL resource values and .icloud stub detection.
public struct FileStateDetector: FileStateDetecting, Sendable {
    public init() {}

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
