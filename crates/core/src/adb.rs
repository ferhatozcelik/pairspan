//! Locating and running the `adb` binary.

use anyhow::{bail, Context, Result};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::process::Command;

/// Order: `PAIRSPAN_ADB`, `PATH`, the usual SDK locations, then the copy bundled next to our executable.
/// The user's own adb wins so two adb versions never fight over the server on port 5037.
pub fn binary() -> Option<PathBuf> {
    static FOUND: OnceLock<Option<PathBuf>> = OnceLock::new();
    FOUND
        .get_or_init(|| {
            let exe = if cfg!(windows) { "adb.exe" } else { "adb" };
            let mut candidates: Vec<PathBuf> = Vec::new();
            if let Ok(p) = std::env::var("PAIRSPAN_ADB") {
                candidates.push(p.into());
            }
            if let Some(paths) = std::env::var_os("PATH") {
                candidates.extend(std::env::split_paths(&paths).map(|d| d.join(exe)));
            }
            for var in ["ANDROID_HOME", "ANDROID_SDK_ROOT"] {
                if let Ok(sdk) = std::env::var(var) {
                    candidates.push(PathBuf::from(sdk).join("platform-tools").join(exe));
                }
            }
            if let Some(home) = dirs::home_dir() {
                candidates.push(home.join("Library/Android/sdk/platform-tools").join(exe));
                candidates.push(home.join("Android/Sdk/platform-tools").join(exe));
                candidates.push(home.join("AppData/Local/Android/Sdk/platform-tools").join(exe));
            }
            candidates.extend(["/opt/homebrew/bin/adb", "/usr/local/bin/adb", "/usr/bin/adb"].map(PathBuf::from));
            if let Ok(me) = std::env::current_exe() {
                if let Some(dir) = me.parent() {
                    candidates.push(dir.join(exe));
                }
            }
            candidates.into_iter().find(|p| p.is_file())
        })
        .clone()
}

pub struct Output {
    pub code: i32,
    pub stdout: Vec<u8>,
    pub stderr: String,
}

impl Output {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }

    pub fn ok(self) -> Result<Self> {
        if self.code != 0 {
            let msg = if self.stderr.trim().is_empty() { self.text() } else { self.stderr.clone() };
            bail!("adb exited with {}: {}", self.code, msg.trim());
        }
        Ok(self)
    }
}

/// Runs `adb [-s serial] args...` with a timeout.
pub async fn run(serial: Option<&str>, args: &[&str], timeout: Duration) -> Result<Output> {
    let bin = binary().context("adb not found. Install Android platform-tools or set PAIRSPAN_ADB.")?;
    let mut cmd = Command::new(bin);
    if let Some(s) = serial {
        cmd.args(["-s", s]);
    }
    cmd.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    let out = tokio::time::timeout(timeout, cmd.output())
        .await
        .with_context(|| format!("adb {} timed out after {}s", args.join(" "), timeout.as_secs()))?
        .context("could not start adb")?;
    Ok(Output {
        code: out.status.code().unwrap_or(-1),
        stdout: out.stdout,
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct UsbDevice {
    pub serial: String,
    pub state: String,
    pub model: String,
}

/// Phones attached over USB (network serials such as `127.0.0.1:47100` are skipped).
pub async fn usb_devices() -> Result<Vec<UsbDevice>> {
    list(false).await
}

/// Every phone adb sees: USB, wireless debugging and Pairspan tunnels (`127.0.0.1:port`).
pub async fn all_devices() -> Result<Vec<UsbDevice>> {
    list(true).await
}

async fn list(include_network: bool) -> Result<Vec<UsbDevice>> {
    let out = run(None, &["devices", "-l"], Duration::from_secs(10)).await?.ok()?;
    Ok(out
        .text()
        .lines()
        .skip(1)
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let serial = parts.next()?.to_string();
            let state = parts.next()?.to_string();
            if !include_network && (serial.contains(':') || serial.contains("._tcp")) {
                return None;
            }
            let model = parts
                .find_map(|p| p.strip_prefix("model:"))
                .unwrap_or("Android")
                .replace('_', " ");
            Some(UsbDevice { serial, state, model })
        })
        .collect())
}
