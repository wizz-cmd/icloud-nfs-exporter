import XCTest
@testable import HydrationCore

final class HydrationTests: XCTestCase {
    func testVersion() {
        XCTAssertEqual(HydrationCore.version, "0.1.0")
    }
}
