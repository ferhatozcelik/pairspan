//! Plug a phone in once: Pairspan installs (or updates) the Android app, pairs over the cable
//! and turns on wireless ADB. Plugging a paired phone in again (after a reboot) only redoes ADB.

use crate::adb::{self, UsbDevice};
use crate::agent::{Agent, Setup};
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

const PACKAGE: &str = "com.pairspan";
const RECEIVER: &str = "com.pairspan/com.pairspan.app.link.PairReceiver";
const RELEASES: &str = "https://api.github.com/repos/ferhatozcelik/pairspan/releases/latest";

impl Agent {
    pub(crate) async fn usb_loop(self: Arc<Self>) {
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            if !self.enabled() {
                continue;
            }
            let devices = adb::usb_devices().await.unwrap_or_default();
            {
                let mut seen = self.usb_seen.lock().unwrap();
                seen.retain(|s| devices.iter().any(|d| &d.serial == s));
            }
            let unplugged = self.setup.lock().unwrap().as_ref().is_some_and(|s| !devices.iter().any(|d| d.serial == s.serial));
            if unplugged {
                *self.setup.lock().unwrap() = None;
                self.changed();
            }
            for d in devices.into_iter().filter(|d| d.state == "device") {
                if self.usb_seen.lock().unwrap().insert(d.serial.clone()) {
                    self.setup_phone(d).await;
                }
            }
        }
    }

    fn set_step(&self, usb: &UsbDevice, step: &'static str, error: Option<String>) {
        *self.setup.lock().unwrap() = Some(Setup { serial: usb.serial.clone(), model: usb.model.clone(), step, error });
        self.changed();
    }

    async fn setup_phone(&self, usb: UsbDevice) {
        match self.setup_steps(&usb).await {
            Ok(()) => {
                self.log(format!("{} is set up; you can unplug it", usb.model));
                self.set_step(&usb, "done", None);
            }
            Err(e) => {
                self.log(format!("Setting up {} failed: {e:#}", usb.model));
                self.set_step(&usb, "failed", Some(format!("{e:#}")));
            }
        }
    }

    async fn setup_steps(&self, usb: &UsbDevice) -> Result<()> {
        let serial = usb.serial.as_str();
        let latest = latest_release().await.ok();
        let installed = installed_version(serial).await;
        let wanted = latest.as_ref().map(|(v, _)| v.as_str());
        if installed.is_none() || (wanted.is_some() && installed.as_deref() != wanted) {
            self.set_step(usb, "install", None);
            let (_, url) = latest.context("Could not reach GitHub to download the Android app.")?;
            install_apk(serial, &url).await?;
        }
        if self.device_by_usb(serial).is_none() {
            self.set_step(usb, "pair", None);
            let info = self.start_pairing().await?;
            let me = self.id().to_string();
            let push = async {
                adb::run(Some(serial), &["shell", "am", "broadcast", "-n", RECEIVER, "--es", "secret", &info.code, "--es", "pc", &me], Duration::from_secs(10))
                    .await?
                    .ok()
            };
            let (paired, pushed) = tokio::join!(self.wait_paired(Duration::from_secs(90)), push);
            if let Err(e) = pushed {
                self.cancel_pairing().await;
                return Err(e.context("could not reach the Pairspan app on the phone"));
            }
            match paired {
                Ok(device) => self.set_usb_serial(device.id, serial),
                Err(e) => {
                    self.cancel_pairing().await;
                    return Err(e);
                }
            }
        }
        self.set_step(usb, "adb", None);
        crate::automation::usb_prepare(serial).await?;
        Ok(())
    }
}

/// `(version, apk url)` of the latest GitHub release.
async fn latest_release() -> Result<(String, String)> {
    let out = tokio::process::Command::new("curl")
        .args(["-fsSL", "--max-time", "15", "-H", "Accept: application/vnd.github+json", RELEASES])
        .output()
        .await?;
    let release: Value = serde_json::from_slice(&out.stdout).context("bad release data")?;
    let version = release["tag_name"].as_str().unwrap_or_default().trim_start_matches('v').to_string();
    let url = release["assets"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|a| a["name"].as_str().is_some_and(|n| n.ends_with(".apk")))
        .and_then(|a| a["browser_download_url"].as_str())
        .context("the latest release has no APK")?
        .to_string();
    Ok((version, url))
}

async fn installed_version(serial: &str) -> Option<String> {
    let out = adb::run(Some(serial), &["shell", "dumpsys", "package", PACKAGE], Duration::from_secs(10)).await.ok()?;
    out.text().lines().find_map(|l| l.trim().strip_prefix("versionName=").map(str::to_string))
}

async fn install_apk(serial: &str, url: &str) -> Result<()> {
    let cache = crate::store::default_dir().join("cache");
    std::fs::create_dir_all(&cache)?;
    let apk = cache.join("pairspan.apk");
    let ok = tokio::process::Command::new("curl").args(["-fsSL", "--max-time", "300", "-o"]).arg(&apk).arg(url).status().await?.success();
    if !ok {
        bail!("Downloading the Android app failed. Check the internet connection.");
    }
    crate::automation::install(serial, &apk.to_string_lossy()).await?;
    Ok(())
}
