//! "Show in folder" with the received items highlighted.

use std::path::{Path, PathBuf};

/// Opens the file manager on the items' folder with those items selected (Windows: all of
/// them via the shell; elsewhere: just opens the containing folder).
pub fn reveal(paths: &[PathBuf]) -> Result<(), String> {
    let Some(first) = paths.first() else {
        return Err("Nothing to show".into());
    };
    #[cfg(windows)]
    {
        if select_in_explorer(paths).is_ok() {
            return Ok(());
        }
        // Fallback: Explorer's own /select (one item).
        use std::os::windows::process::CommandExt;
        return std::process::Command::new("explorer")
            .raw_arg(format!("/select,\"{}\"", first.display()))
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Cannot open folder: {e}"));
    }
    #[cfg(not(windows))]
    {
        let dir = first.parent().unwrap_or(Path::new("."));
        let opener = if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        std::process::Command::new(opener)
            .arg(dir)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Cannot open folder: {e}"))
    }
}

#[cfg(windows)]
fn wide(p: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    p.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// Selects every item that lives in the first item's folder (the shell can only select within one folder).
#[cfg(windows)]
fn select_in_explorer(paths: &[PathBuf]) -> Result<(), String> {
    use windows::core::PCWSTR;
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
    use windows::Win32::UI::Shell::Common::ITEMIDLIST;
    use windows::Win32::UI::Shell::{
        ILCreateFromPathW, ILFindLastID, ILFree, SHOpenFolderAndSelectItems,
    };

    let parent = paths[0].parent().ok_or("no parent folder")?.to_path_buf();
    let same: Vec<&PathBuf> = paths
        .iter()
        .filter(|p| p.parent() == Some(parent.as_path()))
        .collect();

    unsafe {
        let com = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
        let parent_w = wide(&parent);
        let parent_pidl = ILCreateFromPathW(PCWSTR(parent_w.as_ptr()));
        let mut full: Vec<*mut ITEMIDLIST> = Vec::new();
        for p in &same {
            let w = wide(p);
            let pidl = ILCreateFromPathW(PCWSTR(w.as_ptr()));
            if !pidl.is_null() {
                full.push(pidl);
            }
        }
        let result = if parent_pidl.is_null() || full.is_empty() {
            Err("could not resolve the folder".to_string())
        } else {
            // Child ids point inside the full ids, which stay alive until the call returns.
            let children: Vec<*const ITEMIDLIST> = full
                .iter()
                .map(|p| ILFindLastID(*p) as *const ITEMIDLIST)
                .collect();
            SHOpenFolderAndSelectItems(parent_pidl, Some(&children), 0).map_err(|e| e.to_string())
        };
        for p in full {
            ILFree(Some(p));
        }
        if !parent_pidl.is_null() {
            ILFree(Some(parent_pidl));
        }
        if com {
            CoUninitialize();
        }
        result
    }
}

/// File types that run code when opened. "Open" never launches these; they are shown in their folder instead.
pub fn is_risky(p: &Path) -> bool {
    const RISKY: &[&str] = &[
        "exe",
        "com",
        "scr",
        "bat",
        "cmd",
        "msi",
        "msp",
        "ps1",
        "psm1",
        "vbs",
        "vbe",
        "js",
        "jse",
        "wsf",
        "wsh",
        "hta",
        "lnk",
        "url",
        "reg",
        "dll",
        "cpl",
        "msc",
        "pif",
        "jar",
        "appx",
        "msix",
        "application",
        "gadget",
        "inf",
        "sh",
        "command",
        "app",
        "apk",
    ];
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| RISKY.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// Opens a received file (or folder) with the system's default application.
pub fn open_with_default(p: &Path) -> Result<(), String> {
    #[cfg(windows)]
    let opener = "explorer";
    #[cfg(target_os = "macos")]
    let opener = "open";
    #[cfg(all(unix, not(target_os = "macos")))]
    let opener = "xdg-open";
    std::process::Command::new(opener)
        .arg(p)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Cannot open: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn programs_and_scripts_are_never_opened() {
        for bad in [
            "a.exe",
            "B.BAT",
            "x.ps1",
            "setup.msi",
            "evil.lnk",
            "run.JS",
            "app.apk",
        ] {
            assert!(is_risky(Path::new(bad)), "{bad}");
        }
        for ok in [
            "photo.jpg",
            "clip.mp4",
            "doc.pdf",
            "notes.txt",
            "folder",
            "archive.zip",
        ] {
            assert!(!is_risky(Path::new(ok)), "{ok}");
        }
    }
}
