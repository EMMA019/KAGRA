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
    private enum class Control { STICK, JUMP }

    private lateinit var surfaceView: SurfaceView
    private lateinit var status: TextView
    private var rendering = false
    private var frameCallback: Choreographer.FrameCallback? = null

    /** 指 id ごとの役割。両手で同時に操作するために必要。 */
    private val controls = mutableMapOf<Int, Control>()
    private var stickX = 0f
    private var stickZ = 0f
    private var jump = false

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
        // Crest Isle（Kenney 風カプセル。VRM ではない）。運転デモは setScene(0)。
        KagraNative.setScene(2)
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
                if (rendering && frame % 15 == 0L) {
                    status.text = buildString {
                        append("kagra-shared ${KagraNative.version()}  frame=$frame\n")
                        append(KagraNative.statsJson())
                        append("\n\nCrest Isle — 左＝歩き / 右下＝ジャンプ（VRM ではない）")
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

    /**
     * 左半分＝仮想スティック（歩き）、右下＝ジャンプ。
     * 指ごとに役割を覚えるので、両手で同時に操作できる。
     */
    private fun onTouch(width: Int, height: Int, ev: MotionEvent): Boolean {
        if (width <= 0 || height <= 0) return true

        val idx = ev.actionIndex
        val phase = when (ev.actionMasked) {
            MotionEvent.ACTION_DOWN, MotionEvent.ACTION_POINTER_DOWN -> 0
            MotionEvent.ACTION_MOVE -> 1
            MotionEvent.ACTION_UP, MotionEvent.ACTION_POINTER_UP -> 2
            else -> 3
        }
        KagraNative.pushPointer(
            ev.getPointerId(idx), ev.getX(idx), ev.getY(idx), phase, ev.getPressure(idx)
        )

        val jumpLeft = width * 0.62f
        val jumpTop = height * 0.62f
        when (ev.actionMasked) {
            MotionEvent.ACTION_DOWN, MotionEvent.ACTION_POINTER_DOWN ->
                controls[ev.getPointerId(idx)] =
                    if (ev.getX(idx) >= jumpLeft && ev.getY(idx) >= jumpTop) {
                        Control.JUMP
                    } else {
                        Control.STICK
                    }

            MotionEvent.ACTION_UP, MotionEvent.ACTION_POINTER_UP,
            MotionEvent.ACTION_CANCEL -> {
                when (controls.remove(ev.getPointerId(idx))) {
                    Control.STICK -> { stickX = 0f; stickZ = 0f }
                    Control.JUMP -> jump = false
                    null -> Unit
                }
            }
        }

        jump = false
        for (i in 0 until ev.pointerCount) {
            when (controls[ev.getPointerId(i)]) {
                Control.STICK -> {
                    val well = minOf(width, height) * 0.22f
                    val cx = well
                    val cy = height - well
                    stickX = ((ev.getX(i) - cx) / well).coerceIn(-1f, 1f)
                    stickZ = (-(ev.getY(i) - cy) / well).coerceIn(-1f, 1f)
                }
                Control.JUMP -> jump = true
                null -> Unit
            }
        }

        KagraNative.setWalk(stickX, stickZ, jump)
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
