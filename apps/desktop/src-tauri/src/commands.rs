//! Typed Tauri commands. Errors are returned as plain strings for the UI to show.

use crate::core::{now, save_settings, Core, RetrySpec};
use crate::dto::*;
use bolt_discovery::parse_manual_endpoint;
use bolt_security::{DeviceId, PairingUri, TrustStore, TrustedDevice};
use bolt_transfer_core::{pair_with, scan, SendItem, TransferConfig};
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt;

type Res<T> = Result<T, String>;

/// Best-effort LAN address for the pairing QR. A connected UDP socket sends no packets.
fn local_ip() -> String {
    if let Ok(ip) = std::env::var("BOLT_ADVERTISE_IP") {
        return ip;
    }
    std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| {
            s.connect("192.0.2.1:9")?;
            s.local_addr()
        })
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".into())
}

#[tauri::command]
pub fn app_info(core: State<'_, Core>) -> AppInfoDto {
    let s = core
        .settings
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    let (active, err) = core.discovery_state();
    AppInfoDto {
        device_id: core.identity.device_id().to_string(),
        short_id: core.identity.device_id().short().to_string(),
        device_name: s.device_name,
        port: core.port,
        download_dir: s.download_dir,
        discovery_active: active,
        discovery_error: err,
        local_ip: local_ip(),
        version: env!("CARGO_PKG_VERSION"),
    }
}

/// Rebuilds the mDNS session right away (the "refresh" button).
#[tauri::command]
pub async fn refresh_devices(core: State<'_, Core>) -> Res<()> {
    core.start_discovery();
    Ok(())
}

#[tauri::command]
pub fn list_devices(core: State<'_, Core>) -> Vec<DeviceDto> {
    core.devices_dto()
}

#[tauri::command]
pub fn list_trusted(core: State<'_, Core>) -> Res<Vec<TrustedDto>> {
    let online = core.online_ids();
    Ok(core
        .store
        .list()
        .map_err(|e| e.to_string())?
        .iter()
        .map(|t| TrustedDto::from_trusted(t, online.contains(&t.id)))
        .collect())
}

#[tauri::command]
pub fn rename_trusted(core: State<'_, Core>, id: String, name: String) -> Res<()> {
    let name: String = name.chars().filter(|c| !c.is_control()).collect();
    let name = name.trim().to_string();
    if name.is_empty() || name.chars().count() > 60 {
        return Err("Name must be 1 to 60 characters".into());
    }
    let id = DeviceId::parse(&id).map_err(|e| e.to_string())?;
    let existing = core
        .store
        .list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|t| t.id == id)
        .ok_or("Device is not paired")?;
    core.store
        .trust(TrustedDevice { name, ..existing })
        .map_err(|e| e.to_string())?;
    core.emit("trusted-changed", ());
    Ok(())
}

#[tauri::command]
pub fn revoke_trusted(core: State<'_, Core>, id: String) -> Res<()> {
    let id = DeviceId::parse(&id).map_err(|e| e.to_string())?;
    core.store.revoke(&id).map_err(|e| e.to_string())?;
    core.emit("trusted-changed", ());
    core.emit("devices-changed", core.devices_dto());
    Ok(())
}

#[tauri::command]
pub async fn scan_paths(paths: Vec<String>) -> Res<ScanDto> {
    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    let items = tokio::task::spawn_blocking(move || scan(&paths))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("Cannot read selection: {e}"))?;
    Ok(ScanDto {
        file_count: items.len(),
        total_bytes: items.iter().map(|i| i.size).sum(),
        files: items
            .iter()
            .take(100)
            .map(|i| ScanEntryDto {
                name: i.rel_path.clone(),
                size: i.size,
            })
            .collect(),
    })
}

#[tauri::command]
pub fn start_send(
    core: State<'_, Core>,
    paths: Vec<String>,
    target: SendTarget,
    text: Option<String>,
) -> Res<TransferDto> {
    let text = text.filter(|t| !t.is_empty());
    if paths.is_empty() && text.is_none() {
        return Err("Choose at least one file or folder, or enter some text".into());
    }
    if let Some(t) = &text {
        // Same limits as the core; fail here so the UI can say so before connecting.
        SendItem::text(t, 0).map_err(|e| e.to_string())?;
    }
    let peer = target
        .device_id
        .as_deref()
        .map(DeviceId::parse)
        .transpose()
        .map_err(|e| e.to_string())?;
    let (addr, name) = match (&peer, target.address.as_deref()) {
        (_, Some(a)) if !a.trim().is_empty() => (
            parse_manual_endpoint(a)?,
            peer.as_ref()
                .and_then(|p| core.device_name_of(p))
                .unwrap_or_else(|| a.trim().to_string()),
        ),
        (Some(p), _) => (
            core.address_for(p)
                .ok_or("That device is no longer visible on the network")?,
            core.device_name_of(p).unwrap_or_else(|| "Device".into()),
        ),
        _ => return Err("Choose a device or enter an address".into()),
    };
    let spec = RetrySpec {
        paths: paths.into_iter().map(PathBuf::from).collect(),
        addr,
        peer,
        text,
    };
    Ok(core.spawn_send(None, spec, name))
}

#[tauri::command]
pub fn list_transfers(core: State<'_, Core>) -> Vec<TransferDto> {
    core.transfers_snapshot()
}

#[tauri::command]
pub fn cancel_transfer(core: State<'_, Core>, id: String) -> Res<()> {
    core.cancel_transfer(&id)
}

#[tauri::command]
pub fn retry_transfer(core: State<'_, Core>, id: String) -> Res<TransferDto> {
    core.retry_transfer(&id)
}

#[tauri::command]
pub fn dismiss_transfer(core: State<'_, Core>, id: String) {
    core.dismiss_transfer(&id)
}

#[tauri::command]
pub fn respond_offer(
    core: State<'_, Core>,
    id: String,
    accept: bool,
    dest_dir: Option<String>,
) -> Res<()> {
    core.respond_offer(&id, accept, dest_dir)
}

#[tauri::command]
pub fn respond_pair(core: State<'_, Core>, id: String, approve: bool) -> Res<()> {
    core.respond_pair(&id, approve)
}

#[tauri::command]
pub fn begin_pairing(core: State<'_, Core>) -> Res<PairingInfoDto> {
    let address = format!("{}:{}", local_ip(), core.port);
    let uri = core
        .receiver
        .begin_pairing(std::time::Duration::from_secs(300), address.clone());
    let text = uri.encode();
    let svg = qrcode::QrCode::new(text.as_bytes())
        .map_err(|e| e.to_string())?
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(240, 240)
        .quiet_zone(true)
        .dark_color(qrcode::render::svg::Color("#0b0d12"))
        .light_color(qrcode::render::svg::Color("#ffffff"))
        .build();
    Ok(PairingInfoDto {
        uri: text,
        qr_svg: svg,
        expires_unix: uri.expires_unix,
        address,
    })
}

#[tauri::command]
pub fn end_pairing(core: State<'_, Core>) {
    core.receiver.end_pairing();
}

/// Pair with a device by its pairing link. The 6-digit code is delivered as a
/// `pair-code` event as soon as it is known; this call resolves once the other
/// device approves (or fails).
#[tauri::command]
pub async fn pair_with_uri(core: State<'_, Core>, uri: String) -> Res<TrustedDto> {
    let core = core.inner().clone();
    let uri =
        PairingUri::parse(uri.trim()).map_err(|e| format!("Not a valid pairing link: {e}"))?;
    if uri.expires_unix != 0 && (uri.expires_unix as i64) < now() {
        return Err("This pairing link has expired. Ask the other device for a new one.".into());
    }
    let addr = parse_manual_endpoint(&uri.addr)?;
    let local = core.local_info();
    let emitter = core.clone();
    let peer = pair_with(
        &core.ep,
        addr,
        &uri,
        &local,
        &TransferConfig::default(),
        core.store.as_ref(),
        move |code| emitter.emit("pair-code", code),
    )
    .await
    .map_err(|e| e.to_string())?;
    core.emit("trusted-changed", ());
    Ok(TrustedDto {
        id: peer.id.to_string(),
        short_id: peer.id.short().to_string(),
        name: peer.name,
        paired_at: now(),
        last_seen: Some(now()),
        online: true,
    })
}

#[tauri::command]
pub fn history_list(core: State<'_, Core>, filter: Option<HistoryFilter>) -> Res<Vec<HistoryDto>> {
    core.history(&filter.unwrap_or_default())
}

#[tauri::command]
pub fn history_clear(core: State<'_, Core>) -> Res<usize> {
    let n = core.store.clear_history().map_err(|e| e.to_string())?;
    core.emit("history-changed", ());
    Ok(n)
}

#[tauri::command]
pub fn get_settings(core: State<'_, Core>, app: AppHandle) -> SettingsDto {
    let mut s = core
        .settings
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    // The OS is the source of truth for "start on login".
    s.start_on_login = app.autolaunch().is_enabled().unwrap_or(false);
    s.context_menu = crate::shellext::is_registered() || crate::shellmenu::is_enabled();
    s
}

/// Set once when Bolt was started silently (by "start on login"); the window stays hidden until it is opened.
pub struct BackgroundStart(pub std::sync::atomic::AtomicBool);

pub fn show_main_window(app: &AppHandle) {
    if let Some(bg) = app.try_state::<BackgroundStart>() {
        bg.0.store(false, std::sync::atomic::Ordering::Relaxed);
    }
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// The UI calls this once it has painted. Windows started in the background stay hidden (tray only).
#[tauri::command]
pub fn show_when_ready(app: AppHandle) {
    let background = app
        .try_state::<BackgroundStart>()
        .map(|b| b.0.load(std::sync::atomic::Ordering::Relaxed))
        .unwrap_or(false);
    if !background {
        show_main_window(&app);
    }
}

#[tauri::command]
pub async fn update_settings(
    core: State<'_, Core>,
    app: AppHandle,
    settings: SettingsDto,
) -> Res<SettingsDto> {
    let core = core.inner().clone();
    let mut s = settings;
    s.device_name = s
        .device_name
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .to_string();
    if s.device_name.is_empty() || s.device_name.chars().count() > 60 {
        return Err("Device name must be 1 to 60 characters".into());
    }
    if !matches!(s.theme.as_str(), "dark" | "light" | "system")
        || !matches!(s.reduce_motion.as_str(), "system" | "on" | "off")
    {
        return Err("Unknown appearance option".into());
    }
    if s.download_dir.trim().is_empty() {
        return Err("Choose a download folder".into());
    }
    std::fs::create_dir_all(&s.download_dir)
        .map_err(|e| format!("Cannot use that download folder: {e}"))?;
    let previous = core
        .settings
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    save_settings(&core.store, &s)?;
    core.receiver
        .set_download_dir(PathBuf::from(&s.download_dir));
    core.receiver.set_auto_accept_trusted(s.auto_accept_trusted);
    core.receiver.set_device_name(s.device_name.clone());
    *core.settings.lock().unwrap_or_else(|p| p.into_inner()) = s.clone();
    // Start on login is a real OS setting (a Run entry), applied only when you change the switch.
    let launcher = app.autolaunch();
    let enabled = launcher.is_enabled().unwrap_or(false);
    if s.start_on_login != enabled {
        let r = if s.start_on_login {
            launcher.enable()
        } else {
            launcher.disable()
        };
        r.map_err(|e| format!("Could not change start on login: {e}"))?;
    }
    // The right-click entry is a registry setting (current user only), applied when you change the switch.
    let menu_on = crate::shellext::is_registered() || crate::shellmenu::is_enabled();
    if s.context_menu != menu_on {
        if s.context_menu {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let devices = core.menu_devices();
            // Preferred: the compact Windows 11 menu. It needs Developer Mode; otherwise use the classic menu.
            match tokio::task::spawn_blocking({
                let (exe, devices) = (exe.clone(), devices.clone());
                move || crate::shellext::register(&exe, &devices)
            })
            .await
            .map_err(|e| e.to_string())?
            {
                Ok(()) => crate::shellmenu::remove(),
                Err(why) => {
                    tracing::warn!(
                        "compact right-click menu unavailable ({why}); using the classic menu"
                    );
                    crate::shellmenu::write(&exe, &devices)
                        .map_err(|e| format!("Could not add the right-click menu: {e}"))?;
                }
            }
        } else {
            let _ = tokio::task::spawn_blocking(crate::shellext::unregister).await;
            crate::shellmenu::remove();
        }
    }
    if previous.device_name != s.device_name {
        // The mDNS advertisement carries the name; re-announce under the new one.
        core.start_discovery();
    }
    Ok(s)
}

/// Opens a link from received text in the default browser. Only http(s) URLs, and only because the
/// user clicked "Open"; the text itself is never opened automatically.
#[tauri::command]
pub fn open_link(app: AppHandle, url: String) -> Res<()> {
    let url = url.trim();
    let ok = url.len() <= 4096
        && !url.chars().any(|c| c.is_whitespace() || c.is_control())
        && (url.starts_with("https://") || url.starts_with("http://"))
        && url.split("://").nth(1).is_some_and(|rest| !rest.is_empty());
    if !ok {
        return Err("Only http and https links can be opened".into());
    }
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| format!("Cannot open link: {e}"))
}

/// Opens the configured download folder in the system file manager.
#[tauri::command]
pub fn open_download_dir(core: State<'_, Core>) -> Res<()> {
    let dir = core
        .settings
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .download_dir
        .clone();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    #[cfg(windows)]
    let opener = "explorer";
    #[cfg(target_os = "macos")]
    let opener = "open";
    #[cfg(all(unix, not(target_os = "macos")))]
    let opener = "xdg-open";
    std::process::Command::new(opener)
        .arg(&dir)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Cannot open folder: {e}"))
}

/// "Show folder" for a received transfer: opens the file manager with what arrived selected;
/// falls back to the download folder if we no longer know (or the files were moved).
#[tauri::command]
pub async fn reveal_transfer(core: State<'_, Core>, id: String) -> Res<()> {
    let items: Vec<PathBuf> = core
        .reveal_paths(&id)
        .into_iter()
        .filter(|p| p.exists())
        .collect();
    if items.is_empty() {
        return open_download_dir(core);
    }
    // The shell call needs its own thread (it talks to Explorer over COM and must not block the UI thread).
    tokio::task::spawn_blocking(move || crate::reveal::reveal(&items))
        .await
        .map_err(|e| e.to_string())?
}

/// "Open" for a received transfer (live card or history row): opens the file (or folder) with the
/// default app so images/videos can be viewed. Programs and scripts are never launched.
#[tauri::command]
pub async fn open_received(core: State<'_, Core>, id: String) -> Res<()> {
    let mut items: Vec<PathBuf> = core
        .reveal_paths(&id)
        .into_iter()
        .filter(|p| p.exists())
        .collect();
    if items.is_empty() {
        // Not from this session: look the entry up in history and find it in the download folder.
        let entry = core
            .history(&HistoryFilter::default())?
            .into_iter()
            .find(|h| h.transfer_id == id)
            .ok_or("This transfer is not in the history")?;
        if entry.direction != "received" || entry.status != "completed" {
            return Err("Only received files can be opened".into());
        }
        let name = std::path::Path::new(&entry.title);
        let plain = name.components().count() == 1
            && matches!(
                name.components().next(),
                Some(std::path::Component::Normal(_))
            );
        if !plain {
            return Err("Cannot locate this file".into());
        }
        let dir = core
            .settings
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .download_dir
            .clone();
        let p = PathBuf::from(dir).join(&entry.title);
        if p.exists() {
            items.push(p);
        }
    }
    let Some(first) = items.first().cloned() else {
        return Err("The file is no longer in the download folder".into());
    };
    if items.len() > 1 {
        // several items: show them selected instead of guessing which one to open
        return tokio::task::spawn_blocking(move || crate::reveal::reveal(&items))
            .await
            .map_err(|e| e.to_string())?;
    }
    if crate::reveal::is_risky(&first) {
        let _ = crate::reveal::reveal(&items);
        return Err(
            "For safety, programs and scripts are not opened. It was shown in its folder instead."
                .into(),
        );
    }
    crate::reveal::open_with_default(&first)
}

const MAX_PASTE_BYTES: usize = 200 * 1024 * 1024;

fn hex_val(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        _ => c - b'A' + 10,
    }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && b[i + 1].is_ascii_hexdigit()
            && b[i + 2].is_ascii_hexdigit()
        {
            let v = (hex_val(b[i + 1]) << 4) | hex_val(b[i + 2]);
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn safe_file_name(raw: &str) -> String {
    let last = raw.rsplit(['/', '\\']).next().unwrap_or("");
    let cleaned: String = last
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned
        .trim_matches(|c: char| c == '.' || c == ' ')
        .to_string();
    if cleaned.is_empty() {
        "pasted".into()
    } else {
        cleaned.chars().take(120).collect()
    }
}

/// Writes pasted clipboard bytes (raw IPC body, name in the `x-name` header, URL-encoded) to a
/// private temp folder and returns the file path, so it can be sent like any other file.
#[tauri::command]
pub fn stage_clipboard(request: tauri::ipc::Request<'_>) -> Res<String> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err("Expected raw clipboard data".into());
    };
    if bytes.is_empty() {
        return Err("The clipboard is empty".into());
    }
    if bytes.len() > MAX_PASTE_BYTES {
        return Err("Pasted data is too large (limit 200 MB). Send it as a file instead.".into());
    }
    let name = request
        .headers()
        .get("x-name")
        .and_then(|v| v.to_str().ok())
        .map(percent_decode)
        .unwrap_or_default();
    let root = std::env::temp_dir().join("bolt-paste");
    // Opportunistic cleanup of previous pastes older than a day.
    if let Ok(rd) = std::fs::read_dir(&root) {
        let cutoff = std::time::SystemTime::now() - std::time::Duration::from_secs(24 * 3600);
        for e in rd.flatten() {
            if e.metadata()
                .and_then(|m| m.modified())
                .is_ok_and(|t| t < cutoff)
            {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = root.join(format!("{nonce:x}"));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(safe_file_name(&name));
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

/// Brings the main window up, optionally on a given screen (used by the tray dashboard).
#[tauri::command]
pub fn open_main_window(app: AppHandle, view: Option<String>) {
    show_main_window(&app);
    hide_panel(&app);
    if let Some(v) = view {
        let _ = tauri::Emitter::emit_to(&app, "main", "navigate", v);
    }
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DraftRequest {
    paths: Vec<String>,
    device_id: Option<String>,
}

/// Opens the main window on the review screen with these files (the tray dashboard picked them).
#[tauri::command]
pub fn draft_in_main(app: AppHandle, paths: Vec<String>, device_id: Option<String>) -> Res<()> {
    if paths.is_empty() {
        return Err("Choose at least one file or folder".into());
    }
    show_main_window(&app);
    hide_panel(&app);
    tauri::Emitter::emit_to(
        &app,
        "main",
        "start-draft",
        DraftRequest { paths, device_id },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
pub fn hide_tray_panel(app: AppHandle) {
    hide_panel(&app);
}

fn hide_panel(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("tray") {
        let _ = w.hide();
    }
}

const PREVIEW_MAX_BYTES: u64 = 12 * 1024 * 1024;
const DRAG_ICON: &[u8] = include_bytes!("../icons/32x32.png");

fn image_mime(p: &std::path::Path) -> Option<&'static str> {
    match p.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        _ => None,
    }
}

/// A data URL of the first received picture (small enough to show inline), or None for other kinds of files.
#[tauri::command]
pub async fn received_preview(core: State<'_, Core>, id: String) -> Res<Option<String>> {
    let items = core.reveal_paths(&id);
    let found = items
        .into_iter()
        .find(|p| p.is_file() && image_mime(p).is_some());
    let Some(path) = found else { return Ok(None) };
    let mime = image_mime(&path).unwrap_or("image/png");
    tokio::task::spawn_blocking(move || -> Res<Option<String>> {
        let len = std::fs::metadata(&path).map_err(|e| e.to_string())?.len();
        if len > PREVIEW_MAX_BYTES {
            return Ok(None);
        }
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        use base64::Engine;
        Ok(Some(format!(
            "data:{mime};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        )))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Starts a real operating-system drag of the received files, so they can be dropped into any app or folder.
#[tauri::command]
pub fn drag_received(window: tauri::WebviewWindow, core: State<'_, Core>, id: String) -> Res<()> {
    let items: Vec<PathBuf> = core
        .reveal_paths(&id)
        .into_iter()
        .filter(|p| p.exists())
        .collect();
    if items.is_empty() {
        return Err("These files are no longer in the download folder".into());
    }
    let w = window.clone();
    window
        .run_on_main_thread(move || {
            let _ = drag::start_drag(
                &w,
                drag::DragItem::Files(items),
                drag::Image::Raw(DRAG_ICON.to_vec()),
                |_result, _pos| {},
                drag::Options::default(),
            );
        })
        .map_err(|e| e.to_string())
}

/// Files chosen from the Explorer right-click menu, waiting for the window to pick them up.
#[tauri::command]
pub fn take_pending_draft() -> Option<crate::launch::Draft> {
    crate::launch::take()
}
