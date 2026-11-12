// Hides the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use pairspan_core::store::{self, Settings};
use pairspan_core::{adb, automation, rpc, Agent, Event, Options, Status};
use serde_json::{json, Value};
use std::path::PathBuf;
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
fn logs(agent: State<'_, AgentState>) -> Vec<String> {
    agent.logs()
}

#[tauri::command]
async fn start_pairing(agent: State<'_, AgentState>) -> CmdResult<Value> {
    agent.start_pairing().await.map(|p| json!(p)).map_err(err)
}

#[tauri::command]
async fn cancel_pairing(agent: State<'_, AgentState>) -> CmdResult<()> {
    agent.cancel_pairing().await;
    Ok(())
}

#[tauri::command]
async fn unpair(agent: State<'_, AgentState>, id: String) -> CmdResult<()> {
    let id = pairspan_core::identity::PeerId::parse(&id).ok_or("invalid id")?;
    agent.remove_device(id, true).await.map(|_| ()).map_err(err)
}

#[tauri::command]
fn settings(agent: State<'_, AgentState>) -> Settings {
    agent.settings()
}

#[tauri::command]
fn save_settings(agent: State<'_, AgentState>, settings: Settings) -> CmdResult<()> {
    let url = settings.gateway_url.trim();
    if !(url.starts_with("wss://") || url.starts_with("ws://")) {
        return Err("Gateway URL must start with wss:// (or ws:// for a local gateway).".into());
    }
    agent.update_settings(Settings { gateway_url: url.to_string(), ..settings }).map_err(err)
}

#[tauri::command]
fn default_gateway() -> &'static str {
    store::DEFAULT_GATEWAY
}

/// TCP connect time to the gateway host, in milliseconds (a cheap latency figure for the status card).
#[tauri::command]
async fn gateway_ping(agent: State<'_, AgentState>) -> CmdResult<u64> {
    let url = agent.settings().gateway_url;
    let (tls, rest) = match url.split_once("://") {
        Some((scheme, rest)) => (scheme == "wss", rest),
        None => return Err("bad gateway url".into()),
    };
    let host = rest.split('/').next().unwrap_or_default();
    let addr = if host.rsplit_once(':').is_some_and(|(_, p)| p.parse::<u16>().is_ok()) {
        host.to_string()
    } else {
        format!("{host}:{}", if tls { 443 } else { 80 })
    };
    let start = std::time::Instant::now();
    tokio::time::timeout(std::time::Duration::from_secs(4), tokio::net::TcpStream::connect(addr))
        .await
        .map_err(|_| "timeout".to_string())?
        .map_err(err)?;
    Ok(start.elapsed().as_millis() as u64)
}

#[tauri::command]
async fn usb_devices() -> CmdResult<Vec<adb::UsbDevice>> {
    adb::usb_devices().await.map_err(err)
}

#[tauri::command]
async fn usb_prepare(serial: String) -> CmdResult<String> {
    automation::usb_prepare(&serial).await.map_err(err)
}

/// Downloads the latest Pairspan APK from GitHub Releases and installs it on a USB phone.
#[tauri::command]
async fn usb_install_app(agent: State<'_, AgentState>, serial: String) -> CmdResult<String> {
    let cache = store::default_dir().join("cache");
    std::fs::create_dir_all(&cache).map_err(err)?;
    let apk = cache.join("pairspan.apk");
    agent.log(format!("Downloading the Pairspan Android app for {serial}…"));
    let api = tokio::process::Command::new("curl")
        .args(["-fsSL", "-H", "Accept: application/vnd.github+json", "https://api.github.com/repos/ferhatozcelik/pairspan/releases/latest"])
        .output()
        .await
        .map_err(err)?;
    let release: Value = serde_json::from_slice(&api.stdout).map_err(|_| "Could not read the latest release from GitHub.".to_string())?;
    let url = release["assets"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|a| a["name"].as_str().is_some_and(|n| n.ends_with(".apk")))
        .and_then(|a| a["browser_download_url"].as_str())
        .ok_or("The latest release has no APK.")?
        .to_string();
    let ok = tokio::process::Command::new("curl")
        .args(["-fsSL", "-o"])
        .arg(&apk)
        .arg(&url)
        .status()
        .await
        .map_err(err)?
        .success();
    if !ok {
        return Err("Download failed. Check your internet connection.".into());
    }
    let out = automation::install(&serial, &apk.to_string_lossy()).await.map_err(err)?;
    agent.log(format!("Installed the Pairspan app on {serial}"));
    Ok(out)
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

/// Where the bundled CLI lives and whether it is on PATH, for the Automation panel.
#[tauri::command]
fn cli_info() -> Value {
    let bundled: Option<PathBuf> = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("pairspan"))).filter(|p| p.is_file());
    let on_path = std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|d| d.join("pairspan").is_file()))
        .unwrap_or(false)
        || ["/usr/local/bin/pairspan", "/opt/homebrew/bin/pairspan"].iter().any(|p| std::path::Path::new(p).exists());
    json!({
        "bundled": bundled.map(|p| p.display().to_string()),
        "on_path": on_path,
        "socket": store::socket_path(&store::default_dir()).display().to_string(),
    })
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
    match (s.connected, s.devices.len()) {
        (false, _) => "Pairspan — offline".into(),
        (true, 0) => "Pairspan — no phones paired".into(),
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
            let pair = MenuItem::with_id(app, "pair", "Pair a phone…", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit Pairspan", true, Some("CmdOrCtrl+Q"))?;
            let menu = Menu::with_items(app, &[&open, &pair, &PredefinedMenuItem::separator(app)?, &quit])?;
            let tray_icon = Image::from_bytes(include_bytes!("../icons/tray@2x.png"))?;
            let tray = TrayIconBuilder::with_id("main")
                .icon(tray_icon)
                .icon_as_template(true)
                .tooltip("Pairspan")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, e| match e.id.as_ref() {
                    "open" => show_main(app),
                    "pair" => {
                        show_main(app);
                        let _ = app.emit("pairspan://pair", ());
                    }
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
            logs,
            default_gateway,
            gateway_ping,
            start_pairing,
            cancel_pairing,
            unpair,
            settings,
            save_settings,
            usb_devices,
            usb_prepare,
            usb_install_app,
            cli_info,
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
