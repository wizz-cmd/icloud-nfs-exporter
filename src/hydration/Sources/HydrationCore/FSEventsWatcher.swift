import CoreServices
import Foundation

/// Watches directories for filesystem events using the macOS FSEvents API.
///
/// Wraps `FSEventStream` in a Swift-friendly interface. Events are delivered
/// as batches of ``Event`` values to a caller-supplied ``Handler`` closure on
/// a dedicated `DispatchQueue`. The watcher is configured for file-level
/// granularity (`kFSEventStreamCreateFlagFileEvents`) so that individual file
/// changes -- including extended-attribute modifications relevant to iCloud
/// state detection -- are reported.
///
/// Typical usage:
/// ```swift
/// let watcher = FSEventsWatcher(paths: ["/path/to/icloud"]) { events in
///     for event in events where event.isItemXattrMod {
///         try? await manager.handleEvent(for: event.path)
///     }
/// }
/// watcher.start()
/// // ... later ...
/// watcher.stop()
/// ```
public final class FSEventsWatcher: @unchecked Sendable {

    /// A single filesystem event reported by FSEvents.
    public struct Event: Sendable {
        /// The absolute path of the file or directory that changed.
        public let path: String
        /// The raw FSEvents flags bitmask describing what happened.
        public let flags: FSEventStreamEventFlags
        /// The monotonically increasing event identifier assigned by FSEvents.
        public let id: FSEventStreamEventId

        /// Whether the event indicates a newly created item.
        public var isItemCreated: Bool  { flags & UInt32(kFSEventStreamEventFlagItemCreated) != 0 }
        /// Whether the event indicates a removed item.
        public var isItemRemoved: Bool  { flags & UInt32(kFSEventStreamEventFlagItemRemoved) != 0 }
        /// Whether the event indicates a renamed item.
        public var isItemRenamed: Bool  { flags & UInt32(kFSEventStreamEventFlagItemRenamed) != 0 }
        /// Whether the event indicates a modified item (content change).
        public var isItemModified: Bool { flags & UInt32(kFSEventStreamEventFlagItemModified) != 0 }
        /// Whether the changed item is a regular file.
        public var isItemIsFile: Bool   { flags & UInt32(kFSEventStreamEventFlagItemIsFile) != 0 }
        /// Whether the changed item is a directory.
        public var isItemIsDir: Bool    { flags & UInt32(kFSEventStreamEventFlagItemIsDir) != 0 }
        /// Whether the event indicates an extended-attribute change (relevant for iCloud state).
        public var isItemXattrMod: Bool { flags & UInt32(kFSEventStreamEventFlagItemXattrMod) != 0 }
    }

    /// Closure type invoked with a batch of filesystem events.
    public typealias Handler = @Sendable ([Event]) -> Void

    private var stream: FSEventStreamRef?
    private let handler: Handler
    private let queue: DispatchQueue

    /// Creates a new FSEvents watcher for the given directory paths.
    ///
    /// The watcher is created in a stopped state. Call ``start()`` to begin
    /// receiving events.
    ///
    /// - Parameter paths: Directory paths to watch. Subdirectories are watched recursively.
    /// - Parameter latency: Coalescing interval in seconds before events are delivered. Defaults to `0.5`.
    /// - Parameter queue: The dispatch queue on which the ``Handler`` is called. Defaults to a utility-QoS queue.
    /// - Parameter handler: A closure invoked with each batch of filesystem events.
    public init(
        paths: [String],
        latency: TimeInterval = 0.5,
        queue: DispatchQueue = .init(label: "fsevents", qos: .utility),
        handler: @escaping Handler
    ) {
        self.handler = handler
        self.queue = queue

        var context = FSEventStreamContext()
        context.info = Unmanaged.passUnretained(self).toOpaque()

        stream = FSEventStreamCreate(
            nil,
            Self.callback,
            &context,
            paths as CFArray,
            FSEventStreamEventId(kFSEventStreamEventIdSinceNow),
            latency,
            UInt32(
                kFSEventStreamCreateFlagFileEvents
                    | kFSEventStreamCreateFlagUseCFTypes
                    | kFSEventStreamCreateFlagNoDefer
            )
        )
    }

    deinit { stop() }

    /// Starts the FSEvents stream and begins delivering events to the handler.
    ///
    /// Has no effect if the stream was not successfully created during initialization.
    /// The stream is scheduled on the dispatch queue provided at init time.
    public func start() {
        guard let stream else { return }
        FSEventStreamSetDispatchQueue(stream, queue)
        FSEventStreamStart(stream)
    }

    /// Stops the FSEvents stream, invalidates it, and releases the underlying resource.
    ///
    /// After calling this method, no further events will be delivered. It is safe
    /// to call `stop()` multiple times; subsequent calls are no-ops. This method
    /// is also called automatically from `deinit`.
    public func stop() {
        guard let stream else { return }
        FSEventStreamStop(stream)
        FSEventStreamInvalidate(stream)
        FSEventStreamRelease(stream)
        self.stream = nil
    }

    // C function pointer — cannot capture context; uses info pointer.
    private static let callback: FSEventStreamCallback = {
        _, info, numEvents, eventPaths, eventFlags, eventIds in

        guard let info else { return }
        let watcher = Unmanaged<FSEventsWatcher>.fromOpaque(info)
            .takeUnretainedValue()
        let cfArray = unsafeBitCast(eventPaths, to: NSArray.self)

        var events: [Event] = []
        events.reserveCapacity(numEvents)
        for i in 0..<numEvents {
            guard let path = cfArray[i] as? String else { continue }
            events.append(Event(
                path: path,
                flags: eventFlags[i],
                id: eventIds[i]
            ))
        }
        watcher.handler(events)
    }
}
