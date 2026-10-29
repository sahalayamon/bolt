package com.bolt.app

import android.content.ContentValues
import android.content.Context
import android.net.Uri
import android.os.Environment
import android.provider.DocumentsContract
import android.provider.MediaStore
import android.provider.OpenableColumns
import androidx.documentfile.provider.DocumentFile
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

/** One file chosen for sending. [rel] is the `/`-separated path the receiver recreates. */
data class PickedFile(val uri: String, val rel: String, val size: Long, val mtime: Long)

object Saf {
    /** Describe a single document (file or photo) returned by a picker or share intent. */
    fun describe(ctx: Context, uri: Uri, relPrefix: String = ""): PickedFile? {
        var name: String? = null
        var size = -1L
        var mtime = 0L
        if (uri.scheme == "content") {
            runCatching {
                ctx.contentResolver.query(uri, null, null, null, null)?.use { c ->
                    if (c.moveToFirst()) {
                        c.getColumnIndex(OpenableColumns.DISPLAY_NAME).takeIf { it >= 0 }
                            ?.let { name = c.getString(it) }
                        c.getColumnIndex(OpenableColumns.SIZE).takeIf { it >= 0 && !c.isNull(it) }
                            ?.let { size = c.getLong(it) }
                        c.getColumnIndex(DocumentsContract.Document.COLUMN_LAST_MODIFIED)
                            .takeIf { it >= 0 && !c.isNull(it) }?.let { mtime = c.getLong(it) }
                    }
                }
            }
        } else if (uri.scheme == "file") {
            val f = File(uri.path ?: return null)
            name = f.name; size = f.length(); mtime = f.lastModified()
        }
        if (size < 0) { // some providers do not report a size; measure it
            size = runCatching {
                ctx.contentResolver.openAssetFileDescriptor(uri, "r")?.use { it.length }
            }.getOrNull() ?: -1L
        }
        if (size < 0) return null
        val n = (name ?: uri.lastPathSegment ?: "file").substringAfterLast('/')
        return PickedFile(uri.toString(), relPrefix + n, size, mtime)
    }

    /** Recursively expand a folder chosen with ACTION_OPEN_DOCUMENT_TREE. */
    fun expandTree(ctx: Context, tree: Uri): List<PickedFile> {
        val root = DocumentFile.fromTreeUri(ctx, tree) ?: return emptyList()
        val out = ArrayList<PickedFile>()
        fun walk(dir: DocumentFile, prefix: String) {
            for (child in dir.listFiles()) {
                val name = child.name ?: continue
                if (child.isDirectory) walk(child, "$prefix$name/")
                else if (child.isFile) out += PickedFile(child.uri.toString(), "$prefix$name", child.length(), child.lastModified())
            }
        }
        walk(root, (root.name ?: "folder") + "/")
        return out
    }

    fun toJson(files: List<PickedFile>): String = JSONArray().also { arr ->
        files.forEach {
            arr.put(JSONObject().put("uri", it.uri).put("rel", it.rel).put("size", it.size).put("mtime", it.mtime))
        }
    }.toString()

    /** Open a document read-only and give the raw fd to native code (it owns and closes it). */
    fun openDetachedFd(ctx: Context, uri: String): Int {
        val pfd = ctx.contentResolver.openFileDescriptor(Uri.parse(uri), "r") ?: return -1
        return pfd.detachFd()
    }

    /**
     * Copy a received (staged) file to its final home and return a human-readable location.
     * With [treeUri] the user's chosen folder receives it; otherwise Downloads/Bolt via MediaStore.
     */
    fun publish(ctx: Context, staged: File, rel: String, treeUri: Uri?): SavedFile {
        // never trust a relative path: drop empty, '.' and '..' components
        val parts = rel.split('/').filter { it.isNotEmpty() && it != "." && it != ".." }.ifEmpty { listOf("file") }
        val name = parts.last()
        val dirs = parts.dropLast(1)
        if (treeUri != null) {
            var dir = DocumentFile.fromTreeUri(ctx, treeUri) ?: error("destination folder is gone")
            for (d in dirs) dir = dir.findFile(d)?.takeIf { it.isDirectory } ?: dir.createDirectory(d) ?: error("cannot create $d")
            val mime = mimeOf(name)
            // createFile never overwrites: the provider picks a unique name on conflict.
            val target = dir.createFile(mime, name) ?: error("cannot create $name")
            copyInto(ctx, staged, target.uri)
            return SavedFile(name, target.uri.toString(), mime, staged.length())
        }
        val relPath = (listOf(Environment.DIRECTORY_DOWNLOADS, "Bolt") + dirs).joinToString("/")
        val values = ContentValues().apply {
            put(MediaStore.MediaColumns.DISPLAY_NAME, name)
            put(MediaStore.MediaColumns.MIME_TYPE, mimeOf(name))
            put(MediaStore.MediaColumns.RELATIVE_PATH, relPath)
            put(MediaStore.MediaColumns.IS_PENDING, 1)
        }
        val resolver = ctx.contentResolver
        val dest = resolver.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values) ?: error("MediaStore refused $name")
        try {
            copyInto(ctx, staged, dest)
            resolver.update(dest, ContentValues().apply { put(MediaStore.MediaColumns.IS_PENDING, 0) }, null, null)
        } catch (e: Exception) {
            resolver.delete(dest, null, null)
            throw e
        }
        return SavedFile(name, dest.toString(), mimeOf(name), staged.length())
    }

    /**
     * Copy a verified staged file to its destination. Uses a kernel-side channel copy (no user-space buffer round
     * trip) and falls back to a plain stream copy if the provider's descriptor does not support it.
     */
    private fun copyInto(ctx: Context, staged: File, dest: Uri) {
        val fast = runCatching {
            ctx.contentResolver.openFileDescriptor(dest, "wt")!!.use { pfd ->
                java.io.FileOutputStream(pfd.fileDescriptor).use { out ->
                    java.io.FileInputStream(staged).use { input ->
                        val src = input.channel
                        val dst = out.channel
                        val size = src.size()
                        var pos = 0L
                        while (pos < size) {
                            val n = src.transferTo(pos, size - pos, dst)
                            if (n <= 0) error("transferTo made no progress")
                            pos += n
                        }
                        dst.force(false)
                    }
                }
            }
        }
        if (fast.isSuccess) return
        ctx.contentResolver.openOutputStream(dest, "wt")!!.use { out -> staged.inputStream().use { it.copyTo(out, 1024 * 1024) } }
    }

    private fun mimeOf(name: String): String {
        val ext = name.substringAfterLast('.', "").lowercase()
        return android.webkit.MimeTypeMap.getSingleton().getMimeTypeFromExtension(ext) ?: "application/octet-stream"
    }
}
