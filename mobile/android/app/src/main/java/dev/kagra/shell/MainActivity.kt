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
    private enum class Control { STEER, PEDAL }

    private lateinit var surfaceView: SurfaceView
    private lateinit var status: TextView
    private var rendering = false
    private var frameCallback: Choreographer.FrameCallback? = null

    /** 指 id ごとの役割。両手で同時に操作するために必要。 */
    private val controls = mutableMapOf<Int, Control>()
    private var steer = 0f
    private var throttle = 0f
    private var brake = 0f

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
                if (rendering && frame % 15 == 0L) {
                    status.text = buildString {
                        append("kagra-shared ${KagraNative.version()}  frame=$frame\n")
                        append(KagraNative.statsJson())
                        append("\n\n左半分＝ハンドル / 右半分＝上でアクセル・下でブレーキ")
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
     * 画面の下半分を左右に割り、左＝ハンドル、右＝アクセル／ブレーキにする。
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

        when (ev.actionMasked) {
            MotionEvent.ACTION_DOWN, MotionEvent.ACTION_POINTER_DOWN ->
                controls[ev.getPointerId(idx)] =
                    if (ev.getX(idx) < width / 2f) Control.STEER else Control.PEDAL

            MotionEvent.ACTION_UP, MotionEvent.ACTION_POINTER_UP,
            MotionEvent.ACTION_CANCEL -> {
                when (controls.remove(ev.getPointerId(idx))) {
                    Control.STEER -> steer = 0f
                    Control.PEDAL -> { throttle = 0f; brake = 0f }
                    null -> Unit
                }
            }
        }

        // 押されている指をすべて見て、役割ごとに最新値へ更新する。
        for (i in 0 until ev.pointerCount) {
            when (controls[ev.getPointerId(i)]) {
                Control.STEER -> {
                    // 左半分の中心からの左右のずれを切れ角にする。
                    val half = width / 2f
                    steer = ((ev.getX(i) / half) * 2f - 1f).coerceIn(-1f, 1f)
                }
                Control.PEDAL -> {
                    // 上半分がアクセル、下半分がブレーキ。
                    val t = ((ev.getY(i) / height) * 2f - 1f).coerceIn(-1f, 1f)
                    throttle = if (t < 0) -t else 0f
                    brake = if (t > 0) t else 0f
                }
                null -> Unit
            }
        }

        KagraNative.setDrive(steer, throttle, brake)
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
