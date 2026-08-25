package dev.kagra.shell

import android.view.Surface

object KagraNative {
    /** JNI ライブラリが読めたか。false なら以降の external 呼び出しはしない。 */
    var available: Boolean = false
        private set

    init {
        try {
            System.loadLibrary("kagra_jni")
            available = true
        } catch (e: UnsatisfiedLinkError) {
            // CMake が libkagra_shared をリンクできない環境でも UI は起動可能に
        }
    }

    external fun version(): String
    external fun lastError(): String
    external fun create(): Boolean
    external fun destroy()
    external fun createSurface(width: Int, height: Int): Boolean
    external fun setAssetRoot(root: String): Boolean
    external fun pause(): Boolean
    external fun resume(): Boolean
    external fun pushPointer(id: Int, x: Float, y: Float, phase: Int, pressure: Float): Boolean
    external fun setPad(x: Float, y: Float): Boolean

    /** 連続値のドライバ入力。steer は -1..1、throttle と brake は 0..1。 */
    external fun setDrive(steer: Float, throttle: Float, brake: Float): Boolean

    /** 0 = 運転、1 = 2D、2 = Crest Isle。 */
    external fun setScene(kind: Int): Boolean

    /** Crest Isle の歩き。lx/lz は -1..1。 */
    external fun setWalk(lx: Float, lz: Float, jump: Boolean): Boolean

    external fun setJump(held: Boolean): Boolean

    external fun requestFrame(): Long
    external fun statsJson(): String

    /** セーブ JSON。アプリ側が filesDir 等へ書き出す。 */
    external fun saveJson(): String

    /** セーブ JSON を読み込んで状態を復元する。 */
    external fun loadJson(json: String): Boolean

    external fun setSettings(masterVolume: Float, steerSensitivity: Float, muted: Boolean): Boolean

    /** 音声レベル JSON（engine / wind / brake）。再生は AudioTrack 側。 */
    external fun audioJson(): String

    /** SurfaceView の Surface を描画先にする。 */
    external fun attachSurface(surface: Surface, width: Int, height: Int): Boolean
    external fun detachSurface()
    external fun hasRenderer(): Boolean

    /** 現在のシーンを 1 枚描く。[requestFrame] の後に呼ぶ。 */
    external fun render(): Boolean
}
