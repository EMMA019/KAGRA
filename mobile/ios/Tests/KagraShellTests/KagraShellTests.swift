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

    /// stub リンクでは描画できない。UI がフォールバックを出せるよう、
    /// 失敗が戻り値と lastError で分かることを担保する。
    func testRenderingUnavailableWithStub() throws {
        let s = try XCTUnwrap(KagraSession())
        guard !s.hasRenderer else {
            // 本物の libkagra_shared.a をリンクしたビルド。
            XCTAssertTrue(s.render() || !s.lastError.isEmpty)
            return
        }
        XCTAssertFalse(s.render())
        XCTAssertFalse(s.lastError.isEmpty, "a failed render must explain itself")
    }
}
