import Foundation

/// Client for connecting to the hydration daemon's IPC socket.
public final class IPCClient: @unchecked Sendable {
    public let socketPath: String
    private let timeout: TimeInterval

    public init(
        socketPath: String = HydrationCore.defaultSocketPath,
        timeout: TimeInterval = 10
    ) {
        self.socketPath = socketPath
        self.timeout = timeout
    }

    /// Send a request and return the response.
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

    /// Health check.
    public func ping() throws -> Bool {
        let response = try send(.ping)
        if case .pong = response { return true }
        return false
    }

    /// Query file state.
    public func queryState(path: String) throws -> FileState {
        let response = try send(.queryState(path: path))
        if case .state(_, let state) = response { return state }
        throw IPCClientError.unexpectedResponse
    }

    /// Check if the daemon socket exists and responds.
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

public enum IPCClientError: Error, CustomStringConvertible {
    case connectionFailed(errno: Int32)
    case readFailed
    case writeFailed
    case invalidResponse
    case unexpectedResponse

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
