//! Bolt in the Windows 11 *compact* right-click menu.
//!
//! The compact menu only lists commands from a packaged app, so Bolt registers a tiny "sparse" package
//! (identity only; the files stay where Bolt is installed) that points Explorer at `bolt_shell.dll`
//! (`crates/bolt-shell`). The device list and the path of this exe are handed to the DLL in
//! `%LOCALAPPDATA%\Bolt\shellext\config.json`, which `write_config` keeps current.
//!
//! Registering a package without a store signature needs Windows **Developer Mode** (Settings > System > For
//! developers). If that is not available, the caller falls back to the classic menu (see `shellmenu.rs`).

use crate::shellmenu::MenuDevice;
use std::path::{Path, PathBuf};

pub const PACKAGE_NAME: &str = "Bolt.ContextMenu";
pub const DLL_NAME: &str = "bolt_shell.dll";
/// Must equal `CLSID_BOLT` in `crates/bolt-shell`.
const CLSID: &str = "B01A5C3E-4D7F-4E21-9A6C-2F8D3B7E90C1";

const LOGO_44: &[u8] = include_bytes!("../icons/Square44x44Logo.png");
const LOGO_150: &[u8] = include_bytes!("../icons/Square150x150Logo.png");

fn manifest() -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
  xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
  xmlns:uap10="http://schemas.microsoft.com/appx/manifest/uap/windows10/10"
  xmlns:desktop4="http://schemas.microsoft.com/appx/manifest/desktop/windows10/4"
  xmlns:desktop5="http://schemas.microsoft.com/appx/manifest/desktop/windows10/5"
  xmlns:com="http://schemas.microsoft.com/appx/manifest/com/windows10"
  xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
  IgnorableNamespaces="uap uap10 desktop4 desktop5 com rescap">
  <Identity Name="{PACKAGE_NAME}" Publisher="CN=Bolt" Version="1.0.0.0" ProcessorArchitecture="x64" />
  <Properties>
    <DisplayName>Bolt</DisplayName>
    <PublisherDisplayName>Bolt</PublisherDisplayName>
    <Logo>logo150.png</Logo>
    <uap10:AllowExternalContent>true</uap10:AllowExternalContent>
  </Properties>
  <Resources>
    <Resource Language="en-us" />
  </Resources>
  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.19041.0" MaxVersionTested="10.0.26100.0" />
  </Dependencies>
  <Capabilities>
    <rescap:Capability Name="runFullTrust" />
  </Capabilities>
  <Applications>
    <Application Id="BoltMenu" Executable="bolt-desktop.exe" uap10:TrustLevel="mediumIL" uap10:RuntimeBehavior="win32App">
      <uap:VisualElements DisplayName="Bolt" Description="Send with Bolt" BackgroundColor="transparent"
        Square150x150Logo="logo150.png" Square44x44Logo="logo44.png" AppListEntry="none" />
      <Extensions>
        <desktop4:Extension Category="windows.fileExplorerContextMenus">
          <desktop4:FileExplorerContextMenus>
            <desktop5:ItemType Type="*">
              <desktop5:Verb Id="BoltSend" Clsid="{CLSID}" />
            </desktop5:ItemType>
            <desktop5:ItemType Type="Directory">
              <desktop5:Verb Id="BoltSendFolder" Clsid="{CLSID}" />
            </desktop5:ItemType>
          </desktop4:FileExplorerContextMenus>
        </desktop4:Extension>
        <com:Extension Category="windows.comServer">
          <com:ComServer>
            <com:SurrogateServer DisplayName="Bolt context menu">
              <com:Class Id="{CLSID}" Path="{DLL_NAME}" ThreadingModel="STA" />
            </com:SurrogateServer>
          </com:ComServer>
        </com:Extension>
      </Extensions>
    </Application>
  </Applications>
</Package>
"#
    )
}

/// Path text without the `\\?\` prefix that `canonicalize()` adds (the manifest and PowerShell reject it).
fn plain(p: &Path) -> String {
    let s = p.display().to_string();
    s.strip_prefix(r"\\?\").map(str::to_string).unwrap_or(s)
}

pub fn work_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("Bolt").join("shellext"))
}

/// Writes the exe path and device list that the DLL reads each time the menu opens.
pub fn write_config(exe: &Path, devices: &[MenuDevice]) -> std::io::Result<()> {
    let dir = work_dir().ok_or_else(|| std::io::Error::other("no LOCALAPPDATA"))?;
    std::fs::create_dir_all(&dir)?;
    let devs: Vec<serde_json::Value> = devices
        .iter()
        .map(|(id, name)| serde_json::json!({ "id": id, "name": name }))
        .collect();
    let cfg = serde_json::json!({ "exe": plain(exe), "devices": devs });
    // write-then-rename so Explorer never reads a half-written file
    let tmp = dir.join("config.json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(&cfg)?)?;
    std::fs::rename(tmp, dir.join("config.json"))
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::os::windows::process::CommandExt;
    use winreg::{enums::*, RegKey};

    pub fn is_registered() -> bool {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let Ok(repo) = hkcu.open_subkey(
            r"Software\Classes\Local Settings\Software\Microsoft\Windows\CurrentVersion\AppModel\Repository\Packages",
        ) else {
            return false;
        };
        let prefix = format!("{PACKAGE_NAME}_");
        let found = repo.enum_keys().flatten().any(|k| k.starts_with(&prefix));
        found
    }

    fn powershell(script: &str) -> Result<String, String> {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let out = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                script,
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| format!("could not start PowerShell: {e}"))?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).into_owned())
        } else {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            Err(if err.is_empty() {
                String::from_utf8_lossy(&out.stdout).trim().to_string()
            } else {
                err
            })
        }
    }

    fn quote(p: &Path) -> String {
        format!("'{}'", plain(p).replace('\'', "''"))
    }

    pub fn register(exe: &Path, devices: &[MenuDevice]) -> Result<(), String> {
        let exe_dir = exe.parent().ok_or("no install folder")?;
        let dll = exe_dir.join(DLL_NAME);
        if std::fs::metadata(&dll).map(|m| m.len()).unwrap_or(0) == 0 {
            return Err(format!("{DLL_NAME} was not found next to Bolt"));
        }
        let dir = work_dir().ok_or("no LOCALAPPDATA")?;
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("AppxManifest.xml"), manifest()).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("logo44.png"), LOGO_44).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("logo150.png"), LOGO_150).map_err(|e| e.to_string())?;
        write_config(exe, devices).map_err(|e| e.to_string())?;
        if is_registered() {
            let _ = unregister();
        }
        let script = format!(
            "$ErrorActionPreference='Stop'; Add-AppxPackage -Register {} -ExternalLocation {}",
            quote(&dir.join("AppxManifest.xml")),
            quote(exe_dir)
        );
        powershell(&script).map(|_| ())
    }

    pub fn unregister() -> Result<(), String> {
        let script = format!(
            "$ErrorActionPreference='Stop'; Get-AppxPackage -Name {PACKAGE_NAME} | Remove-AppxPackage"
        );
        powershell(&script).map(|_| ())
    }
}

#[cfg(windows)]
pub use imp::{is_registered, register, unregister};

#[cfg(not(windows))]
pub fn is_registered() -> bool {
    false
}
#[cfg(not(windows))]
pub fn register(_exe: &Path, _devices: &[MenuDevice]) -> Result<(), String> {
    Err("Only available on Windows".into())
}
#[cfg(not(windows))]
pub fn unregister() -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_names_the_class_and_dll_consistently() {
        let m = manifest();
        assert!(m.contains(&format!("Clsid=\"{CLSID}\"")));
        assert!(m.contains(&format!("Path=\"{DLL_NAME}\"")));
        assert!(m.contains("windows.fileExplorerContextMenus"));
        // the class id must stay in step with the DLL
        assert_eq!(
            CLSID.to_ascii_lowercase().replace('-', ""),
            "b01a5c3e4d7f4e219a6c2f8d3b7e90c1"
        );
    }
}

#[cfg(all(test, windows))]
mod live_tests {
    use super::*;

    /// Registers the real sparse package for the current user and removes it again.
    /// Needs Windows Developer Mode and `cargo build --release -p bolt-shell` beforehand. Run with `--ignored`.
    #[test]
    #[ignore = "registers and then removes the Bolt compact-menu package for the current user"]
    fn registers_and_unregisters_the_sparse_package() {
        assert!(!is_registered(), "already registered; not touching it");
        let exe = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../target/release/bolt-desktop.exe");
        let exe = exe.canonicalize().expect("build the release exe first");
        let devices = vec![("ab".repeat(32), "Test phone".to_string())];
        register(&exe, &devices).expect("Add-AppxPackage failed");
        assert!(is_registered());
        unregister().expect("Remove-AppxPackage failed");
        assert!(!is_registered());
    }
}
