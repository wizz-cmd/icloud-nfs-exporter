import CoreServices
import Foundation

/// Watches directories for filesystem events using macOS FSEvents.
public final class FSEventsWatcher: @unchecked Sendable {

    /// A single filesystem event.
    public struct Event: Sendable {
        public let path: String
        public let flags: FSEventStreamEventFlags
        public let id: FSEventStreamEventId

        public var isItemCreated: Bool  { flags & UInt32(kFSEventStreamEventFlagItemCreated) != 0 }
        public var isItemRemoved: Bool  { flags & UInt32(kFSEventStreamEventFlagItemRemoved) != 0 }
        public var isItemRenamed: Bool  { flags & UInt32(kFSEventStreamEventFlagItemRenamed) != 0 }
        public var isItemModified: Bool { flags & UInt32(kFSEventStreamEventFlagItemModified) != 0 }
        public var isItemIsFile: Bool   { flags & UInt32(kFSEventStreamEventFlagItemIsFile) != 0 }
        public var isItemIsDir: Bool    { flags & UInt32(kFSEventStreamEventFlagItemIsDir) != 0 }
        public var isItemXattrMod: Bool { flags & UInt32(kFSEventStreamEventFlagItemXattrMod) != 0 }
    }

    public typealias Handler = @Sendable ([Event]) -> Void

    private var stream: FSEventStreamRef?
    private let handler: Handler
    private let queue: DispatchQueue

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

    /// Start delivering events.
    public func start() {
        guard let stream else { return }
        FSEventStreamSetDispatchQueue(stream, queue)
        FSEventStreamStart(stream)
    }

    /// Stop delivering events and release the stream.
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
