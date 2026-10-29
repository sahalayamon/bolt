package com.bolt.app

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.Log
import java.net.Inet4Address
import java.net.InetAddress
import java.net.NetworkInterface
import java.util.ArrayDeque

data class Device(val id: String, val name: String, val kind: String, val host: String, val port: Int) {
    val addr get() = if (host.contains(':')) "[$host]:$port" else "$host:$port"
}

/**
 * mDNS/DNS-SD via the platform NsdManager. Wire format matches crates/discovery
 * (`_bolt._udp`, TXT: v, id, n, k) so desktop and phone see each other.
 *
 * Self-healing, because real networks are messy:
 *  - registration + discovery restart when the default network changes (joining Wi-Fi after the app started),
 *  - failed starts are retried, a hung resolve cannot block the queue (timeout), ALREADY_ACTIVE is retried,
 *  - on Android 14+ the service-info callback API is used (it reports every address of the peer),
 *  - when a peer advertises several addresses (VirtualBox, VPN, Docker…) the one on this phone's subnet wins.
 */
class Nsd(ctx: Context, private val onDevices: (List<Device>) -> Unit) {
    private val app = ctx.applicationContext
    private val nsd = app.getSystemService(Context.NSD_SERVICE) as NsdManager
    private val cm = app.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
    private val main = Handler(Looper.getMainLooper())

    private class Params(val selfId: String, val name: String, val kind: String, val port: Int)

    private var params: Params? = null
    private val found = LinkedHashMap<String, Device>() // by service name
    private val resolveQueue = ArrayDeque<NsdServiceInfo>()
    private val attempts = HashMap<String, Int>()
    private val infoCallbacks = HashMap<String, NsdManager.ServiceInfoCallback>()
    private var resolving = false
    private var resolveToken = 0
    private var reg: NsdManager.RegistrationListener? = null
    private var disc: NsdManager.DiscoveryListener? = null
    private var generation = 0 // bumped on every (re)start so stale callbacks are ignored
    private var netCb: ConnectivityManager.NetworkCallback? = null
    private var startedAt = 0L

    @Synchronized
    fun start(selfId: String, name: String, kind: String, port: Int) {
        params = Params(selfId, name, kind, port)
        begin(0)
        if (netCb == null) {
            val cb = object : ConnectivityManager.NetworkCallback() {
                override fun onAvailable(network: Network) = networkChanged()
                override fun onLost(network: Network) = networkChanged()
            }
            runCatching { cm.registerDefaultNetworkCallback(cb); netCb = cb }
        }
    }

    /** Restart advertising and browsing (network changed, or the user pulled to refresh). */
    fun refresh() {
        synchronized(this) { if (params == null) return }
        teardown()
        publish()
        main.postDelayed({ synchronized(this) { if (params != null) begin(0) } }, 500)
    }

    @Synchronized
    fun stop() {
        netCb?.let { runCatching { cm.unregisterNetworkCallback(it) } }
        netCb = null
        params = null
        teardown()
        publish()
    }

    /** Drop a device that stopped answering (it left without an mDNS goodbye). It returns if it re-announces. */
    fun forget(deviceId: String) {
        synchronized(this) { found.entries.removeAll { it.value.id == deviceId } }
        publish()
    }

    // ---- internals -------------------------------------------------------

    private fun networkChanged() {
        // The first callback fires immediately on registration; ignore it.
        if (System.currentTimeMillis() - startedAt < 3000) return
        Log.i(TAG, "network changed: restarting discovery")
        main.removeCallbacks(refreshRunnable)
        main.postDelayed(refreshRunnable, 1500)
    }

    private val refreshRunnable = Runnable { refresh() }

    @Synchronized
    private fun begin(retry: Int) {
        val p = params ?: return
        startedAt = System.currentTimeMillis()
        val gen = ++generation
        val info = NsdServiceInfo().apply {
            serviceName = "bolt-" + p.selfId.take(8)
            serviceType = TYPE
            setPort(p.port)
            setAttribute("v", "1")
            setAttribute("id", p.selfId)
            setAttribute("n", p.name.take(60))
            setAttribute("k", p.kind)
        }
        reg = object : NsdManager.RegistrationListener {
            override fun onServiceRegistered(i: NsdServiceInfo) { Log.i(TAG, "advertising as ${i.serviceName}") }
            override fun onRegistrationFailed(i: NsdServiceInfo, code: Int) { Log.w(TAG, "register failed $code") }
            override fun onServiceUnregistered(i: NsdServiceInfo) {}
            override fun onUnregistrationFailed(i: NsdServiceInfo, code: Int) {}
        }.also { runCatching { nsd.registerService(info, NsdManager.PROTOCOL_DNS_SD, it) } }

        disc = object : NsdManager.DiscoveryListener {
            override fun onDiscoveryStarted(t: String) { Log.i(TAG, "discovery started") }
            override fun onStartDiscoveryFailed(t: String, code: Int) {
                Log.w(TAG, "discovery failed $code (retry $retry)")
                if (retry < 5) main.postDelayed({ synchronized(this@Nsd) { if (gen == generation) { teardownDiscovery(); begin(retry + 1) } } }, 3000)
            }
            override fun onStopDiscoveryFailed(t: String, code: Int) {}
            override fun onDiscoveryStopped(t: String) {}
            override fun onServiceFound(i: NsdServiceInfo) { if (gen == generation) onFound(i) }
            override fun onServiceLost(i: NsdServiceInfo) {
                if (gen != generation) return
                synchronized(this@Nsd) { found.remove(i.serviceName); unregisterInfoCallback(i.serviceName) }
                publish()
            }
        }.also { runCatching { nsd.discoverServices(TYPE, NsdManager.PROTOCOL_DNS_SD, it) } }
    }

    @Synchronized
    private fun teardownDiscovery() {
        disc?.let { runCatching { nsd.stopServiceDiscovery(it) } }
        disc = null
        reg?.let { runCatching { nsd.unregisterService(it) } }
        reg = null
    }

    @Synchronized
    private fun teardown() {
        generation++ // invalidate callbacks from the previous session
        teardownDiscovery()
        infoCallbacks.keys.toList().forEach { unregisterInfoCallback(it) }
        found.clear(); resolveQueue.clear(); attempts.clear()
        resolving = false; resolveToken++
    }

    private fun unregisterInfoCallback(name: String) {
        if (Build.VERSION.SDK_INT >= 34) infoCallbacks.remove(name)?.let { runCatching { nsd.unregisterServiceInfoCallback(it) } }
    }

    private fun onFound(i: NsdServiceInfo) {
        if (Build.VERSION.SDK_INT >= 34 && registerInfoCallback(i)) return
        synchronized(this) { resolveQueue.add(i) }
        pump()
    }

    // ---- Android 14+: service-info callback (all addresses, live updates) ----

    @android.annotation.SuppressLint("NewApi")
    private fun registerInfoCallback(i: NsdServiceInfo): Boolean {
        val name = i.serviceName
        synchronized(this) { if (infoCallbacks.containsKey(name)) return true }
        val cb = object : NsdManager.ServiceInfoCallback {
            override fun onServiceInfoCallbackRegistrationFailed(errorCode: Int) {
                Log.w(TAG, "info callback failed $errorCode for $name; falling back to resolve")
                synchronized(this@Nsd) { infoCallbacks.remove(name); resolveQueue.add(i) }
                pump()
            }
            override fun onServiceUpdated(info: NsdServiceInfo) { accept(info, info.hostAddresses) }
            override fun onServiceLost() {
                synchronized(this@Nsd) { found.remove(name) }
                publish()
            }
            override fun onServiceInfoCallbackUnregistered() {}
        }
        return runCatching {
            nsd.registerServiceInfoCallback(i, { it.run() }, cb)
            synchronized(this) { infoCallbacks[name] = cb }
            true
        }.getOrElse { false }
    }

    // ---- legacy resolve, one at a time, with a timeout ----

    private fun pump() {
        val next: NsdServiceInfo
        val token: Int
        synchronized(this) {
            if (resolving) return
            next = resolveQueue.poll() ?: return
            resolving = true
            token = ++resolveToken
        }
        val gen = generation
        main.postDelayed({ // a resolve that never calls back must not block everything else
            synchronized(this) { if (resolving && resolveToken == token) { Log.w(TAG, "resolve timed out for ${next.serviceName}"); resolving = false } }
            pump()
        }, 6000)
        @Suppress("DEPRECATION")
        runCatching {
            nsd.resolveService(next, object : NsdManager.ResolveListener {
                override fun onResolveFailed(i: NsdServiceInfo, code: Int) {
                    if (gen == generation && code == NsdManager.FAILURE_ALREADY_ACTIVE) {
                        val n = synchronized(this@Nsd) { (attempts[i.serviceName] ?: 0) + 1 }
                        if (n <= 3) {
                            synchronized(this@Nsd) { attempts[i.serviceName] = n }
                            main.postDelayed({ if (gen == generation) { synchronized(this@Nsd) { resolveQueue.add(i) }; pump() } }, 1000L * n)
                        }
                    }
                    done(token)
                }
                override fun onServiceResolved(i: NsdServiceInfo) {
                    if (gen == generation) @Suppress("DEPRECATION") accept(i, listOfNotNull(i.host))
                    done(token)
                }
            })
        }.onFailure { done(token) }
    }

    private fun done(token: Int) {
        synchronized(this) { if (resolveToken == token) resolving = false }
        pump()
    }

    // ---- shared ----

    private fun accept(i: NsdServiceInfo, addrs: List<InetAddress>) {
        val self = synchronized(this) { params?.selfId } ?: return
        fun a(k: String) = i.attributes[k]?.toString(Charsets.UTF_8)
        val id = a("id")
        val host = pickAddress(addrs)
        if (id == null || id == self || a("v") != "1" || host == null) return
        synchronized(this) { found[i.serviceName] = Device(id, a("n") ?: "Unknown", a("k") ?: "unknown", host, i.port) }
        publish()
    }

    private fun publish() {
        val list = synchronized(this) {
            // one entry per device id; prefer IPv4 literal hosts
            found.values.groupBy { it.id }.map { (_, v) -> v.firstOrNull { !it.host.contains(':') } ?: v.first() }
        }
        onDevices(list)
    }

    companion object {
        const val TYPE = "_bolt._udp."
        private const val TAG = "BoltNsd"

        /** Prefer an IPv4 on our own /24; else any private IPv4; else a global/ULA IPv6; never link-local first. */
        fun pickAddress(addrs: List<InetAddress>): String? {
            val local = localIpv4()?.substringBeforeLast('.')
            val v4 = addrs.filterIsInstance<Inet4Address>().mapNotNull { it.hostAddress }
            v4.firstOrNull { local != null && it.substringBeforeLast('.') == local }?.let { return it }
            addrs.filterIsInstance<Inet4Address>().firstOrNull { it.isSiteLocalAddress }?.hostAddress?.let { return it }
            v4.firstOrNull()?.let { return it }
            return addrs.firstOrNull { !it.isLinkLocalAddress }?.hostAddress?.substringBefore('%')
                ?: addrs.firstOrNull()?.hostAddress?.substringBefore('%')
        }

        /** Best private IPv4 of this phone (what a peer should dial), or null when offline. */
        fun localIpv4(): String? = runCatching {
            NetworkInterface.getNetworkInterfaces().toList()
                .filter { it.isUp && !it.isLoopback && !it.isVirtual }
                .sortedBy { if (it.name.startsWith("wlan") || it.name.startsWith("ap")) 0 else 1 }
                .flatMap { it.inetAddresses.toList() }
                .filterIsInstance<Inet4Address>()
                .firstOrNull { it.isSiteLocalAddress }
                ?.hostAddress
        }.getOrNull()
    }
}
