package dev.kagra.shell

import android.annotation.SuppressLint
import android.os.Bundle
import android.view.MotionEvent
import android.widget.TextView
import androidx.appcompat.app.AppCompatActivity

class MainActivity : AppCompatActivity() {
    private lateinit var status: TextView
    private var running = true

    @SuppressLint("ClickableViewAccessibility")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        status = TextView(this).apply {
            textSize = 16f
            setPadding(32, 64, 32, 32)
            setBackgroundColor(0xFF1C2230.toInt())
            setTextColor(0xFFE8ECF4.toInt())
            text = "KAGRA starting…"
        }
        setContentView(status)

        val ok = KagraNative.create()
        if (!ok) {
            status.text = "native init failed: ${KagraNative.lastError()}"
            return
        }
        KagraNative.createSurface(resources.displayMetrics.widthPixels, resources.displayMetrics.heightPixels)
        KagraNative.setAssetRoot(filesDir.absolutePath)
        status.text = "kagra-shared ${KagraNative.version()}\nTouch to send pointers."

        status.setOnTouchListener { _, ev ->
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
            // 仮想パッド: 画面左半分をスティックに
            if (ev.x < status.width / 2f) {
                val nx = ((ev.x / (status.width / 2f)) * 2f - 1f).coerceIn(-1f, 1f)
                val ny = ((ev.y / status.height) * 2f - 1f).coerceIn(-1f, 1f)
                KagraNative.setPad(nx, ny)
            }
            true
        }

        status.post(object : Runnable {
            override fun run() {
                if (!running) return
                val frame = KagraNative.requestFrame()
                status.text = "kagra-shared ${KagraNative.version()}\nframe=$frame\n${KagraNative.statsJson()}"
                status.postDelayed(this, 16L)
            }
        })
    }

    override fun onPause() {
        super.onPause()
        KagraNative.pause()
    }

    override fun onResume() {
        super.onResume()
        KagraNative.resume()
    }

    override fun onDestroy() {
        running = false
        KagraNative.destroy()
        super.onDestroy()
    }
}
