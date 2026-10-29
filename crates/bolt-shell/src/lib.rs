//! The "Bolt" entry in the Windows 11 *compact* right-click menu.
//!
//! Windows only shows entries there that come from a packaged app (the sparse package Bolt registers) which
//! points at a COM class implementing `IExplorerCommand`. This DLL is that class. It is deliberately tiny:
//!
//! * the device list and the path of `bolt-desktop.exe` come from `%LOCALAPPDATA%\Bolt\shellext\config.json`,
//!   which the running Bolt keeps up to date;
//! * choosing an entry just starts `bolt-desktop.exe --send-to <id> <files...>`; the running Bolt picks it up.
//!
//! Nothing else happens in Explorer's process: no networking, no file access beyond reading the selection.

#![cfg(windows)]

use serde::Deserialize;
use std::{
    cell::Cell, ffi::c_void, os::windows::process::CommandExt, path::PathBuf, process::Command,
};
use windows::{
    core::*,
    Win32::{Foundation::*, System::Com::*, UI::Shell::*},
};

/// Class id of the command; must match the manifest Bolt registers (`shellext.rs` in the desktop app).
pub const CLSID_BOLT: GUID = GUID::from_u128(0xb01a5c3e_4d7f_4e21_9a6c_2f8d3b7e90c1);

#[derive(Deserialize, Clone, Default)]
struct Dev {
    id: String,
    name: String,
}

#[derive(Deserialize, Default)]
struct Config {
    #[serde(default)]
    exe: String,
    #[serde(default)]
    devices: Vec<Dev>,
}

fn load_config() -> Config {
    let path = std::env::var_os("LOCALAPPDATA").map(|p| {
        PathBuf::from(p)
            .join("Bolt")
            .join("shellext")
            .join("config.json")
    });
    path.and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn alloc_wide(s: &str) -> Result<PWSTR> {
    let w: Vec<u16> = s.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe { SHStrDupW(PCWSTR(w.as_ptr())) }
}

#[derive(Clone)]
enum Target {
    Root,
    Device(Dev),
    Choose,
}

#[implement(IExplorerCommand)]
struct Cmd {
    target: Target,
}

impl IExplorerCommand_Impl for Cmd_Impl {
    fn GetTitle(&self, _items: Ref<IShellItemArray>) -> Result<PWSTR> {
        match &self.target {
            Target::Root => alloc_wide("Bolt"),
            Target::Device(d) => alloc_wide(&d.name),
            Target::Choose => alloc_wide("Choose a recipient\u{2026}"),
        }
    }

    fn GetIcon(&self, _items: Ref<IShellItemArray>) -> Result<PWSTR> {
        match &self.target {
            Target::Root => alloc_wide(&format!("{},0", load_config().exe)),
            _ => Err(E_NOTIMPL.into()),
        }
    }

    fn GetToolTip(&self, _items: Ref<IShellItemArray>) -> Result<PWSTR> {
        Err(E_NOTIMPL.into())
    }

    fn GetCanonicalName(&self) -> Result<GUID> {
        Ok(CLSID_BOLT)
    }

    fn GetState(&self, _items: Ref<IShellItemArray>, _ok_to_be_slow: BOOL) -> Result<u32> {
        Ok(ECS_ENABLED.0 as u32)
    }

    fn Invoke(&self, items: Ref<IShellItemArray>, _bind: Ref<IBindCtx>) -> Result<()> {
        let cfg = load_config();
        let mut cmd = Command::new(&cfg.exe);
        match &self.target {
            Target::Root => return Ok(()), // only opens the submenu
            Target::Device(d) => {
                cmd.arg("--send-to").arg(&d.id);
            }
            Target::Choose => {
                cmd.arg("--send");
            }
        }
        let Some(array) = items.as_ref() else {
            return Ok(());
        };
        let mut any = false;
        unsafe {
            for i in 0..array.GetCount()? {
                let item = array.GetItemAt(i)?;
                let name = item.GetDisplayName(SIGDN_FILESYSPATH)?;
                let path = name.to_string().unwrap_or_default();
                CoTaskMemFree(Some(name.0 as *const c_void));
                if !path.is_empty() {
                    cmd.arg(path);
                    any = true;
                }
            }
        }
        if any {
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW)
                .spawn()
                .map_err(|_| Error::from(E_FAIL))?;
        }
        Ok(())
    }

    fn GetFlags(&self) -> Result<u32> {
        const ECF_HASSUBCOMMANDS: u32 = 0x1;
        Ok(match self.target {
            Target::Root => ECF_HASSUBCOMMANDS,
            _ => 0,
        })
    }

    fn EnumSubCommands(&self) -> Result<IEnumExplorerCommand> {
        if !matches!(self.target, Target::Root) {
            return Err(E_NOTIMPL.into());
        }
        let cfg = load_config();
        let mut items: Vec<IExplorerCommand> = cfg
            .devices
            .iter()
            .take(12)
            .map(|d| {
                Cmd {
                    target: Target::Device(d.clone()),
                }
                .into()
            })
            .collect();
        items.push(
            Cmd {
                target: Target::Choose,
            }
            .into(),
        );
        Ok(Enum {
            items,
            pos: Cell::new(0),
        }
        .into())
    }
}

#[implement(IEnumExplorerCommand)]
struct Enum {
    items: Vec<IExplorerCommand>,
    pos: Cell<usize>,
}

impl IEnumExplorerCommand_Impl for Enum_Impl {
    fn Next(&self, celt: u32, out: *mut Option<IExplorerCommand>, fetched: *mut u32) -> HRESULT {
        let mut n = 0u32;
        unsafe {
            while n < celt && self.pos.get() < self.items.len() {
                out.add(n as usize)
                    .write(Some(self.items[self.pos.get()].clone()));
                self.pos.set(self.pos.get() + 1);
                n += 1;
            }
            if !fetched.is_null() {
                *fetched = n;
            }
        }
        if n == celt {
            S_OK
        } else {
            S_FALSE
        }
    }

    fn Skip(&self, celt: u32) -> Result<()> {
        self.pos
            .set((self.pos.get() + celt as usize).min(self.items.len()));
        Ok(())
    }

    fn Reset(&self) -> Result<()> {
        self.pos.set(0);
        Ok(())
    }

    fn Clone(&self) -> Result<IEnumExplorerCommand> {
        Ok(Enum {
            items: self.items.clone(),
            pos: Cell::new(self.pos.get()),
        }
        .into())
    }
}

#[implement(IClassFactory)]
struct Factory;

impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(
        &self,
        outer: Ref<IUnknown>,
        riid: *const GUID,
        object: *mut *mut c_void,
    ) -> Result<()> {
        if outer.as_ref().is_some() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        let cmd: IExplorerCommand = Cmd {
            target: Target::Root,
        }
        .into();
        unsafe { cmd.query(riid, object).ok() }
    }

    fn LockServer(&self, _lock: BOOL) -> Result<()> {
        Ok(())
    }
}

#[no_mangle]
pub extern "system" fn DllGetClassObject(
    clsid: *const GUID,
    iid: *const GUID,
    object: *mut *mut c_void,
) -> HRESULT {
    unsafe {
        if clsid.is_null() || *clsid != CLSID_BOLT {
            return CLASS_E_CLASSNOTAVAILABLE;
        }
        let factory: IClassFactory = Factory.into();
        factory.query(iid, object)
    }
}

#[no_mangle]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    S_FALSE // stay loaded; the surrogate process unloads us when it is done
}
