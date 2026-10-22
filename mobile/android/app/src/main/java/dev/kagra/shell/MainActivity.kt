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
import java.io.File
import java.io.IOException
import org.json.JSONObject

/**
 * 共有コア（kagra-shared）が SurfaceView に直接描く。
 *
 * `assets/kagra_root/kagra_game.json` があるときは、そのゲームの world dump を
 * WorldPlay で進める。無いときは従来どおり Crest Isle（setScene(2)）。
 * `libkagra_shared.so` が無い ABI では JNI がスタブになるので、描画は諦めて
 * 手順をテキストで出す。
 */
class MainActivity : AppCompatActivity(), SurfaceHolder.Callback {
    private enum class Control { STICK, JUMP, ATTACK }

    private lateinit var surfaceView: SurfaceView
    private lateinit var status: TextView
    private var rendering = false
    private var frameCallback: Choreographer.FrameCallback? = null
    private var packaged = false
    private var holdStatus = false
    private var gameTitle = "KAGRA Shell"
    private var audio: AudioBus? = null

    /** 指 id ごとの役割。両手で同時に操作するために必要。 */
    private val controls = mutableMapOf<Int, Control>()
    private var stickX = 0f
    private var stickZ = 0f
    private var jump = false
    private var attack = false

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
        bootGame()
        surfaceView.holder.addCallback(this)
        surfaceView.setOnTouchListener { view, ev -> onTouch(view.width, view.height, ev) }
    }

    /**
     * マニフェスト付き APK なら world dump を展開して WorldPlay を起動する。
     * マニフェストが無ければ Crest Isle。
     */
    private fun bootGame() {
        KagraNative.setSaveRoot(File(filesDir, "kagra").absolutePath)
        val manifest = readAssetText("kagra_root/kagra_game.json")
        if (manifest == null) {
            KagraNative.setAssetRoot(filesDir.absolutePath)
            KagraNative.setScene(2)
            gameTitle = "Crest Isle"
            return
        }
        gameTitle = parseTitle(manifest)
        val extracted = File(filesDir, "kagra_root")
        if (!sameManifest(extracted, manifest)) {
            extracted.deleteRecursively()
            copyAssetTree("kagra_root", extracted)
        }
        KagraNative.setAssetRoot(extracted.absolutePath)
        val world = File(extracted, "world.json")
        if (!world.isFile) {
            packaged = false
            holdStatus = true
            KagraNative.pause()
            status.text = buildString {
                append(gameTitle)
                append("\n\n")
                append("このゲームのルールは Python 側にあります。")
                append("Android で WorldPlay を動かすにはマニフェストの world が必要です。")
                append("Windows の exe が入口の例をそのまま起動します。")
            }
            return
        }
        packaged = KagraNative.loadGameFile(world.absolutePath)
        if (packaged) {
            audio = AudioBus(extracted)
        }
        if (!packaged) {
            status.text = "$gameTitle\n\nnative load failed: ${KagraNative.lastError()}"
        }
    }

    private fun parseTitle(json: String): String {
        return try {
            JSONObject(json).optString("title", gameTitle).ifBlank { gameTitle }
        } catch (_: Exception) {
            gameTitle
        }
    }

    private fun sameManifest(extracted: File, manifest: String): Boolean {
        val existing = File(extracted, "kagra_game.json")
        if (!existing.isFile) return false
        return existing.readText() == manifest
    }

    private fun readAssetText(path: String): String? {
        return try {
            assets.open(path).bufferedReader().use { it.readText() }
        } catch (_: IOException) {
            null
        }
    }

    private fun copyAssetTree(assetPath: String, dest: File) {
        val children = assets.list(assetPath) ?: emptyArray()
        if (children.isEmpty()) {
            dest.parentFile?.mkdirs()
            try {
                assets.open(assetPath).use { input ->
                    dest.outputStream().use { output -> input.copyTo(output) }
                }
            } catch (_: IOException) {
                dest.mkdirs()
            }
            return
        }
        dest.mkdirs()
        for (child in children) {
            copyAssetTree("$assetPath/$child", File(dest, child))
        }
    }

    // ── Surface のライフサイクル ──────────────────────────

    override fun surfaceCreated(holder: SurfaceHolder) = Unit

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
        KagraNative.createSurface(width, height)
        rendering = KagraNative.attachSurface(holder.surface, width, height)
        if (!rendering) {
            status.text = buildString {
                append(gameTitle)
                append("\nkagra-shared ${KagraNative.version()}\n")
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
                if (packaged) {
                    KagraNative.setPlayInput(stickX, stickZ, jump, attack, false)
                } else {
                    KagraNative.setWalk(stickX, stickZ, jump)
                }
                val frame = KagraNative.requestFrame()
                if (packaged) {
                    try {
                        audio?.apply(KagraNative.takeAudioCues())
                    } catch (_: Exception) {
                    }
                }
                if (rendering && !KagraNative.render()) {
                    rendering = false
                    status.text = "render failed: ${KagraNative.lastError()}"
                }
                if (!holdStatus && rendering && frame % 15 == 0L) {
                    status.text = buildString {
                        append(gameTitle)
                        append("  ·  kagra-shared ${KagraNative.version()}  frame=$frame\n")
                        append(KagraNative.statsJson())
                        if (packaged) {
                            append("\n\n左＝移動 / 右下＝ジャンプ / 右上＝アクション。タップで開始")
                        } else {
                            append("\n\nCrest Isle — 左＝歩き / 右下＝ジャンプ（VRM ではない）")
                        }
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
     * 左半分＝仮想スティック（歩き）、右下＝ジャンプ、右上＝アクション。
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
                controls[ev.getPointerId(idx)] = controlFor(ev.getX(idx), ev.getY(idx), jumpLeft, jumpTop)

            MotionEvent.ACTION_UP, MotionEvent.ACTION_POINTER_UP,
            MotionEvent.ACTION_CANCEL -> {
                when (controls.remove(ev.getPointerId(idx))) {
                    Control.STICK -> { stickX = 0f; stickZ = 0f }
                    Control.JUMP -> jump = false
                    Control.ATTACK -> attack = false
                    null -> Unit
                }
            }
        }

        jump = false
        attack = false
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
                Control.ATTACK -> attack = true
                null -> Unit
            }
        }

        if (packaged) {
            KagraNative.setPlayInput(stickX, stickZ, jump, attack, false)
        } else {
            KagraNative.setWalk(stickX, stickZ, jump)
        }
        return true
    }

    private fun controlFor(x: Float, y: Float, jumpLeft: Float, jumpTop: Float): Control {
        if (x >= jumpLeft && y >= jumpTop) return Control.JUMP
        if (x >= jumpLeft) return Control.ATTACK
        return Control.STICK
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
        audio?.release()
        audio = null
        if (KagraNative.available) KagraNative.destroy()
        super.onDestroy()
    }
}
