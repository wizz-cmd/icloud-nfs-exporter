import Foundation

/// Manages per-file hydration state and triggers iCloud downloads.
public actor HydrationManager {
    private var states: [String: FileState] = [:]
    private let detector: any FileStateDetecting
    private let pollInterval: UInt64
    private let timeout: TimeInterval

    public init(
        detector: any FileStateDetecting = FileStateDetector(),
        pollInterval: TimeInterval = 0.5,
        timeout: TimeInterval = 300
    ) {
        self.detector = detector
        self.pollInterval = UInt64(pollInterval * 1_000_000_000)
        self.timeout = timeout
    }

    /// Last known state, or nil if the file is not tracked.
    public func currentState(for path: String) -> FileState? {
        states[path]
    }

    /// Detect the on-disk state and record it.
    @discardableResult
    public func refreshState(for path: String) throws -> FileState {
        let detected = try detector.detectState(at: URL(fileURLWithPath: path))
        states[path] = detected
        return detected
    }

    /// Request hydration.  Returns `.local` on success; throws on failure.
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

    /// Re-detect state from an external event (FSEvents callback).
    @discardableResult
    public func handleEvent(for path: String) throws -> FileState {
        try refreshState(for: path)
    }

    /// Stop tracking a file.
    public func stopTracking(_ path: String) {
        states.removeValue(forKey: path)
    }

    /// Number of files currently tracked.
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

/// Errors from the hydration subsystem.
public enum HydrationError: Error, CustomStringConvertible {
    case invalidState(path: String, state: FileState)
    case invalidTransition(path: String, from: FileState, to: FileState)
    case downloadFailed(path: String, underlying: Error?)
    case timeout(path: String)

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
