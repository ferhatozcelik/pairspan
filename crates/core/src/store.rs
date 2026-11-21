//! Files under the app data directory: identity key, settings and paired phones.

use crate::identity::PeerId;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
    /// The on/off switch: when off, Pairspan leaves the signalling service and closes every link.
    pub enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { enabled: true }
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
    /// The USB serial it was paired over, so plugging it in again only re-enables wireless ADB.
    #[serde(default)]
    pub usb_serial: String,
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
        let old: Settings = serde_json::from_str(r#"{"gateway_url":"wss://example.com/ws","automation_enabled":false}"#).unwrap();
        assert!(old.enabled);
    }
}
