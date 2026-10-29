mod commands;
mod core;
mod dto;
mod launch;
mod netinfo;
mod reveal;
mod shellmenu;

use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};

/// When the tray panel last hid itself because it lost focus (so the tray click that caused it does not re-open it).
static PANEL_BLURRED_AT: AtomicI64 = AtomicI64::new(0);
const PANEL_SIZE: (f64, f64) = (340.0, 470.0);

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Show the small tray dashboard just above the tray icon (or hide it if it is already open).
fn toggle_tray_panel(app: &tauri::AppHandle, click: tauri::PhysicalPosition<f64>) {
    let Some(w) = app.get_webview_window("tray") else {
        return;
    };
    if w.is_visible().unwrap_or(false) {
        let _ = w.hide();
        return;
    }
    if now_ms() - PANEL_BLURRED_AT.load(Ordering::Relaxed) < 350 {
        return;
    }
    let size = w
        .outer_size()
        .map(|s| (s.width as f64, s.height as f64))
        .unwrap_or((PANEL_SIZE.0, PANEL_SIZE.1));
    let (mut x, mut y) = (click.x - size.0 / 2.0, click.y - size.1 - 14.0);
    if let Ok(Some(m)) = w.current_monitor().or_else(|_| w.primary_monitor()) {
        let (mp, ms) = (m.position(), m.size());
        let (left, top) = (mp.x as f64, mp.y as f64);
        let (right, bottom) = (left + ms.width as f64, top + ms.height as f64);
        x = x.clamp(left + 8.0, (right - size.0 - 8.0).max(left + 8.0));
        if y < top + 8.0 {
            y = (click.y + 14.0).min(bottom - size.1 - 8.0); // taskbar on top
        }
    }
    let _ = w.set_position(tauri::PhysicalPosition::new(x, y));
    let _ = w.show();
    let _ = w.set_focus();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .init();

    let mut builder = tauri::Builder::default();
    // One Bolt per user: a second copy would get its own identity and show up as a "nearby device".
    // Tests/dev that set BOLT_DATA_DIR run isolated instances on purpose and skip this.
    if std::env::var_os("BOLT_DATA_DIR").is_none() {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            // Right-click "Bolt > device" hands its files to this running instance.
            if let Some((device, paths)) = launch::parse_args(&args) {
                launch::add(app, device, paths);
                return;
            }
            // Opening Bolt again (Start menu, shortcut) brings the tray instance up.
            commands::show_main_window(app);
        }));
    }
    builder
        .plugin(tauri_plugin_dialog::init())
        // Autostart entries launch "bolt-desktop --background": tray only, window stays hidden.
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--background"]),
        ))
        .on_window_event(|window, event| {
            if window.label() == "tray" {
                // The panel is a popup: clicking anywhere else dismisses it.
                if let WindowEvent::Focused(false) = event {
                    PANEL_BLURRED_AT.store(now_ms(), Ordering::Relaxed);
                    let _ = window.hide();
                }
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
                return;
            }
            // Closing the window keeps Bolt receiving from the tray (unless turned off in Settings).
            if let WindowEvent::CloseRequested { api, .. } = event {
                let keep = window
                    .app_handle()
                    .try_state::<core::Core>()
                    .map(|c| {
                        c.settings
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .run_in_background
                    })
                    .unwrap_or(false);
                if keep {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            app.manage(commands::BackgroundStart(AtomicBool::new(
                std::env::args().any(|a| a == "--background"),
            )));
            let open = MenuItem::with_id(app, "open", "Open Bolt", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit Bolt", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;
            let mut tray = TrayIconBuilder::with_id("bolt")
                .tooltip("Bolt")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => commands::show_main_window(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    // Left click: the small dashboard. (Right click keeps the Open / Quit menu.)
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        position,
                        ..
                    } = event
                    {
                        toggle_tray_panel(tray.app_handle(), position);
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            // The dashboard is a second, small, hidden window that shows the same app (#tray).
            WebviewWindowBuilder::new(app, "tray", WebviewUrl::App("index.html#tray".into()))
                .title("Bolt")
                .inner_size(PANEL_SIZE.0, PANEL_SIZE.1)
                .resizable(false)
                .decorations(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .visible(false)
                .focused(false)
                .shadow(true)
                .build()?;
            let data_dir = match std::env::var("BOLT_DATA_DIR") {
                Ok(d) => std::path::PathBuf::from(d),
                Err(_) => app.path().app_data_dir()?,
            };
            let downloads = app
                .path()
                .download_dir()
                .unwrap_or_else(|_| data_dir.join("downloads"))
                .join("Bolt");
            // Endpoint binding and the receiver spawn tasks, so we need a runtime context.
            let core = tauri::async_runtime::block_on(async {
                core::Core::start(app.handle().clone(), data_dir, downloads)
            })?;
            app.manage(core);
            // Started from the right-click menu (cold start): open the review screen for those files.
            let argv: Vec<String> = std::env::args().collect();
            if let Some((device, paths)) = launch::parse_args(&argv) {
                launch::add(app.handle(), device, paths);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::list_devices,
            commands::refresh_devices,
            commands::list_trusted,
            commands::rename_trusted,
            commands::revoke_trusted,
            commands::scan_paths,
            commands::start_send,
            commands::list_transfers,
            commands::cancel_transfer,
            commands::retry_transfer,
            commands::dismiss_transfer,
            commands::respond_offer,
            commands::respond_pair,
            commands::begin_pairing,
            commands::end_pairing,
            commands::pair_with_uri,
            commands::history_list,
            commands::history_clear,
            commands::get_settings,
            commands::show_when_ready,
            commands::take_pending_draft,
            commands::open_main_window,
            commands::draft_in_main,
            commands::quit_app,
            commands::hide_tray_panel,
            commands::update_settings,
            commands::open_download_dir,
            commands::stage_clipboard,
            commands::reveal_transfer,
            commands::open_received,
            commands::received_preview,
            commands::drag_received,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Bolt");
}
