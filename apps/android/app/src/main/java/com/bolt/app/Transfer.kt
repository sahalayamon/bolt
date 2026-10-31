package com.bolt.app

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.ui.draw.clip
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.clickable
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.ui.semantics.Role
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay

/** Every state of the bottom panel (pick files / incoming offer / transfer) fills at least this much, so it never jumps. */
val PanelContentHeight = 288.dp

/** "Someone wants to send you something": the same card in the home panel and, off the home screen, in a sheet. */
@Composable
fun OfferContent(o: Offer) {
    val c = Fd.colors
    val single = o.fileCount == 1
    val name = o.what()
    val textOnly = o.texts.isNotEmpty() && o.fileCount <= o.texts.size
    Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(Space.xs)) {
        Text("From ${o.peerName}", style = MaterialTheme.typography.titleMedium, color = c.text, textAlign = TextAlign.Center)
        if (textOnly) {
            Box(Modifier.padding(vertical = Space.xs).size(104.dp).clip(CircleShape).background(c.accentSoft), contentAlignment = Alignment.Center) {
                PickIcon("text", c.accent, Modifier.size(52.dp))
            }
            Text(if (o.texts.size == 1) "Text" else "${o.texts.size} texts", style = MaterialTheme.typography.titleMedium, color = c.text, textAlign = TextAlign.Center)
            Text(
                (if (o.texts.size == 1) "${o.texts[0]} character${if (o.texts[0] == 1L) "" else "s"}" else "${o.texts.sum()} characters") + if (o.trusted) " · Paired device" else "",
                style = MaterialTheme.typography.bodyMedium, color = c.textSecondary, textAlign = TextAlign.Center,
            )
        } else {
        Image(painterResource(R.drawable.offer_file), contentDescription = null, modifier = Modifier.padding(vertical = Space.xs).size(104.dp))
        Text(name, style = MaterialTheme.typography.titleMedium, color = c.text, textAlign = TextAlign.Center, maxLines = 2, overflow = TextOverflow.Ellipsis)
        Text(formatBytes(o.total) + if (o.trusted) " · Paired device" else "", style = MaterialTheme.typography.bodyMedium, color = c.textSecondary, textAlign = TextAlign.Center)
        }
        if (!single && !textOnly) {
            val lines = o.files.map { it.first } + o.texts.map { textSizeLabel(it) }
            lines.take(3).forEach { Text(it, style = MaterialTheme.typography.bodySmall, color = c.textSecondary, maxLines = 1, overflow = TextOverflow.Ellipsis) }
            if (o.fileCount > 3) Text("…and ${o.fileCount - 3} more", style = MaterialTheme.typography.bodySmall, color = c.textTertiary)
        }
        o.free?.let { if (it < o.total) Text("Not enough free space (${formatBytes(it)} available).", style = MaterialTheme.typography.bodySmall, color = c.bad, textAlign = TextAlign.Center) }
    }
    Row(Modifier.padding(top = Space.s), horizontalArrangement = Arrangement.spacedBy(Space.m)) {
        FdButton("Decline", { Engine.respondOffer(o.id, false) }, Modifier.weight(1f), BtnStyle.Danger, fill = false, height = 46.dp)
        FdButton("Accept", { Engine.respondOffer(o.id, true) }, Modifier.weight(1f), BtnStyle.Primary, fill = false, height = 46.dp)
    }
}

/** A circular progress ring: determinate when [fraction] is given, a spinning arc while we are only waiting. */
@Composable
fun ProgressRing(fraction: Float?, color: Color, modifier: Modifier = Modifier, stroke: Dp = 10.dp, content: @Composable () -> Unit) {
    val track = Fd.colors.surfaceRaised
    val animated by animateFloatAsState(fraction ?: 0f, tween(Motion.Large, easing = Motion.ease), label = "ring")
    val spin by rememberInfiniteTransition(label = "spin").animateFloat(0f, 360f, infiniteRepeatable(tween(1200, easing = LinearEasing), RepeatMode.Restart), label = "angle")
    Box(modifier, contentAlignment = Alignment.Center) {
        Canvas(Modifier.fillMaxSize()) {
            val s = stroke.toPx()
            val d = size.minDimension - s
            val tl = Offset(s / 2, s / 2)
            val box = Size(d, d)
            drawArc(track, 0f, 360f, false, tl, box, style = Stroke(s))
            if (fraction != null) drawArc(color, -90f, 360f * animated, false, tl, box, style = Stroke(s, cap = StrokeCap.Round))
            else drawArc(color, spin, 100f, false, tl, box, style = Stroke(s, cap = StrokeCap.Round))
        }
        content()
    }
}

/** Text items travel under the name text-N.txt; that is not something to show. */
private fun isTextName(n: String) = Regex("text-\\d+\\.txt").matches(n)

private fun eta(x: Xfer): String? {
    if (x.speed <= 0 || x.total <= x.done) return null
    val s = ((x.total - x.done) / x.speed).toLong()
    return if (s < 60) "$s s left" else "${s / 60} min ${s % 60} s left"
}

/** Progress for the current transfer, in the same card that asked "accept or decline". */
@Composable
fun TransferPanelContent(x: Xfer, more: Int) {
    val c = Fd.colors
    val ctx = androidx.compose.ui.platform.LocalContext.current
    val sending = x.dir == "send"
    val saving = !x.finished && x.state == "saving"
    val moving = !x.finished && x.state == "transferring"
    val done = x.finished && x.ok
    val cancelled = x.finished && !x.ok && x.state == "cancelled"
    val failed = x.finished && !x.ok && !cancelled
    val frac = if (x.total > 0) (x.done.toFloat() / x.total).coerceIn(0f, 1f) else 0f

    // a finished, successful transfer clears itself after 30 s (the user can also tap Done)
    LaunchedEffect(x.job, done) { if (done) { delay(30_000); Engine.dismiss(x.job) } }

    val header = when {
        saving -> "Saving…"
        done -> if (sending) "Sent to ${x.peer}" else "Received from ${x.peer}"
        cancelled -> "Cancelled"
        failed -> failureTitle(x.peer, sending)
        sending -> "Sending to ${x.peer}"
        else -> "Receiving from ${x.peer}"
    }
    val textual = x.textOnly || (x.current?.let { isTextName(it) } == true)
    val ringColor = when { done -> c.good; failed -> c.bad; cancelled -> c.warn; else -> c.accent }
    val detail = when {
        saving -> "Copying to your phone's storage"
        done && x.textOnly -> if (sending) "Text sent" else "Text received"
        done -> x.note ?: "${x.filesTotal} file${if (x.filesTotal == 1) "" else "s"} verified"
        failed -> x.error ?: "Something went wrong. Please try again."
        cancelled -> "Nothing was saved"
        x.state == "connecting" -> "Connecting…"
        x.state == "awaiting_approval" -> "Waiting for them to accept…"
        textual -> if (sending) "Sending text…" else "Receiving text…"
        else -> buildList {
            if (x.filesTotal > 1) add("File ${minOf(x.filesDone + 1, x.filesTotal)} of ${x.filesTotal}")
            add("${formatBytes(x.done)} of ${formatBytes(x.total)}")
            if (x.speed > 0) add("${formatBytes(x.speed.toLong())}/s")
            eta(x)?.let { add(it) }
        }.joinToString(" · ")
    }

    Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(Space.xs)) {
        Text(header, style = MaterialTheme.typography.titleMedium, color = c.text, textAlign = TextAlign.Center, maxLines = 1, overflow = TextOverflow.Ellipsis)
        ProgressRing(
            fraction = when { moving -> frac; done -> 1f; failed || cancelled -> frac; else -> null },
            color = ringColor, modifier = Modifier.padding(vertical = Space.s).size(if (done && !sending && x.files.size > 1) 76.dp else 124.dp),
        ) {
            when {
                moving -> Text("${(frac * 100).toInt()}%", style = MaterialTheme.typography.headlineMedium, color = c.text)
                done -> Icon(Icons.Default.Check, "Done", tint = c.good, modifier = Modifier.size(44.dp))
                failed || cancelled -> Icon(Icons.Default.Close, "Stopped", tint = ringColor, modifier = Modifier.size(40.dp))
                else -> Icon(Icons.Default.Check, null, tint = Color.Transparent, modifier = Modifier.size(1.dp))
            }
        }
        // the header already names the device, so only a file name goes here
        if (!(done && !sending && x.files.size > 1) && !failed && !cancelled && x.current != null && !isTextName(x.current)) Text(x.current, style = MaterialTheme.typography.titleMedium, color = c.text, textAlign = TextAlign.Center, maxLines = 1, overflow = TextOverflow.Ellipsis)
        Text(detail, style = MaterialTheme.typography.bodyMedium, color = if (failed) c.bad else c.textSecondary, textAlign = TextAlign.Center, maxLines = 2, overflow = TextOverflow.Ellipsis)
        if (more > 0) Text("+$more more transfer${if (more == 1) "" else "s"} in progress", style = MaterialTheme.typography.bodySmall, color = c.textTertiary)
        if (done && !sending && x.files.size > 1) {
            // several received files: each one can be opened on its own
            Column(Modifier.fillMaxWidth().heightIn(max = 96.dp).verticalScroll(rememberScrollState())) {
                x.files.forEach { f ->
                    Row(
                        Modifier.fillMaxWidth().heightIn(min = 36.dp).clickable(role = Role.Button) { openSavedFile(ctx, f) },
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(f.name, Modifier.weight(1f), style = MaterialTheme.typography.bodyMedium, color = c.text, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Text("Open", style = MaterialTheme.typography.labelMedium, color = c.accent, modifier = Modifier.padding(start = Space.m))
                    }
                }
            }
        }
    }
    Row(Modifier.padding(top = Space.s)) {
        if (saving) Box(Modifier.height(46.dp)) // nothing to press while saving; keep the card's size
        else if (!x.finished) FdButton("Cancel", { Engine.cancel(x.job) }, style = BtnStyle.Danger, height = 46.dp)
        else if (done && !sending && x.files.size == 1) Row(horizontalArrangement = Arrangement.spacedBy(Space.m)) {
            FdButton("Done", { Engine.dismiss(x.job) }, Modifier.weight(1f), BtnStyle.Secondary, fill = false, height = 46.dp)
            FdButton("Open", { openSavedFile(ctx, x.files[0]) }, Modifier.weight(1f), BtnStyle.Primary, fill = false, height = 46.dp)
        }
        else FdButton(if (done) "Done" else "Dismiss", { Engine.dismiss(x.job) }, style = if (done) BtnStyle.Primary else BtnStyle.Secondary, height = 46.dp)
    }
}
