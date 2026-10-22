package dev.kagra.shell

import android.media.AudioAttributes
import android.media.MediaPlayer
import android.media.SoundPool
import java.io.File
import org.json.JSONObject

/**
 * Rust のミキサーが出したキューを鳴らす。
 *
 * 効果音は SoundPool、BGM はループする MediaPlayer。音量が 0 かミュート、
 * ファイルが無い、デバイスが拒否したときは何もしない。
 */
class AudioBus(private val root: File) {
    private val pool: SoundPool = SoundPool.Builder()
        .setMaxStreams(8)
        .setAudioAttributes(
            AudioAttributes.Builder()
                .setUsage(AudioAttributes.USAGE_GAME)
                .setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION)
                .build()
        )
        .build()
    private val clips = HashMap<String, Int>()
    private var player: MediaPlayer? = null
    private var bgmName: String = ""
    private var ready = false

    fun apply(json: String) {
        ensure()
        val obj = try {
            JSONObject(json)
        } catch (_: Exception) {
            return
        }
        val muted = obj.optBoolean("muted", false)
        val master = obj.optDouble("master", 0.8).toFloat().coerceIn(0f, 1f)
        val bgm = obj.optDouble("bgm", 0.7).toFloat().coerceIn(0f, 1f)
        val sfx = obj.optDouble("sfx", 1.0).toFloat().coerceIn(0f, 1f)
        val name = obj.optString("bgm_name", "")
        val gain = obj.optDouble("bgm_gain", 0.0).toFloat().coerceIn(0f, 1f)
        val bgmVol = if (muted) 0f else master * bgm * gain
        syncBgm(name, bgmVol)
        if (muted || master <= 0f || sfx <= 0f) return
        val shots = obj.optJSONArray("oneshots") ?: return
        val vol = (master * sfx).coerceIn(0f, 1f)
        for (i in 0 until shots.length()) {
            val id = clips[shots.optString(i)] ?: continue
            if (id != 0) pool.play(id, vol, vol, 1, 0, 1f)
        }
    }

    fun release() {
        try {
            player?.release()
        } catch (_: Exception) {
        }
        player = null
        bgmName = ""
        pool.release()
    }

    private fun ensure() {
        if (ready) return
        ready = true
        val dir = File(root, "assets/audio")
        for (name in listOf("jump", "fire", "card", "bgm")) {
            val file = File(dir, "$name.wav")
            if (!file.isFile) continue
            if (name == "bgm") continue
            val id = pool.load(file.absolutePath, 1)
            if (id != 0) clips[name] = id
        }
    }

    private fun syncBgm(name: String, volume: Float) {
        if (name.isEmpty() || volume <= 0.01f) {
            if (player?.isPlaying == true) player?.pause()
            return
        }
        val file = File(root, "assets/audio/$name.wav")
        if (!file.isFile) return
        try {
            if (bgmName != name || player == null) {
                player?.release()
                player = MediaPlayer().apply {
                    setDataSource(file.absolutePath)
                    isLooping = true
                    setVolume(volume, volume)
                    prepare()
                    start()
                }
                bgmName = name
            } else {
                player?.setVolume(volume, volume)
                if (player?.isPlaying != true) player?.start()
            }
        } catch (_: Exception) {
            player?.release()
            player = null
            bgmName = ""
        }
    }
}
