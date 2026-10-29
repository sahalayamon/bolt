package com.bolt.app

import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.widget.Toast
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

/** Where a transferred file lives now (received: its place in Downloads/the chosen folder; sent: the source document). */
data class SavedFile(val name: String, val uri: String, val mime: String, val size: Long)

/**
 * Remembers, per transfer id, the files that can be opened later (from the finished card and from Activity).
 * The core's history only keeps names and sizes, so locations are kept here, in a small JSON file.
 */
object SavedStore {
    private const val MAX_TRANSFERS = 300
    private val cache = LinkedHashMap<String, List<SavedFile>>()
    private var loaded = false
    private val _ids = MutableStateFlow<Set<String>>(emptySet())
    /** Transfer ids that have openable files. */
    val ids: StateFlow<Set<String>> = _ids.asStateFlow()

    private fun file(ctx: Context) = File(ctx.applicationContext.filesDir, "saved-files.json")

    @Synchronized
    private fun load(ctx: Context) {
        if (loaded) return
        loaded = true
        runCatching {
            val f = file(ctx)
            if (!f.exists()) return
            val root = JSONObject(f.readText())
            for (id in root.keys()) {
                val arr = root.getJSONArray(id)
                cache[id] = (0 until arr.length()).map {
                    val o = arr.getJSONObject(it)
                    SavedFile(o.getString("n"), o.getString("u"), o.optString("m", "*/*"), o.optLong("s"))
                }
            }
        }
        _ids.value = cache.keys.toSet()
    }

    fun init(ctx: Context) = load(ctx)

    @Synchronized
    fun put(ctx: Context, id: String, files: List<SavedFile>) {
        load(ctx)
        if (files.isEmpty()) return
        cache.remove(id)
        cache[id] = files
        while (cache.size > MAX_TRANSFERS) cache.remove(cache.keys.first())
        val root = JSONObject()
        cache.forEach { (k, v) ->
            root.put(k, JSONArray().also { a -> v.forEach { a.put(JSONObject().put("n", it.name).put("u", it.uri).put("m", it.mime).put("s", it.size)) } })
        }
        runCatching { file(ctx).writeText(root.toString()) }
        _ids.value = cache.keys.toSet()
    }

    @Synchronized
    fun clear(ctx: Context) {
        load(ctx)
        cache.clear()
        runCatching { file(ctx).delete() }
        _ids.value = emptySet()
    }

    @Synchronized
    fun get(ctx: Context, id: String): List<SavedFile> {
        load(ctx)
        return cache[id].orEmpty()
    }
}

fun toast(ctx: Context, message: String) {
    Handler(Looper.getMainLooper()).post { Toast.makeText(ctx.applicationContext, message, Toast.LENGTH_SHORT).show() }
}

/** Open a file in whatever app handles it; a short toast always says why when it cannot be opened. */
fun openSavedFile(ctx: Context, f: SavedFile) {
    val uri = Uri.parse(f.uri)
    val readable = runCatching { ctx.contentResolver.openFileDescriptor(uri, "r")?.use { true } ?: false }.getOrDefault(false)
    if (!readable) {
        toast(ctx, "File not found")
        return
    }
    val isApk = f.mime == "application/vnd.android.package-archive" || f.name.endsWith(".apk", ignoreCase = true)
    if (isApk && !ctx.packageManager.canRequestPackageInstalls()) {
        // Android only lets an app hand over an APK once the user allowed "install unknown apps" for it
        toast(ctx, "Allow Bolt to install apps, then tap Open again")
        runCatching {
            ctx.startActivity(
                Intent(android.provider.Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:${ctx.packageName}")).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
            )
        }
        return
    }
    val mime = f.mime.takeUnless { it.isEmpty() || it == "*/*" } ?: "application/octet-stream"
    val ext = f.name.substringAfterLast('.', "").ifEmpty { "this type of" }
    fun view(type: String) = Intent(Intent.ACTION_VIEW).setDataAndType(uri, type)
        .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
        .also { it.clipData = android.content.ClipData.newRawUri(f.name, uri) }
    try {
        ctx.startActivity(view(mime))
    } catch (_: ActivityNotFoundException) {
        // some apps register only for the wildcard; try once more before giving up
        try {
            ctx.startActivity(view("*/*"))
        } catch (_: ActivityNotFoundException) {
            toast(ctx, "No app on this phone can open $ext files")
        } catch (e: Exception) {
            toast(ctx, "Can't open this file: ${e.javaClass.simpleName}")
        }
    } catch (_: SecurityException) {
        toast(ctx, "Android blocked access to this file")
    } catch (e: Exception) {
        toast(ctx, "Can't open this file: ${e.javaClass.simpleName}")
    }
}
