package dev.kagra.shell

import android.annotation.SuppressLint
import android.graphics.Color
import android.os.Bundle
import android.view.Choreographer
import android.view.MotionEvent
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.TextView
import androidx.appcompat.app.AppCompatActivity

/**
 * 共有コア（kagra-shared）が SurfaceView に直接描く。
 *
 * `libkagra_shared.so` が無い ABI では JNI がスタブになるので、
 * 描画は諦めて手順をテキストで出す。
 */
class MainActivity : AppCompatActivity(), SurfaceHolder.Callback {
    private lateinit var surfaceView: SurfaceView
    private lateinit var status: TextView
    private var rendering = false
    private var frameCallback: Choreographer.FrameCallback? = null

    @SuppressLint("ClickableViewAccessibility")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        surfaceView = SurfaceView(this)
        status = TextView(this).apply {
            textSize = 13f
            setPadding(32, 64, 32, 32)
            setTextColor(0xFFE8ECF4.toInt())
            text = "KAGRA starting…"
        }

        val root = FrameLayout(this).apply {
            setBackgroundColor(0xFF1C2230.toInt())
            addView(
                surfaceView,
                FrameLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.MATCH_PARENT
                )
            )
            addView(status)
        }
        setContentView(root)

        if (!KagraNative.available) {
            status.text = "kagra_jni could not be loaded — check the NDK build."
            return
        }
        if (!KagraNative.create()) {
            status.text = "native init failed: ${KagraNative.lastError()}"
            return
        }
        KagraNative.setAssetRoot(filesDir.absolutePath)
        surfaceView.holder.addCallback(this)
        surfaceView.setOnTouchListener { view, ev -> onTouch(view.width, view.height, ev) }
    }

    // ── Surface のライフサイクル ──────────────────────────

    override fun surfaceCreated(holder: SurfaceHolder) = Unit

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
        KagraNative.createSurface(width, height)
        rendering = KagraNative.attachSurface(holder.surface, width, height)
        if (!rendering) {
            status.text = buildString {
                append("kagra-shared ${KagraNative.version()}\n")
                append("renderer unavailable: ${KagraNative.lastError()}\n\n")
                append("Build the native lib with rendering:\n")
                append("  ./scripts/build_android_native.sh")
            }
            surfaceView.setBackgroundColor(Color.TRANSPARENT)
        } else {
            status.setBackgroundColor(Color.TRANSPARENT)
        }
        startLoop()
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        stopLoop()
        rendering = false
        KagraNative.detachSurface()
    }

    // ── フレームループ ───────────────────────────────────

    private fun startLoop() {
        if (frameCallback != null) return
        val cb = object : Choreographer.FrameCallback {
            override fun doFrame(frameTimeNanos: Long) {
                val frame = KagraNative.requestFrame()
                if (rendering && !KagraNative.render()) {
                    rendering = false
                    status.text = "render failed: ${KagraNative.lastError()}"
                }
                if (!rendering || frame % 15 == 0L) {
                    if (rendering) {
                        status.text = "kagra-shared ${KagraNative.version()}  frame=$frame"
                    }
                }
                Choreographer.getInstance().postFrameCallback(this)
            }
        }
        frameCallback = cb
        Choreographer.getInstance().postFrameCallback(cb)
    }

    private fun stopLoop() {
        frameCallback?.let { Choreographer.getInstance().removeFrameCallback(it) }
        frameCallback = null
    }

    // ── 入力 ─────────────────────────────────────────────

    private fun onTouch(width: Int, height: Int, ev: MotionEvent): Boolean {
        val phase = when (ev.actionMasked) {
            MotionEvent.ACTION_DOWN, MotionEvent.ACTION_POINTER_DOWN -> 0
            MotionEvent.ACTION_MOVE -> 1
            MotionEvent.ACTION_UP, MotionEvent.ACTION_POINTER_UP -> 2
            else -> 3
        }
        val idx = ev.actionIndex
        KagraNative.pushPointer(
            ev.getPointerId(idx),
            ev.getX(idx),
            ev.getY(idx),
            phase,
            ev.getPressure(idx)
        )
        // 画面中央を原点にした仮想スティック。離したら中立に戻す。
        if (phase == 2 || phase == 3) {
            KagraNative.setPad(0f, 0f)
        } else if (width > 0 && height > 0) {
            KagraNative.setPad(
                ((ev.x / width) * 2f - 1f).coerceIn(-1f, 1f),
                ((ev.y / height) * 2f - 1f).coerceIn(-1f, 1f)
            )
        }
        return true
    }

    override fun onPause() {
        super.onPause()
        if (KagraNative.available) KagraNative.pause()
    }

    override fun onResume() {
        super.onResume()
        if (KagraNative.available) KagraNative.resume()
    }

    override fun onDestroy() {
        stopLoop()
        if (KagraNative.available) KagraNative.destroy()
        super.onDestroy()
    }
}
