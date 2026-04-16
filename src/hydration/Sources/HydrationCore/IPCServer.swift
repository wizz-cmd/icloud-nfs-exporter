import Foundation

/// Unix domain socket server that listens for hydration requests from the FUSE driver.
///
/// `IPCServer` binds a `SOCK_STREAM` Unix domain socket at the configured path,
/// accepts incoming connections, reads ``IPCRequest`` messages using the
/// ``IPCWireFormat`` framing protocol, dispatches them to ``HydrationManager``,
/// and writes back ``IPCResponse`` messages. Each client connection is handled
/// in its own `Task`, so multiple FUSE requests can be served concurrently.
///
/// The socket is created with mode `0600` (owner-only access) and is automatically
/// unlinked when the server starts (to remove stale sockets) and when it stops.
public final class IPCServer: @unchecked Sendable {
    private let socketPath: String
    private let manager: HydrationManager
    private var serverFD: Int32 = -1
    private var acceptSource: DispatchSourceRead?
    private let queue = DispatchQueue(label: "ipc-server", qos: .userInitiated)

    /// Creates a new IPC server.
    ///
    /// - Parameter socketPath: The filesystem path for the Unix domain socket.
    /// - Parameter manager: The ``HydrationManager`` that handles incoming requests.
    public init(socketPath: String, manager: HydrationManager) {
        self.socketPath = socketPath
        self.manager = manager
    }

    deinit { stop() }

    /// Binds the socket, starts listening, and begins accepting client connections.
    ///
    /// Any existing socket file at the configured path is removed before binding.
    /// Connections are accepted asynchronously via a `DispatchSource` and each
    /// client is handled in a separate `Task`.
    ///
    /// - Throws: ``IPCServerError/socketCreation(errno:)`` if the socket cannot be created,
    ///   ``IPCServerError/pathTooLong`` if the socket path exceeds the `sockaddr_un` limit,
    ///   ``IPCServerError/bind(errno:)`` if binding fails, or
    ///   ``IPCServerError/listen(errno:)`` if the listen call fails.
    public func start() throws {
        unlink(socketPath)

        serverFD = socket(AF_UNIX, SOCK_STREAM, 0)
        guard serverFD >= 0 else {
            throw IPCServerError.socketCreation(errno: errno)
        }

        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        let pathBytes = socketPath.utf8CString
        guard pathBytes.count <= MemoryLayout.size(ofValue: addr.sun_path) else {
            Darwin.close(serverFD)
            throw IPCServerError.pathTooLong
        }
        withUnsafeMutablePointer(to: &addr.sun_path) { ptr in
            ptr.withMemoryRebound(to: CChar.self, capacity: pathBytes.count) { dest in
                for (i, byte) in pathBytes.enumerated() { dest[i] = byte }
            }
        }

        let addrLen = socklen_t(MemoryLayout<sockaddr_un>.size)
        let rc = withUnsafePointer(to: &addr) { ptr in
            ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                bind(serverFD, $0, addrLen)
            }
        }
        guard rc == 0 else {
            Darwin.close(serverFD)
            throw IPCServerError.bind(errno: errno)
        }

        chmod(socketPath, 0o600)

        guard listen(serverFD, 5) == 0 else {
            Darwin.close(serverFD)
            throw IPCServerError.listen(errno: errno)
        }

        let source = DispatchSource.makeReadSource(
            fileDescriptor: serverFD, queue: queue)
        source.setEventHandler { [weak self] in self?.acceptConnection() }
        source.setCancelHandler { [weak self] in
            if let fd = self?.serverFD, fd >= 0 { Darwin.close(fd) }
        }
        acceptSource = source
        source.resume()
    }

    /// Stops accepting connections, closes the server socket, and removes the socket file.
    ///
    /// It is safe to call `stop()` multiple times; subsequent calls are no-ops.
    /// This method is also called automatically from `deinit`.
    public func stop() {
        acceptSource?.cancel()
        acceptSource = nil
        if serverFD >= 0 {
            Darwin.close(serverFD)
            serverFD = -1
        }
        unlink(socketPath)
    }

    // MARK: - Private

    private func acceptConnection() {
        let clientFD = accept(serverFD, nil, nil)
        guard clientFD >= 0 else { return }
        Task { [manager] in
            defer { Darwin.close(clientFD) }
            await Self.handleClient(fd: clientFD, manager: manager)
        }
    }

    private static func handleClient(
        fd: Int32, manager: HydrationManager
    ) async {
        guard let header = readExact(fd: fd, count: 4) else { return }
        let length = header.withUnsafeBytes {
            $0.load(as: UInt32.self).bigEndian
        }
        guard length > 0, length < 1_048_576 else { return }

        guard let payload = readExact(fd: fd, count: Int(length)) else {
            return
        }
        guard let request = try? IPCWireFormat.decode(
            IPCRequest.self, from: payload
        ) else {
            let err = IPCResponse.hydrationResult(
                path: "", success: false, error: "invalid request")
            if let data = try? IPCWireFormat.encode(err) {
                writeAll(fd: fd, data: data)
            }
            return
        }

        let response: IPCResponse
        switch request {
        case .ping:
            response = .pong

        case .queryState(let path):
            let state = (try? await manager.refreshState(for: path)) ?? .unknown
            response = .state(path: path, state: state)

        case .hydrate(let path):
            do {
                let result = try await manager.hydrate(path: path)
                response = .hydrationResult(
                    path: path, success: result == .local, error: nil)
            } catch {
                response = .hydrationResult(
                    path: path, success: false, error: "\(error)")
            }
        }

        if let data = try? IPCWireFormat.encode(response) {
            writeAll(fd: fd, data: data)
        }
    }

    private static func readExact(fd: Int32, count: Int) -> Data? {
        var buffer = Data(count: count)
        var offset = 0
        while offset < count {
            let n = buffer.withUnsafeMutableBytes { ptr in
                read(fd, ptr.baseAddress!.advanced(by: offset), count - offset)
            }
            if n <= 0 { return nil }
            offset += n
        }
        return buffer
    }

    private static func writeAll(fd: Int32, data: Data) {
        var offset = 0
        while offset < data.count {
            let n = data.withUnsafeBytes { ptr in
                write(fd, ptr.baseAddress!.advanced(by: offset),
                      data.count - offset)
            }
            if n <= 0 { return }
            offset += n
        }
    }
}

/// Errors produced by ``IPCServer`` during socket setup.
public enum IPCServerError: Error {
    /// The `socket()` system call failed. The associated value is the `errno`.
    case socketCreation(errno: Int32)
    /// The socket path exceeds the maximum length allowed by `sockaddr_un`.
    case pathTooLong
    /// The `bind()` system call failed. The associated value is the `errno`.
    case bind(errno: Int32)
    /// The `listen()` system call failed. The associated value is the `errno`.
    case listen(errno: Int32)
}
