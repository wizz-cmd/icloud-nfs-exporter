import Foundation

/// Manages per-file hydration state and triggers on-demand iCloud downloads.
///
/// `HydrationManager` is the central coordinator for file hydration. It maintains
/// an in-memory map of tracked file paths to their current ``FileState``, validates
/// state transitions, and orchestrates the download-and-poll cycle when a file
/// needs to be materialized from iCloud.
///
/// Because it is declared as an `actor`, all mutable state access is serialized
/// automatically, making it safe to call from multiple concurrent tasks (e.g.,
/// simultaneous NFS client requests routed through ``IPCServer``).
public actor HydrationManager {
    private var states: [String: FileState] = [:]
    private let detector: any FileStateDetecting
    private let pollInterval: UInt64
    private let timeout: TimeInterval

    /// Creates a new hydration manager.
    ///
    /// - Parameter detector: The file state detector to use. Defaults to ``FileStateDetector()``.
    /// - Parameter pollInterval: Seconds between polls while waiting for a download to complete. Defaults to `0.5`.
    /// - Parameter timeout: Maximum seconds to wait for a download before reporting a timeout error. Defaults to `300` (5 minutes).
    public init(
        detector: any FileStateDetecting = FileStateDetector(),
        pollInterval: TimeInterval = 0.5,
        timeout: TimeInterval = 300
    ) {
        self.detector = detector
        self.pollInterval = UInt64(pollInterval * 1_000_000_000)
        self.timeout = timeout
    }

    /// Returns the last known hydration state for the given path, or `nil` if the file is not tracked.
    ///
    /// This returns the cached state without querying the filesystem. To refresh
    /// from disk, use ``refreshState(for:)`` instead.
    ///
    /// - Parameter path: The absolute filesystem path of the file.
    /// - Returns: The cached ``FileState``, or `nil` if the path has never been tracked.
    public func currentState(for path: String) -> FileState? {
        states[path]
    }

    /// Detects the current on-disk state and updates the internal cache.
    ///
    /// Queries the filesystem via the configured ``FileStateDetecting`` detector
    /// and stores the result. This is useful for synchronizing the in-memory
    /// state with changes made externally (e.g., by macOS evicting a file).
    ///
    /// - Parameter path: The absolute filesystem path of the file.
    /// - Returns: The freshly detected ``FileState``.
    /// - Throws: Rethrows any error from the underlying detector.
    @discardableResult
    public func refreshState(for path: String) throws -> FileState {
        let detected = try detector.detectState(at: URL(fileURLWithPath: path))
        states[path] = detected
        return detected
    }

    /// Ensures the file at `path` is fully materialized on the local filesystem.
    ///
    /// If the file is already local, returns immediately. If it is evicted, triggers
    /// an iCloud download via `FileManager.startDownloadingUbiquitousItem(at:)` and
    /// polls until the file reaches the ``FileState/local`` state or the timeout expires.
    /// If the file is currently downloading, waits for the in-progress download to finish.
    ///
    /// - Parameter path: The absolute filesystem path of the file to hydrate.
    /// - Returns: ``FileState/local`` on success.
    /// - Throws: ``HydrationError/invalidState(path:state:)`` if the file is in an unrecoverable state,
    ///   ``HydrationError/downloadFailed(path:underlying:)`` if the download could not be started or failed,
    ///   or ``HydrationError/timeout(path:)`` if the download did not complete in time.
    public func hydrate(path: String) async throws -> FileState {
        let current = try refreshState(for: path)

        switch current {
        case .local:
            return .local
        case .downloading:
            return try await waitForDownload(path: path)
        case .evicted:
            break
        case .unknown, .error:
            let recheck = try refreshState(for: path)
            if recheck == .local { return .local }
            guard recheck == .evicted else {
                throw HydrationError.invalidState(path: path, state: recheck)
            }
        }

        try transition(path: path, to: .downloading)

        do {
            try FileManager.default.startDownloadingUbiquitousItem(
                at: URL(fileURLWithPath: path)
            )
        } catch {
            try? transition(path: path, to: .error)
            throw HydrationError.downloadFailed(path: path, underlying: error)
        }

        return try await waitForDownload(path: path)
    }

    /// Re-detects the file state in response to an external filesystem event.
    ///
    /// Intended to be called from an ``FSEventsWatcher`` callback when a watched
    /// file changes. Delegates to ``refreshState(for:)`` to update the cache.
    ///
    /// - Parameter path: The absolute filesystem path that changed.
    /// - Returns: The freshly detected ``FileState``.
    /// - Throws: Rethrows any error from the underlying detector.
    @discardableResult
    public func handleEvent(for path: String) throws -> FileState {
        try refreshState(for: path)
    }

    /// Removes a file from the internal tracking map.
    ///
    /// After this call, ``currentState(for:)`` will return `nil` for the given path.
    /// Any in-flight hydration for this path is **not** cancelled.
    ///
    /// - Parameter path: The absolute filesystem path to stop tracking.
    public func stopTracking(_ path: String) {
        states.removeValue(forKey: path)
    }

    /// The number of files currently held in the tracking map.
    public var trackedCount: Int { states.count }

    // MARK: - Private

    private func transition(path: String, to newState: FileState) throws {
        let current = states[path] ?? .unknown
        guard current.canTransition(to: newState) else {
            throw HydrationError.invalidTransition(
                path: path, from: current, to: newState
            )
        }
        states[path] = newState
    }

    private func waitForDownload(path: String) async throws -> FileState {
        let start = Date()
        while Date().timeIntervalSince(start) < timeout {
            let detected = try refreshState(for: path)
            switch detected {
            case .local:
                return .local
            case .error:
                throw HydrationError.downloadFailed(path: path, underlying: nil)
            default:
                try await Task.sleep(nanoseconds: pollInterval)
            }
        }
        try? transition(path: path, to: .error)
        throw HydrationError.timeout(path: path)
    }
}

/// Errors produced by ``HydrationManager`` during hydration operations.
public enum HydrationError: Error, CustomStringConvertible {
    /// The file is in a state that does not allow hydration (e.g., ``FileState/unknown`` or ``FileState/error`` after a re-check).
    case invalidState(path: String, state: FileState)
    /// An attempted state transition violated the ``FileState`` state machine rules.
    case invalidTransition(path: String, from: FileState, to: FileState)
    /// The iCloud download could not be started or failed mid-transfer.
    case downloadFailed(path: String, underlying: Error?)
    /// The download did not complete within the configured timeout period.
    case timeout(path: String)

    /// A human-readable description of the error.
    public var description: String {
        switch self {
        case .invalidState(let p, let s):
            "Cannot hydrate \(p): unexpected state \(s.rawValue)"
        case .invalidTransition(let p, let from, let to):
            "Invalid transition for \(p): \(from.rawValue) → \(to.rawValue)"
        case .downloadFailed(let p, let e):
            "Download failed for \(p)\(e.map { ": \($0)" } ?? "")"
        case .timeout(let p):
            "Hydration timed out for \(p)"
        }
    }
}
