import Foundation
import XCTest
@testable import HydrationCore

// MARK: - FileState

final class FileStateTests: XCTestCase {
    func testValidTransitions() {
        XCTAssertTrue(FileState.unknown.canTransition(to: .evicted))
        XCTAssertTrue(FileState.unknown.canTransition(to: .local))
        XCTAssertTrue(FileState.unknown.canTransition(to: .error))
        XCTAssertFalse(FileState.unknown.canTransition(to: .downloading))

        XCTAssertTrue(FileState.evicted.canTransition(to: .downloading))
        XCTAssertTrue(FileState.evicted.canTransition(to: .local))
        XCTAssertTrue(FileState.evicted.canTransition(to: .error))
        XCTAssertFalse(FileState.evicted.canTransition(to: .unknown))

        XCTAssertTrue(FileState.downloading.canTransition(to: .local))
        XCTAssertTrue(FileState.downloading.canTransition(to: .error))
        XCTAssertTrue(FileState.downloading.canTransition(to: .evicted))
        XCTAssertFalse(FileState.downloading.canTransition(to: .unknown))

        XCTAssertTrue(FileState.local.canTransition(to: .evicted))
        XCTAssertTrue(FileState.local.canTransition(to: .error))
        XCTAssertFalse(FileState.local.canTransition(to: .downloading))
        XCTAssertFalse(FileState.local.canTransition(to: .unknown))

        XCTAssertTrue(FileState.error.canTransition(to: .evicted))
        XCTAssertTrue(FileState.error.canTransition(to: .downloading))
        XCTAssertTrue(FileState.error.canTransition(to: .unknown))
        XCTAssertFalse(FileState.error.canTransition(to: .local))
    }

    func testSelfTransitionInvalid() {
        for state in [FileState.unknown, .evicted, .downloading, .local, .error] {
            XCTAssertFalse(state.canTransition(to: state),
                           "\(state) should not transition to itself")
        }
    }

    func testCodable() throws {
        for state in [FileState.unknown, .evicted, .downloading, .local, .error] {
            let data = try JSONEncoder().encode(state)
            let decoded = try JSONDecoder().decode(FileState.self, from: data)
            XCTAssertEqual(state, decoded)
        }
    }
}

// MARK: - IPC Protocol

final class IPCProtocolTests: XCTestCase {
    func testPingRoundTrip() throws {
        let data = try IPCWireFormat.encode(IPCRequest.ping)
        let length = IPCWireFormat.readLength(from: data)!
        let payload = Data(data.dropFirst(4))
        XCTAssertEqual(Int(length), payload.count)

        let decoded = try IPCWireFormat.decode(IPCRequest.self, from: payload)
        if case .ping = decoded {} else { XCTFail("Expected .ping") }
    }

    func testQueryStateRoundTrip() throws {
        let req = IPCRequest.queryState(path: "/tmp/test.txt")
        let data = try IPCWireFormat.encode(req)
        let decoded = try IPCWireFormat.decode(
            IPCRequest.self, from: Data(data.dropFirst(4)))
        if case .queryState(let p) = decoded {
            XCTAssertEqual(p, "/tmp/test.txt")
        } else { XCTFail("Expected .queryState") }
    }

    func testHydrateRoundTrip() throws {
        let req = IPCRequest.hydrate(path: "/Users/test/file.pdf")
        let data = try IPCWireFormat.encode(req)
        let decoded = try IPCWireFormat.decode(
            IPCRequest.self, from: Data(data.dropFirst(4)))
        if case .hydrate(let p) = decoded {
            XCTAssertEqual(p, "/Users/test/file.pdf")
        } else { XCTFail("Expected .hydrate") }
    }

    func testPongRoundTrip() throws {
        let data = try IPCWireFormat.encode(IPCResponse.pong)
        let decoded = try IPCWireFormat.decode(
            IPCResponse.self, from: Data(data.dropFirst(4)))
        if case .pong = decoded {} else { XCTFail("Expected .pong") }
    }

    func testStateResponseRoundTrip() throws {
        let resp = IPCResponse.state(path: "/a/b", state: .evicted)
        let data = try IPCWireFormat.encode(resp)
        let decoded = try IPCWireFormat.decode(
            IPCResponse.self, from: Data(data.dropFirst(4)))
        if case .state(let p, let s) = decoded {
            XCTAssertEqual(p, "/a/b")
            XCTAssertEqual(s, .evicted)
        } else { XCTFail("Expected .state") }
    }

    func testHydrationResultRoundTrip() throws {
        let resp = IPCResponse.hydrationResult(
            path: "/x", success: false, error: "timeout")
        let data = try IPCWireFormat.encode(resp)
        let decoded = try IPCWireFormat.decode(
            IPCResponse.self, from: Data(data.dropFirst(4)))
        if case .hydrationResult(let p, let ok, let e) = decoded {
            XCTAssertEqual(p, "/x")
            XCTAssertFalse(ok)
            XCTAssertEqual(e, "timeout")
        } else { XCTFail("Expected .hydrationResult") }
    }

    func testReadLengthTooShort() {
        XCTAssertNil(IPCWireFormat.readLength(from: Data([0, 1])))
    }
}

// MARK: - HydrationManager (with mock detector)

struct MockDetector: FileStateDetecting {
    var stateMap: [String: FileState]
    func detectState(at url: URL) throws -> FileState {
        stateMap[url.path] ?? .unknown
    }
}

final class HydrationManagerTests: XCTestCase {
    func testRefreshState() async throws {
        let mgr = HydrationManager(
            detector: MockDetector(stateMap: ["/tmp/a": .local]))
        let state = try await mgr.refreshState(for: "/tmp/a")
        XCTAssertEqual(state, .local)
        let current = await mgr.currentState(for: "/tmp/a")
        XCTAssertEqual(current, .local)
    }

    func testTrackedCount() async throws {
        let mgr = HydrationManager(
            detector: MockDetector(stateMap: ["/a": .local, "/b": .evicted]))
        _ = try await mgr.refreshState(for: "/a")
        _ = try await mgr.refreshState(for: "/b")
        let count = await mgr.trackedCount
        XCTAssertEqual(count, 2)
    }

    func testStopTracking() async throws {
        let mgr = HydrationManager(
            detector: MockDetector(stateMap: ["/a": .local]))
        _ = try await mgr.refreshState(for: "/a")
        XCTAssertNotNil(await mgr.currentState(for: "/a"))
        await mgr.stopTracking("/a")
        XCTAssertNil(await mgr.currentState(for: "/a"))
    }

    func testHydrateAlreadyLocal() async throws {
        let mgr = HydrationManager(
            detector: MockDetector(stateMap: ["/local": .local]))
        let state = try await mgr.hydrate(path: "/local")
        XCTAssertEqual(state, .local)
    }

    func testHandleEvent() async throws {
        let mgr = HydrationManager(
            detector: MockDetector(stateMap: ["/f": .evicted]))
        let state = try await mgr.handleEvent(for: "/f")
        XCTAssertEqual(state, .evicted)
    }

    func testUnknownPath() async throws {
        let mgr = HydrationManager(
            detector: MockDetector(stateMap: [:]))
        let state = try await mgr.refreshState(for: "/missing")
        XCTAssertEqual(state, .unknown)
    }
}

// MARK: - Version

final class VersionTests: XCTestCase {
    func testVersion() {
        XCTAssertEqual(HydrationCore.version, "0.1.0")
    }

    func testDefaultSocketPath() {
        XCTAssertFalse(HydrationCore.defaultSocketPath.isEmpty)
    }
}
