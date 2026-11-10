//! Files under the app data directory: identity key, settings and paired phones.

use crate::identity::PeerId;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const DEFAULT_GATEWAY: &str = "wss://pairspan.ferhatozcelik.com/ws";
/// First local TCP port handed to `adb connect` for a paired phone.
pub const ADB_PORT_BASE: u16 = 47_100;

/// `~/Library/Application Support/Pairspan` on macOS, `~/.local/share/Pairspan` on Linux.
pub fn default_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("PAIRSPAN_HOME") {
        return PathBuf::from(dir);
    }
    dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("Pairspan")
}

pub fn socket_path(dir: &Path) -> PathBuf {
    dir.join("pairspan.sock")
}

/// Writes a file readable only by the current user.
pub fn write_private(path: &Path, data: &[u8]) -> Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, data).with_context(|| format!("write {}", tmp.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Missing or unknown fields fall back to defaults, so files left by older Pairspan builds still load.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub gateway_url: String,
    pub gateway_token: Option<String>,
    /// Lets the CLI and MCP server reach the app over the local socket.
    pub automation_enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            gateway_url: std::env::var("PAIRSPAN_GATEWAY").unwrap_or_else(|_| DEFAULT_GATEWAY.to_string()),
            gateway_token: option_env!("PAIRSPAN_GATEWAY_TOKEN").map(str::to_string).filter(|t| !t.is_empty()),
            automation_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: PeerId,
    /// Base64 compressed P-256 key pinned at pairing time.
    pub public: String,
    pub name: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub android: String,
    pub adb_port: u16,
    pub paired_at: String,
}

impl Device {
    pub fn serial(&self) -> String {
        format!("127.0.0.1:{}", self.adb_port)
    }
}

pub struct Store {
    dir: PathBuf,
    pub settings: Settings,
    pub devices: Vec<Device>,
}

impl Store {
    pub fn open(dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
        let settings = read_json(&dir.join("settings.json"))?.unwrap_or_default();
        let devices = read_json(&dir.join("devices.json"))?.unwrap_or_default();
        Ok(Self { dir: dir.to_path_buf(), settings, devices })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn save_settings(&self) -> Result<()> {
        write_private(&self.dir.join("settings.json"), &serde_json::to_vec_pretty(&self.settings)?)
    }

    pub fn save_devices(&self) -> Result<()> {
        write_private(&self.dir.join("devices.json"), &serde_json::to_vec_pretty(&self.devices)?)
    }

    pub fn device(&self, id: &PeerId) -> Option<&Device> {
        self.devices.iter().find(|d| d.id == *id)
    }

    /// First port from `ADB_PORT_BASE` that no paired phone uses and nothing else listens on.
    pub fn next_adb_port(&self) -> u16 {
        let base = std::env::var("PAIRSPAN_ADB_PORT_BASE").ok().and_then(|v| v.parse().ok()).unwrap_or(ADB_PORT_BASE);
        (base..u16::MAX)
            .find(|p| {
                !self.devices.iter().any(|d| d.adb_port == *p) && std::net::TcpListener::bind(("127.0.0.1", *p)).is_ok()
            })
            .expect("free port")
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Option<T>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_from_older_builds_load_with_defaults() {
        let old: Settings = serde_json::from_str(r#"{"enabled":true,"clientId":"110bac11465931c10da60293c9dc28b9"}"#).unwrap();
        assert!(old.gateway_url.starts_with("ws"));
        assert!(old.automation_enabled);
    }
}
