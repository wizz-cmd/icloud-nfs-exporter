import Foundation

// MARK: - Request / Response

/// Request from the FUSE driver to the hydration daemon.
public enum IPCRequest: Codable, Sendable {
    case ping
    case queryState(path: String)
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

/// Response from the hydration daemon to the FUSE driver.
public enum IPCResponse: Codable, Sendable {
    case pong
    case state(path: String, state: FileState)
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

/// Length-prefixed JSON wire format: 4-byte big-endian length + JSON payload.
public enum IPCWireFormat {
    public static func encode<T: Encodable>(_ value: T) throws -> Data {
        let json = try JSONEncoder().encode(value)
        var length = UInt32(json.count).bigEndian
        var data = Data(bytes: &length, count: 4)
        data.append(json)
        return data
    }

    public static func readLength(from data: Data) -> UInt32? {
        guard data.count >= 4 else { return nil }
        return data.withUnsafeBytes { $0.load(as: UInt32.self).bigEndian }
    }

    public static func decode<T: Decodable>(_ type: T.Type, from data: Data) throws -> T {
        try JSONDecoder().decode(type, from: data)
    }
}
