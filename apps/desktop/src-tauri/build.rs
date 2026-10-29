fn main() {
    // Without these, replacing an icon file leaves the old icon embedded in the exe
    // (tauri-build did not re-run), which is how the taskbar kept showing a stale logo.
    println!("cargo:rerun-if-changed=icons");
    println!("cargo:rerun-if-changed=tauri.conf.json");
    tauri_build::build()
}
