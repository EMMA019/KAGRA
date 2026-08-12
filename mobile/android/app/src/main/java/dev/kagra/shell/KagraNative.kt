package dev.kagra.shell

object KagraNative {
    init {
        try {
            System.loadLibrary("kagra_jni")
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
}
