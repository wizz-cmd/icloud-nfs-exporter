import Foundation

// MARK: - Request / Response

/// A request message sent from the FUSE driver (or any client) to the hydration daemon.
///
/// Encoded as JSON over the ``IPCWireFormat`` length-prefixed protocol.
/// The daemon dispatches each variant to the appropriate ``HydrationManager`` method.
public enum IPCRequest: Codable, Sendable {
    /// A health-check request. The daemon responds with ``IPCResponse/pong``.
    case ping
    /// Asks the daemon to detect and return the current ``FileState`` for the file at `path`.
    case queryState(path: String)
    /// Asks the daemon to ensure the file at `path` is fully downloaded from iCloud.
    case hydrate(path: String)

    private enum CodingKeys: String, CodingKey { case type, path }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        switch try c.decode(String.self, forKey: .type) {
        case "ping":
            self = .ping
        case "query_state":
            self = .queryState(path: try c.decode(String.self, forKey: .path))
        case "hydrate":
            self = .hydrate(path: try c.decode(String.self, forKey: .path))
        case let t:
            throw DecodingError.dataCorruptedError(
                forKey: .type, in: c,
                debugDescription: "Unknown request type: \(t)")
        }
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .ping:
            try c.encode("ping", forKey: .type)
        case .queryState(let path):
            try c.encode("query_state", forKey: .type)
            try c.encode(path, forKey: .path)
        case .hydrate(let path):
            try c.encode("hydrate", forKey: .type)
            try c.encode(path, forKey: .path)
        }
    }
}

/// A response message sent from the hydration daemon back to the requesting client.
///
/// Each variant corresponds to a specific ``IPCRequest``:
/// - ``pong`` answers ``IPCRequest/ping``
/// - ``state(path:state:)`` answers ``IPCRequest/queryState(path:)``
/// - ``hydrationResult(path:success:error:)`` answers ``IPCRequest/hydrate(path:)``
public enum IPCResponse: Codable, Sendable {
    /// Acknowledges a ``IPCRequest/ping`` request, confirming the daemon is alive.
    case pong
    /// Returns the detected ``FileState`` for the queried file.
    case state(path: String, state: FileState)
    /// Reports the outcome of a hydration request, including any error message on failure.
    case hydrationResult(path: String, success: Bool, error: String?)

    private enum CodingKeys: String, CodingKey {
        case type, path, state, success, error
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        switch try c.decode(String.self, forKey: .type) {
        case "pong":
            self = .pong
        case "state":
            self = .state(
                path: try c.decode(String.self, forKey: .path),
                state: try c.decode(FileState.self, forKey: .state))
        case "hydration_result":
            self = .hydrationResult(
                path: try c.decode(String.self, forKey: .path),
                success: try c.decode(Bool.self, forKey: .success),
                error: try c.decodeIfPresent(String.self, forKey: .error))
        case let t:
            throw DecodingError.dataCorruptedError(
                forKey: .type, in: c,
                debugDescription: "Unknown response type: \(t)")
        }
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .pong:
            try c.encode("pong", forKey: .type)
        case .state(let path, let state):
            try c.encode("state", forKey: .type)
            try c.encode(path, forKey: .path)
            try c.encode(state, forKey: .state)
        case .hydrationResult(let path, let success, let error):
            try c.encode("hydration_result", forKey: .type)
            try c.encode(path, forKey: .path)
            try c.encode(success, forKey: .success)
            try c.encodeIfPresent(error, forKey: .error)
        }
    }
}

// MARK: - Wire format

/// Length-prefixed JSON wire format for IPC messages.
///
/// Every message on the Unix domain socket is framed as a 4-byte big-endian
/// `UInt32` length prefix followed by exactly that many bytes of JSON payload.
/// This simple framing avoids delimiter-based parsing issues and supports
/// arbitrarily large messages (up to ~4 GB).
///
/// `IPCWireFormat` is a caseless enum used purely as a namespace for the
/// static `encode`, `readLength`, and `decode` functions.
public enum IPCWireFormat {
    /// Encodes a `Codable` value into the length-prefixed wire format.
    ///
    /// - Parameter value: The value to serialize as JSON.
    /// - Returns: A `Data` blob containing the 4-byte big-endian length header followed by the JSON payload.
    /// - Throws: An encoding error if the value cannot be serialized to JSON.
    public static func encode<T: Encodable>(_ value: T) throws -> Data {
        let json = try JSONEncoder().encode(value)
        var length = UInt32(json.count).bigEndian
        var data = Data(bytes: &length, count: 4)
        data.append(json)
        return data
    }

    /// Reads the 4-byte big-endian length prefix from the beginning of `data`.
    ///
    /// - Parameter data: Raw data that begins with the length header. Must contain at least 4 bytes.
    /// - Returns: The decoded payload length, or `nil` if `data` has fewer than 4 bytes.
    public static func readLength(from data: Data) -> UInt32? {
        guard data.count >= 4 else { return nil }
        return data.withUnsafeBytes { $0.load(as: UInt32.self).bigEndian }
    }

    /// Decodes a JSON payload into the specified `Decodable` type.
    ///
    /// - Parameter type: The type to decode into.
    /// - Parameter data: The raw JSON bytes (without the length prefix).
    /// - Returns: The decoded value.
    /// - Throws: A decoding error if the JSON cannot be parsed into `type`.
    public static func decode<T: Decodable>(_ type: T.Type, from data: Data) throws -> T {
        try JSONDecoder().decode(type, from: data)
    }
}
