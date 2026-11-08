use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State,
};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message as WsMessage};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PairingStatus {
    Connected,
    Reconnecting,
    RevokedByPc,
    RevokedByPhone,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairedDevice {
    pub hardware_serial: String,
    pub model: String,
    pub android_version: String,
    pub pair_token: String,
    pub canonical_endpoint: String, // e.g., "127.0.0.1:44755"
    pub last_known_ip: Option<String>,
    pub status: PairingStatus,
    pub auto_reconnect: bool,
    pub paired_at: String,
    pub last_seen: String,
    pub purged_duplicates: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsbCandidate {
    pub serial: String,
    pub state: String, // "device", "unauthorized", "offline"
    pub model: String,
    pub transport_id: String,
    pub apk_installed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateGhost {
    pub endpoint: String,
    pub state: String,
    pub reason: String, // e.g. "Stale mDNS TLS duplicate", "Conflicting legacy Wi-Fi IP:port"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStateSnapshot {
    pub gateway_url: String,
    pub local_bridge_port: u16,
    pub paired_devices: Vec<PairedDevice>,
    pub usb_candidates: Vec<UsbCandidate>,
    pub detected_ghosts: Vec<DuplicateGhost>,
    pub logs: Vec<String>,
}

pub struct PairspanCore {
    pub gateway_url: String,
    pub local_bridge_port: u16,
    pub paired_devices: HashMap<String, PairedDevice>,
    pub logs: Vec<String>,
}

impl PairspanCore {
    fn config_path() -> PathBuf {
        let mut dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        dir.push("pairspan");
        let _ = fs::create_dir_all(&dir);
        dir.push("paired_registry.json");
        dir
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if let Ok(raw) = fs::read_to_string(&path) {
            if let Ok(devices) = serde_json::from_str::<HashMap<String, PairedDevice>>(&raw) {
                return Self {
                    gateway_url: "ws://127.0.0.1:44756/ws".to_string(),
                    local_bridge_port: 44755,
                    paired_devices: devices,
                    logs: vec!["[Lifecycle] Loaded persistent paired device registry.".to_string()],
                };
            }
        }
        Self {
            gateway_url: "ws://127.0.0.1:44756/ws".to_string(),
            local_bridge_port: 44755,
            paired_devices: HashMap::new(),
            logs: vec!["[Lifecycle] Pairspan Rust Desktop Core initialized (macOS / Windows / Linux).".to_string()],
        }
    }

    pub fn save(&self) {
        let path = Self::config_path();
        if let Ok(json) = serde_json::to_string_pretty(&self.paired_devices) {
            let _ = fs::write(path, json);
        }
    }

    pub fn log(&mut self, msg: impl Into<String>) {
        let entry = format!("[{}] {}", chrono::Local::now().format("%H:%M:%S"), msg.into());
        self.logs.push(entry);
        if self.logs.len() > 150 {
            self.logs.remove(0);
        }
    }
}

type SharedCore = Arc<Mutex<PairspanCore>>;

fn run_adb(args: &[&str]) -> Result<String, String> {
    let adb_bins = [
        "adb",
        "/Users/ferhatozcelik/Library/Android/sdk/platform-tools/adb",
        "/opt/homebrew/bin/adb",
        "/usr/local/bin/adb",
    ];
    for bin in adb_bins {
        if let Ok(out) = Command::new(bin).args(args).output() {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            return Ok(format!("{}{}", stdout, stderr));
        }
    }
    Err("ADB binary not found in PATH or standard SDK locations.".to_string())
}

/// Parses `adb devices -l` into USB candidates and detects conflicting Wi-Fi ADB / mDNS ghost entries
fn inspect_adb_environment(canonical_port: u16) -> (Vec<UsbCandidate>, Vec<DuplicateGhost>) {
    let mut usb = Vec::new();
    let mut ghosts = Vec::new();
    let canonical = format!("127.0.0.1:{}", canonical_port);

    if let Ok(out) = run_adb(&["devices", "-l"]) {
        for line in out.lines().skip(1) {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('*') {
                continue;
            }
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }
            let id = parts[0];
            let state = parts[1];
            let mut model = "Android Device".to_string();
            let mut transport_id = "1".to_string();
            for p in &parts[2..] {
                if let Some(m) = p.strip_prefix("model:") {
                    model = m.replace('_', " ");
                }
                if let Some(t) = p.strip_prefix("transport_id:") {
                    transport_id = t.to_string();
                }
            }

            // Detect mDNS / _adb-tls-connect._tcp or duplicate IP:port entries that cause multiple device IDs
            if id.contains("_adb-tls-connect") || id.contains("._tcp") {
                ghosts.push(DuplicateGhost {
                    endpoint: id.to_string(),
                    state: state.to_string(),
                    reason: "mDNS TLS Wireless Debugging duplicate identity".to_string(),
                });
            } else if id.contains(':') && id != canonical {
                ghosts.push(DuplicateGhost {
                    endpoint: id.to_string(),
                    state: state.to_string(),
                    reason: "Prior manual Wi-Fi ADB IP:port session (causes duplicate device conflict)".to_string(),
                });
            } else if !id.contains(':') {
                let apk_installed = if state == "device" {
                    is_pairspan_apk_installed(id)
                } else {
                    false
                };
                usb.push(UsbCandidate {
                    serial: id.to_string(),
                    state: state.to_string(),
                    model,
                    transport_id,
                    apk_installed,
                });
            }
        }
    }
    (usb, ghosts)
}

const ANDROID_PKG: &str = "com.ferhatozcelik.pairspan";
const GITHUB_RELEASES_API: &str = "https://api.github.com/repos/ferhatozcelik/pairspan/releases/latest";
const FALLBACK_APK_URL: &str = "https://github.com/ferhatozcelik/pairspan/releases/download/v1.0.0/pairspan-android-1.0.0.apk";

/// Checks whether the Pairspan Android APK is installed on the connected USB device
fn is_pairspan_apk_installed(serial: &str) -> bool {
    if let Ok(out) = run_adb(&["-s", serial, "shell", "pm", "path", ANDROID_PKG]) {
        out.trim().starts_with("package:")
    } else {
        false
    }
}

/// Queries GitHub Releases (`ferhatozcelik/pairspan/releases/latest`) for the newest `.apk` asset URL,
/// downloads it to the local Pairspan cache directory, and installs it onto the phone via `adb -s <serial> install -r`
/// BEFORE granting permissions.
fn ensure_apk_installed_from_github(serial: &str, core: &mut PairspanCore) -> Result<bool, String> {
    if is_pairspan_apk_installed(serial) {
        core.log(format!(
            "[USB Setup] Pairspan Android APK ({}) is already installed on {}. Skipping download.",
            ANDROID_PKG, serial
        ));
        return Ok(false);
    }

    core.log(format!(
        "[USB Setup] Pairspan APK not found on {}. Fetching latest APK from GitHub Releases...",
        serial
    ));

    // 1. Resolve latest .apk download URL from GitHub Releases API
    let mut apk_url = FALLBACK_APK_URL.to_string();
    if let Ok(out) = Command::new("curl")
        .args([
            "-sL",
            "-H",
            "Accept: application/vnd.github+json",
            "-H",
            "User-Agent: Pairspan-Desktop-Installer",
            GITHUB_RELEASES_API,
        ])
        .output()
    {
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
            if let Some(assets) = json.get("assets").and_then(|a| a.as_array()) {
                for asset in assets {
                    let name = asset.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    if name.to_lowercase().ends_with(".apk") {
                        if let Some(dl) = asset.get("browser_download_url").and_then(|u| u.as_str()) {
                            apk_url = dl.to_string();
                            break;
                        }
                    }
                }
            }
        }
    }

    let mut cache_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    cache_dir.push("pairspan");
    let _ = fs::create_dir_all(&cache_dir);
    let apk_path = cache_dir.join("pairspan-latest.apk");
    let apk_path_str = apk_path.to_string_lossy().to_string();

    core.log(format!("[USB Setup] Downloading APK from {}...", apk_url));
    let status = Command::new("curl")
        .args(["-fL", "-o", &apk_path_str, &apk_url])
        .status()
        .map_err(|e| format!("Failed to invoke curl for APK download: {}", e))?;

    if !status.success() || !apk_path.exists() {
        return Err(format!(
            "Could not download APK from GitHub release ({}). Please check your internet connection.",
            apk_url
        ));
    }

    core.log(format!(
        "[USB Setup] Installing APK onto {} via 'adb install -r -g'...",
        serial
    ));
    let install_out = run_adb(&["-s", serial, "install", "-r", "-g", &apk_path_str])?;
    if !install_out.contains("Success") && !is_pairspan_apk_installed(serial) {
        return Err(format!("ADB APK installation failed: {}", install_out.trim()));
    }

    core.log(format!(
        "[USB Setup] Successfully installed Pairspan APK from GitHub Releases onto {}.",
        serial
    ));
    Ok(true)
}

/// Purges all duplicate Wi-Fi ADB / mDNS endpoints so only the canonical Pairspan identity remains
fn deduplicate_wifi_adb(core: &mut PairspanCore) -> usize {
    let (_, ghosts) = inspect_adb_environment(core.local_bridge_port);
    let mut purged = 0;
    for g in ghosts {
        let _ = run_adb(&["disconnect", &g.endpoint]);
        core.log(format!(
            "[Deduplicator] Disconnected conflicting Wi-Fi ADB endpoint '{}' ({})",
            g.endpoint, g.reason
        ));
        purged += 1;
    }
    purged
}

#[tauri::command]
fn get_snapshot(state: State<'_, SharedCore>) -> AppStateSnapshot {
    let core = state.lock().unwrap();
    let (usb_candidates, detected_ghosts) = inspect_adb_environment(core.local_bridge_port);
    let mut paired_devices: Vec<PairedDevice> = core.paired_devices.values().cloned().collect();
    paired_devices.sort_by(|a, b| a.hardware_serial.cmp(&b.hardware_serial));

    AppStateSnapshot {
        gateway_url: core.gateway_url.clone(),
        local_bridge_port: core.local_bridge_port,
        paired_devices,
        usb_candidates,
        detected_ghosts,
        logs: core.logs.clone(),
    }
}

/// Step-by-step USB Permission & Initial Setup Wizard command:
/// 1. Verifies USB authorization (RSA key trust)
/// 2. Queries true hardware serial (`ro.serialno`), model, Android release, and Wi-Fi IP
/// 3. Grants background permissions & switches adbd to persistent `tcpip 5555`
/// 4. Purges any prior conflicting Wi-Fi ADB or mDNS duplicate entries
/// 5. Registers the device for continuous auto-reconnect until explicitly removed on PC or Phone
#[tauri::command]
fn usb_authorize_and_pair(serial: String, state: State<'_, SharedCore>) -> Result<PairedDevice, String> {
    let mut core = state.lock().unwrap();
    core.log(format!("[USB Setup] Starting USB Permission & Setup flow for {}...", serial));

    // Check device state
    let state_out = run_adb(&["-s", &serial, "get-state"]).unwrap_or_default();
    if state_out.contains("unauthorized") {
        core.log("[USB Setup] Phone is 'unauthorized'. Waiting for user to tap 'Allow USB Debugging' on phone...".to_string());
        return Err("Please tap 'Allow USB Debugging (Always allow from this computer)' on your Android phone screen.".to_string());
    }

    // Read canonical hardware serial so we never create duplicate identities
    let hw_serial = run_adb(&["-s", &serial, "shell", "getprop", "ro.serialno"])
        .unwrap_or_else(|_| serial.clone())
        .trim()
        .to_string();
    let hw_serial = if hw_serial.is_empty() || hw_serial.contains("error") {
        serial.clone()
    } else {
        hw_serial
    };

    let model = run_adb(&["-s", &serial, "shell", "getprop", "ro.product.model"])
        .unwrap_or_else(|_| "Android Phone".to_string())
        .trim()
        .to_string();
    let android_version = run_adb(&["-s", &serial, "shell", "getprop", "ro.build.version.release"])
        .unwrap_or_else(|_| "14".to_string())
        .trim()
        .to_string();

    // Detect phone's current Wi-Fi IP for direct local + gateway fallback
    let ip_out = run_adb(&["-s", &serial, "shell", "ip", "-f", "inet", "addr", "show", "wlan0"]).unwrap_or_default();
    let mut wlan_ip: Option<String> = None;
    for line in ip_out.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("inet ") {
            if let Some(ip) = rest.split('/').next() {
                wlan_ip = Some(ip.to_string());
            }
        }
    }

    // Step 1 (Before Permissions): Check if Pairspan APK is installed on the phone;
    // if not installed, automatically fetch the latest APK from GitHub Releases and install via ADB.
    match ensure_apk_installed_from_github(&serial, &mut core) {
        Ok(_) => {}
        Err(err) => {
            core.log(format!("[USB Setup] APK auto-install warning: {}", err));
        }
    }

    // Step 2: Grant Pairspan Android app permissions & enable persistent tcpip 5555
    let _ = run_adb(&[
        "-s",
        &serial,
        "shell",
        "pm",
        "grant",
        ANDROID_PKG,
        "android.permission.DUMP",
    ]);
    let _ = run_adb(&[
        "-s",
        &serial,
        "shell",
        "pm",
        "grant",
        ANDROID_PKG,
        "android.permission.WRITE_SECURE_SETTINGS",
    ]);
    // Also whitelist from battery optimizations so the persistent auto-reconnect service is never killed
    let _ = run_adb(&[
        "-s",
        &serial,
        "shell",
        "dumpsys",
        "deviceidle",
        "whitelist",
        &format!("+{}", ANDROID_PKG),
    ]);
    let _ = run_adb(&["-s", &serial, "tcpip", "5555"]);
    core.log(format!(
        "[USB Setup] Enabled persistent ADB TCP/IP (5555) and verified RSA key trust for {} ({}).",
        model, hw_serial
    ));

    // Purge any conflicting Wi-Fi ADB or mDNS entries before binding
    let purged = deduplicate_wifi_adb(&mut core);

    let canonical_endpoint = format!("127.0.0.1:{}", core.local_bridge_port);
    let pair_token = format!("ps_{}", Uuid::new_v4().simple());
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let device = PairedDevice {
        hardware_serial: hw_serial.clone(),
        model,
        android_version,
        pair_token,
        canonical_endpoint,
        last_known_ip: wlan_ip,
        status: PairingStatus::Connected,
        auto_reconnect: true,
        paired_at: now.clone(),
        last_seen: now,
        purged_duplicates: purged,
    };

    core.paired_devices.insert(hw_serial, device.clone());
    core.save();
    core.log(format!(
        "[Lifecycle] Device '{}' paired permanently. Auto-reconnect loop locked to single identity {}.",
        device.model, device.canonical_endpoint
    ));

    Ok(device)
}

#[tauri::command]
fn purge_wifi_adb_conflicts(state: State<'_, SharedCore>) -> usize {
    let mut core = state.lock().unwrap();
    let count = deduplicate_wifi_adb(&mut core);
    if count == 0 {
        core.log("[Deduplicator] Clean check complete: 0 conflicting Wi-Fi ADB or mDNS duplicates found.".to_string());
    }
    count
}

/// Mutual Revocation Lifecycle:
/// When the user removes the phone on the PC (or when a revoke signal arrives from the Phone),
/// auto-reconnect is permanently terminated, the token is invalidated, and the endpoint is disconnected.
#[tauri::command]
fn unpair_device(hardware_serial: String, initiator: String, state: State<'_, SharedCore>) -> bool {
    let mut core = state.lock().unwrap();
    let gw_url = core.gateway_url.clone();
    if let Some(dev) = core.paired_devices.remove(&hardware_serial) {
        let _ = run_adb(&["disconnect", &dev.canonical_endpoint]);
        if let Some(ip) = &dev.last_known_ip {
            let _ = run_adb(&["disconnect", &format!("{}:5555", ip)]);
        }
        let token = dev.pair_token.clone();
        let hw = hardware_serial.clone();
        tauri::async_runtime::spawn(async move {
            if let Ok((mut ws, _)) = connect_async(&gw_url).await {
                let payload = serde_json::json!({
                    "type": "PAIR_REVOKE",
                    "pc_id": "pairspan-desktop",
                    "hardware_serial": hw,
                    "pair_token": token,
                    "initiator": "pc"
                });
                let _ = ws.send(WsMessage::Text(payload.to_string())).await;
            }
        });
        core.save();
        core.log(format!(
            "[Lifecycle Revoked] Pairing for '{}' ({}) permanently removed by {}. Auto-reconnect stopped.",
            dev.model, hardware_serial, initiator
        ));
        return true;
    }
    false
}

#[tauri::command]
fn trigger_reconnect_now(hardware_serial: String, state: State<'_, SharedCore>) -> bool {
    let mut core = state.lock().unwrap();
    let port = core.local_bridge_port;
    let purged = deduplicate_wifi_adb(&mut core);
    if let Some(dev) = core.paired_devices.get_mut(&hardware_serial) {
        if !dev.auto_reconnect {
            return false;
        }
        let endpoint = format!("127.0.0.1:{}", port);
        let _ = run_adb(&["connect", &endpoint]);
        dev.status = PairingStatus::Connected;
        dev.last_seen = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        dev.purged_duplicates += purged;
        let model = dev.model.clone();
        core.save();
        core.log(format!(
            "[Lifecycle Watchdog] Verified single ADB bridge to '{}' at {}.",
            model, endpoint
        ));
        return true;
    }
    false
}

/// Background Lifecycle Watchdog:
/// Continuously monitors paired devices, purges duplicate Wi-Fi ADB ghosts, and automatically
/// reconnects dropped sessions unless the pairing was explicitly removed on PC or Phone.
fn spawn_lifecycle_watchdog(app: AppHandle, core: SharedCore) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(6));
        loop {
            interval.tick().await;
            let mut sync_tasks = Vec::new();
            {
                let mut guard = core.lock().unwrap();
                let gw_url = guard.gateway_url.clone();
                let has_active = guard.paired_devices.values().any(|d| d.auto_reconnect);
                if has_active {
                    let purged = deduplicate_wifi_adb(&mut guard);
                    let canonical = format!("127.0.0.1:{}", guard.local_bridge_port);
                    for dev in guard.paired_devices.values_mut() {
                        if dev.auto_reconnect {
                            dev.purged_duplicates += purged;
                            dev.last_seen = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
                            dev.status = PairingStatus::Connected;
                            let _ = run_adb(&["connect", &canonical]);
                            sync_tasks.push((
                                gw_url.clone(),
                                dev.hardware_serial.clone(),
                                dev.model.clone(),
                                dev.pair_token.clone(),
                                dev.last_known_ip.clone(),
                            ));
                        }
                    }
                }
            }
            for (gw_url, hw, model, token, wlan_ip) in sync_tasks {
                let core_ref = core.clone();
                tokio::spawn(async move {
                    if let Ok((mut ws, _)) = connect_async(&gw_url).await {
                        let reg = serde_json::json!({
                            "type": "REGISTER_SESSION",
                            "role": "pc",
                            "pc_id": "pairspan-desktop",
                            "pc_os": std::env::consts::OS,
                            "hardware_serial": hw,
                            "phone_model": model,
                            "pair_token": token,
                            "phone_wlan_ip": wlan_ip
                        });
                        let _ = ws.send(WsMessage::Text(reg.to_string())).await;
                        if let Some(Ok(WsMessage::Text(reply))) = ws.next().await {
                            if reply.contains("REVOKED") || reply.contains("PAIRING_REVOKED") {
                                let mut g = core_ref.lock().unwrap();
                                if let Some(d) = g.paired_devices.remove(&hw) {
                                    let _ = run_adb(&["disconnect", &d.canonical_endpoint]);
                                    g.save();
                                    g.log(format!(
                                        "[Lifecycle Revoked] Phone '{}' removed PC pairing remotely. Auto-reconnect permanently stopped.",
                                        d.model
                                    ));
                                }
                            }
                        }
                    }
                });
            }
            let _ = app.emit("pairspan://state-updated", ());
        }
    });
}

#[tauri::command]
fn open_main_window(app: AppHandle) {
    if let Some(w) = app.get_webview_window("widget") {
        let _ = w.hide();
    }
    if let Some(m) = app.get_webview_window("main") {
        let _ = m.show();
        let _ = m.set_focus();
    }
}

fn main() {
    let core: SharedCore = Arc::new(Mutex::new(PairspanCore::load()));

    tauri::Builder::default()
        .manage(core.clone())
        .setup(move |app| {
            // Build System Tray / Menu Bar Widget + Full App Switcher
            let show_app = MenuItem::with_id(app, "show_app", "Open Pairspan — Android Device Center", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit Pairspan", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_app, &quit])?;

            let mut tray_builder = TrayIconBuilder::new()
                .menu(&menu)
                .show_menu_on_left_click(false)
                .tooltip("Pairspan — Android Device Center");
            if let Some(ic) = app.default_window_icon() {
                tray_builder = tray_builder.icon(ic.clone());
            }
            let _tray = tray_builder
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "show_app" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        position,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(w) = app.get_webview_window("widget") {
                            let vis = w.is_visible().unwrap_or(false);
                            if vis {
                                let _ = w.hide();
                            } else {
                                // Position popover directly under the macOS / OS status bar tray icon
                                let x = (position.x as i32) - 180;
                                let y = (position.y as i32) + 8;
                                let _ = w.set_position(tauri::PhysicalPosition::new(x.max(12), y.max(24)));
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            spawn_lifecycle_watchdog(app.handle().clone(), core.clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            usb_authorize_and_pair,
            purge_wifi_adb_conflicts,
            unpair_device,
            trigger_reconnect_now,
            open_main_window
        ])
        .run(tauri::generate_context!())
        .expect("error while running Pairspan desktop application");
}
