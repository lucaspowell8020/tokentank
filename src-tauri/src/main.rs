// TokenTank — a gas gauge for Claude Code, in your tray.
// Reads local transcripts only. No network calls, no telemetry.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod icon;
mod parser;
mod state;

use std::sync::Mutex;
use std::time::Duration;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, State,
};

struct AppState(Mutex<state::Gauge>);

fn now_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[tauri::command]
fn get_state(app_state: State<AppState>) -> state::Snapshot {
    app_state.0.lock().unwrap().snapshot(now_epoch())
}

#[tauri::command]
fn needs_setup(app_state: State<AppState>) -> bool {
    app_state.0.lock().unwrap().needs_setup()
}

#[tauri::command]
fn get_autostart(app: tauri::AppHandle) -> bool {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
fn set_autostart(app: tauri::AppHandle, enabled: bool) -> bool {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    let _ = if enabled { manager.enable() } else { manager.disable() };
    let mut settings = config::load_settings();
    settings.autostart = Some(enabled);
    config::save_settings(&settings);
    manager.is_enabled().unwrap_or(false)
}

#[tauri::command]
fn save_setup(
    app: tauri::AppHandle,
    app_state: State<AppState>,
    plan: String,
    weekly_reset: Option<String>,
    session_pct: Option<f64>,
    week_pct: Option<f64>,
) -> state::Snapshot {
    let snap = {
        let mut gauge = app_state.0.lock().unwrap();
        let now = now_epoch();
        gauge.apply_setup(now, &plan, weekly_reset.as_deref(), session_pct, week_pct);
        gauge.snapshot(now)
    };
    push_update(&app, &snap);
    snap
}

fn fmt_dur(secs: i64) -> String {
    let secs = secs.max(0);
    let (d, h, m) = (secs / 86400, (secs % 86400) / 3600, (secs % 3600) / 60);
    if d > 0 {
        format!("{d}d {h}h")
    } else {
        format!("{h}h {m:02}m")
    }
}

fn tooltip(snap: &state::Snapshot) -> String {
    let now = now_epoch();
    let session = match snap.five_h_reset {
        Some(reset) => format!(
            "session: {:.0}% left, resets in {}",
            (1.0 - snap.five_h_cost / snap.five_h_ceiling.max(0.01)).clamp(0.0, 1.0) * 100.0,
            fmt_dur(reset - now)
        ),
        None => "session: full tank".to_string(),
    };
    let week_pct =
        (1.0 - snap.weekly_cost / snap.weekly_ceiling.max(0.01)).clamp(0.0, 1.0) * 100.0;
    let week = match snap.weekly_reset {
        Some(reset) => format!("week: {:.0}% left, resets in {}", week_pct, fmt_dur(reset - now)),
        None => format!("week: {week_pct:.0}% left"),
    };
    format!("TokenTank — {session} · {week} · ${:.2}/h", snap.burn_per_hour)
}

fn push_update(app: &tauri::AppHandle, snap: &state::Snapshot) {
    if let Some(tray) = app.tray_by_id("gauge") {
        let (rgba, w, h) = icon::render(snap.remaining);
        let _ = tray.set_icon(Some(Image::new_owned(rgba, w, h)));
        let _ = tray.set_tooltip(Some(tooltip(snap)));
    }
    let _ = app.emit("gauge://state", snap);
}

fn main() {
    let cfg = config::load();
    let base = dirs::home_dir()
        .map(|h| h.join(".claude").join("projects"))
        .expect("no home directory");
    let gauge = state::Gauge::new(cfg, base);

    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(AppState(Mutex::new(gauge)))
        .invoke_handler(tauri::generate_handler![
            get_state,
            needs_setup,
            save_setup,
            get_autostart,
            set_autostart
        ])
        .setup(|app| {
            // A tray gauge only works if it's running: default autostart ON
            // the first time this build runs, but respect any choice the
            // user has made via the popover toggle.
            {
                use tauri_plugin_autostart::ManagerExt;
                let mut settings = config::load_settings();
                if settings.autostart.is_none() {
                    let _ = app.autolaunch().enable();
                    settings.autostart = Some(true);
                    config::save_settings(&settings);
                }
            }

            // On macOS, this is a menu-bar app: no Dock icon, no app-switcher
            // entry. The tray gauge and popover are the whole UI.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Tray with menu
            let open = MenuItem::with_id(app, "open", "Open TokenTank", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;

            let (rgba, w, h) = icon::render(1.0);
            TrayIconBuilder::with_id("gauge")
                .icon(Image::new_owned(rgba, w, h))
                .tooltip("TokenTank — reading transcripts…")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => {
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(win) = app.get_webview_window("main") {
                            if win.is_visible().unwrap_or(false) {
                                let _ = win.hide();
                            } else {
                                let _ = win.show();
                                let _ = win.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            // Hide instead of close, so the tray app keeps running.
            if let Some(win) = app.get_webview_window("main") {
                let win_handle = win.clone();
                win.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = win_handle.hide();
                    }
                });
            }

            // Background refresh loop: initial scan, then poll every 15s.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                loop {
                    let snap = {
                        let state: State<AppState> = handle.state();
                        let mut gauge = state.0.lock().unwrap();
                        let now = now_epoch();
                        gauge.refresh(now);
                        gauge.snapshot(now)
                    };
                    println!(
                        "[gauge] plan {}{} · 5h ${:.2}/{:.0} · week ${:.2}/{:.0} · burn ${:.2}/h · remaining {:.0}%",
                        snap.plan.as_deref().unwrap_or("(unset)"),
                        if snap.plan_detected { " (detected)" } else { "" },
                        snap.five_h_cost,
                        snap.five_h_ceiling,
                        snap.weekly_cost,
                        snap.weekly_ceiling,
                        snap.burn_per_hour,
                        snap.remaining * 100.0
                    );
                    push_update(&handle, &snap);
                    std::thread::sleep(Duration::from_secs(15));
                }
            });

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building TokenTank")
        .run(|_app, event| {
            // Keep running when the window is hidden; only exit on explicit quit.
            if let tauri::RunEvent::ExitRequested { code, api, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}
