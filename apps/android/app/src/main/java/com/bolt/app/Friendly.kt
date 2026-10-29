package com.bolt.app

import java.util.Locale

/**
 * Turns the core's technical error text into something a person can act on.
 * Matching is on the stable parts of the messages in transfer-core's error type and the JNI layer; anything
 * unrecognised falls back to a calm generic sentence (the raw text stays in the log for debugging).
 */
fun humanError(raw: String?, peer: String? = null, sending: Boolean = true, named: Boolean = false): String {
    val m = raw?.trim().orEmpty()
    val l = m.lowercase(Locale.ROOT)
    // Most places that show the message already say who it is about (card title, notification title, activity row),
    // so the sentence itself says "they" unless it has to stand alone (named = true).
    val name = peer?.takeIf { it.isNotBlank() }
    val who = if (named && name != null) name else "they"
    val Who = who.replaceFirstChar { it.uppercase() }
    fun has(vararg k: String) = k.any { l.contains(it) }

    return when {
        m.isEmpty() -> "Something went wrong. Please try again."
        // ---- user decisions ----
        has("declined by the receiver") -> "$Who declined, so nothing was sent."
        has("cancelled by the other device") -> "$Who stopped the transfer."
        l == "cancelled" -> "Cancelled."
        // ---- finding / reaching the other device ----
        has("identity mismatch", "not the one expected") ->
            "That address now belongs to a different device than the one you picked. Refresh the list and pick it again."
        has("timed out") ->
            "$Who didn't answer in time. Check that Bolt is open on the other device and that you're on the same Wi-Fi."
        has("connection error", "connection refused", "unreachable", "no route", "connection reset", "connection lost", "closed by peer", "network is down") ->
            if (sending) (if (named && name != null) "Couldn't reach $name. " else "Couldn't connect. ") + "Make sure Bolt is open on the other device and you're both on the same Wi-Fi."
            else "The connection dropped. Make sure you're both still on the same Wi-Fi and try again."
        // ---- the files ----
        has("integrity check failed") -> "A file arrived damaged, so it wasn't kept. Try sending it again."
        has("not enough free disk space") -> {
            val nums = Regex("\\d+").findAll(m).map { it.value.toLong() }.toList()
            if (nums.size >= 2) "Not enough space: this needs ${formatBytes(nums[0])} and only ${formatBytes(nums[1])} is free. Free up some space and try again."
            else "There isn't enough free space on this phone for these files."
        }
        has("no such file", "not found", "could not open content", "openfd failed", "could not open") ->
            "One of the files was moved or deleted before it could be sent. Pick it again."
        has("permission denied", "securityexception") -> "Bolt isn't allowed to read or write that file. Pick it again, or choose another save folder in Settings."
        has("i/o error") -> "Couldn't read or write a file (${m.substringAfter(':').trim().replaceFirstChar { it.lowercase() }}). Check there is free space and try again."
        has("destination folder is gone", "cannot create", "mediastore refused") ->
            "Bolt couldn't save into the chosen folder. Pick the folder again in Settings."
        // ---- the other end misbehaving / version mismatch ----
        has("protocol violation", "other device reported an error", "invalid offer") ->
            "The two devices couldn't agree on the transfer. Make sure both are running the latest Bolt."
        // ---- local state ----
        l == "not started" || has("not running") -> "Bolt is still starting up. Give it a moment and try again."
        has("nothing to send", "nothing selected") -> "Pick something to send first."
        has("is not a valid ip") -> "That doesn't look like an address. Use something like 192.168.1.20 or 192.168.1.20:47600."
        has("not a bolt pairing code", "bad device id") -> "That doesn't look like a Bolt pairing code. Scan the QR on the other device again."
        has("bind:") -> "Bolt couldn't start listening for transfers. Close it fully and open it again."
        has("identity:", "store:", "setup error", "jni setup", "already started") -> "Bolt couldn't get ready. Close it fully and open it again."
        has("panic in native") -> "Bolt hit an unexpected problem. Please try again."
        else -> if (sending) "That didn't go through. Please try again." else "That transfer didn't finish. Please try again."
    }
}

/** Short title for the failure card / notification. */
fun failureTitle(peer: String?, sending: Boolean): String {
    val p = peer?.takeIf { it.isNotBlank() }
    return if (sending) (if (p != null) "Couldn't send to $p" else "Couldn't send")
    else (if (p != null) "Couldn't receive from $p" else "Couldn't receive")
}
