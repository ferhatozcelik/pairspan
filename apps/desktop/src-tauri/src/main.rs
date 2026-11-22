// Hides the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use pairspan_core::store;
use pairspan_core::{adb, automation, rpc, Agent, Event, Options, Status};
use serde_json::{json, Value};
use std::sync::Arc;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};

type AgentState = Arc<Agent>;
type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[tauri::command]
fn status(agent: State<'_, AgentState>) -> Status {
    agent.status()
}

#[tauri::command]
async fn unpair(agent: State<'_, AgentState>, id: String) -> CmdResult<()> {
    let id = pairspan_core::identity::PeerId::parse(&id).ok_or("invalid id")?;
    agent.remove_device(id, true).await.map(|_| ()).map_err(err)
}

/// The on/off switch.
#[tauri::command]
fn set_enabled(agent: State<'_, AgentState>, enabled: bool) -> CmdResult<()> {
    agent.set_enabled(enabled).map_err(err)
}

#[tauri::command]
fn set_user(
    agent: State<'_, AgentState>,
    user_id: Option<String>,
    auth_token: Option<String>,
    user_email: Option<String>,
) -> CmdResult<()> {
    agent.set_user(user_id, auth_token, user_email).map_err(err)
}

/// Latest GitHub release vs this build: `{ current, latest, newer, url }`.
#[tauri::command]
async fn check_update() -> CmdResult<Value> {
    let out = tokio::process::Command::new("curl")
        .args(["-fsSL", "--max-time", "15", "-H", "Accept: application/vnd.github+json", "https://api.github.com/repos/ferhatozcelik/pairspan/releases/latest"])
        .output()
        .await
        .map_err(err)?;
    let release: Value = serde_json::from_slice(&out.stdout).map_err(|_| "Could not reach GitHub.".to_string())?;
    let latest = release["tag_name"].as_str().unwrap_or_default().trim_start_matches('v').to_string();
    let current = env!("CARGO_PKG_VERSION");
    let parse = |v: &str| v.split('.').map(|n| n.parse::<u64>().unwrap_or(0)).collect::<Vec<_>>();
    Ok(json!({ "current": current, "latest": latest, "newer": parse(&latest) > parse(current), "url": release["html_url"] }))
}

// ---- Direct control over ADB (works for paired phones through the tunnel and for USB / Wi-Fi adb) ----

/// Every phone adb can reach right now, with paired phones named after their Pairspan name.
#[tauri::command]
async fn control_targets(agent: State<'_, AgentState>) -> CmdResult<Vec<Value>> {
    let paired = agent.status().devices;
    let attached = adb::all_devices().await.unwrap_or_default();
    let mut out = Vec::new();
    for d in &paired {
        let ready = attached.iter().any(|a| a.serial == d.serial && a.state == "device");
        out.push(json!({ "serial": d.serial, "name": d.device.name, "kind": "paired", "ready": d.online && ready,
            "detail": if d.online { "Pairspan tunnel" } else { "offline" } }));
    }
    for a in attached.iter().filter(|a| !paired.iter().any(|p| p.serial == a.serial)) {
        let kind = if a.serial.contains(':') || a.serial.contains("._tcp") { "wifi" } else { "usb" };
        out.push(json!({ "serial": a.serial, "name": a.model, "kind": kind, "ready": a.state == "device",
            "detail": if a.state == "device" { if kind == "usb" { "USB" } else { "Wi-Fi adb" } } else { a.state.as_str() } }));
    }
    Ok(out)
}

/// Raw PNG bytes (an ArrayBuffer in JS), so frames skip JSON/base64.
#[tauri::command]
async fn control_screen(serial: String) -> CmdResult<tauri::ipc::Response> {
    automation::screenshot(&serial).await.map(tauri::ipc::Response::new).map_err(err)
}

#[tauri::command]
async fn control_tap(serial: String, x: i32, y: i32) -> CmdResult<()> {
    automation::tap(&serial, x, y).await.map_err(err)
}

#[tauri::command]
async fn control_swipe(serial: String, x1: i32, y1: i32, x2: i32, y2: i32, ms: u32) -> CmdResult<()> {
    automation::swipe(&serial, (x1, y1), (x2, y2), ms.clamp(50, 3000)).await.map_err(err)
}

#[tauri::command]
async fn control_key(serial: String, key: String) -> CmdResult<()> {
    automation::key(&serial, &key).await.map_err(err)
}

#[tauri::command]
async fn control_text(serial: String, text: String) -> CmdResult<()> {
    automation::text(&serial, &text).await.map_err(err)
}

#[tauri::command]
async fn control_shell(serial: String, command: String) -> CmdResult<automation::ShellResult> {
    automation::shell(&serial, &command).await.map_err(err)
}

#[tauri::command]
async fn control_info(serial: String) -> CmdResult<Value> {
    let current = automation::current(&serial).await.ok();
    let size = automation::screen_size(&serial).await.ok();
    Ok(json!({ "current": current, "size": size }))
}

/// Saves a screenshot to ~/Desktop and returns the path.
#[tauri::command]
async fn control_save_screenshot(serial: String) -> CmdResult<String> {
    let png = automation::screenshot(&serial).await.map_err(err)?;
    let dir = dirs::desktop_dir().or_else(dirs::home_dir).ok_or("no Desktop folder")?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let path = dir.join(format!("Pairspan-{stamp}.png"));
    std::fs::write(&path, png).map_err(err)?;
    Ok(path.display().to_string())
}

#[tauri::command]
fn open_url(url: String) -> CmdResult<()> {
    if !url.starts_with("https://") {
        return Err("only https links".into());
    }
    let opener = if cfg!(target_os = "macos") { "open" } else if cfg!(windows) { "explorer" } else { "xdg-open" };
    std::process::Command::new(opener).arg(url).spawn().map(|_| ()).map_err(err)
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn tray_tooltip(s: &Status) -> String {
    let online = s.devices.iter().filter(|d| d.online).count();
    match (s.enabled, s.devices.len()) {
        (false, _) => "Pairspan — off".into(),
        (true, 0) => "Pairspan — plug a phone in over USB".into(),
        (true, n) => format!("Pairspan — {online} of {n} phones online"),
    }
}

fn main() {
    let background = std::env::args().any(|a| a == "--background");
    let dir = store::default_dir();

    let agent = tauri::async_runtime::block_on(async { Agent::start(&dir, Options::default()) }).unwrap_or_else(|e| {
        eprintln!("Pairspan could not start: {e:#}");
        std::process::exit(1);
    });

    let socket = store::socket_path(&dir);
    {
        let agent = agent.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = rpc::serve(agent.clone(), &socket).await {
                agent.log(format!("Automation socket unavailable: {e:#}"));
            }
        });
    }

    tauri::Builder::default()
        .manage(agent.clone())
        .setup(move |app| {
            let open = MenuItem::with_id(app, "open", "Open Pairspan", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit Pairspan", true, Some("CmdOrCtrl+Q"))?;
            let menu = Menu::with_items(app, &[&open, &PredefinedMenuItem::separator(app)?, &quit])?;
            let tray_icon = Image::from_bytes(include_bytes!("../icons/tray@2x.png"))?;
            let tray = TrayIconBuilder::with_id("main")
                .icon(tray_icon)
                .icon_as_template(true)
                .tooltip("Pairspan")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, e| match e.id.as_ref() {
                    "open" => show_main(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, e| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                        show_main(tray.app_handle());
                    }
                })
                .build(app)?;

            // Push agent events to the UI and keep the tray tooltip current.
            let handle = app.handle().clone();
            let agent = agent.clone();
            tauri::async_runtime::spawn(async move {
                let mut events = agent.subscribe();
                let _ = tray.set_tooltip(Some(tray_tooltip(&agent.status())));
                loop {
                    match events.recv().await {
                        Ok(ev) => {
                            if matches!(ev, Event::Changed | Event::Paired { .. }) {
                                let _ = tray.set_tooltip(Some(tray_tooltip(&agent.status())));
                            }
                            let _ = handle.emit("pairspan://event", &ev);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(_) => break,
                    }
                }
            });

            if !background {
                show_main(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window keeps Pairspan running in the menu bar; phones stay connected.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            status,
            unpair,
            set_enabled,
            set_user,
            check_update,
            open_url,
            control_targets,
            control_screen,
            control_tap,
            control_swipe,
            control_key,
            control_text,
            control_shell,
            control_info,
            control_save_screenshot
        ])
        .build(tauri::generate_context!())
        .expect("failed to build Pairspan")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                show_main(app);
            }
            let _ = (app, event);
        });
}
