//! Files handed to Bolt from the Explorer right-click menu (`--send-to <id> <path>` / `--send <path>`).
//! Explorer starts one process per selected file, so paths are collected for a moment and then handed to the
//! window as a single batch.

use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::Duration,
};
use tauri::{AppHandle, Emitter};

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Draft {
    pub paths: Vec<String>,
    pub device_id: Option<String>,
}

/// Parsed command line: `Some((device, paths))` when it carries files to send.
pub fn parse_args(args: &[String]) -> Option<(Option<String>, Vec<PathBuf>)> {
    let mut device = None;
    let mut sending = false;
    let mut paths = Vec::new();
    let mut it = args.iter().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--send" => sending = true,
            "--send-to" => {
                sending = true;
                let id = it.next()?;
                // a device id is 64 hex characters; anything else is ignored rather than trusted
                if id.len() == 64 && id.chars().all(|c| c.is_ascii_hexdigit()) {
                    device = Some(id.to_ascii_lowercase());
                }
            }
            "--background" => {}
            other if other.starts_with("--") => {}
            path => {
                let p = PathBuf::from(path);
                if p.exists() {
                    paths.push(p);
                }
            }
        }
    }
    (sending && !paths.is_empty()).then_some((device, paths))
}

static PENDING: Mutex<Option<Draft>> = Mutex::new(None);
static GENERATION: AtomicU64 = AtomicU64::new(0);

fn lock() -> std::sync::MutexGuard<'static, Option<Draft>> {
    PENDING.lock().unwrap_or_else(|p| p.into_inner())
}

/// Adds files to the batch; once no more arrive for a moment the window opens on the review screen.
pub fn add(app: &AppHandle, device: Option<String>, paths: Vec<PathBuf>) {
    {
        let mut g = lock();
        let d = g.get_or_insert(Draft {
            paths: Vec::new(),
            device_id: device.clone(),
        });
        if device.is_some() {
            d.device_id = device;
        }
        for p in paths {
            let s = p.to_string_lossy().into_owned();
            if !d.paths.contains(&s) {
                d.paths.push(s);
            }
        }
    }
    let ticket = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(700)).await;
        if GENERATION.load(Ordering::SeqCst) != ticket {
            return; // more files arrived; the newer call will fire
        }
        crate::commands::show_main_window(&app);
        let _ = app.emit_to("main", "draft-ready", ());
    });
}

/// The window asks for the batch (also right after a cold start, before it could receive the event).
pub fn take() -> Option<Draft> {
    lock().take()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_send_to_with_a_device_and_existing_paths() {
        let here = std::env::current_dir()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let id = "ab".repeat(32);
        let got = parse_args(&args(&[
            "bolt",
            "--send-to",
            &id,
            &here,
            "C:/definitely/missing",
        ]))
        .unwrap();
        assert_eq!(got.0.as_deref(), Some(id.as_str()));
        assert_eq!(got.1.len(), 1);
    }

    #[test]
    fn ignores_bad_ids_and_plain_launches() {
        let here = std::env::current_dir()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let got = parse_args(&args(&["bolt", "--send-to", "not-an-id", &here])).unwrap();
        assert!(got.0.is_none());
        assert!(parse_args(&args(&["bolt"])).is_none());
        assert!(parse_args(&args(&["bolt", "--background"])).is_none());
        assert!(parse_args(&args(&["bolt", &here])).is_none()); // paths alone never trigger a send
    }
}
