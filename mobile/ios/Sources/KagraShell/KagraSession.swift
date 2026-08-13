import KagraSharedC
import Foundation

public final class KagraSession {
    private var ptr: OpaquePointer?

    public init?() {
        ptr = kagra_shared_create()
        if ptr == nil { return nil }
    }

    deinit {
        if let ptr { kagra_shared_destroy(ptr) }
    }

    public var version: String {
        String(cString: kagra_shared_version())
    }

    public var lastError: String {
        String(cString: kagra_shared_last_error())
    }

    /// 描画が有効なビルドかどうか。stub リンク時は常に false。
    public var hasRenderer: Bool {
        guard let ptr else { return false }
        return kagra_shared_has_renderer(ptr) == 1
    }

    /// `UIView` を描画先にする。成功したら以降 `render()` が絵を出す。
    @discardableResult
    public func attach(view: UnsafeMutableRawPointer, width: UInt32, height: UInt32) -> Bool {
        guard let ptr else { return false }
        return kagra_shared_attach_ios_view(ptr, view, width, height) == 0
    }

    public func detachSurface() {
        guard let ptr else { return }
        _ = kagra_shared_detach_surface(ptr)
    }

    /// 現在のシーンを 1 枚描く。`requestFrame()` の後に呼ぶ。
    @discardableResult
    public func render() -> Bool {
        guard let ptr else { return false }
        return kagra_shared_render(ptr) == 0
    }

    public func createSurface(width: UInt32, height: UInt32) {
        guard let ptr else { return }
        _ = kagra_shared_create_surface(ptr, width, height)
    }

    public func setAssetRoot(_ root: String) {
        guard let ptr else { return }
        root.withCString { _ = kagra_shared_set_asset_root(ptr, $0) }
    }

    public func pause() { if let ptr { _ = kagra_shared_pause(ptr) } }
    public func resume() { if let ptr { _ = kagra_shared_resume(ptr) } }

    public func pushPointer(id: UInt32, x: Float, y: Float, phase: UInt32, pressure: Float = 1) {
        guard let ptr else { return }
        _ = kagra_shared_push_pointer(ptr, id, x, y, phase, pressure)
    }

    public func setPad(x: Float, y: Float) {
        guard let ptr else { return }
        _ = kagra_shared_set_pad(ptr, x, y)
    }

    @discardableResult
    public func requestFrame() -> Int64 {
        guard let ptr else { return -1 }
        return kagra_shared_request_frame(ptr)
    }

    public func statsJSON() -> String {
        guard let ptr else { return "{}" }
        var buf = [CChar](repeating: 0, count: 512)
        _ = kagra_shared_stats_json(ptr, &buf, 512)
        return String(cString: buf)
    }
}
