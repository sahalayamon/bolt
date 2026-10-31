package com.bolt.app

import android.content.ActivityNotFoundException
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp

/** Largest text the core accepts (UTF-8 bytes); keep in step with `MAX_TEXT_BYTES` in the protocol. */
private const val MAX_TEXT_BYTES = 1024 * 1024

/** Received text is shown up to this many characters; Copy always copies all of it. */
private const val SHOWN_CHARS = 20_000

/** Prefix of the history title of a text item (see `text_title` in the native bridge). */
const val TEXT_TITLE_PREFIX = "Text · "

/** The whole text is one http(s) URL (no spaces, a host): the only thing Bolt will offer to open. */
fun webUrlOrNull(text: String): Uri? {
    val t = text.trim()
    if (t.isEmpty() || t.any { it.isWhitespace() || it.isISOControl() }) return null
    val uri = runCatching { Uri.parse(t) }.getOrNull() ?: return null
    val scheme = uri.scheme?.lowercase()
    if (scheme != "http" && scheme != "https") return null
    return uri.takeIf { !it.host.isNullOrEmpty() }
}

/** One line for an offer: "Text · 12 characters", "3 files", "2 files and a text", ... */
fun Offer.what(): String {
    val fileOnly = fileCount - texts.size
    return when {
        texts.isEmpty() -> if (fileCount == 1) files.firstOrNull()?.first?.substringAfterLast('/') ?: "1 file" else "$fileCount files"
        fileOnly <= 0 -> if (texts.size == 1) textSizeLabel(texts[0]) else "${texts.size} texts"
        else -> "$fileOnly file${if (fileOnly == 1) "" else "s"} and ${if (texts.size == 1) "a text" else "${texts.size} texts"}"
    }
}

/** The offer only carries the size in bytes, which equals the character count for plain ASCII text and links. */
fun textSizeLabel(n: Long) = "Text · $n character${if (n == 1L) "" else "s"}"

/** Sheet for sending a note or a link: a multi-line field, Paste, and where to send it (a device or an address). */
@Composable
fun TextSendSheet(initial: String, onDismiss: () -> Unit) {
    val c = Fd.colors
    val ctx = LocalContext.current
    val devices by Engine.devices.collectAsState()
    var text by remember { mutableStateOf(initial) }
    // "address" = the typed ip[:port]; otherwise a device id. A lone device is picked for you.
    var target by remember { mutableStateOf(Engine.armed.value?.id ?: devices.singleOrNull()?.id) }
    var manual by remember { mutableStateOf(Engine.lastManual) }
    var err by remember { mutableStateOf<String?>(null) }
    val tooBig = text.toByteArray(Charsets.UTF_8).size > MAX_TEXT_BYTES

    DecisionSheet(onDismiss) {
        Text("Send text or a link", style = MaterialTheme.typography.titleLarge, color = c.text)
        FdField(text, { text = it; err = null }, "Type or paste text, or a link", singleLine = false, maxLines = 6, isError = tooBig)
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            Text(
                if (tooBig) "Too long. The limit is 1 MiB." else "${text.length} character${if (text.length == 1) "" else "s"}",
                Modifier.weight(1f), style = MaterialTheme.typography.bodySmall, color = if (tooBig) c.bad else c.textSecondary,
            )
            FdButton("Paste", { pasteFromClipboard(ctx)?.let { text = it; err = null } }, style = BtnStyle.Secondary, fill = false, height = 40.dp)
        }
        Text("Send to", style = MaterialTheme.typography.labelMedium, color = c.textSecondary)
        if (devices.isEmpty()) Text("No devices found on this network yet. You can still send to an address.", style = MaterialTheme.typography.bodyMedium, color = c.textTertiary)
        Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(Space.s)) {
            devices.forEach { d -> TargetChip(d.name, target == d.id) { target = d.id; err = null } }
            TargetChip("Address", target == "address") { target = "address"; err = null }
        }
        if (target == "address") {
            FdField(manual, { manual = it; Engine.lastManual = it; err = null }, "192.168.1.20:47600", isError = err != null)
        }
        err?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = c.bad) }
        Row(horizontalArrangement = Arrangement.spacedBy(Space.m)) {
        FdButton("Cancel", onDismiss, Modifier.weight(1f), BtnStyle.Secondary, fill = false)
        FdButton(
            "Send",
            {
                val dev = devices.firstOrNull { it.id == target }
                err = when {
                    target == "address" -> Engine.sendText(manual.trim(), "", manual.trim(), text)
                    dev != null -> Engine.sendText(dev.addr, dev.id, dev.name, text)
                    else -> "Choose where to send it."
                }
                if (err == null) onDismiss()
            },
            Modifier.weight(1f), fill = false,
            enabled = text.isNotBlank() && !tooBig && target != null && (target != "address" || manual.isNotBlank()),
        )
        }
    }
}

@Composable
private fun TargetChip(label: String, selected: Boolean, onClick: () -> Unit) {
    val c = Fd.colors
    Box(
        Modifier.height(40.dp).clip(RoundedCornerShape(20.dp)).background(if (selected) c.accent else c.accentSoft)
            .clickable(role = Role.RadioButton, onClick = onClick).padding(horizontal = Space.l),
        contentAlignment = Alignment.Center,
    ) {
        Text(label, style = MaterialTheme.typography.labelLarge, color = if (selected) c.onAccent else c.accent, maxLines = 1)
    }
}

/**
 * Text or a link that someone sent. It is drawn as plain text (never parsed as markup or made clickable), and a link
 * is only opened by tapping Open, which is offered for a well-formed http(s) URL.
 */
@Composable
fun ReceivedTextSheet(t: ReceivedText, more: Int) {
    val c = Fd.colors
    val ctx = LocalContext.current
    val url = remember(t.id) { webUrlOrNull(t.text) }
    val shown = if (t.text.length > SHOWN_CHARS) t.text.take(SHOWN_CHARS) + "…" else t.text
    DecisionSheet({ Engine.dismissText(t.id) }) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(Space.s)) {
            PickIcon(if (url != null) "link" else "text", c.accent, Modifier.size(26.dp))
            Text("${if (url != null) "Link" else "Text"} from ${t.peer}", style = MaterialTheme.typography.titleLarge, color = c.text)
        }
        // read-only but selectable; plain text, never parsed or linkified
        Box(Modifier.fillMaxWidth().heightIn(min = 72.dp, max = 260.dp).clip(RoundedCornerShape(14.dp)).background(c.background).verticalScroll(rememberScrollState()).padding(Space.m)) {
            SelectionContainer { Text(shown, style = MaterialTheme.typography.bodyLarge, color = c.text) }
        }
        if (t.text.length > SHOWN_CHARS) Text("Showing the first $SHOWN_CHARS characters. Copy gets all of it.", style = MaterialTheme.typography.bodySmall, color = c.textTertiary)
        if (url != null) Text("Only open links from people you trust.", style = MaterialTheme.typography.bodySmall, color = c.textTertiary)
        if (more > 0) Text("+$more more", style = MaterialTheme.typography.bodySmall, color = c.textTertiary)
        Row(horizontalArrangement = Arrangement.spacedBy(Space.m)) {
            FdButton("Done", { Engine.dismissText(t.id) }, Modifier.weight(1f), BtnStyle.Secondary, fill = false)
            FdButton("Copy", { copyToClipboard(ctx, t.text) }, Modifier.weight(1f), if (url == null) BtnStyle.Primary else BtnStyle.Secondary, fill = false)
            if (url != null) FdButton("Open", { openLink(ctx, url) }, Modifier.weight(1f), BtnStyle.Primary, fill = false)
        }
    }
}

private fun copyToClipboard(ctx: Context, text: String) {
    val cm = ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
    cm.setPrimaryClip(ClipData.newPlainText("Bolt", text))
    toast(ctx, "Copied")
}

private fun pasteFromClipboard(ctx: Context): String? {
    val cm = ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
    val s = cm.primaryClip?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.coerceToText(ctx)?.toString()
    if (s.isNullOrEmpty()) toast(ctx, "Nothing to paste")
    return s?.takeIf { it.isNotEmpty() }
}

private fun openLink(ctx: Context, url: Uri) {
    try {
        ctx.startActivity(Intent(Intent.ACTION_VIEW, url).addCategory(Intent.CATEGORY_BROWSABLE).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
    } catch (_: ActivityNotFoundException) {
        toast(ctx, "No app on this phone can open this link")
    }
}
