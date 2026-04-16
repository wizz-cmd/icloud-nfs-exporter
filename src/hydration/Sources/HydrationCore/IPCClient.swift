import Foundation

/// Client for communicating with the hydration daemon over a Unix domain socket.
///
/// `IPCClient` opens a new `SOCK_STREAM` connection for each request, sends an
/// ``IPCRequest`` using the ``IPCWireFormat`` length-prefixed framing, reads the
/// ``IPCResponse``, and closes the connection. This connect-per-request model
/// keeps the implementation simple and avoids connection-state management.
///
/// Higher-level convenience methods (``ping()``, ``queryState(path:)``,
/// ``isAvailable``) are built on top of the generic ``send(_:)`` method.
public final class IPCClient: @unchecked Sendable {
    /// The filesystem path of the Unix domain socket to connect to.
    public let socketPath: String
    private let timeout: TimeInterval

    /// Creates a new IPC client.
    ///
    /// - Parameter socketPath: Path to the daemon's Unix domain socket. Defaults to ``HydrationCore/defaultSocketPath``.
    /// - Parameter timeout: Read/write timeout in seconds for socket operations. Defaults to `10`.
    public init(
        socketPath: String = HydrationCore.defaultSocketPath,
        timeout: TimeInterval = 10
    ) {
        self.socketPath = socketPath
        self.timeout = timeout
    }

    /// Sends an IPC request to the daemon and returns the response.
    ///
    /// Opens a new Unix domain socket connection, writes the request using
    /// ``IPCWireFormat``, reads the framed response, and closes the connection.
    ///
    /// - Parameter request: The ``IPCRequest`` to send.
    /// - Returns: The ``IPCResponse`` received from the daemon.
    /// - Throws: ``IPCClientError/connectionFailed(errno:)`` if the connection cannot be established,
    ///   ``IPCClientError/readFailed`` or ``IPCClientError/writeFailed`` on I/O errors,
    ///   ``IPCClientError/invalidResponse`` if the response frame is malformed,
    ///   or a `DecodingError` if the JSON payload cannot be decoded.
    public func send(_ request: IPCRequest) throws -> IPCResponse {
        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else {
            throw IPCClientError.connectionFailed(errno: errno)
        }
        defer { Darwin.close(fd) }

        // Timeouts
        var tv = timeval(tv_sec: Int(timeout), tv_usec: 0)
        setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &tv,
                   socklen_t(MemoryLayout<timeval>.size))
        setsockopt(fd, SOL_SOCKET, SO_SNDTIMEO, &tv,
                   socklen_t(MemoryLayout<timeval>.size))

        // Connect
        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        let pathBytes = socketPath.utf8CString
        withUnsafeMutablePointer(to: &addr.sun_path) { ptr in
            ptr.withMemoryRebound(to: CChar.self, capacity: pathBytes.count) { dest in
                for (i, byte) in pathBytes.enumerated() { dest[i] = byte }
            }
        }
        let rc = withUnsafePointer(to: &addr) { ptr in
            ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                connect(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard rc == 0 else {
            throw IPCClientError.connectionFailed(errno: errno)
        }

        // Send
        let wire = try IPCWireFormat.encode(request)
        try writeAll(fd: fd, data: wire)

        // Read response header
        let header = try readExact(fd: fd, count: 4)
        guard let length = IPCWireFormat.readLength(from: header),
              length > 0, length < 1_048_576 else {
            throw IPCClientError.invalidResponse
        }

        // Read response payload
        let payload = try readExact(fd: fd, count: Int(length))
        return try IPCWireFormat.decode(IPCResponse.self, from: payload)
    }

    /// Sends a ping request to verify the daemon is alive and responding.
    ///
    /// - Returns: `true` if the daemon responded with ``IPCResponse/pong``; `false` otherwise.
    /// - Throws: Any error from ``send(_:)`` if the connection or I/O fails.
    public func ping() throws -> Bool {
        let response = try send(.ping)
        if case .pong = response { return true }
        return false
    }

    /// Queries the daemon for the current hydration state of a file.
    ///
    /// - Parameter path: The absolute filesystem path of the file to query.
    /// - Returns: The ``FileState`` reported by the daemon.
    /// - Throws: ``IPCClientError/unexpectedResponse`` if the daemon returns a non-state response,
    ///   or any error from ``send(_:)``.
    public func queryState(path: String) throws -> FileState {
        let response = try send(.queryState(path: path))
        if case .state(_, let state) = response { return state }
        throw IPCClientError.unexpectedResponse
    }

    /// Returns whether the daemon socket exists on disk and responds to a ping.
    ///
    /// This is a non-throwing convenience that returns `false` on any error,
    /// making it suitable for quick availability checks (e.g., in CLI status output).
    public var isAvailable: Bool {
        guard FileManager.default.fileExists(atPath: socketPath) else {
            return false
        }
        return (try? ping()) == true
    }

    // MARK: - Private

    private func readExact(fd: Int32, count: Int) throws -> Data {
        var buffer = Data(count: count)
        var offset = 0
        while offset < count {
            let n = buffer.withUnsafeMutableBytes { ptr in
                read(fd, ptr.baseAddress!.advanced(by: offset), count - offset)
            }
            guard n > 0 else { throw IPCClientError.readFailed }
            offset += n
        }
        return buffer
    }

    private func writeAll(fd: Int32, data: Data) throws {
        var offset = 0
        while offset < data.count {
            let n = data.withUnsafeBytes { ptr in
                write(fd, ptr.baseAddress!.advanced(by: offset),
                      data.count - offset)
            }
            guard n > 0 else { throw IPCClientError.writeFailed }
            offset += n
        }
    }
}

/// Errors produced by ``IPCClient`` during communication with the hydration daemon.
public enum IPCClientError: Error, CustomStringConvertible {
    /// The `connect()` system call to the daemon socket failed. The associated value is the `errno`.
    case connectionFailed(errno: Int32)
    /// A `read()` call returned zero or a negative value before the expected bytes were received.
    case readFailed
    /// A `write()` call returned zero or a negative value before all bytes were sent.
    case writeFailed
    /// The response frame header was missing, too short, or indicated an invalid length.
    case invalidResponse
    /// The daemon returned a valid response, but its type did not match the expected variant for the request.
    case unexpectedResponse

    /// A human-readable description of the error.
    public var description: String {
        switch self {
        case .connectionFailed(let e):
            "Connection failed: \(String(cString: strerror(e)))"
        case .readFailed:
            "Read failed"
        case .writeFailed:
            "Write failed"
        case .invalidResponse:
            "Invalid response from daemon"
        case .unexpectedResponse:
            "Unexpected response type"
        }
    }
}
