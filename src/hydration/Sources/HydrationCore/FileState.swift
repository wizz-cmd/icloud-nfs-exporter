import Foundation

/// Hydration state of an iCloud-managed file.
///
/// Each file tracked by the hydration subsystem occupies exactly one of these
/// states at any given time. The states form a finite state machine with
/// validated transitions (see ``validTransitions`` and ``canTransition(to:)``).
///
/// State machine:
/// ```
/// UNKNOWN → EVICTED → DOWNLOADING → LOCAL
///                          ↑            │
///                          └────────────┘  (re-eviction by macOS)
/// Any state → ERROR; ERROR → EVICTED / DOWNLOADING / UNKNOWN
/// ```
public enum FileState: String, Codable, Sendable, Equatable {
    /// The file's iCloud status could not be determined.
    case unknown
    /// The file is an iCloud placeholder stub and its content is not available locally.
    case evicted
    /// The file's content is actively being downloaded from iCloud.
    case downloading
    /// The file's content is fully available on the local filesystem.
    case local
    /// An error occurred while detecting state or downloading the file.
    case error
}

extension FileState {
    /// Returns the set of states that are valid successors of the current state.
    ///
    /// Use this property to inspect which transitions the state machine permits
    /// from the receiver. For a simple boolean check, prefer ``canTransition(to:)``.
    ///
    /// - Returns: A set of ``FileState`` values that the current state may transition to.
    public var validTransitions: Set<FileState> {
        switch self {
        case .unknown:     return [.evicted, .local, .error]
        case .evicted:     return [.downloading, .local, .error]
        case .downloading: return [.local, .error, .evicted]
        case .local:       return [.evicted, .error]
        case .error:       return [.evicted, .downloading, .unknown]
        }
    }

    /// Returns whether a transition from the current state to `next` is permitted.
    ///
    /// - Parameter next: The target state to validate.
    /// - Returns: `true` if the transition is allowed by the state machine; `false` otherwise.
    public func canTransition(to next: FileState) -> Bool {
        validTransitions.contains(next)
    }
}
