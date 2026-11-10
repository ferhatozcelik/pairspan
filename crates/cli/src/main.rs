//! `pairspan`: drives phones paired with the Pairspan desktop app through its local socket.

mod mcp;

use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use clap::{Parser, Subcommand};
use pairspan_core::rpc::Client;
use pairspan_core::{store, wire};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Parser)]
#[command(name = "pairspan", version, about = "Control Android phones paired with the Pairspan app.")]
struct Cli {
    /// Phone to use: name, id prefix or serial. Defaults to the only online phone.
    #[arg(short, long, global = true)]
    device: Option<String>,
    /// Print raw JSON results.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// App, gateway and phone status.
    Status,
    /// Paired phones.
    Devices,
    /// Pair a new phone: shows a QR code to scan with the Pairspan Android app.
    Pair {
        /// Seconds to wait for the phone.
        #[arg(long, default_value_t = 300)]
        timeout: u64,
    },
    /// Remove a paired phone.
    Unpair { device: String },
    /// Run a shell command on the phone.
    Shell {
        #[arg(trailing_var_arg = true, required = true)]
        command: Vec<String>,
    },
    /// Run any adb command against the phone, e.g. `pairspan adb -- pull /sdcard/x.txt`.
    Adb {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
        args: Vec<String>,
    },
    /// Tap at screen coordinates.
    Tap { x: i32, y: i32 },
    /// Tap the first element whose text, description or id matches.
    Click {
        value: String,
        /// text, desc, id or any.
        #[arg(long, default_value = "any")]
        by: String,
        #[arg(long, default_value_t = 10)]
        timeout: u64,
    },
    /// Swipe between two points.
    Swipe {
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        #[arg(long, default_value_t = 300)]
        ms: u64,
    },
    /// Type text into the focused field.
    Text { text: String },
    /// Press a key: home, back, enter, recents, power, volume_up… or a KEYCODE_ name.
    Key { key: String },
    /// Save a screenshot as PNG.
    Screenshot { file: Option<PathBuf> },
    /// Print the UI hierarchy XML.
    Dump,
    /// List on-screen elements (optionally filtered).
    Elements { query: Option<String> },
    /// Start an app by package name.
    Launch { package: String },
    /// Force-stop an app.
    Stop { package: String },
    /// Install an APK.
    Install { apk: PathBuf },
    /// Installed (non-system) apps.
    Apps,
    /// Foreground app and activity.
    Current,
    /// Recent logcat lines.
    Logcat {
        #[arg(short = 'n', long, default_value_t = 200)]
        lines: u32,
    },
    /// Phones attached over USB; `usb prepare <serial>` enables the wireless ADB port once.
    Usb {
        #[command(subcommand)]
        action: Option<UsbCmd>,
    },
    /// Recent app log.
    Logs,
    /// Run as an MCP server on stdio (for Claude, Cursor, VS Code…).
    Mcp {
        /// Only expose tools that read from the phone.
        #[arg(long)]
        read_only: bool,
    },
}

#[derive(Subcommand)]
enum UsbCmd {
    /// Run `adb tcpip 5555` on a USB phone (needed once after each phone reboot).
    Prepare { serial: String },
}

/// Connects to the running app, starting it in the background when needed.
pub async fn connect() -> Result<Client> {
    let socket = store::socket_path(&store::default_dir());
    if let Ok(c) = Client::connect(&socket).await {
        return Ok(c);
    }
    launch_app()?;
    for _ in 0..60 {
        tokio::time::sleep(Duration::from_millis(250)).await;
        if let Ok(c) = Client::connect(&socket).await {
            return Ok(c);
        }
    }
    bail!("The Pairspan app is not running and could not be started. Open Pairspan and try again.")
}

fn launch_app() -> Result<()> {
    let me = std::env::current_exe().ok();
    // Inside Pairspan.app/Contents/MacOS the app binary sits next to us.
    if let Some(sibling) = me.as_deref().and_then(Path::parent).map(|d| d.join("pairspan-desktop")) {
        if sibling.is_file() {
            std::process::Command::new(sibling).arg("--background").spawn().context("start Pairspan")?;
            return Ok(());
        }
    }
    if cfg!(target_os = "macos") {
        let ok = std::process::Command::new("open").args(["-g", "-a", "Pairspan", "--args", "--background"]).status()?.success();
        if ok {
            return Ok(());
        }
    }
    bail!("Pairspan app not found")
}

fn print(value: &Value, json_out: bool) {
    if json_out {
        println!("{}", serde_json::to_string_pretty(value).unwrap_or_default());
        return;
    }
    match value {
        Value::String(s) => println!("{s}"),
        Value::Bool(true) => {}
        Value::Null => {}
        v => println!("{}", serde_json::to_string_pretty(v).unwrap_or_default()),
    }
}

fn print_devices(devices: &Value) {
    let list = devices.as_array().cloned().unwrap_or_default();
    if list.is_empty() {
        println!("No paired phones. Run `pairspan pair`.");
        return;
    }
    for d in list {
        println!(
            "{} {:<20} {:<10} {:<22} {}",
            if d["online"] == true { "●" } else { "○" },
            d["name"].as_str().unwrap_or(""),
            &d["id"].as_str().unwrap_or("")[..8.min(d["id"].as_str().unwrap_or("").len())],
            d["serial"].as_str().unwrap_or(""),
            [d["model"].as_str().unwrap_or(""), d["android"].as_str().map(|a| if a.is_empty() { String::new() } else { format!("Android {a}") }).as_deref().unwrap_or("")]
                .iter()
                .filter(|s| !s.is_empty())
                .cloned()
                .collect::<Vec<_>>()
                .join(" · "),
        );
    }
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("pairspan: {e:#}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();
    if let Cmd::Mcp { read_only } = cli.cmd {
        return mcp::serve(read_only).await;
    }
    let mut c = connect().await?;
    let dev = cli.device.clone();
    let with = |extra: Value| {
        let mut p = extra;
        if let Some(d) = &dev {
            p["device"] = json!(d);
        }
        p
    };
    let j = cli.json;
    match cli.cmd {
        Cmd::Status => {
            let s = c.call("status", json!({})).await?;
            if j {
                print(&s, true);
            } else {
                println!("Pairspan {}  ·  PC id {}", s["version"].as_str().unwrap_or(""), &s["id"].as_str().unwrap_or("")[..8]);
                println!(
                    "Gateway   {} ({})",
                    s["gateway"].as_str().unwrap_or(""),
                    if s["connected"] == true { "connected" } else { "offline" }
                );
                println!("Automation {}", if s["automation"] == true { "allowed" } else { "turned off in the app" });
                print_devices(&s["devices"]);
            }
        }
        Cmd::Devices => {
            let d = c.call("devices", json!({})).await?;
            if j { print(&d, true) } else { print_devices(&d) }
        }
        Cmd::Pair { timeout } => {
            let info = c.call("pair.start", json!({})).await?;
            let qr = info["qr"].as_str().unwrap_or("");
            println!("{}", wire::qr_text(qr));
            println!("Scan with the Pairspan Android app, or enter the code: {}", info["code"].as_str().unwrap_or(""));
            println!("Waiting for the phone…");
            let d = c.call("pair.wait", json!({ "timeout": timeout })).await?;
            println!("✓ Paired with {} ({})", d["name"].as_str().unwrap_or("phone"), &d["id"].as_str().unwrap_or("")[..8]);
        }
        Cmd::Unpair { device } => {
            let d = c.call("unpair", json!({ "device": device })).await?;
            println!("Removed {}", d["name"].as_str().unwrap_or("phone"));
        }
        Cmd::Shell { command } => {
            let r = c.call("shell", with(json!({ "command": command.join(" ") }))).await?;
            if j {
                print(&r, true);
            } else {
                print!("{}", r["output"].as_str().unwrap_or(""));
                std::process::exit(r["code"].as_i64().unwrap_or(1) as i32);
            }
        }
        Cmd::Adb { args } => {
            let r = c.call("adb", with(json!({ "args": args }))).await?;
            print!("{}", r["output"].as_str().unwrap_or(""));
            std::process::exit(r["code"].as_i64().unwrap_or(1) as i32);
        }
        Cmd::Tap { x, y } => print(&c.call("tap", with(json!({ "x": x, "y": y }))).await?, j),
        Cmd::Click { value, by, timeout } => {
            let e = c.call("tap_element", with(json!({ "value": value, "by": by, "timeout": timeout }))).await?;
            if j { print(&e, true) } else { println!("Tapped '{}' at {},{}", value, e["center"][0], e["center"][1]) }
        }
        Cmd::Swipe { x1, y1, x2, y2, ms } => {
            print(&c.call("swipe", with(json!({ "x1": x1, "y1": y1, "x2": x2, "y2": y2, "duration_ms": ms }))).await?, j)
        }
        Cmd::Text { text } => print(&c.call("text", with(json!({ "text": text }))).await?, j),
        Cmd::Key { key } => print(&c.call("key", with(json!({ "key": key }))).await?, j),
        Cmd::Screenshot { file } => {
            let r = c.call("screenshot", with(json!({}))).await?;
            let png = B64.decode(r["png_base64"].as_str().unwrap_or(""))?;
            let path = file.unwrap_or_else(|| {
                let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                PathBuf::from(format!("screenshot-{ts}.png"))
            });
            std::fs::write(&path, png)?;
            println!("{}", path.display());
        }
        Cmd::Dump => println!("{}", c.call("dump", with(json!({}))).await?["xml"].as_str().unwrap_or("")),
        Cmd::Elements { query } => {
            let els = c.call("elements", with(json!({ "query": query.unwrap_or_default() }))).await?;
            if j {
                print(&els, true);
            } else {
                for e in els.as_array().cloned().unwrap_or_default() {
                    let label = [e["text"].as_str(), e["desc"].as_str()].into_iter().flatten().find(|s| !s.is_empty()).unwrap_or("");
                    println!(
                        "{:>5},{:<5} {} {:<32} {}",
                        e["center"][0],
                        e["center"][1],
                        if e["clickable"] == true { "▸" } else { " " },
                        label.chars().take(32).collect::<String>(),
                        e["id"].as_str().unwrap_or("")
                    );
                }
            }
        }
        Cmd::Launch { package } => print(&c.call("launch", with(json!({ "package": package }))).await?, j),
        Cmd::Stop { package } => print(&c.call("stop", with(json!({ "package": package }))).await?, j),
        Cmd::Install { apk } => {
            let path = std::fs::canonicalize(&apk).with_context(|| format!("{} not found", apk.display()))?;
            print(&c.call("install", with(json!({ "path": path }))).await?, j)
        }
        Cmd::Apps => {
            let apps = c.call("apps", with(json!({}))).await?;
            if j { print(&apps, true) } else { apps.as_array().into_iter().flatten().for_each(|a| println!("{}", a.as_str().unwrap_or(""))) }
        }
        Cmd::Current => print(&c.call("current", with(json!({}))).await?, j),
        Cmd::Logcat { lines } => print!("{}", c.call("logcat", with(json!({ "lines": lines }))).await?.as_str().unwrap_or("")),
        Cmd::Usb { action } => match action {
            None => {
                let list = c.call("usb.list", json!({})).await?;
                if j {
                    print(&list, true);
                } else if list.as_array().is_none_or(|a| a.is_empty()) {
                    println!("No phones on USB.");
                } else {
                    for d in list.as_array().unwrap() {
                        println!("{:<20} {:<14} {}", d["serial"].as_str().unwrap_or(""), d["state"].as_str().unwrap_or(""), d["model"].as_str().unwrap_or(""));
                    }
                }
            }
            Some(UsbCmd::Prepare { serial }) => print(&c.call("usb.prepare", json!({ "serial": serial })).await?, j),
        },
        Cmd::Logs => {
            for l in c.call("logs", json!({})).await?.as_array().into_iter().flatten() {
                println!("{}", l.as_str().unwrap_or(""));
            }
        }
        Cmd::Mcp { .. } => unreachable!(),
    }
    Ok(())
}
