import XCTest
@testable import KagraShell

final class KagraShellTests: XCTestCase {
    func testSessionFrameAdvances() {
        let s = KagraSession()
        XCTAssertNotNil(s)
        s?.createSurface(width: 100, height: 100)
        let a = s?.requestFrame() ?? 0
        let b = s?.requestFrame() ?? 0
        XCTAssertGreaterThan(b, a)
        XCTAssertTrue(s?.version.isEmpty == false)
    }
}
