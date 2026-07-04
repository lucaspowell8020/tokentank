// Claude Gauge — a gas gauge for Claude Code, in your tray.
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

fn tooltip(snap: &state::Snapshot) -> String {
    format!(
        "Claude Gauge — 5h: {:.0}% left · week: {:.0}% left · ${:.2}/h",
        (1.0 - snap.five_h_cost / snap.five_h_ceiling.max(0.01)).clamp(0.0, 1.0) * 100.0,
        (1.0 - snap.weekly_cost / snap.weekly_ceiling.max(0.01)).clamp(0.0, 1.0) * 100.0,
        snap.burn_per_hour
    )
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
        .manage(AppState(Mutex::new(gauge)))
        .invoke_handler(tauri::generate_handler![get_state])
        .setup(|app| {
            // Tray with menu
            let open = MenuItem::with_id(app, "open", "Open Claude Gauge", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;

            let (rgba, w, h) = icon::render(1.0);
            TrayIconBuilder::with_id("gauge")
                .icon(Image::new_owned(rgba, w, h))
                .tooltip("Claude Gauge — reading transcripts…")
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
                        "[gauge] 5h ${:.2}/{:.0} · week ${:.2}/{:.0} · burn ${:.2}/h · remaining {:.0}%",
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
        .expect("error while building Claude Gauge")
        .run(|_app, event| {
            // Keep running when the window is hidden; only exit on explicit quit.
            if let tauri::RunEvent::ExitRequested { code, api, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}
