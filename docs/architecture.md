# Bolt architecture

## Goal
Direct, encrypted, offline file transfer between Windows and Android over a local
network (same Wi-Fi or phone hotspot). No cloud, accounts, relays or telemetry.

## Toolchain found on the dev machine (2026-10-18)
| Tool | Status |
|---|---|
| Rust 1.96 (MSVC), VS 2022 Build Tools | OK |
| Node 24 / npm 11 | OK (needed for the Tauri UI) |
| Android SDK (platforms 34/36, build-tools 34–36) | OK |
| JDK | system `java` is **1.8** — too old; use Android Studio's bundled JBR 17+ (`C:\Program Files\Android\Android Studio\jbr`) |
| `cargo tauri` CLI | OK (tauri-cli 2.12.3) |
| Android NDK 27.2 / `cargo-ndk` 4.1.2, Rust Android targets | installed (2026-10-18) |

## Layout
```
crates/
  protocol/        wire messages, framing, path validation          (no I/O deps)
  security/        identity, TLS verifiers, pairing, TrustStore trait
  transfer-core/   QUIC endpoint, sender, receiver, staging/resume, progress
  discovery/       mDNS advertise/browse, device registry, manual endpoints
  transfer-store/  SQLite: trusted devices, history, settings (implements TrustStore)
  bolt-cli/   headless client: real two-process proof + benchmark
apps/desktop/      Tauri 2 + React shell over these crates (see desktop.md)
apps/android/      Kotlin/Compose app; Rust core via JNI (crates/bolt-android)
```
Dependency direction: `protocol` ← `security` ← `transfer-core`; `discovery` and
`transfer-store` depend on `protocol`/`security` only. UIs depend on all of them.

## Key decisions and trade-offs
1. **QUIC (quinn) + TLS 1.3 only.** Multiplexed streams, built-in encryption,
   flow control (our backpressure), connection migration across Wi-Fi changes.
   Trade-off: UDP can be blocked on some guest/corporate networks. There is
   currently **no TCP fallback**; the transport is chosen explicitly, never
   silently downgraded. A TLS-over-TCP fallback would be a separate, explicit
   transport selected by capability negotiation.
2. **Identity = self-signed certificate, trust = explicit pairing.** The
   `DeviceId` is BLAKE3 of the certificate. Custom rustls verifiers accept any
   self-signed cert *but still verify the handshake signature*, proving key
   possession; the app then checks the id against the pinned/trusted one. No CA.
3. **One bidirectional stream per transfer, files sequential.** Simple,
   deterministic ordering, trivially bounded memory (one chunk buffer). Parallel
   streams per file are a future optimisation; concurrency today is *across
   transfers* (receiver limits via a semaphore).
4. **Raw bytes after a small JSON header.** Control messages are JSON (debuggable,
   versionable, trivially implementable in Kotlin); file data has no per-chunk framing.
5. **Whole-file BLAKE3 verified by the receiver before a file is published.**
   Fast (SIMD), streaming, resumable (receiver re-hashes its own prefix).
6. **Receiver owns filesystem safety.** Paths are validated on arrival and
   re-checked at finalisation (symlink/escape). Sender sanitises names it emits.
7. **UI-agnostic core.** The receiver emits events (`OfferReceived`,
   `PairRequested`, `TransferStarted/Finished`) and progress via `watch`
   channels throttled to ≈10 Hz; there is no UI code in the engine.
8. **Android**: discovery and file pickers use native APIs (NsdManager / SAF).
   The Rust core is reused via JNI where practical; SAF `content://` streams
   need an fd-based file abstraction in `transfer-core` (not done yet — see
   "Remaining work").

## Data flow (send)
`scan` → `SendRequest` → connect + identity pin → HELLO → OFFER → (user accepts)
→ ACCEPT(resume points) → per file: FILE_START, raw bytes, FILE_COMPLETE(hash)
→ FILE_ACK(ok) → TRANSFER_COMPLETE. "Completed" is reported only after the final ack.

## Status
See README.md ("What works / what does not").
