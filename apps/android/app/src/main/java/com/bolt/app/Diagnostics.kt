package com.bolt.app

import android.content.Context
import android.net.wifi.WifiManager
import android.os.PowerManager
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Info
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.sp
import java.util.Locale

private fun f(v: Double, d: Int = 2) = String.format(Locale.US, "%.${d}f", v)

@Suppress("DEPRECATION")
private fun wifiSummary(ctx: Context): List<String> {
    val wm = ctx.applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
    val i = wm.connectionInfo
    val pm = ctx.getSystemService(Context.POWER_SERVICE) as PowerManager
    val lines = ArrayList<String>()
    if (i == null || i.networkId == -1) lines += "Not connected to Wi-Fi"
    else {
        val band = when { i.frequency in 2400..2500 -> "2.4 GHz"; i.frequency in 4900..5900 -> "5 GHz"; i.frequency >= 5925 -> "6 GHz"; else -> "unknown band" }
        lines += "Link speed ${i.linkSpeed} Mbit/s (negotiated, not real throughput)"
        lines += "$band (${i.frequency} MHz) · signal ${i.rssi} dBm"
    }
    lines += "Battery saver: " + if (pm.isPowerSaveMode) "ON (can throttle Wi-Fi and background work)" else "off"
    lines += "Display over other apps: " + if (canShowOverOtherApps(ctx)) "allowed" else "NOT allowed (incoming requests only show as a notification)"
    return lines
}

/** Where is the time going? Network-only test, stage timings of the last transfer, and Wi-Fi facts. */
@Composable
fun DiagnosticsScreen(onBack: () -> Unit) {
    val c = Fd.colors
    val ctx = LocalContext.current
    val devices by Engine.devices.collectAsState()
    val link by Engine.link.collectAsState()
    val last by Engine.lastTransfer.collectAsState()
    var cc by remember { mutableStateOf(if (Engine.tuning.contains("cc=cubic")) "cubic" else "bbr") }
    var mtu by remember { mutableStateOf(if (Engine.tuning.contains("mtu=9000")) "9000" else "1452") }
    var applied by remember { mutableStateOf(false) }

    Page("Diagnostics", "Find out whether the network or this phone is the bottleneck.", onBack) {
        item { SectionLabel("Network speed test") }
        item {
            Group {
                Block {
                    Text(
                        "Sends 32 MB each way over the same encrypted connection Bolt uses, with no files, no disk and no checksums. " +
                            "If this is fast but a real transfer is slow, the phone's storage or CPU is the limit; if this is slow too, it's Wi-Fi.",
                        style = MaterialTheme.typography.bodyMedium, color = c.textSecondary,
                    )
                }
                if (devices.isEmpty()) ListRow("No devices found", subtitle = "Open Bolt on the other device and join the same Wi-Fi.")
                devices.forEachIndexed { i, d ->
                    GroupDivider()
                    ListRow(
                        d.name, subtitle = "Run test", leading = { Avatar(d.name) },
                        onClick = { Engine.runLinkTest(d.addr, d.id, d.name) },
                    )
                }
            }
        }
        item {
            when (val l = link) {
                LinkState.Idle -> {}
                is LinkState.Running -> Group { Block { Text("Testing with ${l.target}… (this takes a little while on slow links)", color = c.text) } }
                is LinkState.Failed -> Group { Block { Text(l.message, color = c.bad) } }
                is LinkState.Done -> Group {
                    val r = l.result
                    ListRow("Upload  ${f(r.upMBps)} MB/s", subtitle = "${f(r.upMBps * 8, 1)} Mbit/s · this phone → ${r.target} · loss ${f(r.upLossPct, 1)}%")
                    GroupDivider()
                    ListRow("Download  ${f(r.downMBps)} MB/s", subtitle = "${f(r.downMBps * 8, 1)} Mbit/s · ${r.target} → this phone · loss ${f(r.downLossPct, 1)}%")
                    GroupDivider()
                    ListRow("Round trip ${f(r.rttMs, 1)} ms", subtitle = "connect ${r.connectMs} ms · datagram size ${r.mtu} B")
                }
            }
        }

        item { SectionLabel("Last transfer") }
        item {
            Group {
                val t = last
                if (t == null) EmptyState(Icons.Default.Info, "No transfer yet", "Send or receive something, then come back here.")
                else Block {
                    val mbps = t.bytes / 1e6 / (t.elapsedMs.coerceAtLeast(1) / 1000.0)
                    Text("${if (t.dir == "send") "Sent to" else "Received from"} ${t.peer}: ${formatBytes(t.bytes)} in ${f(t.elapsedMs / 1000.0)} s (${f(mbps)} MB/s)", style = MaterialTheme.typography.titleMedium, color = c.text)
                    Text(t.stages, fontFamily = FontFamily.Monospace, fontSize = 12.sp, color = c.textSecondary)
                    Text(t.path, fontFamily = FontFamily.Monospace, fontSize = 12.sp, color = c.textSecondary)
                    t.copyMs?.let { Text("Copy to Downloads: ${it} ms (separate from the transfer)", style = MaterialTheme.typography.bodyMedium, color = c.textSecondary) }
                    Text("net = waiting on the network · disk = file I/O · hash = checksum · sync = flush to storage", style = MaterialTheme.typography.bodySmall, color = c.textTertiary)
                }
            }
        }

        item { SectionLabel("Wi-Fi") }
        item {
            Group {
                Block { wifiSummary(ctx).forEach { Text(it, style = MaterialTheme.typography.bodyMedium, color = c.text) } }
            }
        }

        item {
            val (rcv, snd) = Engine.udpBuffers.collectAsState().value
            Group {
                Block {
                    Text("UDP buffers granted by Android: receive ${rcv / 1024} KiB · send ${snd / 1024} KiB (Bolt asks for 8192 KiB)", style = MaterialTheme.typography.bodyMedium, color = c.text)
                    if (rcv in 1 until 1_000_000) Text("The receive buffer is small, so fast bursts can overflow it and be counted as packet loss. BBR copes with this better than Cubic.", style = MaterialTheme.typography.bodySmall, color = c.warn)
                }
            }
        }

        item { SectionLabel("Network tuning (experimental)") }
        item {
            Group {
                Block {
                    Text("Congestion control", style = MaterialTheme.typography.titleMedium, color = c.text)
                    Segmented(listOf("cubic" to "Cubic", "bbr" to "BBR"), cc) { cc = it; applied = false }
                    Text("Largest packet size to probe", style = MaterialTheme.typography.titleMedium, color = c.text, modifier = Modifier.padding(top = Space.s))
                    Segmented(listOf("1452" to "1452 (Wi-Fi safe)", "9000" to "Up to 9000"), mtu) { mtu = it; applied = false }
                    FdButton(if (applied) "Applied" else "Apply and restart Bolt", {
                        Engine.tuning = "cc=$cc,mtu=$mtu"
                        Engine.restart()
                        applied = true
                    }, style = BtnStyle.Secondary, enabled = !applied)
                    Text("Both devices use their own setting for what they send. Re-run the speed test after changing it.", style = MaterialTheme.typography.bodySmall, color = c.textTertiary)
                }
            }
        }
    }
}
