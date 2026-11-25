package io.github.untypequicode.looty.call

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.content.pm.ServiceInfo
import android.media.AudioDeviceCallback
import android.media.AudioDeviceInfo
import android.media.AudioManager
import android.os.Build
import android.os.IBinder
import android.os.PowerManager
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.WebActivity

/**
 * Service au premier plan pendant un appel : le micro (et la caméra) restent utilisables écran éteint ou
 * application en arrière-plan, avec une notification « Appel en cours » et un bouton Raccrocher.
 * Démarré par l'écran web (WebActivity) quand la page signale un appel, arrêté quand elle signale la fin.
 * L'audio lui-même reste dans la WebView : le service ne fait que garder le processus actif et régler la sortie.
 */
class CallService : Service() {
    companion object {
        const val CHANNEL = "call"
        private const val ID = 4207
        private const val ACTION_START = "io.github.untypequicode.looty.call.START"
        private const val ACTION_HANGUP = "io.github.untypequicode.looty.call.HANGUP"
        private const val ACTION_SPEAKER = "io.github.untypequicode.looty.call.SPEAKER"
        private const val EXTRA_TITLE = "title"
        private const val EXTRA_HOST = "host"
        private const val EXTRA_VIDEO = "video"
        private const val EXTRA_MUTED = "muted"

        /** Raccrocher depuis la notification : renseigné par l'écran web, qui quitte l'appel dans la page. */
        @Volatile var onHangup: (() -> Unit)? = null
        @Volatile var running = false
            private set

        fun start(ctx: Context, title: String, host: String, video: Boolean, muted: Boolean) {
            // Sans micro (ni caméra) accordé, le système refuserait le service : rien à garder en vie.
            val mic = ctx.checkSelfPermission(android.Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED
            val cam = video && ctx.checkSelfPermission(android.Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED
            if (!mic && !cam) return
            val i = Intent(ctx, CallService::class.java).setAction(ACTION_START)
                .putExtra(EXTRA_TITLE, title).putExtra(EXTRA_HOST, host).putExtra(EXTRA_VIDEO, video).putExtra(EXTRA_MUTED, muted)
            runCatching { ContextCompat.startForegroundService(ctx, i) }
        }

        fun stop(ctx: Context) {
            ctx.stopService(Intent(ctx, CallService::class.java))
        }

        /** Choix du haut-parleur (true) ou de l'écouteur (false) demandé par la page. */
        fun speaker(ctx: Context, on: Boolean) {
            if (running) runCatching { ctx.startService(Intent(ctx, CallService::class.java).setAction(ACTION_SPEAKER).putExtra("on", on)) }
        }
    }

    private lateinit var audio: CallAudio
    private var wake: PowerManager.WakeLock? = null
    private var title = ""
    private var host = ""
    private var video = false
    private var muted = false

    private val devices = object : AudioDeviceCallback() {
        override fun onAudioDevicesAdded(added: Array<out AudioDeviceInfo>) = audio.refresh()
        override fun onAudioDevicesRemoved(removed: Array<out AudioDeviceInfo>) = audio.refresh()
    }
    private val noisy = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) = audio.refresh()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        audio = CallAudio(this)
        val nm = getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel(CHANNEL, getString(R.string.call_channel), NotificationManager.IMPORTANCE_LOW).apply {
            description = getString(R.string.call_channel_hint)
            setShowBadge(false)
        })
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_START -> {
                title = intent.getStringExtra(EXTRA_TITLE).orEmpty()
                host = intent.getStringExtra(EXTRA_HOST).orEmpty()
                video = intent.getBooleanExtra(EXTRA_VIDEO, false)
                muted = intent.getBooleanExtra(EXTRA_MUTED, false)
                if (!running) begin() else notifyUpdate()
            }
            ACTION_SPEAKER -> {
                audio.setSpeaker(intent.getBooleanExtra("on", true))
                notifyUpdate()
            }
            ACTION_HANGUP -> {
                // La page quitte l'appel et signale la fin ; sans écran web (processus relancé), on s'arrête seul.
                val cb = onHangup
                if (cb != null) cb() else end()
            }
            // Redémarrage par le système sans appel connu : rien à faire.
            else -> if (!running) end()
        }
        return START_NOT_STICKY
    }

    private fun granted(p: String) = checkSelfPermission(p) == PackageManager.PERMISSION_GRANTED

    private fun begin() {
        val mic = granted(android.Manifest.permission.RECORD_AUDIO)
        val cam = video && granted(android.Manifest.permission.CAMERA)
        try {
            if (Build.VERSION.SDK_INT >= 29) {
                var type = 0
                if (mic) type = type or ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE
                if (cam) type = type or ServiceInfo.FOREGROUND_SERVICE_TYPE_CAMERA
                // Sans micro accordé, un type « microphone » serait refusé : l'appel sans micro n'a pas besoin du service.
                if (type == 0) { stopSelf(); return }
                androidx.core.app.ServiceCompat.startForeground(this, ID, notification(), type)
            } else {
                startForeground(ID, notification())
            }
        } catch (_: Exception) {
            // Démarrage refusé (application en arrière-plan, permission retirée) : l'appel continue tant que l'écran est allumé.
            stopSelf()
            return
        }
        running = true
        audio.start()
        getSystemService(AudioManager::class.java).registerAudioDeviceCallback(devices, null)
        ContextCompat.registerReceiver(this, noisy, IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY), ContextCompat.RECEIVER_NOT_EXPORTED)
        wake = getSystemService(PowerManager::class.java).newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "looty:call").apply {
            setReferenceCounted(false)
            // Garde-fou : jamais plus de 6 h (un appel oublié ne vide pas la batterie).
            acquire(6 * 60 * 60 * 1000L)
        }
    }

    private fun end() {
        if (running) {
            running = false
            runCatching { getSystemService(AudioManager::class.java).unregisterAudioDeviceCallback(devices) }
            runCatching { unregisterReceiver(noisy) }
            audio.stop()
            wake?.takeIf { it.isHeld }?.release()
            wake = null
        }
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    override fun onDestroy() {
        end()
        super.onDestroy()
    }

    /** L'utilisateur retire l'application des récents : l'écran web est détruit, l'appel avec lui. */
    override fun onTaskRemoved(rootIntent: Intent?) {
        end()
    }

    private fun notifyUpdate() {
        getSystemService(NotificationManager::class.java).notify(ID, notification())
    }

    private fun pending(action: String, code: Int): PendingIntent =
        PendingIntent.getService(this, code, Intent(this, CallService::class.java).setAction(action).putExtra("on", !audio.speaker), PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)

    private fun notification(): Notification {
        val open = PendingIntent.getActivity(
            this, 0, Intent(this, WebActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_REORDER_TO_FRONT),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        val text = listOfNotNull(title.ifBlank { null }, host.ifBlank { null }).joinToString(" · ")
        val b = NotificationCompat.Builder(this, CHANNEL)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle(getString(if (muted) R.string.call_ongoing_muted else R.string.call_ongoing))
            .setContentText(text)
            .setCategory(NotificationCompat.CATEGORY_CALL)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setSilent(true)
            .setForegroundServiceBehavior(NotificationCompat.FOREGROUND_SERVICE_IMMEDIATE)
            .setContentIntent(open)
            .addAction(0, getString(R.string.call_hangup), pending(ACTION_HANGUP, 1))
        // Bascule haut-parleur / écouteur, seulement sans casque (avec un casque, la sortie suit le casque).
        if (!audio.headsetAvailable()) {
            b.addAction(0, getString(if (audio.speaker) R.string.call_earpiece else R.string.call_speaker), pending(ACTION_SPEAKER, 2))
        }
        return b.build()
    }
}
