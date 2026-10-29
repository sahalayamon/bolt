# Android verification log (emulator)

Date: 2026-10-18. Device: Android 14 (API 34) `google_apis` x86_64 emulator, software GL, on a Windows 11 host.
App: debug APK built by `apps/android` (Rust core compiled with cargo-ndk, NDK 27.2). Peer: `bolt` CLI
(release build) running on the host. Everything below was actually run; things not listed were not verified.

## Result summary
| Check | Result |
|---|---|
| APK builds (`assembleDebug`, includes `cargo ndk` for arm64-v8a + x86_64) | OK |
| App launches, JNI core starts, Compose UI renders | OK (screenshots taken) |
| mDNS: phone discovers desktop devices through `NsdManager` (CLI and the desktop app both seen, with names) | OK |
| mDNS: host discovers the emulator | **No** (emulator network is NATed; one-way) |
| **Phone â†’ CLI**, 20 MiB file picked through the real system file picker (SAF `content://`, opened as an fd) | OK â€” receiver reported "verified", SHA-256 identical (`f1c8a6a7â€¦`) |
| **CLI â†’ phone**, folder of 3 files (5 MB, empty file, Unicode name `note Ã¼.txt`, nested dir) | OK â€” offer dialog shown, accepted in UI, files copied to `Download/Bolt/Trip/â€¦`, SHA-256 identical, empty file kept |
| Unknown-device warning in the accept dialog | shown |
| Pairing: phone scans/pastes CLI's URI (opened via the `bolt://pair` link) | OK â€” both sides showed the same 6-digit code, "Paired with CLI-A" |
| Pairing: CLI pairs with the phone's QR (QR screenshot decoded with OpenCV, third-party decoder) | OK â€” phone showed the code + "Codes match"; CLI reported paired |
| Foreground-service notification ("Ready to receive as â€¦", "Transfer complete") | present in `dumpsys notification` |
| Throughput | emulator only: ~1.1 MB/s phoneâ†’CLI (software-emulated device), ~5.5 MB/s CLIâ†’phone. **Not a real-device or Wi-Fi figure.** |

## Not verified (honest list)
* A physical phone; real Wi-Fi/hotspot; arm64 build was compiled but never executed.
* Camera QR **scanning** (the emulator AVD has no camera). The scanner code (CameraX + ZXing) compiles; the paste/link path was used instead.
* Share-sheet receiving (`ACTION_SEND`), Photos picker, Folder picker, "choose destination folder" â€” implemented, not exercised.
* Accept/Decline from the notification while the app is in the background; long-running background behaviour/battery.
* Resume, cancel from the phone UI, multi-GB files, thousands of files.
* Android Keystore protection of the identity key (the key sits in app-private storage).

## Design notes found while testing
* Received files are verified into app-private staging (`files/incoming`), then copied to `Download/Bolt`
  (MediaStore) or a chosen SAF folder. Costs temporary double storage; keeps the verified/atomic guarantee of the core.
  The core gained a `FileOpener` abstraction (send side) so `content://` URIs never need a path.
* Bug caught on device: Rust reports canonical paths (`/data/user/0/â€¦`), the Kotlin side compared against `/data/data/â€¦`,
  which mangled the destination folder. Fixed (canonical compare + `..`/`.` stripped before publishing).
* An ANR dialog appeared once, on first launch, right after install, while the software-rendered emulator was saturated;
  it did not recur. Not reproduced on a device; treat as unproven.

## Emulator setup that worked (and what didn't)
* `android-36.1` image: SurfaceFlinger asserts `hasReadColorBufferDma` every minute or so (framework restarts), making
  testing impossible. The AVD also kernel-panicked with 1.5 GB RAM. Switched to the API 34 `google_apis` image
  (downloaded from dl.google.com, SHA-1 checked against the repository manifest).
* Launch: `emulator -avd BoltApi34 -no-window -no-audio -no-snapshot -gpu swiftshader_indirect`.
* Wi-Fi was disabled in the guest (`svc wifi disable`) for the hostâ†’phone test: with both `eth0` (10.0.2.15) and `wlan0`
  (10.0.2.16) up, QUIC replies left from `wlan0`'s address and the emulator's NAT dropped them (connect timed out).
* Hostâ†’phone: `adb emu redir add udp:47620:47600`, then `bolt send --to 127.0.0.1:47620 â€¦`.
  Phoneâ†’host: `10.0.2.2:<port>` (or mDNS, which worked for discovery).

## UI redesign pass (2026-10-19)
Android-only; no business logic changed. Design tokens live in `Theme.kt` (light + dark palettes, spacing scale, radii, type scale),
shared components in `Components.kt` (`Page`, `Group`/`ListRow`, `FdButton`, `FdField`, `DecisionSheet`, `ThinProgress`, `EmptyState`),
screens in `Screens.kt`. Light/dark follow the system, with a System/Light/Dark control in Settings.
Offers and pairing requests are now bottom sheets (swiping one away = decline).
Motion uses 140/200/300 ms eased tweens and follows the system animator scale.

Checked on the API 34 emulator after the redesign: Send, Pair and Settings tabs in light mode; Settings, Send and the incoming-offer
sheet in dark mode; and a full CLIâ†’phone transfer accepted from the new sheet (SHA-256 identical, files in `Download/Bolt/Trip`).
`assembleDebug` and `lintDebug` pass (0 errors; 23 warnings, mostly newer-dependency notices).
**Not re-checked visually:** the Activity tab, the QR/pair-request sheet, the camera scanner view, transfer progress bars while
active, tablet/landscape layouts, TalkBack, font scaling, and reduced-motion behaviour. The Material icon set used is the small core
one, so the icons are generic. Android's system font (Roboto) is used; SF Pro is not available on Android.

## Home layout and blue theme (2026-10-19)
Layout inspired by a screenshot of the Blip Android app (not a copy): one home screen with a black header, a **Your Devices** row of
large circular device bubbles (laptop/phone/tablet glyphs drawn in code, an "Address" bubble that opens a sheet, a pulsing placeholder while
searching), and a lower panel holding a **dashed drop-zone** with the Files / Photos / Folder pickers and the current selection
(tapping a device with nothing selected opens the file picker). Active transfers appear above the zone; a warning row appears only if
notifications are off or the core failed to start. Pair, Activity and Settings are sub-pages from the overflow menu.
Accent is the lime the user supplied (`#D7F62F`, dark text on it) on near-black neutrals; Dark is the default, Light/System in Settings.
Typography: Manrope (SIL OFL 1.1, bundled; licence in `app/OFL-Manrope.txt`).

Alignment audit done on screenshots of Home, Pair, Activity, Settings (dark) and Home, Settings (light): fixed bubble/margin alignment,
status-bar band colour on sub-pages, back-arrow offset, a 4 dp text misalignment in the selection card, and a crash in the back button
(negative padding). `assembleDebug` passes.
**Not re-checked after this pass:** a full transfer, the offer/pair-request sheets, QR/camera views, active-transfer cards, the
Address sheet, tablet/landscape, TalkBack and font scaling.

### Update: panel redesign (2026-10-19)
The dashed drop-zone was replaced, following a user-supplied reference image: a "Visible to nearby devices as <name>" line with a
broadcast icon under the device row, a rounded panel with a drag handle, a plus badge + "Choose what to send", and three icon tiles
(Files / Photos / Folder) drawn in code. Device bubbles were reduced from 80 dp to 64 dp. Light mode got a tinted top area and tinted
tiles so the panel separates from the page. Checked on the emulator in light and dark. Not re-checked: selection state in the new
panel, transfers, sheets, tablet layouts.

### Update: bottom-pinned panel (2026-10-19)
Per a user-supplied Photoshop mock: the send panel is pinned to the bottom and hugs its content (no blank area under the tiles), the
"Visible to nearby devices as â€¦" line sits directly above it, and the free height goes to the black device area. The panel colour now
runs behind the system navigation bar (gesture handle sits on the panel grey, not on a black strip); the navigation-bar contrast scrim is
disabled so 3-button navigation is not tinted either. Checked on the emulator (dark, gesture navigation). Not checked: 3-button navigation,
light mode after this change, many transfers at once (the panel scrolls above 520 dp), selected-files state.

### Update: "phone can't see the PC" (2026-10-19)
Reported on real devices: the PC detected the phone and could send to it, but the phone's device list never showed the PC.
Evidence from the dev PC: the already-running desktop app advertised only `127.0.0.1`, `192.168.56.1` (VirtualBox host-only) and `::1` â€” no Wi-Fi
address (`10.20.234.65`) â€” because it had started before the Wi-Fi came up and mDNS addresses were never refreshed. A freshly started
process on the same PC advertised the Wi-Fi address correctly. The phone could not resolve a reachable address for such a peer.
Fixes: `crates/discovery` now re-registers its mDNS record whenever the machine's IPv4 set changes (polled every 4 s; restart the desktop app
to pick this up). Android discovery now restarts on network change and on returning to the app when nothing was found, retries failed starts,
cannot be blocked by a hung resolve, uses the Android 14 service-info API, and prefers the peer address on the phone's own /24. "Refresh
devices" is in the â‹® menu and the "Searching" bubble is tappable.
Verified: discovery restarts on a Wi-Fi off/on on the emulator (log), Rust workspace tests + clippy pass, a fresh CLI advertises the LAN address.
**Not verified:** that the desktop watcher recovers from a real network change (needs admin to toggle an adapter here), and the Android
fixes on a physical phone. The bottom tile gap now equals the side margin (checked on the emulator).

### Update: stale devices (2026-10-19)
A device whose PC app was closed stayed in the phone's list (mDNS entries live until their TTL expires unless a goodbye packet arrives).
The phone now probes every listed device every 12 s (`probe` in transfer-core: QUIC handshake + identity check, no data) and hides it after two
consecutive misses; it reappears when it re-announces. Verified on the emulator: a CLI receiver appeared, was killed without a goodbye, and
both it and the phone's own other entry were gone ~55 s later. (A first attempt looked like a failure only because the "killed" process had
survived.) Caveat: each probe is a short incoming connection on the PC every 12 s per listed device. Quitting the desktop app cleanly should
also send an mDNS goodbye (`Discovery::stop`/drop) â€” that is the desktop app's job.

### Update: naming (2026-10-19)
The whole repository uses the name Bolt: Android package/applicationId `com.bolt.app`, JNI symbols `Java_com_bolt_app_*`, `libbolt_android.so`,
`crates/bolt-*`, the `bolt` CLI, the `bolt://pair` link, the `_bolt._udp` mDNS type, the `bolt/1` ALPN, the `bolt.local` TLS name, `BOLT_*` environment
variables and `Download/Bolt`. Changing the package name makes this a new app to Android: an earlier install under a different package is a separate app
with its own identity and pairings and has to be uninstalled by hand.
Verified after the rename: Rust workspace tests, clippy and rustfmt; desktop `tsc` and 37 frontend tests; a fresh APK on the emulator discovered a `bolt`
CLI receiver over mDNS and sent it a 4 MiB file picked through the system picker (SHA-256 identical on both sides).

### Update: incoming-file sheet (2026-10-19)
The offer sheet follows a user-supplied mock: centered "From <device>", a large file icon (`res/drawable-nodpi/offer_file.png`, supplied by the user),
file name and size, a solid red **Decline** and a blue **Accept** (56 dp pills). The unpaired-device warning is kept as a small amber line (a safety cue not
in the mock). Pairing requests use the same red/blue pair. Checked on the emulator with a real incoming CLI transfer (sheet shown, Accept completes it).
Multi-file offers show "N files" with the first three names; that layout was not visually checked.

### Update: smaller offer sheet (2026-10-19)
Offer sheet reduced (104 dp icon, 46 dp buttons) and the unpaired-device line removed on request. Decline colour sampled from the user's mock:
`#A13939` (4.5:1 with the mock's `#FFC7C7` text), so the text was lightened to `#FFE4E4` (~5.7:1); light mode uses `#C62F36` with white (5.4:1).

### Update: one card for pick / offer / progress (2026-10-19)
The bottom panel on the home screen is now a single card with three states of identical size and position: pick files, incoming offer (Decline/Accept),
and transfer progress (ring with percent, file name, "x MB of y MB Â· speed Â· time left", Cancel; then a green check with "Saved to â€¦" and Done, which also
clears itself after 10 s; failures/cancels show a red/amber ring). Off the home screen an offer still appears as a sheet. Checked on the emulator with a real
300 MB incoming transfer (offer â†’ accept â†’ progress at ~12 MB/s â†’ done). Not checked: sending from the phone, several simultaneous transfers, light mode.

### Update: slide-up offer, Open, Activity (2026-10-19)
* The incoming-offer card slides up from the bottom of the panel (and slides back down when answered); other panel changes fade. The idle card no longer has the plus badge.
* Finished card: one file â†’ **Done | Open**; several files â†’ a list with an **Open** per file. It clears itself after 30 s.
* Activity: completed rows are tappable (one file opens directly, several show a list). Locations are kept in `files/saved-files.json` (the core's history has only names/sizes),
  so transfers made before this version show "File not found". A missing or unreadable file gives a "File not found" toast; if no app can open the type, a "No app can open this file" toast.
* Checked on the emulator: multi-file and single-file finished cards, Open launching the system chooser, Activity sheet and both toasts. The slide animation itself was not captured
  mid-flight (screenshots lag), only that the card ends up in place. Opening files you *sent* relies on the original document permission and may report "File not found" later.

### Update: Open fixes (2026-10-19)
* **Open did nothing for `.apk` files.** Android only passes an APK to the installer if the user allowed "Install unknown apps" for the calling app. Bolt now declares
  `REQUEST_INSTALL_PACKAGES`; tapping Open on an APK shows a toast and opens that Settings page for Bolt (verified on the emulator). After switching it on, tap Open again.
  Other failures now say why: "File not found", "No app on this phone can open <ext> files", "Android blocked access to this file", or the exception name.
* **Open appeared late.** A received transfer was marked done as soon as the network part ended, but the file locations only exist after the copy into Downloads. The card now
  shows "Savingâ€¦" (spinning ring, no buttons) until the copy finishes, then **Done | Open** appear together (verified with a 200 MB file). Sends already had their locations, so no change there.
* Not verified: opening non-APK files on a real phone (only the emulator's Chrome/HTML viewer chooser), and actually completing an APK install.

## 2026-10-19: Clear, header row, tap-device-first

Verified on the API 34 emulator against the CLI receiver: Activity shows back arrow, title and Clear in one row; Clear asks for confirmation and empties the list (received files stay). Tapping a device with nothing selected opens the file picker and the chosen files are sent to that device automatically (CLI received armed.txt, verified). Tapping a device after selecting files still sends immediately. Cancelling the picker forgets the device; a tapped device is remembered for 120 s. Photos/Folder tiles do not auto-send.

### Follow-up (same day)

Tap-device-then-pick no longer sends by itself: a confirmation card (file image, filename or "N files selected", Cancel / Send to <device>) appears and nothing is sent until Send is tapped (verified on the emulator). The idle home card hugs its content, page titles are 26 sp, and Save name is blue when there is something to save and shows a green Saved for 2 s afterwards.

### Over-other-apps card and new icon (same day)

With Display over other apps allowed (needed once; Settings has a row, Home shows a prompt until it is on), an incoming offer while Bolt is not in front shows the accept/decline card on top of the current app (emulator: CLI send while on the launcher; Accept delivered the file). Without the permission the existing heads-up notification with Accept/Decline is used. The launcher, themed, notification and in-app logo icons come from apps/android/Icons.png; desktop icons were regenerated from apps/desktop/app-icon.png with cargo tauri icon.

### Over-other-apps card: when Android hides it

Reproduced on the emulator: with the system Settings app in front the overlay window exists but Android hides it (dumpsys window: mForceHideNonSystemOverlayWindow=true, policy visibility false); it appears as soon as the user goes to the launcher. Apps and screens that ask the system to hide overlays (Settings, permission dialogs, some banking/secure apps, Play install pages) do this, and an app cannot override it; the Accept/Decline heads-up notification is the fallback there. Bolt now counts as 'in front' only while its activity is resumed (onPause/onResume), so the card no longer waits for onStop when you switch away or open recents.

### Home-screen widget and quick-settings tile

A 4x1 widget (Bolt status, Send, Stop/Start) and a quick-settings tile turn Bolt on or off. On the emulator the widget was added from the launcher's widget picker; Stop switched the status to Off and the button to Start, Start brought it back to Ready, and Send opened the system file picker. The quick-settings tile was built but not exercised.

### Phone hosting a hotspot: laptop not listed (2026-10-29)

Symptom: with the laptop joined to the phone's hotspot, the laptop lists the phone but the phone's device list stays empty, while transfers from the laptop still arrive. NsdManager does not browse the tethering interface. Fix: bolt-android now also runs bolt-discovery in a new browse-only mode (raw multicast on every interface; `Discovery::browse`) and reports it as a `mdns` event, which Engine merges with the NsdManager list (NsdManager entries win on the same id). Verified on the emulator that the core browse alone finds LAN peers (logcat: `core mDNS browse sees N device(s)`). NOT verified: the hotspot case itself, which needs a real phone hosting a hotspot with the laptop joined to it.
