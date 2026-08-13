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
    external fun requestFrame(): Long
    external fun statsJson(): String

    /** SurfaceView の Surface を描画先にする。 */
    external fun attachSurface(surface: Surface, width: Int, height: Int): Boolean
    external fun detachSurface()
    external fun hasRenderer(): Boolean

    /** 現在のシーンを 1 枚描く。[requestFrame] の後に呼ぶ。 */
    external fun render(): Boolean
}
