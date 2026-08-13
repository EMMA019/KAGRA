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

    /// 運転入力とシーン切り替えが C ABI を通ること。stub でも通る形にしてあるので、
    /// ヘッダとスタブが本体と食い違ったらリンクで落ちる。
    func testDriveInputCrossesTheABI() throws {
        let s = try XCTUnwrap(KagraSession())
        s.createSurface(width: 320, height: 240)
        s.setDrive(steer: 0.5, throttle: 1.0, brake: 0.0)
        XCTAssertTrue(s.setScene(.driving))
        XCTAssertTrue(s.setScene(.demo2D))
        for _ in 0..<10 { s.requestFrame() }
        XCTAssertTrue(s.statsJSON().contains("frame"))
    }
}
