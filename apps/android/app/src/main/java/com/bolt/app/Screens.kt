package com.bolt.app

import android.Manifest
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.pm.PackageManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.Crossfade
import androidx.compose.animation.core.tween
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationBarItemDefaults
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.ContextCompat
import java.text.DateFormat
import java.util.Date

@Composable
fun BoltApp(picks: Pickers) {
    var dest by remember { mutableStateOf<Dest?>(null) }
    val offers by Engine.offers.collectAsState()
    val pairReqs by Engine.pairReqs.collectAsState()
    val c = Fd.colors

    androidx.activity.compose.BackHandler(enabled = dest != null) { dest = null }

    Box(Modifier.fillMaxSize().background(if (dest == null) c.top else c.background).statusBarsPadding()) {
        Crossfade(dest, animationSpec = tween(Motion.Standard, easing = Motion.ease), label = "nav") { d ->
            Box(Modifier.fillMaxSize().let { if (d != null) it.navigationBarsPadding() else it }) {
                when (d) {
                    null -> HomeScreen(picks) { dest = it }
                    Dest.Pair -> PairScreen { dest = null }
                    Dest.Activity -> ActivityScreen { dest = null }
                    Dest.Settings -> SettingsScreen(picks) { dest = null }
                    Dest.Diagnostics -> DiagnosticsScreen { dest = null }
                }
            }
        }
    }

    if (dest != null) offers.firstOrNull()?.let { OfferSheet(it) }
    if (offers.isEmpty()) pairReqs.firstOrNull()?.let { PairRequestSheet(it) }
}

private fun kindLabel(k: String) = when (k) { "desktop" -> "Computer"; "phone" -> "Phone"; "tablet" -> "Tablet"; else -> "Device" }

@Composable
private fun CodeDisplay(code: String) {
    Text(
        code.chunked(3).joinToString("  "), modifier = Modifier.fillMaxWidth(), textAlign = TextAlign.Center, color = Fd.colors.text,
        style = TextStyle(fontSize = 38.sp, fontWeight = FontWeight.SemiBold, letterSpacing = 2.sp, fontFeatureSettings = "tnum"),
    )
}

// ---- Pair ----------------------------------------------------------------

@Composable
private fun PairScreen(onBack: () -> Unit) {
    val ctx = LocalContext.current
    val uri by Engine.pairUri.collectAsState()
    val status by Engine.pairStatus.collectAsState()
    val trusted by Engine.trusted.collectAsState()
    var scanning by remember { mutableStateOf(false) }
    var paste by remember { mutableStateOf("") }
    var err by remember { mutableStateOf<String?>(null) }
    val c = Fd.colors

    val camera = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        scanning = granted
        if (!granted) err = "Camera permission is needed to scan. You can paste the code instead."
    }

    Page("Pair", "Pairing proves you trust a device. Both screens show the same 6-digit code.", onBack) {
        item { SectionLabel("Show my code") }
        item {
            Group {
                Block {
                    if (uri == null) {
                        Text("The other device scans this to connect to you. Valid for 5 minutes, single use.", style = MaterialTheme.typography.bodyMedium, color = c.textSecondary)
                        FdButton("Show QR code", { err = Engine.beginPairing() })
                    } else {
                        QrCode(uri!!, Modifier.fillMaxWidth())
                        Text(uri!!, fontFamily = FontFamily.Monospace, fontSize = 11.sp, color = c.textSecondary, maxLines = 2, overflow = TextOverflow.Ellipsis)
                        Row(horizontalArrangement = Arrangement.spacedBy(Space.s)) {
                            FdButton("Copy", {
                                (ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager).setPrimaryClip(ClipData.newPlainText("Bolt pairing", uri))
                            }, Modifier.weight(1f), BtnStyle.Secondary, fill = false)
                            FdButton("Stop", { Engine.endPairing() }, Modifier.weight(1f), BtnStyle.Secondary, fill = false)
                        }
                    }
                }
            }
        }

        item { SectionLabel("Scan or paste a code") }
        item {
            Group {
                Block {
                    if (scanning) {
                        QrScanner(Modifier.fillMaxWidth().height(280.dp)) { text -> scanning = false; err = Engine.pairWith(text) }
                        FdButton("Stop scanning", { scanning = false }, style = BtnStyle.Secondary)
                    } else {
                        FdButton("Open camera", style = BtnStyle.Secondary, onClick = {
                            if (ContextCompat.checkSelfPermission(ctx, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) scanning = true
                            else camera.launch(Manifest.permission.CAMERA)
                        })
                    }
                    FdField(paste, { paste = it }, "Paste a bolt://pair code", singleLine = false)
                    FdButton("Pair with pasted code", { err = Engine.pairWith(paste) }, style = BtnStyle.Secondary, enabled = paste.startsWith("bolt://"))
                }
            }
        }

        if (status.text != null || err != null) {
            item { SectionLabel("Status") }
            item {
                Group {
                    Block {
                        status.code?.let { CodeDisplay(it) }
                        val failed = err != null || status.failed
                        Text(
                            err ?: status.text.orEmpty(), textAlign = TextAlign.Center, modifier = Modifier.fillMaxWidth(), style = MaterialTheme.typography.bodyLarge,
                            color = if (failed) c.bad else if (status.done) c.good else c.text,
                        )
                        if (status.done || failed) FdButton("Dismiss", { Engine.resetPairStatus(); err = null }, style = BtnStyle.Plain)
                    }
                }
            }
        }

        item { SectionLabel("Paired devices") }
        item {
            Group {
                if (trusted.isEmpty()) EmptyState(Icons.Default.Lock, "No paired devices", "Paired devices can be trusted to send without a prompt, if you turn that on in Settings.")
                trusted.forEachIndexed { i, t ->
                    if (i > 0) GroupDivider(inset = 68.dp)
                    ListRow(
                        t.name, subtitle = t.id.take(12) + "…", leading = { Avatar(t.name) },
                        trailing = { FdButton("Remove", { Engine.revoke(t.id) }, style = BtnStyle.Plain, fill = false) },
                    )
                }
            }
        }
    }
}

// ---- Activity ------------------------------------------------------------

@Composable
private fun ActivityScreen(onBack: () -> Unit) {
    val history by Engine.history.collectAsState()
    val openable by SavedStore.ids.collectAsState()
    val ctx = LocalContext.current
    var sheetFiles by remember { mutableStateOf<List<SavedFile>?>(null) }
    var confirmClear by remember { mutableStateOf(false) }
    val c = Fd.colors
    Page(
        "Activity", "Recent transfers on this phone", onBack = onBack,
        action = if (history.isNotEmpty()) {
            { FdButton("Clear", { confirmClear = true }, style = BtnStyle.Plain, fill = false, height = 40.dp) }
        } else null,
    ) {
        item {
            Group {
                if (history.isEmpty()) EmptyState(Icons.Default.Refresh, "Nothing yet", "Sent and received files will appear here.")
                history.forEachIndexed { i, h ->
                    if (i > 0) GroupDivider(inset = 40.dp)
                    val dot = when (h.status) { "completed" -> c.good; "cancelled", "rejected" -> c.warn; else -> c.bad }
                    ListRow(
                        if (h.files > 1) "${h.title} +${h.files - 1}" else h.title.ifEmpty { "Transfer" },
                        subtitle = (if (h.dir == "send") "Sent to " else "Received from ") + h.peer + " · " + formatBytes(h.bytes) + " · " +
                            DateFormat.getDateTimeInstance(DateFormat.SHORT, DateFormat.SHORT).format(Date(h.at * 1000)) +
                            (if (h.status != "completed") "\n" + when (h.status) {
                                "cancelled" -> "Cancelled"
                                "rejected" -> "Declined"
                                else -> humanError(h.error, h.peer, h.dir == "send")
                            } else ""),
                        leading = { StatusDot(dot, h.status) },
                        // only received files can be opened; what you sent stays where it was
                        trailing = if (h.status == "completed" && h.dir != "send") {
                            { Text("Open", style = MaterialTheme.typography.labelMedium, color = if (h.id in openable) c.accent else c.textTertiary) }
                        } else null,
                        onClick = if (h.status == "completed" && h.dir != "send") {
                            {
                                val fs = SavedStore.get(ctx, h.id)
                                when {
                                    fs.isEmpty() -> toast(ctx, "File not found")
                                    fs.size == 1 -> openSavedFile(ctx, fs[0])
                                    else -> sheetFiles = fs
                                }
                            }
                        } else null,
                    )
                }
            }
        }
    }
    sheetFiles?.let { FilesSheet(it) { sheetFiles = null } }
    if (confirmClear) {
        DecisionSheet(onDismiss = { confirmClear = false }) {
            Text("Clear activity?", style = MaterialTheme.typography.titleLarge, color = c.text)
            Text("This only clears the list. Files you received stay on your phone.", style = MaterialTheme.typography.bodyMedium, color = c.textSecondary)
            Row(Modifier.padding(top = Space.s), horizontalArrangement = Arrangement.spacedBy(Space.m)) {
                FdButton("Cancel", { confirmClear = false }, Modifier.weight(1f), BtnStyle.Secondary, fill = false, height = 46.dp)
                FdButton("Clear", { Engine.clearHistory(); confirmClear = false }, Modifier.weight(1f), BtnStyle.Danger, fill = false, height = 46.dp)
            }
        }
    }
}

@Composable
private fun FilesSheet(files: List<SavedFile>, onClose: () -> Unit) {
    val ctx = LocalContext.current
    val c = Fd.colors
    DecisionSheet(onDismiss = onClose) {
        Text("${files.size} files", style = MaterialTheme.typography.titleLarge, color = c.text)
        Group {
            files.forEachIndexed { i, f ->
                if (i > 0) GroupDivider()
                ListRow(
                    f.name, subtitle = formatBytes(f.size), maxSubtitleLines = 1,
                    trailing = { Text("Open", style = MaterialTheme.typography.labelMedium, color = c.accent) },
                    onClick = { openSavedFile(ctx, f) },
                )
            }
        }
    }
}

// ---- Settings ------------------------------------------------------------

@Composable
fun Segmented(options: List<Pair<String, String>>, selected: String, onSelect: (String) -> Unit) {
    val c = Fd.colors
    Row(Modifier.fillMaxWidth().clip(Radius.control).background(c.background).padding(3.dp)) {
        options.forEach { (key, label) ->
            val on = key == selected
            val bg by androidx.compose.animation.animateColorAsState(if (on) c.surfaceRaised else androidx.compose.ui.graphics.Color.Transparent, tween(Motion.Quick), label = "seg")
            Box(
                Modifier.weight(1f).height(38.dp).clip(RoundedCornerShape(10.dp)).background(bg)
                    .clickable(role = Role.RadioButton) { onSelect(key) },
                contentAlignment = Alignment.Center,
            ) { Text(label, style = MaterialTheme.typography.labelMedium, color = if (on) c.text else c.textSecondary) }
        }
    }
}

@Composable
private fun SettingsScreen(picks: Pickers, onBack: () -> Unit) {
    val me by Engine.me.collectAsState()
    val mode by Engine.themeMode.collectAsState()
    var name by remember { mutableStateOf(Engine.deviceName) }
    var auto by remember { mutableStateOf(Engine.autoAccept) }
    var tree by remember { mutableStateOf(Engine.destTree) }
    val c = Fd.colors

    Page("Settings", onBack = onBack) {
        item { SectionLabel("This device") }
        item {
            Group {
                Block {
                    FdField(name, { name = it }, "Device name")
                    val dirty = name.trim() != Engine.deviceName
                    var saved by remember { mutableStateOf(false) }
                    androidx.compose.runtime.LaunchedEffect(saved) { if (saved) { kotlinx.coroutines.delay(2000); saved = false } }
                    // blue when there is something to save, green "Saved" right after, muted when nothing changed
                    FdButton(
                        if (saved && !dirty) "Saved" else "Save name",
                        { if (dirty) { Engine.deviceName = name.trim().ifEmpty { "Android" }; saved = true } },
                        style = if (saved && !dirty) BtnStyle.Success else BtnStyle.Primary,
                        icon = if (saved && !dirty) androidx.compose.material.icons.Icons.Default.Check else null,
                        enabled = dirty || saved, // stays full-strength while it says "Saved"
                    )
                    me?.let { Text("ID ${it.id.take(16)}…  ·  port ${it.port}", fontFamily = FontFamily.Monospace, fontSize = 12.sp, color = c.textSecondary) }
                }
            }
        }

        item { SectionLabel("Receiving") }
        item {
            val on = rememberOverlayAllowed()
            val ctx = androidx.compose.ui.platform.LocalContext.current
            Group {
                ListRow(
                    "Show requests over other apps",
                    subtitle = if (on) "On: the accept card appears on top of whatever you are using." else "Off: you only get a notification.",
                    trailing = { Text(if (on) "Manage" else "Turn on", style = MaterialTheme.typography.labelLarge, color = c.accent) },
                    onClick = { openOverlaySettings(ctx) },
                )
            }
        }
        item {
            Group {
                ListRow(
                    "Auto-accept from paired devices", subtitle = "Unknown devices always need your approval.",
                    trailing = {
                        Switch(auto, { auto = it; Engine.autoAccept = it }, colors = SwitchDefaults.colors(checkedTrackColor = c.good, checkedThumbColor = androidx.compose.ui.graphics.Color.White, uncheckedTrackColor = c.separator, uncheckedThumbColor = androidx.compose.ui.graphics.Color.White, uncheckedBorderColor = androidx.compose.ui.graphics.Color.Transparent))
                    },
                    onClick = { auto = !auto; Engine.autoAccept = auto },
                )
                GroupDivider()
                ListRow("Save received files to", subtitle = tree?.let { Uri_lastSegment(it) } ?: "Downloads/Bolt")
                Row(Modifier.padding(start = Space.l, end = Space.l, bottom = Space.m), horizontalArrangement = Arrangement.spacedBy(Space.s)) {
                    FdButton("Choose folder", { picks.destination() }, Modifier.weight(1f), BtnStyle.Secondary, fill = false)
                    if (tree != null) FdButton("Use Downloads", { Engine.destTree = null; tree = null }, Modifier.weight(1f), BtnStyle.Plain, fill = false)
                }
            }
        }
        item { Text("Files are verified in app storage first, then copied here.", Modifier.padding(horizontal = Space.l, vertical = Space.s), style = MaterialTheme.typography.bodySmall, color = c.textSecondary) }

        item { SectionLabel("Appearance") }
        item {
            Group {
                Block { Segmented(listOf("system" to "System", "light" to "Light", "dark" to "Dark"), mode) { Engine.setTheme(it) } }
            }
        }

        item { SectionLabel("Privacy") }
        item {
            Group {
                Block { Text("Transfers go directly between devices over your local network, encrypted with TLS 1.3. No account, no cloud, no analytics.", style = MaterialTheme.typography.bodyMedium, color = c.textSecondary) }
            }
        }

        item { Box(Modifier.padding(top = Space.xl)) { FdButton("Stop Bolt", { Engine.shutdown() }, style = BtnStyle.Destructive) } }
        item { Text("Leaves the network and stops receiving until the app is opened again.", Modifier.padding(horizontal = Space.l, vertical = Space.s), style = MaterialTheme.typography.bodySmall, color = c.textSecondary) }
    }
}

private fun Uri_lastSegment(s: String) = android.net.Uri.parse(s).lastPathSegment ?: s

// ---- Decision sheets -----------------------------------------------------

@Composable
private fun OfferSheet(o: Offer) {
    DecisionSheet(onDismiss = { Engine.respondOffer(o.id, false) }) { OfferContent(o) }
}

@Composable
private fun PairRequestSheet(p: PairReq) {
    val c = Fd.colors
    DecisionSheet(onDismiss = { Engine.respondPair(p.id, false) }) {
        Text("Pair with ${p.peerName}?", style = MaterialTheme.typography.titleLarge, color = c.text)
        Text("Approve only if the other device shows this same code.", style = MaterialTheme.typography.bodyMedium, color = c.textSecondary)
        Box(Modifier.fillMaxWidth().padding(vertical = Space.s)) { CodeDisplay(p.code) }
        Row(horizontalArrangement = Arrangement.spacedBy(Space.s)) {
            FdButton("Deny", { Engine.respondPair(p.id, false) }, Modifier.weight(1f), BtnStyle.Danger, fill = false, height = 46.dp)
            FdButton("Codes match", { Engine.respondPair(p.id, true) }, Modifier.weight(1f), BtnStyle.Primary, fill = false, height = 46.dp)
        }
    }
}
