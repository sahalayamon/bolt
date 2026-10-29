# Bolt

Offline, encrypted, direct file sharing between Windows and Android over the local
network (same Wi-Fi or hotspot). No internet, accounts, cloud or telemetry.

**Current milestone: Phase 1–3 engine + Windows desktop app, verified on Windows.** The Android app is not
built yet — see below. Nothing here claims otherwise.

## What works (verified by automated tests and a real two-process run)
- QUIC + TLS 1.3 transport with mutual device certificates; identity pinning.
- Explicit pairing: QR/URI token + 6-digit code shown on both devices; single use, expiring, lock-out.
- Send/receive of files and whole folders (structure preserved, symlinks not followed),
  empty files, Unicode names, duplicate names (never overwritten).
- Receiver approval for unknown devices; optional auto-accept only for paired devices.
- Streaming I/O with a single bounded buffer, QUIC flow-control backpressure.
- Whole-file BLAKE3 verification, fsync, atomic finalisation; "completed" only after verified ack.
- Cancellation (either side), resume from partial files bound to sender+file version, batch retry skips finished files.
- Hostile-input defences: traversal, reserved names, oversize frames, surplus bytes, wrong hashes.
- mDNS discovery with duplicate suppression/expiry, manual `IP[:port]` connection (verified discovering a live receiver).
- SQLite store: trusted devices, searchable/filterable history, settings.
- Measured loopback throughput on this machine (Ryzen 7 5800H, NVMe): ~170 MB/s for 1 GiB
  through the full path (transport is the limit). Not a LAN/Wi-Fi figure.

Tests: `cargo test --workspace` passes and clippy is clean; the desktop UI adds 32 Vitest tests (see below).

## Desktop app (`apps/desktop`) — Tauri 2 + React + strict TS + Tailwind 4 + Framer Motion + Zustand
Wraps the crates above in typed Tauri commands/events (no networking reimplemented). Layout: compact two-pane window (device list + search on the left, selected-device detail on the right; one pane at a time on narrow windows). Screens: Home (real mDNS-discovered
devices only, drag-and-drop send), Send review, incoming-offer dialog, Transfers (progress/speed/ETA from `SpeedMeter`,
cancel/retry), History (search/filter/clear), Devices (pair via QR + link, 6-digit code compare, rename, revoke), Settings
(persisted in the SQLite store). Dark-first with light/system theme, reduced-motion support (OS or in-app), keyboard
navigation (Ctrl+1…5, section icons in the list header, focus-trapped dialogs, skip link). Details: [docs/desktop.md](docs/desktop.md).

Verified on this Windows 11 machine:
- `cargo tauri build` succeeds (release exe + MSI + NSIS installers); `npm run typecheck` and `npm run build` are clean.
- `npm test`: 32 UI tests (offer dialog trust/space/focus states, pair-code dialog, progress/cancel/retry rows, history
  search/filter/clear, rename/revoke, QR pairing, settings, shell/theme/motion/keyboard) with the backend mocked.
- **Real transfers between the built desktop app and the `bolt` CLI** (`apps/desktop/scripts/e2e-cli.mjs`, 23 checks,
  all passing). The script launches the real app and drives its real UI/commands over WebView2's DevTools port:
  mDNS discovery of the CLI; pairing in both directions with matching 6-digit codes; app→CLI folder send (4 files incl.
  empty + Unicode names, byte-identical); CLI→app send accepted by clicking the real offer dialog (byte-identical);
  decline reported to the sender; cancel mid-transfer then retry (64 MB, hash-identical); unreachable peer → failed +
  retryable; rename/revoke; settings; history rows written for sent and received transfers.

Not verified / known limits of the desktop app:
- Both ends ran on **one machine** (loopback + local interfaces); no two-computer LAN, no Wi-Fi, no Windows↔Android run.
  Speeds seen in the run (tens to ~140 MB/s) are loopback numbers, not network performance.
- The OS drag-and-drop path (Tauri `onDragDropEvent`) and the native file/folder pickers are wired but were **not**
  exercised by automation; sends in the e2e run were started through the same `start_send` command the UI calls.
- Installers were built but not installed/uninstalled; the app is unsigned (SmartScreen will warn). Auto-update, tray,
  start-on-login, native OS notifications and sounds do not exist (in-app toasts + taskbar attention only).
- An unanswered incoming offer is not auto-dismissed in the UI when the sender gives up; answering then reports that the
  offer is no longer available.
- The pairing QR encodes the machine's outbound-route LAN IP (override with `BOLT_ADVERTISE_IP`); multi-NIC/VPN
  setups may pick the wrong address. The desktop renders the QR but cannot scan one (paste the link instead).
- Light theme and the Devices/Settings/History screens were checked by tests and a screenshot pass, not a full visual audit.

## Android app (`apps/android`) — what was verified
Kotlin + Jetpack Compose + Material 3 (custom dark theme), Rust core via JNI (`crates/bolt-android`),
`NsdManager` discovery/advertising, SAF pickers (files/photos/folder), share-sheet receiving, foreground service
with notifications (progress, Accept/Decline, results), QR display + CameraX/ZXing scanner, paste/link pairing.
The core gained a `FileOpener` abstraction so a SAF `content://` document is read through a file descriptor, never a path.

Run on an Android 14 x86_64 **emulator** against the `bolt` CLI (details: [docs/android-testing.md](docs/android-testing.md)):
- Phone → CLI: 20 MiB file chosen in the system picker, received and hash-verified (SHA-256 identical).
- CLI → phone: 3-file folder (nested, Unicode name, empty file) accepted in the app, saved to `Download/Bolt`, hashes identical.
- Pairing in both directions with matching 6-digit codes; the phone's QR decodes (third-party decoder) to a valid pairing URI.
- The phone discovered the desktop devices over mDNS. (The host cannot see the emulator: NAT.)

Not verified: a physical phone or real Wi-Fi, camera QR scanning (emulator has no camera), share-sheet receive, Photos/Folder pickers,
notification actions while backgrounded, resume/cancel UI, large transfers. Emulator speeds (~1–5 MB/s) say nothing about real devices.

## What does not exist yet
- Desktop app (Tauri 2 + React) — handled separately, not part of this milestone.
- Sounds, onboarding, Windows DPAPI / Android Keystore protection of the private key, per-IP rate limiting.
- Parallel per-file streams, pause UI (resume exists in the protocol), disk-space exhaustion test on a real full volume.
- TCP fallback, Wi-Fi Direct, network-interface selection.
- Physical Windows↔Android transfer: **never run**.
- Multi-GB and 10k-file benchmarks beyond the 1 GiB loopback run.
- Android: received files are staged in app storage then copied (temporary double disk use); release signing/minification.

## Docs
[pending](docs/pending.md) · [architecture](docs/architecture.md) · [protocol](docs/protocol.md) · [security](docs/security.md) · [development](docs/development.md) · [performance](docs/performance.md) · [android testing](docs/android-testing.md)

## Quick start
See [docs/development.md](docs/development.md).
