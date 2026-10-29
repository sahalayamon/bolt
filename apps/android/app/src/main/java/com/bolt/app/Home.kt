package com.bolt.app

import android.Manifest
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.Settings
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalLifecycleOwner
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.app.NotificationManagerCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver

enum class Dest { Pair, Activity, Settings, Diagnostics }

private val Glyph = Color(0xFF14170A) // dark glyphs on lime accent bubbles

/** Line-art device glyphs, drawn so no icon pack is needed. */
@Composable
private fun DeviceGlyph(kind: String, modifier: Modifier = Modifier, tint: Color = Glyph) {
    Canvas(modifier) {
        val w = size.width; val h = size.height
        val sw = w * 0.07f
        when (kind) {
            "phone" -> {
                drawRoundRect(tint.copy(alpha = 0.35f), Offset(w * 0.32f, h * 0.14f), Size(w * 0.36f, h * 0.72f), CornerRadius(w * 0.07f))
                drawRoundRect(tint, Offset(w * 0.32f, h * 0.14f), Size(w * 0.36f, h * 0.72f), CornerRadius(w * 0.07f), style = Stroke(sw))
                drawLine(tint, Offset(w * 0.44f, h * 0.2f), Offset(w * 0.56f, h * 0.2f), sw, StrokeCap.Round)
            }
            "desktop" -> {
                drawRoundRect(tint.copy(alpha = 0.35f), Offset(w * 0.2f, h * 0.26f), Size(w * 0.6f, h * 0.4f), CornerRadius(w * 0.05f))
                drawRoundRect(tint, Offset(w * 0.2f, h * 0.26f), Size(w * 0.6f, h * 0.4f), CornerRadius(w * 0.05f), style = Stroke(sw))
                drawLine(tint, Offset(w * 0.12f, h * 0.76f), Offset(w * 0.88f, h * 0.76f), sw * 1.3f, StrokeCap.Round)
            }
            "tablet" -> {
                drawRoundRect(tint.copy(alpha = 0.35f), Offset(w * 0.24f, h * 0.18f), Size(w * 0.52f, h * 0.64f), CornerRadius(w * 0.07f))
                drawRoundRect(tint, Offset(w * 0.24f, h * 0.18f), Size(w * 0.52f, h * 0.64f), CornerRadius(w * 0.07f), style = Stroke(sw))
            }
            "address" -> { // a globe: send to an address on the network
                val cx = w * 0.5f; val cy = h * 0.5f; val r = w * 0.27f
                drawCircle(tint, r, Offset(cx, cy), style = Stroke(sw))
                drawOval(tint, Offset(cx - r * 0.42f, cy - r), Size(r * 0.84f, r * 2), style = Stroke(sw * 0.85f))
                drawLine(tint, Offset(cx - r, cy), Offset(cx + r, cy), sw * 0.85f, StrokeCap.Round)
                val half = r * 0.866f
                drawLine(tint, Offset(cx - half, cy - r * 0.5f), Offset(cx + half, cy - r * 0.5f), sw * 0.7f, StrokeCap.Round)
                drawLine(tint, Offset(cx - half, cy + r * 0.5f), Offset(cx + half, cy + r * 0.5f), sw * 0.7f, StrokeCap.Round)
            }
            else -> { // generic device: a rounded square with a dot
                drawRoundRect(tint, Offset(w * 0.26f, h * 0.26f), Size(w * 0.48f, h * 0.48f), CornerRadius(w * 0.1f), style = Stroke(sw))
                drawCircle(tint, w * 0.05f, Offset(w * 0.5f, h * 0.5f))
            }
        }
    }
}

@Composable
private fun Bubble(label: String, kind: String, onClick: (() -> Unit)?, dim: Boolean = false, badge: Boolean = false) {
    val c = Fd.colors
    Column(
        Modifier.width(68.dp).let { if (onClick != null) it.clickable(role = Role.Button, onClickLabel = "Send to $label", onClick = onClick) else it },
        horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(Space.s),
    ) {
        Box(
            Modifier.size(64.dp).clip(CircleShape).background(if (dim) (if (c.dark) Color(0xFF262D0A) else c.accentSoft) else c.accent),
            contentAlignment = Alignment.Center,
        ) {
            DeviceGlyph(kind, Modifier.size(46.dp), if (dim) c.accent else Glyph)
            if (badge) Box(Modifier.align(Alignment.TopEnd).padding(5.dp).size(12.dp).clip(CircleShape).background(c.good).border(2.dp, c.top, CircleShape))
        }
        Text(label, style = MaterialTheme.typography.bodySmall.copy(fontSize = 13.sp), color = c.text, textAlign = TextAlign.Center, maxLines = 2, overflow = TextOverflow.Ellipsis)
    }
}

@Composable
private fun SearchingBubble(onClick: () -> Unit) {
    val pulse by rememberInfiniteTransition(label = "search").animateFloat(0.35f, 1f, infiniteRepeatable(tween(1100), RepeatMode.Reverse), label = "a")
    Column(Modifier.width(68.dp).clickable(role = Role.Button, onClickLabel = "Search again", onClick = onClick), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(Space.s)) {
        Box(Modifier.size(64.dp).alpha(pulse).clip(CircleShape).background(Fd.colors.surface))
        Text("Searching", style = MaterialTheme.typography.bodySmall.copy(fontSize = 13.sp), color = Fd.colors.textSecondary, maxLines = 1)
    }
}

@Composable
private fun rememberNotificationsEnabled(): Boolean {
    val ctx = LocalContext.current
    var on by remember { mutableStateOf(NotificationManagerCompat.from(ctx).areNotificationsEnabled()) }
    val owner = LocalLifecycleOwner.current
    DisposableEffect(owner) {
        val obs = LifecycleEventObserver { _, e -> if (e == Lifecycle.Event.ON_RESUME) on = NotificationManagerCompat.from(ctx).areNotificationsEnabled() }
        owner.lifecycle.addObserver(obs)
        onDispose { owner.lifecycle.removeObserver(obs) }
    }
    return on
}

@Composable
fun rememberOverlayAllowed(): Boolean {
    val ctx = LocalContext.current
    var on by remember { mutableStateOf(canShowOverOtherApps(ctx)) }
    val owner = LocalLifecycleOwner.current
    DisposableEffect(owner) {
        val obs = LifecycleEventObserver { _, e -> if (e == Lifecycle.Event.ON_RESUME) on = canShowOverOtherApps(ctx) }
        owner.lifecycle.addObserver(obs)
        onDispose { owner.lifecycle.removeObserver(obs) }
    }
    return on
}

fun openOverlaySettings(ctx: Context) {
    ctx.startActivity(Intent(Settings.ACTION_MANAGE_OVERLAY_PERMISSION, Uri.parse("package:${ctx.packageName}")).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
}

private fun openNotificationSettings(ctx: Context) {
    ctx.startActivity(Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).putExtra(Settings.EXTRA_APP_PACKAGE, ctx.packageName).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
}

/** Small line-art icons for the pick tiles and the status line. */
@Composable
private fun PickIcon(kind: String, tint: Color, modifier: Modifier = Modifier) {
    Canvas(modifier) {
        val w = size.width; val h = size.height; val sw = w * 0.09f
        when (kind) {
            "file" -> {
                val x0 = w * 0.22f; val x1 = w * 0.78f; val y0 = h * 0.1f; val y1 = h * 0.9f; val fold = w * 0.22f
                val path = androidx.compose.ui.graphics.Path().apply {
                    moveTo(x0, y0); lineTo(x1 - fold, y0); lineTo(x1, y0 + fold); lineTo(x1, y1); lineTo(x0, y1); close()
                }
                drawPath(path, tint, style = Stroke(sw, join = androidx.compose.ui.graphics.StrokeJoin.Round))
                drawLine(tint, Offset(x1 - fold, y0), Offset(x1 - fold, y0 + fold), sw, StrokeCap.Round)
                drawLine(tint, Offset(x1 - fold, y0 + fold), Offset(x1, y0 + fold), sw, StrokeCap.Round)
            }
            "image" -> {
                drawRoundRect(tint, Offset(w * 0.1f, h * 0.16f), Size(w * 0.8f, h * 0.68f), CornerRadius(w * 0.12f), style = Stroke(sw))
                drawCircle(tint, w * 0.07f, Offset(w * 0.34f, h * 0.38f))
                val mtn = androidx.compose.ui.graphics.Path().apply {
                    moveTo(w * 0.14f, h * 0.78f); lineTo(w * 0.42f, h * 0.52f); lineTo(w * 0.62f, h * 0.7f); lineTo(w * 0.74f, h * 0.6f); lineTo(w * 0.88f, h * 0.76f)
                }
                drawPath(mtn, tint, style = Stroke(sw, cap = StrokeCap.Round, join = androidx.compose.ui.graphics.StrokeJoin.Round))
            }
            "folder" -> {
                val path = androidx.compose.ui.graphics.Path().apply {
                    moveTo(w * 0.1f, h * 0.28f); lineTo(w * 0.38f, h * 0.28f); lineTo(w * 0.46f, h * 0.38f); lineTo(w * 0.9f, h * 0.38f)
                    lineTo(w * 0.9f, h * 0.8f); lineTo(w * 0.1f, h * 0.8f); close()
                }
                drawPath(path, tint, style = Stroke(sw, join = androidx.compose.ui.graphics.StrokeJoin.Round))
            }
            "broadcast" -> {
                drawCircle(tint, w * 0.09f, Offset(w * 0.5f, h * 0.5f))
                for ((i, r) in listOf(0.26f, 0.42f).withIndex()) {
                    val d = w * r * 2
                    for (start in listOf(-50f, 130f)) {
                        drawArc(tint, start, 100f, false, Offset(w * 0.5f - d / 2, h * 0.5f - d / 2), Size(d, d), style = Stroke(sw * 0.8f, cap = StrokeCap.Round), alpha = if (i == 0) 1f else 0.7f)
                    }
                }
            }
        }
    }
}

@Composable
private fun PickTile(label: String, kind: String, onClick: () -> Unit, modifier: Modifier) {
    val c = Fd.colors
    Column(
        modifier.height(104.dp).clip(RoundedCornerShape(18.dp)).background(if (c.dark) c.surfaceRaised else c.background)
            .clickable(role = Role.Button, onClick = onClick),
        horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(Space.s, Alignment.CenterVertically),
    ) {
        PickIcon(kind, c.accent, Modifier.size(30.dp))
        Text(label, style = MaterialTheme.typography.titleMedium, color = c.text)
    }
}

/** Content of the send panel: prompt or current selection, then the three pick tiles. */
@Composable
private fun SendPanelContent(picks: Pickers, pending: List<PickedFile>, armed: Device?, onAddress: () -> Unit, onSendError: (String) -> Unit) {
    val c = Fd.colors
    if (armed != null && pending.isNotEmpty()) {
        // the user tapped a device and chose files: confirm before anything is sent
        val single = pending.size == 1
        Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(Space.xs)) {
            Image(painterResource(R.drawable.offer_file), contentDescription = null, modifier = Modifier.padding(top = Space.xs, bottom = Space.xs).size(104.dp))
            Text(
                if (single) pending[0].rel.substringAfterLast('/') else "${pending.size} files selected",
                style = MaterialTheme.typography.titleMedium, color = c.text, textAlign = TextAlign.Center, maxLines = 2, overflow = TextOverflow.Ellipsis,
            )
            Text(
                "To ${armed.name} · " + formatBytes(pending.sumOf { it.size }) + if (single) "" else " in total",
                style = MaterialTheme.typography.bodyMedium, color = c.textSecondary, textAlign = TextAlign.Center, maxLines = 1, overflow = TextOverflow.Ellipsis,
            )
        }
        Row(Modifier.fillMaxWidth().padding(top = Space.s), horizontalArrangement = Arrangement.spacedBy(Space.s)) {
            FdButton("Cancel", { Engine.clearPending() }, Modifier.weight(1f), BtnStyle.Danger, fill = false, height = 46.dp)
            FdButton("Add more", picks.files, Modifier.weight(1.1f), BtnStyle.Secondary, fill = false, height = 46.dp)
            FdButton("Send", { Engine.sendPending(armed.addr, armed.id, armed.name)?.let(onSendError) }, Modifier.weight(1f), BtnStyle.Primary, fill = false, height = 46.dp)
        }
        return
    }
    if (pending.isNotEmpty()) {
        // files chosen first: the same card as above, with the available devices to pick from
        val devices by Engine.devices.collectAsState()
        val single = pending.size == 1
        Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(Space.xs)) {
            Image(painterResource(R.drawable.offer_file), contentDescription = null, modifier = Modifier.padding(top = Space.xs, bottom = Space.xs).size(104.dp))
            Text(
                if (single) pending[0].rel.substringAfterLast('/') else "${pending.size} files selected",
                style = MaterialTheme.typography.titleMedium, color = c.text, textAlign = TextAlign.Center, maxLines = 2, overflow = TextOverflow.Ellipsis,
            )
            Text(
                formatBytes(pending.sumOf { it.size }) + if (single) "" else " in total",
                style = MaterialTheme.typography.bodyMedium, color = c.textSecondary, textAlign = TextAlign.Center,
            )
        }
        Text("Send to", style = MaterialTheme.typography.labelMedium, color = c.textSecondary, modifier = Modifier.fillMaxWidth().padding(top = Space.m))
        if (devices.isEmpty()) {
            Text("Looking for devices on this network…", style = MaterialTheme.typography.bodyMedium, color = c.textTertiary, modifier = Modifier.fillMaxWidth().padding(top = Space.xs))
        }
        Row(Modifier.fillMaxWidth().padding(top = Space.s).horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(Space.s)) {
            devices.forEach { d ->
                Row(
                    Modifier.height(46.dp).clip(RoundedCornerShape(23.dp)).background(c.accentSoft)
                        .clickable(role = Role.Button, onClickLabel = "Send to ${d.name}") { Engine.sendPending(d.addr, d.id, d.name)?.let(onSendError) }
                        .padding(start = Space.s, end = Space.l),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Box(Modifier.size(34.dp).clip(CircleShape).background(c.accent), contentAlignment = Alignment.Center) {
                        DeviceGlyph(d.kind, Modifier.size(26.dp))
                    }
                    Text(d.name, Modifier.padding(start = Space.s), style = MaterialTheme.typography.labelLarge, color = c.text, maxLines = 1)
                }
            }
            // send to an address: just the globe
            Box(
                Modifier.size(46.dp).clip(CircleShape).background(c.accentSoft)
                    .clickable(role = Role.Button, onClickLabel = "Send to an address", onClick = onAddress),
                contentAlignment = Alignment.Center,
            ) { DeviceGlyph("address", Modifier.size(34.dp), c.accent) }
        }
        Row(Modifier.fillMaxWidth().padding(top = Space.m), horizontalArrangement = Arrangement.spacedBy(Space.s)) {
            FdButton("Cancel", { Engine.clearPending() }, Modifier.weight(1f), BtnStyle.Danger, fill = false, height = 46.dp)
            FdButton("Add more", picks.files, Modifier.weight(1f), BtnStyle.Secondary, fill = false, height = 46.dp)
        }
        return
    }
    Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(Space.m)) {
        Text("Choose what to send", style = MaterialTheme.typography.headlineMedium.copy(fontSize = 22.sp), color = c.text)
        Text("Tap a device above to send to it, or pick files first.", style = MaterialTheme.typography.bodyLarge, color = c.textSecondary, textAlign = TextAlign.Center)
        Row(Modifier.fillMaxWidth().padding(top = Space.s), horizontalArrangement = Arrangement.spacedBy(Space.m)) {
            PickTile("Files", "file", picks.files, Modifier.weight(1f))
            PickTile("Photos", "image", picks.photos, Modifier.weight(1f))
            PickTile("Folder", "folder", picks.folder, Modifier.weight(1f))
        }
    }
}

@Composable
fun HomeScreen(picks: Pickers, open: (Dest) -> Unit) {
    val c = Fd.colors
    val ctx = LocalContext.current
    val devices by Engine.devices.collectAsState()
    val pending by Engine.pending.collectAsState()
    val armed by Engine.armed.collectAsState()
    val xfers by Engine.xfers.collectAsState()
    val me by Engine.me.collectAsState()
    val error by Engine.error.collectAsState()
    val trusted by Engine.trusted.collectAsState()
    val notificationsOn = rememberNotificationsEnabled()
    val overlayOn = rememberOverlayAllowed()
    val offers by Engine.offers.collectAsState()
    // Gap under the tiles equals the side margin. The gesture pill (small inset) overlays the panel;
    // a 3-button bar (large inset) is cleared instead.
    val navBottom = with(androidx.compose.ui.platform.LocalDensity.current) { WindowInsets.navigationBars.getBottom(this).toDp() }
    val bottomGap = if (navBottom > 32.dp) navBottom + Space.s else Space.page
    var menu by remember { mutableStateOf(false) }
    var msg by remember { mutableStateOf<String?>(null) }
    var addressSheet by remember { mutableStateOf(false) }
    var manual by remember { mutableStateOf(Engine.lastManual) }

    val askNotifications = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        if (!granted) openNotificationSettings(ctx)
    }

    Column(Modifier.fillMaxSize().background(c.top)) {
        // ---- header + devices (darkest surface) ----
        Column(Modifier.weight(1f).fillMaxWidth().widthIn(max = 640.dp).align(Alignment.CenterHorizontally).padding(horizontal = Space.page)) {
            Row(Modifier.fillMaxWidth().padding(top = Space.m), verticalAlignment = Alignment.CenterVertically) {
                androidx.compose.foundation.Image(
                    painterResource(R.drawable.bolt_logo), contentDescription = null,
                    modifier = Modifier.size(28.dp),
                )
                Text("Bolt", Modifier.padding(start = Space.m).weight(1f), style = MaterialTheme.typography.headlineMedium, color = c.text)
                Box(Modifier.offset(x = 12.dp)) {
                    IconButton({ menu = true }) { Icon(Icons.Default.MoreVert, "More", tint = c.text) }
                    DropdownMenu(menu, { menu = false }, containerColor = c.surfaceRaised) {
                        DropdownMenuItem({ Text("Refresh devices", color = c.text) }, { menu = false; Engine.refreshDiscovery() })
                        DropdownMenuItem({ Text("Pair devices", color = c.text) }, { menu = false; open(Dest.Pair) })
                        DropdownMenuItem({ Text("Activity", color = c.text) }, { menu = false; open(Dest.Activity) })
                        DropdownMenuItem({ Text("Settings", color = c.text) }, { menu = false; open(Dest.Settings) })
                        DropdownMenuItem({ Text("Diagnostics", color = c.text) }, { menu = false; open(Dest.Diagnostics) })
                    }
                }
            }

            Text("Your Devices", Modifier.padding(top = Space.xl, bottom = Space.l), style = MaterialTheme.typography.headlineMedium, color = c.accent)

            LazyRow(Modifier.offset(x = (-2).dp).padding(bottom = Space.xl), horizontalArrangement = Arrangement.spacedBy(Space.m)) {
                items(devices, key = { it.id }) { d ->
                    Bubble(
                        d.name, d.kind, badge = trusted.any { it.id == d.id },
                        // nothing selected yet: pick files now and send them straight to this device
                        onClick = { if (pending.isEmpty()) { Engine.arm(d); picks.files() } else msg = Engine.sendPending(d.addr, d.id, d.name) },
                    )
                }
                if (devices.isEmpty()) item { SearchingBubble { Engine.refreshDiscovery() } }
                item { Bubble("Address", "address", onClick = { addressSheet = true }, dim = true) }
            }

            Spacer(Modifier.weight(1f))
            Row(Modifier.fillMaxWidth().padding(bottom = Space.l), horizontalArrangement = Arrangement.Center, verticalAlignment = Alignment.CenterVertically) {
                PickIcon("broadcast", c.accent, Modifier.size(20.dp))
                Text(
                    androidx.compose.ui.text.buildAnnotatedString {
                        append("  Visible to nearby devices as ")
                        if (me != null) {
                            pushStyle(androidx.compose.ui.text.SpanStyle(color = c.accent, fontWeight = FontWeight.Bold)); append(me!!.name); pop()
                        } else append("…")
                    },
                    style = MaterialTheme.typography.bodyMedium, color = c.textSecondary, maxLines = 1, overflow = TextOverflow.Ellipsis,
                )
            }
        }

        // ---- send panel: pinned to the bottom, hugs its content, runs behind the navigation bar ----
        Box(
            Modifier.fillMaxWidth().clip(RoundedCornerShape(topStart = 28.dp, topEnd = 28.dp)).background(c.surface),
            contentAlignment = Alignment.TopCenter,
        ) {
            Column(
                Modifier.widthIn(max = 640.dp).fillMaxWidth().heightIn(max = 520.dp).verticalScroll(rememberScrollState())
                    .padding(start = Space.page, end = Space.page, top = 34.dp, bottom = bottomGap),
                verticalArrangement = Arrangement.spacedBy(Space.m),
            ) {
                // only shown when something needs attention
                if (error != null || !notificationsOn || !overlayOn) {
                    Group {
                        if (error == null && notificationsOn && !overlayOn) ListRow(
                            "Show requests over other apps", subtitle = "Accept incoming files without opening Bolt.",
                            leading = { Icon(Icons.Default.Close, null, tint = c.warn) },
                            trailing = { Text("Turn on", style = MaterialTheme.typography.labelLarge, color = c.accent) },
                            onClick = { openOverlaySettings(ctx) },
                        ) else ListRow(
                            if (error != null) "Bolt isn't running" else "Notifications are off",
                            subtitle = error?.let { humanError(it) } ?: "Turn them on so you can accept incoming transfers.",
                            leading = { Icon(Icons.Default.Close, null, tint = c.warn) },
                            trailing = if (error == null) {
                                { Text("Turn on", style = MaterialTheme.typography.labelLarge, color = c.accent) }
                            } else null,
                            onClick = if (error == null) {
                                { if (Build.VERSION.SDK_INT >= 33) askNotifications.launch(Manifest.permission.POST_NOTIFICATIONS) else openNotificationSettings(ctx) }
                            } else null,
                        )
                    }
                }
                // One card, three states, same size and place: pick files -> incoming offer -> transfer progress.
                val offer = offers.firstOrNull()
                val xfer = xfers.firstOrNull()
                // remember the last offer/transfer so the card keeps its content while it animates out
                var shownOffer by remember { mutableStateOf<Offer?>(null) }
                var shownXfer by remember { mutableStateOf<Xfer?>(null) }
                if (offer != null) shownOffer = offer
                if (xfer != null) shownXfer = xfer
                // finer states so choosing files, confirming and sending each animate into the next
                val state = when {
                    offer != null -> "offer"
                    xfer != null -> "transfer"
                    pending.isNotEmpty() && armed != null -> "confirm"
                    pending.isNotEmpty() -> "picked"
                    else -> "send"
                }
                androidx.compose.animation.AnimatedContent(
                    targetState = state,
                    transitionSpec = {
                        val intoOffer = targetState == "offer"
                        val outOfOffer = initialState == "offer"
                        val enter = if (intoOffer) androidx.compose.animation.slideInVertically(androidx.compose.animation.core.tween(460, easing = Motion.ease)) { it } +
                            androidx.compose.animation.fadeIn(androidx.compose.animation.core.tween(300))
                        else androidx.compose.animation.slideInVertically(androidx.compose.animation.core.tween(420, easing = Motion.ease)) { it / 6 } +
                            androidx.compose.animation.fadeIn(androidx.compose.animation.core.tween(300, delayMillis = 90)) +
                            androidx.compose.animation.scaleIn(androidx.compose.animation.core.tween(420, easing = Motion.ease), initialScale = 0.94f)
                        val exit = if (outOfOffer) androidx.compose.animation.slideOutVertically(androidx.compose.animation.core.tween(380, easing = Motion.ease)) { it } +
                            androidx.compose.animation.fadeOut(androidx.compose.animation.core.tween(240))
                        else androidx.compose.animation.fadeOut(androidx.compose.animation.core.tween(180)) +
                            androidx.compose.animation.scaleOut(androidx.compose.animation.core.tween(260, easing = Motion.ease), targetScale = 0.96f)
                        // the card grows and shrinks smoothly to the size of whatever it now shows
                        (enter togetherWith exit).using(androidx.compose.animation.SizeTransform(clip = false) { _, _ -> androidx.compose.animation.core.tween(420, easing = Motion.ease) })
                    },
                    label = "panel",
                ) { st ->
                    // every card hugs its own content
                    Column(
                        Modifier.fillMaxWidth(),
                        horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.Center,
                    ) {
                        val o = offer ?: shownOffer
                        val xf = xfer ?: shownXfer
                        when {
                            st == "offer" && o != null -> OfferContent(o)
                            st == "transfer" && xf != null -> TransferPanelContent(xf, xfers.size - 1)
                            else -> SendPanelContent(picks, pending, armed, onAddress = { addressSheet = true }) { msg = it }
                        }
                    }
                }
            }
            Box(Modifier.padding(top = 10.dp).size(width = 40.dp, height = 4.dp).clip(CircleShape).background(c.textTertiary))
        }
    }

    msg?.let { m ->
        DecisionSheet({ msg = null }) {
            Text("That didn't go through", style = MaterialTheme.typography.titleLarge, color = c.text)
            Text(m, style = MaterialTheme.typography.bodyLarge, color = c.textSecondary)
            FdButton("OK", { msg = null })
        }
    }
    if (addressSheet) {
        var err by remember { mutableStateOf<String?>(null) }
        DecisionSheet({ addressSheet = false }) {
            Text("Send to an address", style = MaterialTheme.typography.titleLarge, color = c.text)
            Text("Use this when discovery is blocked on the network. The port defaults to 47600.", style = MaterialTheme.typography.bodyMedium, color = c.textSecondary)
            FdField(manual, { manual = it; Engine.lastManual = it; err = null }, "192.168.1.20:47600", isError = err != null)
            err?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = c.bad) }
            if (pending.isEmpty()) Text("Choose files first, then send.", style = MaterialTheme.typography.bodyMedium, color = c.warn)
            FdButton("Send", {
                err = Engine.sendPending(manual.trim(), "", manual.trim())
                if (err == null) addressSheet = false
            }, enabled = manual.isNotBlank() && pending.isNotEmpty())
        }
    }
}
