package com.bolt.app

import org.json.JSONObject

/**
 * JNI surface of the Rust core (crates/bolt-android). Strings in, JSON out;
 * asynchronous results arrive through [Callbacks.onEvent].
 */
object NativeBridge {
    init {
        System.loadLibrary("bolt_android")
    }

    external fun start(
        dataDir: String,
        downloadDir: String,
        deviceName: String,
        port: Int,
        autoAccept: Boolean,
        tuning: String,
        cb: Callbacks,
    ): String

    external fun stop()
    external fun beginPairing(addr: String, ttlSecs: Int): String
    external fun endPairing()
    external fun respondOffer(offerId: String, accept: Boolean, reason: String)
    external fun respondPair(pairId: String, approve: Boolean)
    external fun cancel(job: String)
    external fun send(addr: String, expectedPeer: String, peerName: String, itemsJson: String): String
    external fun pairWith(uri: String): String
    external fun trusted(): String
    external fun revoke(id: String)
    external fun history(limit: Int): String
    external fun clearHistory(): Int
    external fun setAutoAccept(on: Boolean)
    external fun setDeviceName(name: String)
    /** Is a Bolt receiver reachable at addr (and is it peerId, if given)? Blocks up to ~2 s. */
    external fun probe(addr: String, peerId: String): Boolean
    /** Network-only speed test in both directions (no disk, no hashing). Blocks: call off the main thread. */
    external fun linkTest(addr: String, peerId: String, megabytes: Int): String
}

/** Called by Rust, from arbitrary native threads. */
class Callbacks(
    private val onEventJson: (JSONObject) -> Unit,
    private val opener: (String) -> Int,
) {
    @Suppress("unused") // invoked via JNI
    fun onEvent(json: String) {
        runCatching { onEventJson(JSONObject(json)) }
    }

    /** Returns a detached file descriptor for a content:// (or file) URI, or -1. */
    @Suppress("unused") // invoked via JNI
    fun openFd(uri: String): Int = runCatching { opener(uri) }.getOrDefault(-1)
}
