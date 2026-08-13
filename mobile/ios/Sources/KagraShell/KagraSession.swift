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

    /// 連続値のドライバ入力。`steer` は -1..1、`throttle` と `brake` は 0..1。
    public func setDrive(steer: Float, throttle: Float, brake: Float) {
        guard let ptr else { return }
        _ = kagra_shared_set_drive(ptr, steer, throttle, brake)
    }

    public enum Scene: UInt32 {
        case driving = 0
        case demo2D = 1
    }

    @discardableResult
    public func setScene(_ scene: Scene) -> Bool {
        guard let ptr else { return false }
        return kagra_shared_set_scene(ptr, scene.rawValue) == 0
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

    /// セーブ JSON。アプリ側が Documents 等へ書き出す。
    public func saveJSON() -> String? {
        guard let ptr else { return nil }
        var need = kagra_shared_save_json(ptr, nil, 0)
        if need <= 0 { return nil }
        var buf = [CChar](repeating: 0, count: Int(need))
        need = kagra_shared_save_json(ptr, &buf, UInt32(buf.count))
        guard need > 0 else { return nil }
        return String(cString: buf)
    }

    @discardableResult
    public func loadJSON(_ json: String) -> Bool {
        guard let ptr else { return false }
        return json.withCString { kagra_shared_load_json(ptr, $0) == 0 }
    }

    public func setSettings(masterVolume: Float, steerSensitivity: Float, muted: Bool) {
        guard let ptr else { return }
        _ = kagra_shared_set_settings(ptr, masterVolume, steerSensitivity, muted ? 1 : 0)
    }

    /// 音声レベル JSON（engine / wind / brake）。再生は AVAudioEngine 側。
    public func audioJSON() -> String {
        guard let ptr else { return "{}" }
        var buf = [CChar](repeating: 0, count: 256)
        _ = kagra_shared_audio_json(ptr, &buf, 256)
        return String(cString: buf)
    }
}
