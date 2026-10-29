# Development

## Build and test (verified on Windows 11, Rust 1.96)
```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets
cargo test --workspace                      # 55 tests, ~10 s
cargo build --release -p bolt-cli      # target/release/bolt.exe
```

## Real two-process demo (same machine, or two machines on one LAN)
```bash
# machine A (receiver); set BOLT_ADVERTISE_IP to A's LAN IP when pairing from another machine
bolt --data a-data receive --dir inbox --port 47600 --pair
# machine B
bolt --data b-data pair --uri 'bolt://pair?...'     # compare the 6-digit codes
bolt --data b-data send --to <A-ip>:47600 --peer <A-device-id> folder-or-files...
bolt discover --secs 6                                   # mDNS
```
Manual connection (no mDNS) is `--to IP[:PORT]`.

## Benchmarks
```bash
bolt bench --mib 1024
cargo test -p bolt-transfer-core --release -- --ignored --nocapture raw_quic
```
Loopback numbers measure CPU/crypto/UDP-stack limits only. Real Wi-Fi throughput
must be measured on devices; none is claimed here.

## Android app (apps/android)
One-time toolchain setup (done on the dev machine; versions that were used):
```bash
rustup target add aarch64-linux-android x86_64-linux-android
cargo install cargo-ndk                                   # 4.1.2
# SDK command-line tools are not bundled with Android Studio's SDK here; download
# commandlinetools-win-*_latest.zip into %LOCALAPPDATA%\Android\Sdk\cmdline-tools\latest, then:
sdkmanager --sdk_root=%LOCALAPPDATA%\Android\Sdk "ndk;27.2.12479018"
```
Build (Gradle runs `cargo ndk` for arm64-v8a + x86_64 first; system `java` is 1.8, so point at Android Studio's JBR):
```bash
cd apps/android
export JAVA_HOME="C:\Program Files\Android\Android Studio\jbr"
./gradlew.bat assembleDebug            # add -PskipRust to reuse already-built .so files
# -> app/build/outputs/apk/debug/app-debug.apk
```
`local.properties` (git-ignored) must contain `sdk.dir=C:/Users/<you>/AppData/Local/Android/Sdk`.

### Emulator notes (hard-won)
* Create the AVD with plenty of RAM (the default 1.5 GB kernel-panics Android 16 with "deadlocked on memory").
* On this machine the `android-36.1` image's SurfaceFlinger aborts periodically
  (`hasReadColorBufferDma`), restarting the framework. See `docs/android-testing.md` for what was used instead.
* The emulator is NATed: it reaches the host at `10.0.2.2`; the host reaches the emulator through
  `adb emu redir add udp:<hostport>:47600` (adb's own `forward` is TCP-only and useless for QUIC).
  mDNS cannot cross that NAT, so emulator tests use manual `IP:port`.

## Remaining toolchain steps
```bash
cargo install tauri-cli --version "^2"                 # desktop app (already installed here: cargo-tauri)
```

### Fast emulator loop
Do **not** launch with `-no-snapshot`: every launch is then a cold boot of several minutes. Launch with
`emulator -avd BoltApi34 -no-window -no-audio -gpu swiftshader_indirect`; after the first boot, `adb emu avd snapshot save default_boot` then `adb emu kill`.
Later launches restore the saved state in ~6 s. UI-only changes need just `./gradlew assembleDebug -PskipRust` (~10 s); only Rust changes need the
full `cargo ndk` build (1–7 min).
