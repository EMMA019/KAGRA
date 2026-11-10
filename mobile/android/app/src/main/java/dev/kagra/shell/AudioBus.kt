package dev.kagra.shell

import android.media.AudioAttributes
import android.media.MediaPlayer
import android.media.SoundPool
import java.io.File
import org.json.JSONArray
import org.json.JSONObject

/**
 * Plays what the Rust side decided for this frame ([KagraNative.takeAudioPlan]).
 *
 * Which file a cue plays and how loud is the game's `audio` block, worked out in Rust
 * (`kagra-shared/src/audio_plan.rs`, the same block the browser page reads). This class
 * only loads files and plays them: one-shots through SoundPool, the looping music through
 * MediaPlayer. A missing file, a clip still loading or a device that refuses is silence,
 * never a crash.
 */
class AudioBus(private val root: File) {
    private val pool: SoundPool = SoundPool.Builder()
        .setMaxStreams(10)
        .setAudioAttributes(
            AudioAttributes.Builder()
                .setUsage(AudioAttributes.USAGE_GAME)
                .setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION)
                .build()
        )
        .build()

    /** Path under the asset root → SoundPool id (0: no such file, or it would not load). */
    private val clips = HashMap<String, Int>()

    /** Ids SoundPool has finished decoding. Playing one before that does nothing. */
    private val loaded = HashSet<Int>()

    private var player: MediaPlayer? = null
    private var playerReady = false
    private var bgmFile = ""
    private var bgmVolume = 0f

    /** Music files that would not play: not tried again every frame. */
    private val badMusic = HashSet<String>()

    init {
        pool.setOnLoadCompleteListener { _, id, status ->
            if (status == 0) synchronized(loaded) { loaded.add(id) }
        }
    }

    /** Load the game's one-shot files ([KagraNative.audioFiles], a JSON array of paths). */
    fun preload(filesJson: String) {
        val files = try {
            JSONArray(filesJson)
        } catch (_: Exception) {
            return
        }
        for (i in 0 until files.length()) clip(files.optString(i, ""))
    }

    /** Play one frame's plan: start its one-shots, hold the music at its volume. */
    fun apply(planJson: String) {
        val plan = try {
            JSONObject(planJson)
        } catch (_: Exception) {
            return
        }
        val bgm = plan.optJSONObject("bgm")
        if (bgm == null) {
            syncBgm("", 0f)
        } else {
            syncBgm(bgm.optString("file", ""), unit(bgm.optDouble("volume", 0.0)))
        }
        val shots = plan.optJSONArray("shots") ?: return
        for (i in 0 until shots.length()) {
            val shot = shots.optJSONObject(i) ?: continue
            val id = clip(shot.optString("file", ""))
            if (id == 0 || !synchronized(loaded) { loaded.contains(id) }) continue
            val volume = unit(shot.optDouble("volume", 1.0))
            if (volume <= 0f) continue
            val rate = shot.optDouble("rate", 1.0).toFloat().coerceIn(0.5f, 2f)
            try {
                pool.play(id, volume, volume, 1, 0, rate)
            } catch (_: Exception) {
            }
        }
    }

    /** The app left the screen: no music in the background. The next [apply] resumes it. */
    fun pause() {
        try {
            if (playerReady && player?.isPlaying == true) player?.pause()
            pool.autoPause()
        } catch (_: Exception) {
        }
    }

    /** Back on screen. */
    fun resume() {
        try {
            pool.autoResume()
        } catch (_: Exception) {
        }
    }

    fun release() {
        dropPlayer()
        try {
            pool.release()
        } catch (_: Exception) {
        }
    }

    private fun unit(v: Double): Float = if (v.isNaN()) 0f else v.toFloat().coerceIn(0f, 1f)

    /** The file under the asset root, or null for a path that leaves it or is not there. */
    private fun resolve(rel: String): File? {
        if (rel.isEmpty() || rel.startsWith("/") || rel.contains("..") || rel.contains("\\")) return null
        val file = File(root, rel)
        return if (file.isFile) file else null
    }

    private fun clip(rel: String): Int {
        clips[rel]?.let { return it }
        val file = resolve(rel)
        val id = if (file == null) {
            0
        } else {
            try {
                pool.load(file.absolutePath, 1)
            } catch (_: Exception) {
                0
            }
        }
        clips[rel] = id
        return id
    }

    private fun syncBgm(rel: String, volume: Float) {
        if (rel.isEmpty() || volume <= 0.005f) {
            try {
                if (playerReady && player?.isPlaying == true) player?.pause()
            } catch (_: Exception) {
                dropPlayer()
            }
            return
        }
        bgmVolume = volume
        if (bgmFile == rel && player != null) {
            if (!playerReady) return
            try {
                player?.setVolume(volume, volume)
                if (player?.isPlaying != true) player?.start()
            } catch (_: Exception) {
                dropPlayer()
            }
            return
        }
        dropPlayer()
        if (badMusic.contains(rel)) return
        val file = resolve(rel)
        if (file == null) {
            badMusic.add(rel)
            return
        }
        try {
            val p = MediaPlayer()
            p.setAudioAttributes(
                AudioAttributes.Builder()
                    .setUsage(AudioAttributes.USAGE_GAME)
                    .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC)
                    .build()
            )
            p.setDataSource(file.absolutePath)
            p.isLooping = true
            p.setOnPreparedListener { ready ->
                // Decoded off the frame loop: no hitch when the music first starts.
                if (ready === player) {
                    playerReady = true
                    try {
                        ready.setVolume(bgmVolume, bgmVolume)
                        ready.start()
                    } catch (_: Exception) {
                        dropPlayer()
                    }
                }
            }
            p.setOnErrorListener { broken, _, _ ->
                if (broken === player) {
                    badMusic.add(rel)
                    dropPlayer()
                }
                true
            }
            player = p
            bgmFile = rel
            p.prepareAsync()
        } catch (_: Exception) {
            badMusic.add(rel)
            dropPlayer()
        }
    }

    private fun dropPlayer() {
        try {
            player?.release()
        } catch (_: Exception) {
        }
        player = null
        playerReady = false
        bgmFile = ""
    }
}
