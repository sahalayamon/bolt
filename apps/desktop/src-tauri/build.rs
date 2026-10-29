fn main() {
    // Without these, replacing an icon file leaves the old icon embedded in the exe
    // (tauri-build did not re-run), which is how the taskbar kept showing a stale logo.
    println!("cargo:rerun-if-changed=icons");
    println!("cargo:rerun-if-changed=tauri.conf.json");
    // The bundled resource must exist for the build to succeed; `npm run build:shell` (or the release build's
    // beforeBuildCommand) replaces this empty placeholder with the real handler DLL.
    let dll = std::path::Path::new("shellext").join("bolt_shell.dll");
    if !dll.exists() {
        let _ = std::fs::create_dir_all("shellext");
        let _ = std::fs::write(&dll, b"");
    }
    tauri_build::build()
}
