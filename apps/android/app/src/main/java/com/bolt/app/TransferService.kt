package com.bolt.app

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.wifi.WifiManager
import android.os.IBinder
import android.os.PowerManager
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.launch

/**
 * Foreground service that keeps the receiver, mDNS advertising and any running
 * transfer alive while the app is in the background, and surfaces progress,
 * incoming-offer prompts (with Accept / Decline) and results as notifications.
 */
class TransferService : Service() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private var wake: PowerManager.WakeLock? = null
    private var multicast: WifiManager.MulticastLock? = null
    private var wifiLock: WifiManager.WifiLock? = null
    private val shownOffers = HashSet<String>()
    private val shownPairs = HashSet<String>()
    private var overlay: OfferOverlay? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        createChannels()
        ServiceCompat.startForeground(this, ONGOING_ID, ongoing("Starting…", null, 0), ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
        Engine.init(this)
        Engine.ensureStarted(this)
        val wm = applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
        multicast = wm.createMulticastLock("bolt").apply { setReferenceCounted(false); acquire() }
        observe()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_STOP -> {
                Engine.shutdown()
                stopSelf()
            }
            ACTION_ACCEPT, ACTION_DECLINE -> {
                val id = intent.getStringExtra(EXTRA_ID) ?: return START_STICKY
                Engine.respondOffer(id, intent.action == ACTION_ACCEPT)
                nm().cancel(offerNotifId(id))
            }
            ACTION_CANCEL -> intent.getStringExtra(EXTRA_ID)?.let { Engine.cancel(it) }
        }
        return START_STICKY
    }

    override fun onDestroy() {
        overlay?.hide()
        BoltWidget.refresh(this, running = false)
        scope.cancel()
        multicast?.takeIf { it.isHeld }?.release()
        wake?.takeIf { it.isHeld }?.release()
        wifiLock?.takeIf { it.isHeld }?.release()
        super.onDestroy()
    }

    private fun nm() = getSystemService(NotificationManager::class.java)

    private fun observe() {
        scope.launch {
            combine(Engine.xfers, Engine.me, Engine.error, Engine.linkTesting) { x, me, err, _ -> Triple(x, me, err) }.collect { (xfers, me, err) ->
                val active = xfers.filter { it.active }
                // Keep the Wi-Fi radio out of power-save while bytes are moving (otherwise the phone batches
                // packets and throughput collapses), and only then.
                val moving = active.any { it.state == "transferring" } || Engine.linkTesting.value
                if (moving) {
                    if (wifiLock == null) {
                        val wm = applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
                        wifiLock = wm.createWifiLock(WifiManager.WIFI_MODE_FULL_LOW_LATENCY, "bolt:transfer").apply { setReferenceCounted(false) }
                    }
                    if (wifiLock?.isHeld == false) wifiLock?.acquire()
                } else wifiLock?.takeIf { it.isHeld }?.release()
                // Hold a CPU wake lock only while bytes are moving.
                if (active.any { it.state == "transferring" }) {
                    if (wake == null) wake = (getSystemService(POWER_SERVICE) as PowerManager).newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "bolt:transfer")
                    if (wake?.isHeld == false) wake?.acquire(2 * 60 * 60 * 1000L)
                } else wake?.takeIf { it.isHeld }?.release()

                val n = when {
                    active.isNotEmpty() -> {
                        val x = active.first()
                        val pct = if (x.total > 0) (x.done * 100 / x.total).toInt() else 0
                        val verb = if (x.dir == "send") "Sending to" else "Receiving from"
                        ongoing("$verb ${x.peer} — $pct%", x.job, pct, x.state != "transferring")
                    }
                    err != null -> ongoing(humanError(err), null, 0)
                    me != null -> ongoing("Ready to receive as ${me.name}", null, 0)
                    else -> ongoing("Starting…", null, 0)
                }
                nm().notify(ONGOING_ID, n)
            }
        }
        scope.launch {
            // keep the home-screen widget and quick-settings tile in step with the real state
            combine(Engine.me, Engine.error) { me, err -> me?.name to err }.collect { BoltWidget.refresh(this@TransferService, Engine.isRunning()) }
        }
        scope.launch {
            // the card on top of other apps: only while Bolt itself is not in front
            combine(Engine.offers, Engine.appVisible) { offers, visible -> if (visible) null else offers.firstOrNull() }.collect { o ->
                if (o != null && canShowOverOtherApps(this@TransferService)) {
                    val ov = overlay ?: OfferOverlay(this@TransferService).also { overlay = it }
                    ov.show(o)
                } else overlay?.hide()
            }
        }
        scope.launch {
            Engine.offers.collect { offers ->
                offers.filter { shownOffers.add(it.id) }.forEach { postOffer(it) }
                val live = offers.map { it.id }.toSet()
                shownOffers.filter { it !in live }.forEach { nm().cancel(offerNotifId(it)) }
                shownOffers.retainAll(live)
            }
        }
        scope.launch {
            Engine.pairReqs.collect { reqs ->
                reqs.filter { shownPairs.add(it.id) }.forEach { r ->
                    nm().notify(
                        2000 + r.id.hashCode().and(0xffff),
                        NotificationCompat.Builder(this@TransferService, CH_ALERT)
                            .setSmallIcon(R.drawable.ic_stat).setContentTitle("Pairing request")
                            .setContentText("${r.peerName} — code ${r.code}")
                            .setContentIntent(openApp()).setAutoCancel(true).setCategory(Notification.CATEGORY_MESSAGE).build(),
                    )
                }
            }
        }
        scope.launch {
            Engine.events.collect { (type, e) ->
                if (type == "finished") {
                    val ok = e.optBoolean("ok")
                    if (e.optBoolean("cancelled")) return@collect
                    val peer = e.optString("peer")
                    val sending = e.optString("dir") == "send"
                    val n = e.optInt("files")
                    val text = when {
                        ok && sending -> if (n == 1) "Sent 1 file to $peer" else "Sent $n files to $peer"
                        ok -> e.optString("note").ifEmpty { if (n == 1) "Received 1 file from $peer" else "Received $n files from $peer" }
                        else -> e.optString("note").ifEmpty { humanError(e.optString("error"), peer, sending) }
                    }
                    nm().notify(
                        3000 + e.optString("job").hashCode().and(0xffff),
                        NotificationCompat.Builder(this@TransferService, CH_RESULT)
                            .setSmallIcon(R.drawable.ic_stat).setContentTitle(if (ok) (if (sending) "Sent" else "Received") else failureTitle(peer, sending))
                            .setContentText(text).setStyle(NotificationCompat.BigTextStyle().bigText(text))
                            .setContentIntent(openApp()).setAutoCancel(true).build(),
                    )
                }
            }
        }
    }

    private fun postOffer(o: Offer) {
        fun action(act: String, label: String, req: Int) = NotificationCompat.Action.Builder(
            0, label,
            PendingIntent.getService(this, req + o.id.hashCode().and(0xfff), Intent(this, TransferService::class.java).setAction(act).putExtra(EXTRA_ID, o.id), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT),
        ).build()
        val what = if (o.fileCount == 1) o.files.firstOrNull()?.first ?: "1 file" else "${o.fileCount} files"
        nm().notify(
            offerNotifId(o.id),
            NotificationCompat.Builder(this, CH_ALERT)
                .setSmallIcon(R.drawable.ic_stat)
                .setContentTitle("${o.peerName} wants to send you $what")
                .setContentText(formatBytes(o.total) + if (o.trusted) " · paired device" else " · unknown device")
                .setPriority(NotificationCompat.PRIORITY_HIGH).setCategory(NotificationCompat.CATEGORY_MESSAGE)
                .setContentIntent(openApp())
                .addAction(action(ACTION_ACCEPT, "Accept", 1))
                .addAction(action(ACTION_DECLINE, "Decline", 2))
                .setOngoing(true).build(),
        )
    }

    private fun offerNotifId(id: String) = 1000 + id.hashCode().and(0xffff)

    private fun openApp() = PendingIntent.getActivity(
        this, 0, Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP), PendingIntent.FLAG_IMMUTABLE,
    )

    private fun ongoing(text: String, job: String?, pct: Int, indeterminate: Boolean = false): Notification {
        val b = NotificationCompat.Builder(this, CH_ONGOING)
            .setSmallIcon(R.drawable.ic_stat).setContentTitle("Bolt").setContentText(text)
            .setOngoing(true).setOnlyAlertOnce(true).setContentIntent(openApp()).setForegroundServiceBehavior(NotificationCompat.FOREGROUND_SERVICE_IMMEDIATE)
        if (job != null) {
            b.setProgress(100, pct, indeterminate)
            b.addAction(0, "Cancel", PendingIntent.getService(this, 7, Intent(this, TransferService::class.java).setAction(ACTION_CANCEL).putExtra(EXTRA_ID, job), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT))
        } else {
            b.addAction(0, "Stop", PendingIntent.getService(this, 8, Intent(this, TransferService::class.java).setAction(ACTION_STOP), PendingIntent.FLAG_IMMUTABLE))
        }
        return b.build()
    }

    private fun createChannels() {
        nm().createNotificationChannel(NotificationChannel(CH_ONGOING, "Transfers", NotificationManager.IMPORTANCE_LOW).apply { description = "Shows that Bolt is ready and the progress of transfers" })
        nm().createNotificationChannel(NotificationChannel(CH_ALERT, "Incoming requests", NotificationManager.IMPORTANCE_HIGH).apply { description = "Someone wants to send you files or pair" })
        nm().createNotificationChannel(NotificationChannel(CH_RESULT, "Results", NotificationManager.IMPORTANCE_DEFAULT))
    }

    companion object {
        const val CH_ONGOING = "ongoing"
        const val CH_ALERT = "alert"
        const val CH_RESULT = "result"
        const val ONGOING_ID = 1
        const val ACTION_STOP = "com.bolt.STOP"
        const val ACTION_ACCEPT = "com.bolt.ACCEPT"
        const val ACTION_DECLINE = "com.bolt.DECLINE"
        const val ACTION_CANCEL = "com.bolt.CANCEL"
        const val EXTRA_ID = "id"

        fun start(ctx: Context) {
            ctx.startForegroundService(Intent(ctx, TransferService::class.java))
        }
    }
}

fun formatBytes(b: Long): String {
    if (b < 1000) return "$b B"
    val units = arrayOf("KB", "MB", "GB", "TB")
    var v = b / 1000.0
    var i = 0
    while (v >= 1000 && i < units.lastIndex) { v /= 1000; i++ }
    return String.format(java.util.Locale.US, "%.1f %s", v, units[i])
}
