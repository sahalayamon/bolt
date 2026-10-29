//! "Bolt" entry in the Windows right-click menu (classic menu / "Show more options"):
//!
//! ```text
//! Bolt >  Sahal's S25
//!         Office PC
//!         Choose a recipient...
//! ```
//!
//! Everything lives under the current user's registry (`HKCU\Software\Classes`), needs no admin rights and is
//! removed again by turning the setting off (or by uninstalling). Each entry just starts Bolt with
//! `--send-to <device id> "<file>"`; the running Bolt then opens its review screen for those files.

use std::path::Path;

/// One device in the submenu: (id, display name).
pub type MenuDevice = (String, String);

#[cfg(windows)]
mod imp {
    use super::MenuDevice;
    use std::io;
    use std::path::Path;
    use winreg::{enums::*, RegKey};

    const ROOTS: [&str; 2] = [
        r"Software\Classes\*\shell\Bolt",
        r"Software\Classes\Directory\shell\Bolt",
    ];

    pub fn is_enabled() -> bool {
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey(ROOTS[0])
            .is_ok()
    }

    pub fn remove() {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        for r in ROOTS {
            let _ = hkcu.delete_subkey_all(r);
        }
    }

    /// Menu text: no control characters, and `&` doubled so Explorer does not treat it as a shortcut key.
    fn label(name: &str) -> String {
        let clean: String = name.chars().filter(|c| !c.is_control()).take(60).collect();
        clean.replace('&', "&&")
    }

    pub fn write(exe: &Path, devices: &[MenuDevice]) -> io::Result<()> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let exe = exe.display().to_string();
        for root in ROOTS {
            let _ = hkcu.delete_subkey_all(root);
            let (menu, _) = hkcu.create_subkey(root)?;
            menu.set_value("MUIVerb", &"Bolt")?;
            menu.set_value("Icon", &format!("\"{exe}\",0"))?;
            menu.set_value("MultiSelectModel", &"Player")?;
            menu.set_value("SubCommands", &"")?;
            let (sub, _) = menu.create_subkey("shell")?;

            // Explorer lists sub-entries alphabetically, so the numbers keep the order: devices, then "Choose".
            for (i, (id, name)) in devices.iter().enumerate() {
                let (entry, _) = sub.create_subkey(format!("{:02}_device", i + 1))?;
                entry.set_value("MUIVerb", &label(name))?;
                let (cmd, _) = entry.create_subkey("command")?;
                cmd.set_value("", &format!("\"{exe}\" --send-to {id} \"%1\""))?;
            }
            let (choose, _) = sub.create_subkey("99_choose")?;
            choose.set_value("MUIVerb", &"Choose a recipient\u{2026}")?;
            if !devices.is_empty() {
                choose.set_value("CommandFlags", &0x20u32)?; // separator above
            }
            let (cmd, _) = choose.create_subkey("command")?;
            cmd.set_value("", &format!("\"{exe}\" --send \"%1\""))?;
        }
        Ok(())
    }
}

#[cfg(windows)]
pub fn is_enabled() -> bool {
    imp::is_enabled()
}
#[cfg(windows)]
pub fn remove() {
    imp::remove()
}
#[cfg(windows)]
pub fn write(exe: &Path, devices: &[MenuDevice]) -> std::io::Result<()> {
    imp::write(exe, devices)
}

#[cfg(not(windows))]
pub fn is_enabled() -> bool {
    false
}
#[cfg(not(windows))]
pub fn remove() {}
#[cfg(not(windows))]
pub fn write(_exe: &Path, _devices: &[MenuDevice]) -> std::io::Result<()> {
    Ok(())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use winreg::{enums::*, RegKey};

    /// Touches the real registry (current user) briefly: run with `--ignored`.
    #[test]
    #[ignore = "writes and removes HKCU\Software\Classes\*\shell\Bolt"]
    fn writes_the_submenu_and_removes_it_again() {
        assert!(!is_enabled(), "a Bolt menu is already installed; not touching it");
        let devices = vec![
            ("ab".repeat(32), "Sahal's S25".to_string()),
            ("cd".repeat(32), "Tom & Jerry".to_string()),
        ];
        write(Path::new(r"C:\Program Files\Bolt\bolt-desktop.exe"), &devices).unwrap();
        assert!(is_enabled());
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let sub = hkcu
            .open_subkey(r"Software\Classes\Directory\shell\Bolt\shell")
            .unwrap();
        let names: Vec<String> = sub.enum_keys().map(|k| k.unwrap()).collect();
        assert_eq!(names, vec!["01_device", "02_device", "99_choose"]);
        let second: String = sub.open_subkey("02_device").unwrap().get_value("MUIVerb").unwrap();
        assert_eq!(second, "Tom && Jerry");
        let cmd: String = sub.open_subkey(r"01_device\command").unwrap().get_value("").unwrap();
        assert!(cmd.contains("--send-to abab") && cmd.ends_with("\"%1\""), "{cmd}");
        remove();
        assert!(!is_enabled());
    }
}
