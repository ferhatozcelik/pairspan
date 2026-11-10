//! Phone automation over ADB (through the encrypted tunnel): input, screen, UI tree, apps.

use crate::adb;
use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::time::Duration;

const SHORT: Duration = Duration::from_secs(20);
const LONG: Duration = Duration::from_secs(300);

#[derive(Debug, Serialize)]
pub struct ShellResult {
    pub code: i32,
    pub output: String,
}

pub async fn shell(serial: &str, command: &str) -> Result<ShellResult> {
    let out = adb::run(Some(serial), &["shell", command], SHORT).await?;
    let mut output = out.text();
    if !out.stderr.trim().is_empty() {
        output.push_str(&out.stderr);
    }
    Ok(ShellResult { code: out.code, output })
}

pub async fn adb_raw(serial: &str, args: &[String]) -> Result<ShellResult> {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = adb::run(Some(serial), &args, LONG).await?;
    Ok(ShellResult { code: out.code, output: format!("{}{}", out.text(), out.stderr) })
}

pub async fn tap(serial: &str, x: i32, y: i32) -> Result<()> {
    adb::run(Some(serial), &["shell", "input", "tap", &x.to_string(), &y.to_string()], SHORT).await?.ok()?;
    Ok(())
}

pub async fn swipe(serial: &str, from: (i32, i32), to: (i32, i32), ms: u32) -> Result<()> {
    let args = [from.0, from.1, to.0, to.1].map(|v| v.to_string());
    adb::run(
        Some(serial),
        &["shell", "input", "swipe", &args[0], &args[1], &args[2], &args[3], &ms.to_string()],
        SHORT,
    )
    .await?
    .ok()?;
    Ok(())
}

/// `input text` treats spaces as separators and runs through the device shell, so escape both.
pub fn escape_input_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for c in text.chars() {
        match c {
            ' ' => out.push_str("%s"),
            c if c.is_ascii_alphanumeric() || "-_.,:/@+=".contains(c) => out.push(c),
            c => {
                out.push('\\');
                out.push(c);
            }
        }
    }
    out
}

pub async fn text(serial: &str, text: &str) -> Result<()> {
    if text.is_empty() {
        return Ok(());
    }
    if !text.is_ascii() {
        bail!("`input text` only supports ASCII; non-ASCII text needs the Pairspan keyboard (not available yet)");
    }
    let escaped = escape_input_text(text);
    adb::run(Some(serial), &["shell", &format!("input text {escaped}")], SHORT).await?.ok()?;
    Ok(())
}

/// Accepts names (`home`, `back`, `enter`…), `KEYCODE_*` or a numeric key code.
pub fn keycode(name: &str) -> String {
    let n = name.trim();
    if n.chars().all(|c| c.is_ascii_digit()) || n.starts_with("KEYCODE_") {
        return n.to_string();
    }
    let k = match n.to_ascii_lowercase().as_str() {
        "home" => "HOME",
        "back" => "BACK",
        "enter" | "return" => "ENTER",
        "menu" => "MENU",
        "recents" | "app_switch" | "overview" => "APP_SWITCH",
        "power" => "POWER",
        "volume_up" | "volup" => "VOLUME_UP",
        "volume_down" | "voldown" => "VOLUME_DOWN",
        "tab" => "TAB",
        "delete" | "backspace" | "del" => "DEL",
        "escape" | "esc" => "ESCAPE",
        "search" => "SEARCH",
        "wakeup" | "wake" => "WAKEUP",
        "sleep" => "SLEEP",
        "up" => "DPAD_UP",
        "down" => "DPAD_DOWN",
        "left" => "DPAD_LEFT",
        "right" => "DPAD_RIGHT",
        other => return format!("KEYCODE_{}", other.to_ascii_uppercase()),
    };
    format!("KEYCODE_{k}")
}

pub async fn key(serial: &str, name: &str) -> Result<()> {
    adb::run(Some(serial), &["shell", "input", "keyevent", &keycode(name)], SHORT).await?.ok()?;
    Ok(())
}

pub async fn screenshot(serial: &str) -> Result<Vec<u8>> {
    let out = adb::run(Some(serial), &["exec-out", "screencap", "-p"], SHORT).await?.ok()?;
    if !out.stdout.starts_with(b"\x89PNG") {
        bail!("screencap did not return a PNG (is the screen secure or off?)");
    }
    Ok(out.stdout)
}

/// UI hierarchy XML from uiautomator.
pub async fn dump(serial: &str) -> Result<String> {
    let out = adb::run(Some(serial), &["exec-out", "uiautomator", "dump", "/dev/tty"], SHORT).await?;
    let text = out.text();
    if let (Some(start), Some(end)) = (text.find("<?xml"), text.rfind("</hierarchy>")) {
        return Ok(text[start..end + "</hierarchy>".len()].to_string());
    }
    // Some devices refuse /dev/tty; fall back to a file.
    adb::run(Some(serial), &["shell", "uiautomator", "dump", "/sdcard/pairspan_dump.xml"], SHORT).await?.ok()?;
    let out = adb::run(Some(serial), &["exec-out", "cat", "/sdcard/pairspan_dump.xml"], SHORT).await?.ok()?;
    let text = out.text();
    let start = text.find("<?xml").context("uiautomator returned no XML (screen locked or secure?)")?;
    Ok(text[start..].trim().to_string())
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Element {
    pub text: String,
    pub desc: String,
    pub id: String,
    pub class: String,
    pub package: String,
    pub clickable: bool,
    pub enabled: bool,
    pub checked: bool,
    pub bounds: [i32; 4],
    pub center: [i32; 2],
}

fn attr<'a>(node: &'a str, name: &str) -> &'a str {
    let key = format!(" {name}=\"");
    node.find(&key)
        .and_then(|i| {
            let rest = &node[i + key.len()..];
            rest.find('"').map(|j| &rest[..j])
        })
        .unwrap_or("")
}

fn unescape(s: &str) -> String {
    s.replace("&quot;", "\"").replace("&apos;", "'").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

fn parse_bounds(b: &str) -> Option<[i32; 4]> {
    let nums: Vec<i32> = b
        .split(|c: char| !c.is_ascii_digit() && c != '-')
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect();
    (nums.len() == 4).then(|| [nums[0], nums[1], nums[2], nums[3]])
}

/// Flattens the uiautomator XML into elements with text, description, id or a click handler.
pub fn parse_elements(xml: &str) -> Vec<Element> {
    xml.split("<node")
        .skip(1)
        .filter_map(|chunk| {
            let node = &chunk[..chunk.find('>').unwrap_or(chunk.len())];
            let bounds = parse_bounds(attr(node, "bounds"))?;
            let e = Element {
                text: unescape(attr(node, "text")),
                desc: unescape(attr(node, "content-desc")),
                id: attr(node, "resource-id").rsplit(":id/").next().unwrap_or("").to_string(),
                class: attr(node, "class").to_string(),
                package: attr(node, "package").to_string(),
                clickable: attr(node, "clickable") == "true",
                enabled: attr(node, "enabled") != "false",
                checked: attr(node, "checked") == "true",
                center: [(bounds[0] + bounds[2]) / 2, (bounds[1] + bounds[3]) / 2],
                bounds,
            };
            (!e.text.is_empty() || !e.desc.is_empty() || !e.id.is_empty() || e.clickable).then_some(e)
        })
        .collect()
}

/// `by` is `text`, `desc`, `id` or `any`; matching is exact first, then case-insensitive substring.
pub fn find_element<'a>(elements: &'a [Element], by: &str, value: &str) -> Option<&'a Element> {
    let field = |e: &'a Element| -> Vec<&'a str> {
        match by {
            "text" => vec![&e.text],
            "desc" => vec![&e.desc],
            "id" => vec![&e.id],
            _ => vec![&e.text, &e.desc, &e.id],
        }
    };
    let lower = value.to_lowercase();
    elements
        .iter()
        .find(|e| field(e).iter().any(|f| *f == value))
        .or_else(|| elements.iter().find(|e| field(e).iter().any(|f| !f.is_empty() && f.to_lowercase().contains(&lower))))
}

pub async fn tap_element(serial: &str, by: &str, value: &str, timeout: Duration) -> Result<Element> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let elements = parse_elements(&dump(serial).await?);
        if let Some(e) = find_element(&elements, by, value) {
            tap(serial, e.center[0], e.center[1]).await?;
            return Ok(e.clone());
        }
        if tokio::time::Instant::now() >= deadline {
            bail!("no element with {by} '{value}' on screen");
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

pub async fn launch(serial: &str, package: &str) -> Result<()> {
    if !package.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_') {
        bail!("invalid package name");
    }
    let out = adb::run(
        Some(serial),
        &["shell", "monkey", "-p", package, "-c", "android.intent.category.LAUNCHER", "1"],
        SHORT,
    )
    .await?;
    if out.text().contains("No activities found") {
        bail!("{package} has no launcher activity (is it installed?)");
    }
    out.ok()?;
    Ok(())
}

pub async fn stop(serial: &str, package: &str) -> Result<()> {
    adb::run(Some(serial), &["shell", "am", "force-stop", package], SHORT).await?.ok()?;
    Ok(())
}

pub async fn install(serial: &str, path: &str) -> Result<String> {
    if !std::path::Path::new(path).is_file() {
        bail!("APK not found: {path}");
    }
    let out = adb::run(Some(serial), &["install", "-r", path], LONG).await?.ok()?;
    Ok(out.text().trim().to_string())
}

pub async fn apps(serial: &str) -> Result<Vec<String>> {
    let out = adb::run(Some(serial), &["shell", "pm", "list", "packages", "-3"], SHORT).await?.ok()?;
    let mut list: Vec<String> = out.text().lines().filter_map(|l| l.strip_prefix("package:")).map(|s| s.trim().to_string()).collect();
    list.sort();
    Ok(list)
}

pub async fn logcat(serial: &str, lines: u32) -> Result<String> {
    let out = adb::run(Some(serial), &["logcat", "-d", "-t", &lines.to_string()], SHORT).await?.ok()?;
    Ok(out.text())
}

/// Foreground package/activity, e.g. `com.android.settings/.Settings`.
pub async fn current(serial: &str) -> Result<String> {
    let out = adb::run(Some(serial), &["shell", "dumpsys", "activity", "activities"], SHORT).await?.ok()?;
    let text = out.text();
    let line = text
        .lines()
        .find(|l| l.contains("mResumedActivity") || l.contains("topResumedActivity"))
        .context("no resumed activity")?;
    line.split_whitespace()
        .find(|w| w.contains('/'))
        .map(|w| w.trim_end_matches('}').to_string())
        .context("could not parse the foreground activity")
}

pub async fn screen_size(serial: &str) -> Result<(i32, i32)> {
    let out = adb::run(Some(serial), &["shell", "wm", "size"], SHORT).await?.ok()?;
    let text = out.text();
    let size = text.lines().last().and_then(|l| l.rsplit(' ').next()).context("no size")?;
    let (w, h) = size.trim().split_once('x').context("bad size")?;
    Ok((w.parse()?, h.parse()?))
}

/// One-time USB preparation: classic ADB over TCP on 5555, which the tunnel connects to on the phone.
pub async fn usb_prepare(serial: &str) -> Result<String> {
    let out = adb::run(Some(serial), &["tcpip", "5555"], SHORT).await?.ok()?;
    Ok(out.text().trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version='1.0' encoding='UTF-8' standalone='yes' ?><hierarchy rotation="0"><node index="0" text="" resource-id="" class="android.widget.FrameLayout" package="com.example" content-desc="" checkable="false" checked="false" clickable="false" enabled="true" bounds="[0,0][1080,2400]"><node index="1" text="Log in" resource-id="com.example:id/login" class="android.widget.Button" package="com.example" content-desc="" checked="false" clickable="true" enabled="true" bounds="[100,1000][980,1120]" /><node index="2" text="" resource-id="" class="android.widget.ImageView" package="com.example" content-desc="Close &quot;ad&quot;" clickable="true" enabled="true" bounds="[900,80][1040,220]" /></node></hierarchy>"#;

    #[test]
    fn parses_ui_tree() {
        let els = parse_elements(XML);
        assert_eq!(els.len(), 2);
        assert_eq!(els[0].text, "Log in");
        assert_eq!(els[0].id, "login");
        assert_eq!(els[0].center, [540, 1060]);
        assert_eq!(els[1].desc, "Close \"ad\"");
        assert_eq!(find_element(&els, "any", "log in").unwrap().id, "login");
        assert_eq!(find_element(&els, "id", "login").unwrap().text, "Log in");
        assert!(find_element(&els, "text", "Sign up").is_none());
    }

    #[test]
    fn escapes_input() {
        assert_eq!(escape_input_text("hi there"), "hi%sthere");
        assert_eq!(escape_input_text("a&b;c"), "a\\&b\\;c");
        assert_eq!(escape_input_text("me@x.com"), "me@x.com");
    }

    #[test]
    fn maps_keys() {
        assert_eq!(keycode("home"), "KEYCODE_HOME");
        assert_eq!(keycode("KEYCODE_CAMERA"), "KEYCODE_CAMERA");
        assert_eq!(keycode("66"), "66");
        assert_eq!(keycode("camera"), "KEYCODE_CAMERA");
    }
}
