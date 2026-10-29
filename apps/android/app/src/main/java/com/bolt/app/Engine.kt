package com.bolt.app

import android.content.Context
import android.net.Uri
import android.os.Build
import android.os.SystemClock
import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import org.json.JSONObject
import java.io.File

data class Offer(
    val id: String, val peerName: String, val peerId: String, val trusted: Boolean,
    val fileCount: Int, val total: Long, val free: Long?, val files: List<Pair<String, Long>>,
)

data class PairReq(val id: String, val peerName: String, val code: String)

data class Xfer(
    val job: String, val dir: String, val peer: String, val state: String,
    val done: Long = 0, val total: Long = 0, val filesDone: Int = 0, val filesTotal: Int = 0,
    val current: String? = null, val speed: Double = 0.0, val finished: Boolean = false,
    val ok: Boolean = false, val error: String? = null, val note: String? = null,
    /** Files that can be opened once the transfer is done. */
    val files: List<SavedFile> = emptyList(),
) {
    val active get() = !finished
}

data class Me(val id: String, val port: Int, val name: String)
data class HistoryItem(val id: String, val dir: String, val peer: String, val files: Int, val title: String, val bytes: Long, val at: Long, val status: String, val error: String?)
data class Trusted(val id: String, val name: String)
data class PairStatus(val job: String? = null, val code: String? = null, val text: String? = null, val done: Boolean = false, val failed: Boolean = false)

data class LinkResult(
    val target: String, val upMBps: Double, val downMBps: Double, val connectMs: Long,
    val rttMs: Double, val mtu: Int, val upLossPct: Double, val downLossPct: Double,
)

sealed interface LinkState {
    data object Idle : LinkState
    data class Running(val target: String) : LinkState
    data class Done(val result: LinkResult) : LinkState
    data class Failed(val message: String) : LinkState
}

/** What the last transfer spent its time on (from the Rust core), plus our own copy-to-Downloads time. */
data class TransferDiag(
    val dir: String, val peer: String, val bytes: Long, val elapsedMs: Long,
    val stages: String, val path: String, val copyMs: Long? = null,
)

/** Process-wide owner of the native core and all UI-visible state. The service keeps the process alive. */
object Engine {
    private const val TAG = "Bolt"
    const val DEFAULT_TUNING = "cc=bbr,mtu=1452"
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private lateinit var app: Context
    private var nsd: Nsd? = null
    private var started = false
    /** Source files of running sends, so a finished send can offer to open them. */
    private val sentFiles = java.util.concurrent.ConcurrentHashMap<String, List<PickedFile>>()
    private val lastSample = java.util.concurrent.ConcurrentHashMap<String, Pair<Long, Long>>() // job -> (uptime ms, bytes)

    private val _me = MutableStateFlow<Me?>(null)
    val me: StateFlow<Me?> = _me.asStateFlow()
    private val _error = MutableStateFlow<String?>(null)
    val error: StateFlow<String?> = _error.asStateFlow()
    private val _devices = MutableStateFlow<List<Device>>(emptyList())
    val devices: StateFlow<List<Device>> = _devices.asStateFlow()
    private val _offers = MutableStateFlow<List<Offer>>(emptyList())
    val offers: StateFlow<List<Offer>> = _offers.asStateFlow()
    private val _pairReqs = MutableStateFlow<List<PairReq>>(emptyList())
    val pairReqs: StateFlow<List<PairReq>> = _pairReqs.asStateFlow()
    private val _xfers = MutableStateFlow<List<Xfer>>(emptyList())
    val xfers: StateFlow<List<Xfer>> = _xfers.asStateFlow()
    private val _history = MutableStateFlow<List<HistoryItem>>(emptyList())
    val history: StateFlow<List<HistoryItem>> = _history.asStateFlow()
    private val _trusted = MutableStateFlow<List<Trusted>>(emptyList())
    val trusted: StateFlow<List<Trusted>> = _trusted.asStateFlow()
    private val _pairUri = MutableStateFlow<String?>(null)
    val pairUri: StateFlow<String?> = _pairUri.asStateFlow()
    private val _pairStatus = MutableStateFlow(PairStatus())
    val pairStatus: StateFlow<PairStatus> = _pairStatus.asStateFlow()
    /** Files waiting for the user to choose a destination device (from a picker or the share sheet). */
    /** True while MainActivity is on screen; an incoming offer is shown as an overlay only when it is not. */
    val appVisible = MutableStateFlow(false)

    /** Set by the home-screen widget: open the file picker as soon as the app is up. */
    val pickerRequest = MutableStateFlow(false)

    private val _pending = MutableStateFlow<List<PickedFile>>(emptyList())
    val pending: StateFlow<List<PickedFile>> = _pending.asStateFlow()
    private val _link = MutableStateFlow<LinkState>(LinkState.Idle)
    val link: StateFlow<LinkState> = _link.asStateFlow()
    val linkTesting: StateFlow<Boolean> get() = _linkTesting
    private val _linkTesting = MutableStateFlow(false)
    private val _buffers = MutableStateFlow(0L to 0L)
    /** UDP (receive, send) buffer bytes the OS granted to the native endpoint. */
    val udpBuffers: StateFlow<Pair<Long, Long>> = _buffers.asStateFlow()
    private val _lastTransfer = MutableStateFlow<TransferDiag?>(null)
    val lastTransfer: StateFlow<TransferDiag?> = _lastTransfer.asStateFlow()
    private val _events = MutableSharedFlow<Pair<String, JSONObject>>(extraBufferCapacity = 64)
    val events: SharedFlow<Pair<String, JSONObject>> = _events.asSharedFlow()

    private fun prefs() = app.getSharedPreferences("bolt", Context.MODE_PRIVATE)
    var deviceName: String
        get() = prefs().getString("name", null) ?: Build.MODEL ?: "Android"
        set(v) { prefs().edit().putString("name", v).apply(); if (started) NativeBridge.setDeviceName(v) }
    var autoAccept: Boolean
        get() = prefs().getBoolean("auto", false)
        set(v) { prefs().edit().putBoolean("auto", v).apply(); if (started) NativeBridge.setAutoAccept(v) }
    var destTree: String?
        get() = prefs().getString("tree", null)
        set(v) { prefs().edit().putString("tree", v).apply() }
    /** Network tuning for the next start, e.g. "cc=bbr,mtu=1452". See EndpointTuning in the core. */
    var tuning: String
        get() = prefs().getString("tuning", DEFAULT_TUNING) ?: DEFAULT_TUNING
        set(v) { prefs().edit().putString("tuning", v).apply() }
    var lastManual: String
        get() = prefs().getString("manual", "") ?: ""
        set(v) { prefs().edit().putString("manual", v).apply() }

    private val incoming get() = File(app.filesDir, "incoming")

    private val callbacks = Callbacks(
        onEventJson = { handle(it) },
        opener = { uri -> Saf.openDetachedFd(app, uri) },
    )

    @Synchronized
    fun ensureStarted(ctx: Context) {
        if (started) return
        app = ctx.applicationContext
        incoming.mkdirs()
        val r = JSONObject(
            NativeBridge.start(app.filesDir.absolutePath, incoming.absolutePath, deviceName, 47600, autoAccept, tuning, callbacks),
        )
        if (r.has("error")) {
            _error.value = r.getString("error")
            Log.e(TAG, "native start failed: ${r.getString("error")}")
            return
        }
        started = true
        _error.value = null
        _buffers.value = r.optLong("rcvBuf") to r.optLong("sndBuf")
        val me = Me(r.getString("id"), r.getInt("port"), deviceName)
        _me.value = me
        nsd = Nsd(app) { nsdList = it; publishDevices() }.also {
            it.start(me.id, me.name, "phone", me.port)
        }
        refreshLists()
        startLivenessSweep()
    }

    // Two independent sources of nearby devices: the platform's NsdManager, and the core's own multicast browse.
    // The second one matters on a phone that hosts a hotspot, where NsdManager never sees the laptop joined to it.
    private var nsdList: List<Device> = emptyList()
    private var coreList: List<Device> = emptyList()

    @Synchronized
    private fun publishDevices() {
        val seen = nsdList.map { it.id }.toSet()
        _devices.value = (nsdList + coreList.filter { it.id !in seen }).sortedBy { it.name.lowercase() }
    }

    private var sweep: kotlinx.coroutines.Job? = null

    /**
     * mDNS entries linger until their TTL runs out when a peer quits without a goodbye (crash, kill, sleep, Wi-Fi drop).
     * Every few seconds, check that each listed device still answers; hide it after two misses in a row.
     */
    private fun startLivenessSweep() {
        sweep?.cancel()
        sweep = scope.launch {
            val misses = HashMap<String, Int>()
            while (true) {
                kotlinx.coroutines.delay(12_000)
                val snapshot = _devices.value
                misses.keys.retainAll(snapshot.map { it.id }.toSet())
                for (d in snapshot) {
                    val up = runCatching { NativeBridge.probe(d.addr, d.id) }.getOrDefault(true)
                    if (up) { misses.remove(d.id); continue }
                    val n = (misses[d.id] ?: 0) + 1
                    misses[d.id] = n
                    if (n >= 2) { Log.i(TAG, "device ${d.name} stopped answering; hiding it"); nsd?.forget(d.id); coreList = coreList.filter { it.id != d.id }; publishDevices(); misses.remove(d.id) }
                }
            }
        }
    }

    @Synchronized
    fun shutdown() {
        sweep?.cancel(); sweep = null
        if (!started) return
        started = false
        nsd?.stop(); nsd = null
        NativeBridge.stop()
        _me.value = null
        nsdList = emptyList(); coreList = emptyList()
        _devices.value = emptyList()
    }

    fun isRunning() = started

    private var lastNudge = 0L

    /** Restart mDNS now (user asked, or the network changed). */
    fun refreshDiscovery() { nsd?.refresh(); lastNudge = System.currentTimeMillis() }

    /** Called when the app returns to the foreground: restart discovery only if nothing was found recently. */
    fun nudgeDiscovery() {
        if (started && _devices.value.isEmpty() && System.currentTimeMillis() - lastNudge > 10_000) refreshDiscovery()
    }

    /** "system" | "light" | "dark" */
    val themeMode = MutableStateFlow("dark")

    fun setTheme(mode: String) {
        prefs().edit().putString("theme", mode).apply()
        themeMode.value = mode
    }

    fun init(ctx: Context) {
        if (!::app.isInitialized) app = ctx.applicationContext
        SavedStore.init(app)
        themeMode.value = prefs().getString("theme", "dark") ?: "dark"
    }

    fun refreshLists() {
        if (!started) return
        scope.launch {
            runCatching {
                val h = JSONObject(NativeBridge.history(100)).optJSONArray("items")
                _history.value = (0 until (h?.length() ?: 0)).map {
                    val o = h!!.getJSONObject(it)
                    HistoryItem(o.getString("id"), o.getString("dir"), o.getString("peer"), o.getInt("files"), o.getString("title"),
                        o.getLong("bytes"), o.getLong("at"), o.getString("status"), o.optString("error").takeIf { s -> s.isNotEmpty() && s != "null" })
                }
                val t = JSONObject(NativeBridge.trusted()).optJSONArray("devices")
                _trusted.value = (0 until (t?.length() ?: 0)).map { Trusted(t!!.getJSONObject(it).getString("id"), t.getJSONObject(it).getString("name")) }
            }.onFailure { Log.w(TAG, "refresh failed", it) }
        }
    }

    // ---- user actions ------------------------------------------------------

    /** The device the user tapped before choosing files: once files are picked they go straight to it. */
    private val _armed = MutableStateFlow<Device?>(null)
    val armed: StateFlow<Device?> = _armed.asStateFlow()
    private var armedAt = 0L

    /** Tap on a device with nothing selected: remember it, then the caller opens the file picker. */
    fun arm(device: Device) { _armed.value = device; armedAt = SystemClock.elapsedRealtime() }

    /** Files were picked. With a tapped device they wait on a "Send to ..." card; they are never sent without a tap. */
    fun stage(files: List<PickedFile>) {
        if (files.isEmpty() || SystemClock.elapsedRealtime() - armedAt > 120_000) _armed.value = null // cancelled or stale
        if (files.isEmpty()) return
        // picking again adds to what is already selected (the same file is never listed twice)
        val have = _pending.value
        _pending.value = have + files.filter { f -> have.none { it.uri == f.uri } }
    }

    /** Remove the activity list (the received files themselves stay where they are). */
    fun clearHistory() {
        scope.launch {
            runCatching { NativeBridge.clearHistory() }
            SavedStore.clear(app)
            refreshLists()
        }
    }
    fun clearPending() { _pending.value = emptyList(); _armed.value = null }

    /** Send the staged files to [device] (discovered) or to a typed `ip[:port]`. Returns an error text or null. */
    fun sendPending(addr: String, peerId: String, peerName: String): String? {
        val files = _pending.value
        if (files.isEmpty()) return humanError("nothing selected")
        if (!started) return humanError("not started")
        val r = JSONObject(NativeBridge.send(addr, peerId, peerName, Saf.toJson(files)))
        if (r.has("error")) return humanError(r.getString("error"), peerName, sending = true, named = true)
        sentFiles[r.getString("job")] = files
        _pending.value = emptyList()
        _armed.value = null
        return null
    }

    fun respondOffer(id: String, accept: Boolean) {
        NativeBridge.respondOffer(id, accept, "")
        _offers.update { l -> l.filterNot { it.id == id } }
    }

    fun respondPair(id: String, approve: Boolean) {
        NativeBridge.respondPair(id, approve)
        _pairReqs.update { l -> l.filterNot { it.id == id } }
        if (approve) refreshLists()
    }

    fun cancel(job: String) = NativeBridge.cancel(job)

    /** Restart the native core (e.g. after changing the network tuning). */
    fun restart() {
        shutdown()
        ensureStarted(app)
    }

    /** Network-only speed test to a device: separates "the network is slow" from "disk/CPU is slow". */
    fun runLinkTest(addr: String, peerId: String, name: String, megabytes: Int = 32) {
        if (!started || _linkTesting.value) return
        _linkTesting.value = true
        _link.value = LinkState.Running(name)
        scope.launch(Dispatchers.IO) {
            val r = runCatching { JSONObject(NativeBridge.linkTest(addr, peerId, megabytes)) }
            _link.value = r.fold(
                onSuccess = { j ->
                    if (j.has("error")) LinkState.Failed(humanError(j.getString("error"), name))
                    else LinkState.Done(LinkResult(name, j.getDouble("upMBps"), j.getDouble("downMBps"), j.getLong("connectMs"),
                        j.getDouble("rttMs"), j.getInt("mtu"), j.getDouble("upLossPct"), j.getDouble("downLossPct")))
                },
                onFailure = { LinkState.Failed(humanError(it.message, name)) },
            )
            _linkTesting.value = false
        }
    }

    fun dismiss(job: String) = _xfers.update { l -> l.filterNot { it.job == job } }

    fun beginPairing(): String? {
        val ip = Nsd.localIpv4() ?: return "Connect to Wi-Fi first, then show the code."
        val port = _me.value?.port ?: return humanError("not started")
        val r = JSONObject(NativeBridge.beginPairing("$ip:$port", 300))
        if (r.has("error")) return humanError(r.getString("error"))
        _pairUri.value = r.getString("uri")
        return null
    }

    fun endPairing() {
        NativeBridge.endPairing()
        _pairUri.value = null
    }

    fun pairWith(uri: String): String? {
        val r = JSONObject(NativeBridge.pairWith(uri.trim()))
        if (r.has("error")) return humanError(r.getString("error"))
        _pairStatus.value = PairStatus(job = r.getString("job"), text = "Connecting…")
        return null
    }

    fun resetPairStatus() { _pairStatus.value = PairStatus() }

    fun revoke(id: String) { NativeBridge.revoke(id); refreshLists() }

    // ---- events from Rust --------------------------------------------------

    private fun upsert(x: Xfer) = _xfers.update { l ->
        if (l.any { it.job == x.job }) l.map { if (it.job == x.job) x else it } else listOf(x) + l
    }

    private fun handle(e: JSONObject) {
        val type = e.getString("t")
        when (type) {
            "offer" -> {
                val files = e.getJSONArray("files")
                val o = Offer(
                    e.getString("offerId"), e.getString("peerName"), e.getString("peerId"), e.getBoolean("trusted"),
                    e.getInt("fileCount"), e.getLong("total"), if (e.isNull("free")) null else e.getLong("free"),
                    (0 until files.length()).map { files.getJSONObject(it).let { f -> f.getString("path") to f.getLong("size") } },
                )
                _offers.update { it + o }
            }
            "pair_request" -> _pairReqs.update { it + PairReq(e.getString("pairId"), e.getString("peerName"), e.getString("code")) }
            "progress" -> {
                val job = e.getString("job")
                val done = e.getLong("done")
                val now = SystemClock.elapsedRealtime()
                val prev = lastSample[job]
                val speed = if (prev != null && now > prev.first) (done - prev.second) * 1000.0 / (now - prev.first) else 0.0
                if (prev == null || now - prev.first >= 500) lastSample[job] = now to done
                val old = _xfers.value.firstOrNull { it.job == job }
                if (old?.finished == true || old?.state == "saving") return
                upsert(Xfer(job, e.getString("dir"), e.getString("peer"), e.getString("state"), done, e.getLong("total"),
                    e.getInt("filesDone"), e.getInt("filesTotal"), e.optString("current").takeIf { it.isNotEmpty() && it != "null" },
                    if (speed > 0) speed else old?.speed ?: 0.0))
            }
            "finished" -> {
                val job = e.getString("job")
                lastSample.remove(job)
                val old = _xfers.value.firstOrNull { it.job == job }
                val ok = e.getBoolean("ok")
                val x = Xfer(job, e.getString("dir"), e.optString("peer", old?.peer ?: ""),
                    if (ok) "completed" else if (e.optBoolean("cancelled")) "cancelled" else "failed",
                    old?.total ?: e.optLong("bytes"), old?.total ?: e.optLong("bytes"), e.optInt("files"), e.optInt("files"),
                    old?.current, 0.0, true, ok,
                    e.optString("error").takeIf { it.isNotEmpty() && !e.optBoolean("cancelled") }?.let { humanError(it, e.optString("peer", old?.peer ?: ""), e.getString("dir") == "send") })
                val sent = sentFiles.remove(job)
                val x2 = if (ok && sent != null) {
                    val files = sent.map { SavedFile(it.rel.substringAfterLast('/'), it.uri, mimeFor(it.rel), it.size) }
                    SavedStore.put(app, e.optString("transferId").ifEmpty { job }, files)
                    x.copy(files = files)
                } else x
                upsert(x2)
                e.optJSONObject("stages")?.let { st ->
                    _lastTransfer.value = TransferDiag(e.getString("dir"), x.peer, x.total, e.optLong("elapsedMs"), st.optString("text"), st.optString("path"))
                }
                if (ok && e.getString("dir") == "recv") {
                    // not "done" yet: the files still have to be copied to Downloads, and only then can they be opened
                    upsert(x.copy(finished = false, ok = false, state = "saving"))
                    val paths = e.getJSONArray("paths")
                    scope.launch { publishReceived(x, (0 until paths.length()).map { paths.getString(it) }) }
                } else {
                    _events.tryEmit("finished" to e)
                }
                refreshLists()
            }
            "pair_code" -> _pairStatus.update { it.copy(code = e.getString("code"), text = "Check that the other device shows the same code") }
            "paired" -> {
                _pairStatus.value = PairStatus(code = _pairStatus.value.code, text = "Paired with ${e.getString("peerName")}", done = true)
                refreshLists()
            }
            "mdns" -> {
                val arr = e.getJSONArray("devices")
                coreList = (0 until arr.length()).map {
                    val o = arr.getJSONObject(it)
                    Device(o.getString("id"), o.getString("name"), o.optString("kind", "unknown"), o.getString("host"), o.getInt("port"))
                }
                Log.i(TAG, "core mDNS browse sees ${coreList.size} device(s): ${coreList.joinToString { it.name + "@" + it.addr }}")
                publishDevices()
            }
            "pair_failed" -> _pairStatus.value = PairStatus(text = humanError(e.getString("error")), failed = true)
        }
        if (type == "offer" || type == "pair_request") _events.tryEmit(type to e)
    }

    /** Rust reports canonical paths (/data/user/0 vs /data/data), so compare canonical forms. */
    private fun mimeFor(name: String): String =
        android.webkit.MimeTypeMap.getSingleton().getMimeTypeFromExtension(name.substringAfterLast('.', "").lowercase()) ?: "application/octet-stream"

    private fun relativeToIncoming(f: File): String {
        val root = incoming.canonicalFile
        val c = f.canonicalFile
        return if (c.startsWith(root)) c.relativeTo(root).invariantSeparatorsPath else f.name
    }

    /** Move verified, staged files to Downloads/Bolt (or the folder the user chose). */
    private fun publishReceived(x: Xfer, paths: List<String>) {
        val tree = destTree?.let { Uri.parse(it) }
        val saved = ArrayList<SavedFile>()
        var failures = 0
        val copyStart = SystemClock.elapsedRealtime()
        for (p in paths) {
            val f = File(p)
            val rel = relativeToIncoming(f)
            runCatching {
                saved += Saf.publish(app, f, rel, tree)
                f.delete()
            }.onFailure {
                failures++
                Log.e(TAG, "could not save $rel", it)
            }
        }
        // remove now-empty staging folders
        incoming.walkBottomUp().filter { it.isDirectory && it != incoming && it.list().isNullOrEmpty() }.forEach { it.delete() }
        val copyMs = SystemClock.elapsedRealtime() - copyStart
        _lastTransfer.update { it?.copy(copyMs = copyMs) }
        val dest = tree?.lastPathSegment ?: "Download/Bolt"
        val note = if (failures == 0) "Saved to $dest" else if (failures == 1) "One file couldn't be copied to your storage. It's still safe in Bolt's own folder." else "$failures files couldn't be copied to your storage. They're still safe in Bolt's own folder."
        SavedStore.put(app, x.job, saved)
        upsert(x.copy(note = note, ok = failures == 0, error = if (failures == 0) null else note, files = saved))
        _events.tryEmit("finished" to JSONObject().put("t", "finished").put("job", x.job).put("dir", "recv").put("ok", failures == 0).put("peer", x.peer).put("files", paths.size).put("note", note))
    }
}
