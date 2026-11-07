package com.bolt.app

import android.content.Context
import android.media.AudioAttributes
import android.media.AudioManager
import android.media.SoundPool

/** The moments Bolt makes a sound for. Replace the files in `res/raw` to change how they sound. */
enum class Sound(val res: Int) {
    Offer(R.raw.offer), Done(R.raw.done), Fail(R.raw.fail), Text(R.raw.text), Pair(R.raw.pair),
}

/** Short sound effects through the notification volume. They stay silent in silent and vibrate mode. */
object Sounds {
    private var pool: SoundPool? = null
    private val ids = HashMap<Sound, Int>()
    private lateinit var app: Context

    fun init(ctx: Context) {
        if (pool != null) return
        app = ctx.applicationContext
        val p = SoundPool.Builder()
            .setMaxStreams(2)
            .setAudioAttributes(
                AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_NOTIFICATION_EVENT).setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION).build(),
            ).build()
        Sound.values().forEach { ids[it] = p.load(app, it.res, 1) }
        pool = p
    }

    var enabled: Boolean
        get() = app.getSharedPreferences("bolt", Context.MODE_PRIVATE).getBoolean("sounds", true)
        set(v) { app.getSharedPreferences("bolt", Context.MODE_PRIVATE).edit().putBoolean("sounds", v).apply() }

    fun play(s: Sound) {
        val p = pool ?: return
        if (!enabled) return
        val audio = app.getSystemService(Context.AUDIO_SERVICE) as AudioManager
        if (audio.ringerMode != AudioManager.RINGER_MODE_NORMAL) return
        ids[s]?.let { p.play(it, 0.7f, 0.7f, 1, 0, 1f) }
    }
}
