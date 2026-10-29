# Pending (not verified or not built)

## Desktop app: not verified
- [ ] Transfer between **two computers** on a real LAN/Wi-Fi/hotspot (all runs so far were one machine, loopback).
- [ ] Real Wi-Fi throughput (loopback speeds are not network speeds).
- [ ] OS **drag-and-drop** of files/folders onto the window (wired via Tauri `onDragDropEvent`, not automated).
- [ ] Native **file/folder pickers** (tauri-plugin-dialog) end to end.
- [ ] Installing/uninstalling the MSI and NSIS installers; first-run on a clean machine.
- [ ] Light theme and a full visual/responsive audit of Devices, Settings, History.
- [ ] Screen-reader pass (roles/labels exist and are unit-tested; no real AT run).
- [ ] Gateway-probe fallback against a real phone hotspot (unit + loopback tests only; the phone was on a normal Wi-Fi when last tried). Only the default gateway is probed, not the whole subnet.
- [ ] Tray icon, close-to-tray, "Start Bolt when I sign in" (compiled and unit-tested only; never run: check login start, tray menu, window raise, idle CPU)
- [ ] Multi-NIC/VPN machines: pairing QR may advertise the wrong IP.
- [ ] Multi-GB and 10k-file transfers through the UI.

## Desktop app: not built
- [ ] Android app and Windows↔Android transfer (`apps/android`).
- [ ] QR **scanning** on desktop (it only renders QR; paste the link instead).
- [ ] Native OS notifications, sounds, onboarding flow.
- [ ] Auto-dismiss of an incoming offer when the sender gives up (answering shows "no longer available").
- [ ] Pause/resume buttons (resume exists in the engine and is used by Retry).
- [ ] Network-interface selection; configurable port in Settings (only `BOLT_PORT` env).
- [ ] Code signing, auto-update.
- [ ] Per-IP rate limiting; DPAPI protection of the private key.
- [ ] TCP fallback, Wi-Fi Direct, parallel per-file streams.
