//! Local JSON-RPC over a Unix socket (one JSON object per line). The CLI and MCP server use it;
//! nothing here is reachable from the network.

use crate::agent::Agent;
use crate::automation as auto;
use crate::identity::PeerId;
use anyhow::{anyhow, bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Deserialize)]
pub struct Request {
    pub id: Value,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RpcError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

/// Methods that act on a phone; refused while automation is switched off in the app.
const AUTOMATION: &[&str] = &[
    "adb", "shell", "tap", "swipe", "text", "key", "screenshot", "dump", "elements", "tap_element", "launch", "stop",
    "install", "apps", "logcat", "current", "screen_size",
];

#[cfg(unix)]
pub async fn serve(agent: Arc<Agent>, socket: &Path) -> Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixListener;

    if socket.exists() {
        // A live app already owns it: refuse instead of stealing the socket.
        if tokio::net::UnixStream::connect(socket).await.is_ok() {
            bail!("another Pairspan instance is already running ({})", socket.display());
        }
        std::fs::remove_file(socket)?;
    }
    let listener = UnixListener::bind(socket).with_context(|| format!("bind {}", socket.display()))?;
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))?;
    }
    agent.log(format!("Local automation socket: {}", socket.display()));
    loop {
        let (stream, _) = listener.accept().await?;
        let agent = agent.clone();
        tokio::spawn(async move {
            let (rd, mut wr) = stream.into_split();
            let mut lines = BufReader::new(rd).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let resp = match serde_json::from_str::<Request>(&line) {
                    Ok(req) => handle(&agent, req).await,
                    Err(e) => Response {
                        id: Value::Null,
                        result: None,
                        error: Some(RpcError { code: "bad_request".into(), message: e.to_string() }),
                    },
                };
                let mut out = serde_json::to_vec(&resp).unwrap_or_default();
                out.push(b'\n');
                if wr.write_all(&out).await.is_err() {
                    break;
                }
            }
        });
    }
}

#[cfg(not(unix))]
pub async fn serve(agent: Arc<Agent>, _socket: &Path) -> Result<()> {
    agent.log("Local automation socket is not available on this platform yet.");
    std::future::pending::<()>().await;
    Ok(())
}

pub async fn handle(agent: &Agent, req: Request) -> Response {
    let id = req.id.clone();
    match dispatch(agent, &req.method, &req.params).await {
        Ok(result) => Response { id, result: Some(result), error: None },
        Err(e) => Response {
            id,
            result: None,
            error: Some(RpcError { code: "failed".into(), message: format!("{e:#}") }),
        },
    }
}

fn str_param<'a>(p: &'a Value, key: &str) -> Result<&'a str> {
    p[key].as_str().ok_or_else(|| anyhow!("missing parameter `{key}`"))
}

fn int_param(p: &Value, key: &str) -> Result<i32> {
    p[key].as_i64().map(|v| v as i32).ok_or_else(|| anyhow!("missing parameter `{key}`"))
}

/// Picks a phone by id prefix, name or serial; with no selector, the only online phone.
pub fn resolve_device(agent: &Agent, selector: Option<&str>) -> Result<(PeerId, String)> {
    let devices = agent.devices();
    if devices.is_empty() {
        bail!("No paired phones. Pair one in the Pairspan app (or run `pairspan pair`).");
    }
    let pick = match selector.filter(|s| !s.is_empty()) {
        Some(sel) => {
            let s = sel.to_lowercase();
            devices
                .iter()
                .find(|d| d.device.id.to_string().starts_with(&s) || d.device.name.to_lowercase() == s || d.serial == sel)
                .with_context(|| format!("no paired phone matches '{sel}'"))?
        }
        None => {
            let online: Vec<_> = devices.iter().filter(|d| d.online).collect();
            match online.len() {
                1 => online[0],
                0 => bail!("No phone is online. Open Pairspan on the phone."),
                _ => bail!(
                    "Several phones are online; choose one with --device: {}",
                    online.iter().map(|d| format!("{} ({})", d.device.name, d.device.id.short())).collect::<Vec<_>>().join(", ")
                ),
            }
        }
    };
    if !pick.online {
        bail!("{} is offline. Open Pairspan on the phone.", pick.device.name);
    }
    Ok((pick.device.id, pick.serial.clone()))
}

pub async fn dispatch(agent: &Agent, method: &str, p: &Value) -> Result<Value> {
    if AUTOMATION.contains(&method) && !agent.automation_enabled() {
        bail!("Automation is turned off in the Pairspan app (Settings → Allow CLI and MCP).");
    }
    let serial = || -> Result<String> { Ok(resolve_device(agent, p["device"].as_str())?.1) };
    Ok(match method {
        "status" => serde_json::to_value(agent.status())?,
        "devices" => serde_json::to_value(agent.devices())?,
        "logs" => json!(agent.logs()),
        "pair.start" => serde_json::to_value(agent.start_pairing().await?)?,
        "pair.wait" => {
            let secs = p["timeout"].as_u64().unwrap_or(300);
            serde_json::to_value(agent.wait_paired(Duration::from_secs(secs)).await?)?
        }
        "pair.cancel" => {
            agent.cancel_pairing().await;
            json!(true)
        }
        "unpair" => {
            let (id, _) = resolve_device_any(agent, str_param(p, "device")?)?;
            serde_json::to_value(agent.remove_device(id, true).await?)?
        }
        "usb.list" => serde_json::to_value(crate::adb::usb_devices().await?)?,
        "usb.prepare" => json!(auto::usb_prepare(str_param(p, "serial")?).await?),
        "adb" => {
            let args: Vec<String> = serde_json::from_value(p["args"].clone()).context("`args` must be a list of strings")?;
            serde_json::to_value(auto::adb_raw(&serial()?, &args).await?)?
        }
        "shell" => serde_json::to_value(auto::shell(&serial()?, str_param(p, "command")?).await?)?,
        "tap" => {
            auto::tap(&serial()?, int_param(p, "x")?, int_param(p, "y")?).await?;
            json!(true)
        }
        "swipe" => {
            let ms = p["duration_ms"].as_u64().unwrap_or(300) as u32;
            auto::swipe(&serial()?, (int_param(p, "x1")?, int_param(p, "y1")?), (int_param(p, "x2")?, int_param(p, "y2")?), ms).await?;
            json!(true)
        }
        "text" => {
            auto::text(&serial()?, str_param(p, "text")?).await?;
            json!(true)
        }
        "key" => {
            auto::key(&serial()?, str_param(p, "key")?).await?;
            json!(true)
        }
        "screenshot" => json!({ "png_base64": B64.encode(auto::screenshot(&serial()?).await?) }),
        "dump" => json!({ "xml": auto::dump(&serial()?).await? }),
        "elements" => {
            let all = auto::parse_elements(&auto::dump(&serial()?).await?);
            let q = p["query"].as_str().unwrap_or("").to_lowercase();
            let filtered: Vec<_> = all
                .into_iter()
                .filter(|e| q.is_empty() || [&e.text, &e.desc, &e.id].iter().any(|f| f.to_lowercase().contains(&q)))
                .collect();
            serde_json::to_value(filtered)?
        }
        "tap_element" => {
            let by = p["by"].as_str().unwrap_or("any");
            let timeout = Duration::from_secs(p["timeout"].as_u64().unwrap_or(10));
            serde_json::to_value(auto::tap_element(&serial()?, by, str_param(p, "value")?, timeout).await?)?
        }
        "launch" => {
            auto::launch(&serial()?, str_param(p, "package")?).await?;
            json!(true)
        }
        "stop" => {
            auto::stop(&serial()?, str_param(p, "package")?).await?;
            json!(true)
        }
        "install" => json!(auto::install(&serial()?, str_param(p, "path")?).await?),
        "apps" => json!(auto::apps(&serial()?).await?),
        "logcat" => json!(auto::logcat(&serial()?, p["lines"].as_u64().unwrap_or(200) as u32).await?),
        "current" => json!(auto::current(&serial()?).await?),
        "screen_size" => {
            let (w, h) = auto::screen_size(&serial()?).await?;
            json!({ "width": w, "height": h })
        }
        other => bail!("unknown method `{other}`"),
    })
}

/// Like `resolve_device` but also matches offline phones (for unpair).
fn resolve_device_any(agent: &Agent, sel: &str) -> Result<(PeerId, String)> {
    let s = sel.to_lowercase();
    agent
        .devices()
        .into_iter()
        .find(|d| d.device.id.to_string().starts_with(&s) || d.device.name.to_lowercase() == s || d.serial == sel)
        .map(|d| (d.device.id, d.serial))
        .with_context(|| format!("no paired phone matches '{sel}'"))
}

/// Client side, used by the CLI.
#[cfg(unix)]
pub struct Client {
    lines: tokio::io::Lines<tokio::io::BufReader<tokio::net::unix::OwnedReadHalf>>,
    wr: tokio::net::unix::OwnedWriteHalf,
    next: u64,
}

#[cfg(unix)]
impl Client {
    pub async fn connect(socket: &Path) -> Result<Self> {
        use tokio::io::{AsyncBufReadExt, BufReader};
        let stream = tokio::net::UnixStream::connect(socket).await?;
        let (rd, wr) = stream.into_split();
        Ok(Self { lines: BufReader::new(rd).lines(), wr, next: 1 })
    }

    pub async fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        use tokio::io::AsyncWriteExt;
        let id = self.next;
        self.next += 1;
        let mut line = serde_json::to_vec(&json!({ "id": id, "method": method, "params": params }))?;
        line.push(b'\n');
        self.wr.write_all(&line).await?;
        let reply = self.lines.next_line().await?.context("Pairspan app closed the connection")?;
        let resp: Response = serde_json::from_str(&reply)?;
        match (resp.result, resp.error) {
            (_, Some(e)) => Err(anyhow!(e.message)),
            (Some(r), None) => Ok(r),
            (None, None) => Ok(Value::Null),
        }
    }
}
