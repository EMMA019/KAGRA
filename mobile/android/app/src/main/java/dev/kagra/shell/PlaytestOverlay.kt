package dev.kagra.shell

import android.content.ContentValues
import android.content.Intent
import android.graphics.Bitmap
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.os.Handler
import android.os.Looper
import android.provider.MediaStore
import android.view.Gravity
import android.view.PixelCopy
import android.view.SurfaceView
import android.view.View
import android.text.InputType
import android.view.inputmethod.EditorInfo
import android.widget.Button
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.TextView
import java.io.File
import org.json.JSONObject

/**
 * IME note and screenshot handoff for the debug APK. The native panel stays
 * off (`external chrome`) so this EditText can compose Japanese. No token.
 */
class PlaytestOverlay(
    private val activity: MainActivity,
    private val root: FrameLayout,
    private val surface: SurfaceView,
) {
    private val handler = Handler(Looper.getMainLooper())
    private val edit = EditText(activity)
    private val status = TextView(activity)
    private val panel = LinearLayout(activity)
    private var open = false
    private var capturing = false
    private var lastSeq = 0
    private var shotPath = ""

    init {
        val density = activity.resources.displayMetrics.density
        fun dp(v: Int) = (v * density).toInt()
        edit.apply {
            hint = "メモ（日本語）"
            setTextColor(0xFFECE8DC.toInt())
            setHintTextColor(0xFF969288.toInt())
            setBackgroundColor(0xFF0E1018.toInt())
            setPadding(dp(8), dp(8), dp(8), dp(8))
            imeOptions = EditorInfo.IME_FLAG_NO_FULLSCREEN
            inputType = InputType.TYPE_CLASS_TEXT or
                InputType.TYPE_TEXT_FLAG_MULTI_LINE or
                InputType.TYPE_TEXT_FLAG_CAP_SENTENCES
            gravity = Gravity.TOP or Gravity.START
            minLines = 3
        }
        status.apply {
            setTextColor(0xFFD2CEC4.toInt())
            textSize = 13f
            text = "一時停止。メモを書いて「送る」。"
        }
        val row = LinearLayout(activity).apply { orientation = LinearLayout.HORIZONTAL }
        row.addView(button("送る") { tap("send") }, weight())
        row.addView(button("コピー") { tap("copy") }, weight())
        row.addView(button("閉じる") { tap("close") }, weight())
        panel.apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(0xEE1C202C.toInt())
            setPadding(dp(14), dp(14), dp(14), dp(14))
            visibility = View.GONE
            addView(TextView(activity).apply {
                text = "直してここ"
                setTextColor(0xFFE8B054.toInt())
                textSize = 18f
            })
            addView(edit, LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, dp(72)
            ).apply { topMargin = dp(8) })
            addView(status, LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply { topMargin = dp(8) })
            addView(row, LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply { topMargin = dp(8) })
        }
        root.addView(panel, FrameLayout.LayoutParams(dp(440), FrameLayout.LayoutParams.WRAP_CONTENT, Gravity.CENTER))
    }

    fun syncNote() {
        if (!open || !KagraNative.available) return
        KagraNative.playtestSetNote(edit.text?.toString().orEmpty())
    }

    /** True while a screenshot of the previous frame is still being copied. */
    fun onFrame(): Boolean {
        if (!KagraNative.available) return capturing
        val state = try {
            JSONObject(KagraNative.playtestStateJson())
        } catch (_: Exception) {
            return capturing
        }
        val now = state.optBoolean("open")
        if (now && !open) show()
        if (!now && open) hide()
        if (state.optBoolean("shot")) capture()
        val seq = state.optInt("packet_seq")
        if (state.optBoolean("packet") && seq != lastSeq) {
            lastSeq = seq
            try {
                deliver(JSONObject(KagraNative.playtestPacketJson()))
            } catch (_: Exception) {
            }
        }
        return capturing
    }

    private fun show() {
        open = true
        panel.visibility = View.VISIBLE
        edit.requestFocus()
    }

    private fun hide() {
        open = false
        panel.visibility = View.GONE
    }

    private fun tap(action: String) {
        syncNote()
        KagraNative.playtestAction(action)
    }

    private fun capture() {
        if (capturing || surface.width <= 0 || surface.height <= 0) return
        capturing = true
        val bitmap = Bitmap.createBitmap(surface.width, surface.height, Bitmap.Config.ARGB_8888)
        try {
            PixelCopy.request(surface, bitmap, { result ->
                capturing = false
                if (result == PixelCopy.SUCCESS) {
                    val saved = saveBitmap(bitmap)
                    shotPath = saved ?: ""
                    if (shotPath.isNotEmpty()) {
                        KagraNative.playtestSetShotPath(shotPath)
                        status.text = "スクリーンショットを保存しました ($shotPath)。issue に貼り付けてください。"
                    }
                } else {
                    status.text = "スクリーンショットを issue に貼り付けてください。"
                }
                bitmap.recycle()
            }, handler)
        } catch (_: Exception) {
            capturing = false
            bitmap.recycle()
        }
    }

    private fun saveBitmap(bitmap: Bitmap): String? {
        val name = "kagra-playtest-${System.currentTimeMillis()}.png"
        return try {
            val values = ContentValues().apply {
                put(MediaStore.Images.Media.DISPLAY_NAME, name)
                put(MediaStore.Images.Media.MIME_TYPE, "image/png")
                if (Build.VERSION.SDK_INT >= 29) {
                    put(MediaStore.Images.Media.RELATIVE_PATH, Environment.DIRECTORY_PICTURES + "/KAGRA")
                    put(MediaStore.Images.Media.IS_PENDING, 1)
                }
            }
            val uri = activity.contentResolver.insert(MediaStore.Images.Media.EXTERNAL_CONTENT_URI, values)
                ?: return saveFallback(bitmap, name)
            activity.contentResolver.openOutputStream(uri)?.use { out ->
                bitmap.compress(Bitmap.CompressFormat.PNG, 100, out)
            }
            if (Build.VERSION.SDK_INT >= 29) {
                values.clear()
                values.put(MediaStore.Images.Media.IS_PENDING, 0)
                activity.contentResolver.update(uri, values, null, null)
            }
            copyImage(uri)
            uri.toString()
        } catch (_: Exception) {
            saveFallback(bitmap, name)
        }
    }

    private fun saveFallback(bitmap: Bitmap, name: String): String? {
        val dir = activity.getExternalFilesDir(Environment.DIRECTORY_PICTURES) ?: activity.filesDir
        val file = File(dir, name)
        return try {
            file.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
            file.absolutePath
        } catch (_: Exception) {
            null
        }
    }

    private fun copyImage(uri: Uri) {
        try {
            val clip = android.content.ClipData.newUri(activity.contentResolver, "KAGRA", uri)
            val clipboard = activity.getSystemService(android.content.Context.CLIPBOARD_SERVICE) as android.content.ClipboardManager
            clipboard.setPrimaryClip(clip)
        } catch (_: Exception) {
        }
    }

    private fun deliver(packet: JSONObject) {
        val block = packet.optString("block").ifBlank { packet.optString("copy") }
        try {
            val clipboard = activity.getSystemService(android.content.Context.CLIPBOARD_SERVICE) as android.content.ClipboardManager
            clipboard.setPrimaryClip(android.content.ClipData.newPlainText("KAGRA", block))
        } catch (_: Exception) {
        }
        val instruction = packet.optString("instruction")
        if (instruction.isNotBlank()) status.text = instruction
        if (packet.optString("action") == "send") {
            val url = packet.optString("url")
            if (url.startsWith("https://github.com/")) {
                try {
                    activity.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)))
                } catch (_: Exception) {
                    status.text = url
                }
            }
        }
    }

    private fun button(label: String, onClick: () -> Unit): Button {
        return Button(activity).apply {
            text = label
            setOnClickListener { onClick() }
        }
    }

    private fun weight(): LinearLayout.LayoutParams {
        return LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
    }
}
