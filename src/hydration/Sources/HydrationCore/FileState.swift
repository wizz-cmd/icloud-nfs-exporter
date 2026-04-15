import Foundation

/// Hydration state of an iCloud-managed file.
///
/// State machine:
/// ```
/// UNKNOWN → EVICTED → DOWNLOADING → LOCAL
///                          ↑            │
///                          └────────────┘  (re-eviction by macOS)
/// Any state → ERROR; ERROR → EVICTED / DOWNLOADING / UNKNOWN
/// ```
public enum FileState: String, Codable, Sendable, Equatable {
    case unknown
    case evicted
    case downloading
    case local
    case error
}

extension FileState {
    /// Set of states reachable from the current state.
    public var validTransitions: Set<FileState> {
        switch self {
        case .unknown:     return [.evicted, .local, .error]
        case .evicted:     return [.downloading, .local, .error]
        case .downloading: return [.local, .error, .evicted]
        case .local:       return [.evicted, .error]
        case .error:       return [.evicted, .downloading, .unknown]
        }
    }

    /// Whether transitioning to `next` is valid.
    public func canTransition(to next: FileState) -> Bool {
        validTransitions.contains(next)
    }
}
