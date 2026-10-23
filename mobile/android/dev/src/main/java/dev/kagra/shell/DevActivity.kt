package dev.kagra.shell

import android.annotation.SuppressLint
import android.graphics.Color
import android.graphics.Typeface
import android.os.Bundle
import android.view.Choreographer
import android.view.Gravity
import android.view.MotionEvent
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.View
import android.view.ViewGroup
import android.widget.Button
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import androidx.appcompat.app.AppCompatActivity
import java.io.File
import java.net.HttpURLConnection
import java.net.URL
import java.net.URLEncoder
import kotlin.concurrent.thread
import org.json.JSONObject

/**
 * デバッグ専用。ゲームは同梱しない。
 *
 * 起動画面で master か PR 番号を選び、GitHub Pages のプレビューから
 * world / assets / manifest を落として WorldPlay する。再読込ボタンか、
 * 上のバーを下に引くと、同じ URL を取り直す。ケーブルは最初のインストールだけ。
 */
class DevActivity : AppCompatActivity(), SurfaceHolder.Callback {
    private enum class Control { STICK, JUMP, ATTACK }

    private lateinit var surfaceView: SurfaceView
    private lateinit var status: TextView
    private lateinit var bar: LinearLayout
    private lateinit var picker: ScrollView
    private lateinit var baseField: EditText
    private lateinit var prField: EditText
    private lateinit var slugField: EditText
    private lateinit var list: LinearLayout
    private lateinit var pickerStatus: TextView

    private var rendering = false
    private var frameCallback: Choreographer.FrameCallback? = null
    private var packaged = false
    private var busy = false
    private var prMode = false
    private var loadGen = 0
    private var audio: AudioBus? = null
    private var gameTitle = "KAGRA Dev"

    private val controls = mutableMapOf<Int, Control>()
    private var stickX = 0f
    private var stickZ = 0f
    private var jump = false
    private var attack = false
    private var pullStartY = 0f
    private var pulled = false

    @SuppressLint("ClickableViewAccessibility", "SetTextI18n")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val prefs = prefs()

        surfaceView = SurfaceView(this)
        status = TextView(this).apply {
            setTextColor(Color.WHITE)
            textSize = 12f
            setPadding(dp(12), dp(8), dp(12), dp(8))
        }
        val reload = Button(this).apply {
            text = "再読込"
            setOnClickListener { beginLoad() }
        }
        bar = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setBackgroundColor(0xCC1C2230.toInt())
            setPadding(dp(12), dp(8), dp(8), dp(8))
            addView(status, LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f))
            addView(reload)
        }
        status.setOnTouchListener { _, ev ->
            onPull(ev)
            true
        }

        baseField = field(prefs.getString("pages_base", BuildConfig.PAGES_BASE) ?: BuildConfig.PAGES_BASE)
        baseField.hint = "https://emma019.github.io/KAGRA"
        prField = field(prefs.getString("pr", "") ?: "")
        prField.hint = "PR 番号"
        prField.inputType = android.text.InputType.TYPE_CLASS_NUMBER
        slugField = field(prefs.getString("slug", "platformer") ?: "platformer")
        slugField.hint = "slug（例: platformer）"
        prMode = prefs.getBoolean("pr_mode", false)

        val masterBtn = Button(this).apply { text = "master" }
        val prBtn = Button(this).apply { text = "PR" }
        fun paintMode() {
            masterBtn.setTypeface(null, if (prMode) Typeface.NORMAL else Typeface.BOLD)
            prBtn.setTypeface(null, if (prMode) Typeface.BOLD else Typeface.NORMAL)
            prField.isEnabled = prMode
        }
        masterBtn.setOnClickListener { prMode = false; paintMode() }
        prBtn.setOnClickListener { prMode = true; paintMode() }
        paintMode()

        pickerStatus = TextView(this).apply {
            setTextColor(0xFFE8ECF4.toInt())
            text = "master か PR 番号を選んで、ゲームを読みます。"
        }
        list = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }

        val form = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(dp(16), dp(20), dp(16), dp(24))
            addView(heading("KAGRA Dev"))
            addView(note("デバッグ専用。Pages の最新バンドルを落として遊べます。"))
            addView(label("Pages の URL"))
            addView(baseField)
            val row = LinearLayout(this@DevActivity).apply {
                orientation = LinearLayout.HORIZONTAL
                addView(masterBtn, LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f))
                addView(prBtn, LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f))
            }
            addView(row)
            addView(prField)
            addView(label("ゲーム"))
            addView(slugField)
            addView(Button(this@DevActivity).apply {
                text = "一覧を読む"
                setOnClickListener { fetchCatalog() }
            })
            addView(Button(this@DevActivity).apply {
                text = "このゲームを読み込む"
                setOnClickListener { beginLoad() }
            })
            addView(pickerStatus)
            addView(list)
        }
        picker = ScrollView(this).apply {
            setBackgroundColor(0xFF1C2230.toInt())
            addView(form)
        }

        val root = FrameLayout(this).apply {
            setBackgroundColor(Color.BLACK)
            addView(surfaceView, FrameLayout.LayoutParams(MATCH, MATCH))
            addView(bar, FrameLayout.LayoutParams(MATCH, ViewGroup.LayoutParams.WRAP_CONTENT).apply {
                topMargin = dp(28)
            })
            addView(picker, FrameLayout.LayoutParams(MATCH, MATCH))
        }
        setContentView(root)
        surfaceView.holder.addCallback(this)
        surfaceView.setOnTouchListener { _, ev -> onGameTouch(surfaceView.width, surfaceView.height, ev) }

        if (!KagraNative.available) {
            val detail = KagraNative.loadError
            pickerStatus.text = if (detail.isNullOrBlank()) {
                "kagra_jni を読めません。NDK ビルドを確認してください。"
            } else {
                "kagra_jni を読めません。NDK ビルドを確認してください。\n$detail"
            }
            return
        }
        if (!KagraNative.create()) {
            pickerStatus.text = "native init failed: ${KagraNative.lastError()}"
            return
        }
        KagraNative.setSaveRoot(File(filesDir, "kagra").absolutePath)
        status.text = "KAGRA Dev"
    }

    private fun beginLoad() {
        if (busy) return
        if (!KagraNative.available) {
            pickerStatus.text = "kagra_jni を読めません。"
            return
        }
        val root = try {
            channelRoot()
        } catch (e: Exception) {
            pickerStatus.text = e.message
            picker.visibility = View.VISIBLE
            return
        }
        val slug = slugField.text.toString().trim().ifBlank { "platformer" }
        slugField.setText(slug)
        remember()
        busy = true
        pickerStatus.text = "ダウンロード中… $root"
        status.text = "$gameTitle  ダウンロード中"
        audio?.release()
        audio = null
        val gen = ++loadGen
        thread(name = "kagra-dev-fetch") {
            try {
                val dir = downloadBundle(root, slug)
                runOnUiThread {
                    if (gen != loadGen || isDestroyed) return@runOnUiThread
                    stopLoop()
                    val current = swapIn(dir)
                    boot(current)
                    busy = false
                    picker.visibility = View.GONE
                    if (surfaceView.holder.surface?.isValid == true) startLoop()
                }
            } catch (e: Exception) {
                runOnUiThread {
                    if (gen != loadGen || isDestroyed) return@runOnUiThread
                    busy = false
                    picker.visibility = View.VISIBLE
                    pickerStatus.text = e.message ?: "download failed"
                    status.text = "再読込に失敗しました"
                }
            }
        }
    }

    private fun fetchCatalog() {
        if (busy) return
        val root = try {
            channelRoot()
        } catch (e: Exception) {
            pickerStatus.text = e.message
            return
        }
        remember()
        busy = true
        pickerStatus.text = "一覧を読んでいます…"
        thread(name = "kagra-dev-catalog") {
            try {
                val text = String(httpGet(urlJoin(root, "catalog.json")), Charsets.UTF_8)
                val games = JSONObject(text).getJSONArray("games")
                val rows = mutableListOf<Pair<String, String>>()
                for (i in 0 until games.length()) {
                    val obj = games.getJSONObject(i)
                    if (!obj.optBoolean("browser", false)) continue
                    val slug = obj.optString("slug")
                    val title = obj.optString("title", slug)
                    if (slug.isNotBlank()) rows.add(slug to title)
                }
                runOnUiThread {
                    busy = false
                    list.removeAllViews()
                    if (rows.isEmpty()) {
                        pickerStatus.text = "ブラウザで遊べるゲームがありません。"
                        return@runOnUiThread
                    }
                    pickerStatus.text = "${rows.size} 件。タップすると読み込みます。"
                    for ((slug, title) in rows) {
                        list.addView(Button(this).apply {
                            setText(title)
                            setOnClickListener {
                                slugField.setText(slug)
                                beginLoad()
                            }
                        })
                    }
                }
            } catch (e: Exception) {
                runOnUiThread {
                    busy = false
                    pickerStatus.text = e.message ?: "catalog failed"
                }
            }
        }
    }

    private fun boot(root: File) {
        val manifest = File(root, "kagra_game.json")
        if (manifest.isFile) {
            gameTitle = try {
                JSONObject(manifest.readText()).optString("title", gameTitle).ifBlank { gameTitle }
            } catch (_: Exception) {
                gameTitle
            }
        }
        KagraNative.setAssetRoot(root.absolutePath)
        val world = File(root, "world.json")
        if (!world.isFile) {
            packaged = false
            picker.visibility = View.VISIBLE
            pickerStatus.text = "$gameTitle に world.json がありません。"
            return
        }
        packaged = KagraNative.loadGameFile(world.absolutePath)
        if (!packaged) {
            picker.visibility = View.VISIBLE
            pickerStatus.text = "$gameTitle\nnative load failed: ${KagraNative.lastError()}"
            return
        }
        audio = AudioBus(root)
        status.text = "$gameTitle  ·  左=移動 右下=ジャンプ 右上=アクション"
    }

    private fun downloadBundle(rootUrl: String, slug: String): File {
        val incoming = File(filesDir, "kagra_dev/incoming")
        if (incoming.exists()) incoming.deleteRecursively()
        incoming.mkdirs()
        val bundleUrl = urlJoin(rootUrl, "games/$slug/bundle.json")
        val bundle = JSONObject(String(httpGet(bundleUrl), Charsets.UTF_8))
        val files = bundle.getJSONArray("files")
        for (i in 0 until files.length()) {
            val item = files.getJSONObject(i)
            val rel = item.getString("path")
            val remote = item.getString("url")
            val dest = safeFile(incoming, rel) ?: throw IllegalArgumentException("bad path: $rel")
            dest.parentFile?.mkdirs()
            dest.writeBytes(httpGet(urlJoin(rootUrl, remote)))
        }
        val marker = JSONObject()
        marker.put("title", bundle.optString("title", slug))
        marker.put("version", bundle.optString("version", ""))
        marker.put("slug", slug)
        marker.put("source", rootUrl)
        File(incoming, "kagra_game.json").writeText(marker.toString())
        return incoming
    }

    private fun swapIn(incoming: File): File {
        val current = File(filesDir, "kagra_dev/current")
        if (current.exists()) current.deleteRecursively()
        if (!incoming.renameTo(current)) {
            incoming.copyRecursively(current, overwrite = true)
            incoming.deleteRecursively()
        }
        return current
    }

    private fun channelRoot(): String {
        val base = baseField.text.toString().trim().trimEnd('/')
        if (!base.startsWith("https://") && !base.startsWith("http://")) {
            throw IllegalArgumentException("Pages の URL は http(s) で始めてください。")
        }
        if (!prMode) return "$base/"
        val pr = prField.text.toString().trim()
        if (!pr.all { it.isDigit() } || pr.isEmpty()) {
            throw IllegalArgumentException("PR 番号は数字だけです。")
        }
        return "$base/pr/$pr/"
    }

    private fun remember() {
        prefs().edit()
            .putString("pages_base", baseField.text.toString().trim())
            .putString("pr", prField.text.toString().trim())
            .putString("slug", slugField.text.toString().trim())
            .putBoolean("pr_mode", prMode)
            .apply()
    }

    private fun prefs() = getSharedPreferences("kagra_dev", MODE_PRIVATE)

    private fun onPull(ev: MotionEvent): Boolean {
        when (ev.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                pullStartY = ev.rawY
                pulled = false
            }
            MotionEvent.ACTION_MOVE -> {
                if (!pulled && ev.rawY - pullStartY > dp(72)) {
                    pulled = true
                    beginLoad()
                }
            }
        }
        return false
    }

    override fun surfaceCreated(holder: SurfaceHolder) = Unit

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
        if (!KagraNative.available) return
        KagraNative.createSurface(width, height)
        rendering = KagraNative.attachSurface(holder.surface, width, height)
        if (!rendering) {
            status.text = "renderer unavailable: ${KagraNative.lastError()}"
        }
        startLoop()
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        stopLoop()
        rendering = false
        if (KagraNative.available) KagraNative.detachSurface()
    }

    private fun startLoop() {
        if (frameCallback != null || !KagraNative.available) return
        val cb = object : Choreographer.FrameCallback {
            override fun doFrame(frameTimeNanos: Long) {
                if (packaged) {
                    KagraNative.setPlayInput(stickX, stickZ, jump, attack, false)
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
                } else if (rendering && packaged && frame % 30L == 0L && !busy) {
                    status.text = "$gameTitle  ·  再読込は上のボタンか、バーを下に引く"
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

    @SuppressLint("ClickableViewAccessibility")
    private fun onGameTouch(width: Int, height: Int, ev: MotionEvent): Boolean {
        if (width <= 0 || height <= 0 || !KagraNative.available) return true
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
            MotionEvent.ACTION_UP, MotionEvent.ACTION_POINTER_UP, MotionEvent.ACTION_CANCEL -> {
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
        if (packaged) KagraNative.setPlayInput(stickX, stickZ, jump, attack, false)
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

    private fun heading(text: String) = TextView(this).apply {
        this.text = text
        textSize = 28f
        setTextColor(Color.WHITE)
        setPadding(0, 0, 0, dp(8))
    }

    private fun note(text: String) = TextView(this).apply {
        this.text = text
        setTextColor(0xFFD5DBE8.toInt())
        setPadding(0, 0, 0, dp(12))
    }

    private fun label(text: String) = TextView(this).apply {
        this.text = text
        setTextColor(0xFFD5DBE8.toInt())
        setPadding(0, dp(8), 0, dp(4))
    }

    private fun field(value: String) = EditText(this).apply {
        setText(value)
        setSingleLine(true)
        setTextColor(Color.WHITE)
        setHintTextColor(0xFF9AA3B5.toInt())
    }

    private fun dp(value: Int): Int = (value * resources.displayMetrics.density).toInt()

    companion object {
        private const val MATCH = ViewGroup.LayoutParams.MATCH_PARENT

        fun urlJoin(base: String, rel: String): URL {
            val root = if (base.endsWith("/")) base else "$base/"
            val encoded = rel.trimStart('/').split("/").joinToString("/") { part ->
                URLEncoder.encode(part, "UTF-8").replace("+", "%20")
            }
            return URL(root + encoded)
        }

        fun safeFile(root: File, rel: String): File? {
            val clean = rel.replace('\\', '/').trim().trimStart('/')
            if (clean.isEmpty() || clean.split("/").any { it == ".." || it == "." || it.isEmpty() }) {
                return null
            }
            val dest = File(root, clean)
            val base = root.canonicalFile
            val parent = dest.parentFile ?: return null
            parent.mkdirs()
            val canon = dest.canonicalFile
            val prefix = base.path + File.separator
            if (canon.path != base.path && !canon.path.startsWith(prefix)) return null
            return canon
        }

        fun httpGet(url: URL): ByteArray {
            val conn = (url.openConnection() as HttpURLConnection).apply {
                instanceFollowRedirects = true
                connectTimeout = 20000
                readTimeout = 60000
                setRequestProperty("User-Agent", "KAGRA-Dev")
            }
            try {
                val code = conn.responseCode
                if (code !in 200..299) {
                    throw java.io.IOException("$url -> $code")
                }
                return conn.inputStream.use { it.readBytes() }
            } finally {
                conn.disconnect()
            }
        }
    }
}
