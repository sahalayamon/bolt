# Desktop app (`apps/desktop`)

Tauri 2 shell + React 19 / strict TypeScript / Vite / Tailwind v4 / Framer Motion / Zustand / Sonner / Lucide.
It **wraps** the existing crates (`transfer-core`, `discovery`, `transfer-store`, `security`); there is no
networking code in the app itself.

## Layout
```
apps/desktop
  src/                      React UI
    lib/api.ts              typed wrappers over the Tauri commands + events (only place that calls invoke/listen)
    lib/types.ts            TS mirror of src-tauri/src/dto.rs
    store/app.ts            Zustand store; subscribes to backend events
    views/                  Home, SendReview, Transfers, History, Devices, Settings
    components/             DevicePane (left list + section icons), DeviceAvatar, dialogs (incoming offer, pair request), UI primitives (Modal with focus trap, ...)
    test/                   Vitest + Testing Library (backend mocked at lib/api)
  src-tauri/src/
    core.rs                 owns Identity/Store/FlashEndpoint/Receiver/Discovery; bridges channels to Tauri events
    commands.rs             #[tauri::command] surface
    dto.rs                  serde (camelCase) shapes
  scripts/e2e-cli.mjs       real-network verification against the `bolt` CLI (drives the built app over DevTools)
```

## Data flow
* **Receive**: `Receiver` emits `IncomingEvent`s on an mpsc channel → `core.rs` keeps the `IncomingOffer` /
  `IncomingPair` handle in a map and emits `incoming-offer` / `incoming-pair` → UI dialog → `respond_offer` /
  `respond_pair` consume the handle (`accept()` / `reject()` / `approve()` / `deny()`). Dropping unanswered handles
  declines, as in the engine.
* **Progress**: each transfer's `watch::Receiver<Progress>` (≈10 Hz, throttled by the engine) is forwarded through a
  `SpeedMeter`; the UI gets `transfer-update` with `bytesDone/bytesTotal/speedBps/etaSecs`. Terminal state always
  comes from the authoritative `send_files` / `TransferFinished` result, never from the watch.
* **Send**: `start_send` resolves a discovered device id (or a manual `IP[:port]`) to an address, scans, and runs
  `send_files` in a task with a `CancellationToken`. `retry_transfer` re-runs the same selection (the engine resumes
  verified partials).
* **Discovery**: `Discovery::devices()` watch → `devices-changed`. Only devices actually announced over mDNS are shown.
* **Persistence**: history, trusted devices and settings use `transfer-store` (SQLite) in the app data dir.

## Commands
`app_info, list_devices, list_trusted, rename_trusted, revoke_trusted, scan_paths, start_send, list_transfers,
cancel_transfer, retry_transfer, dismiss_transfer, respond_offer, respond_pair, begin_pairing, end_pairing,
pair_with_uri, history_list, history_clear, get_settings, update_settings, open_download_dir`.

Events: `devices-changed, trusted-changed, transfer-update, history-changed, incoming-offer, incoming-pair, pair-code`.

## Environment overrides (testing/dev)
`BOLT_DATA_DIR`, `BOLT_DOWNLOAD_DIR` (default only), `BOLT_PORT`, `BOLT_ADVERTISE_IP`
(address put in the pairing QR; otherwise the outbound-route LAN IP is used).

## Develop / build
```bash
cd apps/desktop
npm install
npm test                 # Vitest UI tests
npm run typecheck
cargo tauri dev          # hot-reloading app
cargo tauri build        # release exe + installers (target/release/bundle)
node scripts/e2e-cli.mjs # real transfer between the built app and the bolt CLI (Windows)
```

## Hotspot discovery fallback
When this computer joins a phone hotspot, the phone is the default gateway and Android often does not answer mDNS on that link. Every ~10 s the app dials the default gateway(s) on the Bolt port with `probe_peer` (HELLO exchange only, nothing offered) and lists whoever answers next to the mDNS devices. mDNS wins when both see a device.

## Naming
The product is **Bolt** everywhere: window title, UI text, installer name, default device name `Bolt Desktop`, default download folder `Downloads\Bolt`,
crate and CLI names, `BOLT_*` environment variables, the `_bolt._udp` mDNS service, the `bolt://pair` link scheme, the `bolt/1` ALPN and the Tauri
identifier `app.bolt.desktop`. The identifier decides where Windows keeps the app's data, so an install that predates it starts with a fresh device
identity and no pairings.
