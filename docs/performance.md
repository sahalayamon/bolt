# Transfer performance: investigation and tools

Reported: a 56 MB file between the PC and a real phone over home Wi-Fi took ~20 s (~2.8 MB/s). The same engine does ~140–160 MB/s over loopback
on the PC, so the limit is the phone or the network path, not the core.

**What could and could not be measured here.** No real phone was available. All phone-side numbers below come from the Android emulator, whose
network is NAT-limited by design, so they validate the *tools*, not Wi-Fi speed. The congestion-control and MTU comparison comes from an
in-process **simulated** path (rate limit, delay, random loss, tail-drop queue, MTU cap) — a model, not a Wi-Fi measurement.

## 1. Where does the time go? (per-stage timing)
Every send and receive now reports stage times and QUIC's own path statistics:
```
stages: net 1.56s (85%) · disk 0.03s (1%) · hash 0.10s (5%) · sync 0.14s (8%) · finalize 0.00s (0%) · 19000 calls × ~13473 B
path:   rtt 0.2 ms · cwnd 76091 KiB · mtu 1452 · lost 0/180310 pkts (0.00%) · 0 congestion events
```
`net` = waiting on the network (read, or flow-controlled write), `disk` = file I/O, `hash` = BLAKE3, `sync` = fsync, `finalize` = publish/ack
round trips. Shown by the `bolt` CLI after each transfer, logged under `RUST_LOG=bolt::timing=info`, returned in the Android "finished" events and
shown on the phone under **Diagnostics → Last transfer** (plus the time of the copy into Downloads).

## 2. Network-only link test
`bolt linktest --to IP[:PORT] [--mb N]` and **Diagnostics → Network speed test** on the phone stream N MB over the real QUIC connection in both
directions with no files, disk or hashing, and print MB/s, round trip, loss, datagram size. How to read it:
* link test fast, real transfer slow → storage/CPU (see the stage breakdown);
* link test slow too → Wi-Fi/path. Compare upload vs download, and run it near the router.
(The peer enforces a 1 GiB cap and uses a transfer slot, so it cannot be used to hog the receiver.)

## 3. Congestion control and MTU
Loopback (`bolt bench --mib 512`, 4 settings): 148–162 MB/s for every combination — no difference.

Simulated paths (`cargo test -p bolt-transfer-core --release --lib -- --ignored --nocapture sim_matrix`, one run each, goodput as % of the link rate):

| path | Cubic | BBR |
|---|---|---|
| 100 Mbit/s, 2 ms RTT, 0 % loss | 39–64 % | 57–71 % |
| 60 Mbit/s, 6 ms RTT, 0.5 % loss | **13–15 %** (~1.1 MB/s) | **63–91 %** |
| 30 Mbit/s, 10 ms RTT, 2 % loss | **13–14 %** (~0.5 MB/s) | **92–94 %** |
| 15 Mbit/s, 20 ms RTT, 4 % loss | **14–16 %** (~0.3 MB/s) | **91–92 %** |

Cubic is loss-based: with 0.5 % random loss its window sits at ~32 KiB (its throughput is capped near `1.22·MSS/(RTT·√p)`), which at a ~10 ms RTT is
the same few MB/s seen on the phone. Wi-Fi loses packets for radio reasons that are not congestion, so this is a plausible mechanism, **not a proven
cause** — the phone's own link test and stage timings decide it. BBR does not treat random loss as congestion. Its caveat: with a shallow queue it
overshoots (the simulator shows 40–60 % of its packets dropped at the bottleneck), which wastes airtime on a shared link.
**Change made:** `EndpointTuning::default()` is now BBR (previously Cubic). `BOLT_CC=cubic|bbr|reno` and `BOLT_MTU` override it for experiments;
the phone has the same switch under Diagnostics → Network tuning.

MTU: probing up to 9000 vs capping at 1452 showed no consistent difference (probing found 1472 on a 1500 path, +1.4 %; results differ by run noise),
and none on loopback. **Change made:** default upper bound is now 1452 (no probing beyond a normal Ethernet/Wi-Fi MTU).

## 4. Other findings and fixes
* **UDP buffers.** The core asks for 8 MiB socket buffers; Android granted **512 KiB** (even on the emulator). A small receive buffer drops bursts, which a
  loss-based controller reads as congestion. The granted size is now logged and shown under Diagnostics (with a warning when small).
* **Per-packet receive loop.** The receiver wrote and hashed every ~1.4 KB packet separately (16–19 k iterations for 24 MB). It now fills a full chunk first:
  file-write time for 256 MB fell from 0.26 s to 0.03 s on the PC. On a slow phone CPU this overhead is larger; the effect there is unmeasured.
* **Wi-Fi power-save.** A `WIFI_MODE_FULL_LOW_LATENCY` Wi-Fi lock is now held while bytes move (and during link tests), in addition to the foreground service and
  partial wake lock. Android only honours low-latency mode while the app is in the foreground with the screen on, so background transfers may still be
  power-managed. Battery saver state is shown in Diagnostics. The effect is unmeasured (needs a real phone).
* **Staging → copy double write (Android).** Received files are verified in app storage and then copied to Downloads (MediaStore/SAF). The copy now uses a
  kernel-side channel copy with a stream fallback and its duration is shown in Diagnostics. It is an extra read+write per file (typically well under a
  second for tens of MB on phone flash), so it is unlikely to explain 20 s; the new `copy` figure will confirm or rule it out.
* Link-test bug found while building it: the responder closed the connection before a slow download had been delivered; fixed (waits for acknowledgement).

## 5. What to do on the phone
1. Install the new APK; open **Diagnostics**; run the speed test against the PC (and note the Wi-Fi band/link speed and the UDP buffer line).
2. Send the 56 MB file and read **Last transfer**: if `net` dominates and the link test is also slow, it is the network; if `disk`/`hash`/`finalize` or `copy` dominate, it is the phone.
3. Try BBR vs Cubic and 1452 vs 9000 there (Apply restarts Bolt), re-running the speed test each time.
