//! MCP server on stdio (JSON-RPC 2.0, one message per line). Every tool call is forwarded to the
//! Pairspan app's local socket, so the app stays the only holder of keys and pairings.

use anyhow::Result;
use pairspan_core::rpc::Client;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

struct Tool {
    name: &'static str,
    method: &'static str,
    description: &'static str,
    read_only: bool,
    /// (name, JSON type, description, required)
    params: &'static [(&'static str, &'static str, &'static str, bool)],
}

const DEVICE: (&str, &str, &str, bool) = ("device", "string", "Phone name, id prefix or serial. Optional when one phone is online.", false);

const TOOLS: &[Tool] = &[
    Tool { name: "list_devices", method: "devices", description: "List phones paired with this PC and whether they are online.", read_only: true, params: &[] },
    Tool { name: "screenshot", method: "screenshot", description: "Capture the phone screen as a PNG image.", read_only: true, params: &[DEVICE] },
    Tool {
        name: "ui_elements",
        method: "elements",
        description: "List on-screen UI elements with text, content description, resource id, bounds and tap center. Prefer this over screenshots to find what to tap.",
        read_only: true,
        params: &[("query", "string", "Only elements whose text, description or id contains this (case-insensitive).", false), DEVICE],
    },
    Tool { name: "screen_size", method: "screen_size", description: "Screen width and height in pixels.", read_only: true, params: &[DEVICE] },
    Tool { name: "current_app", method: "current", description: "Package and activity in the foreground.", read_only: true, params: &[DEVICE] },
    Tool { name: "list_apps", method: "apps", description: "Installed third-party app packages.", read_only: true, params: &[DEVICE] },
    Tool { name: "logcat", method: "logcat", description: "Recent logcat lines.", read_only: true, params: &[("lines", "integer", "How many lines (default 200).", false), DEVICE] },
    Tool { name: "tap", method: "tap", description: "Tap at screen coordinates.", read_only: false, params: &[("x", "integer", "X in pixels.", true), ("y", "integer", "Y in pixels.", true), DEVICE] },
    Tool {
        name: "tap_element",
        method: "tap_element",
        description: "Wait for an element by text, content description or resource id and tap its center.",
        read_only: false,
        params: &[
            ("value", "string", "Text, description or id to match (exact first, then substring).", true),
            ("by", "string", "One of text, desc, id, any (default any).", false),
            ("timeout", "integer", "Seconds to wait for it (default 10).", false),
            DEVICE,
        ],
    },
    Tool {
        name: "swipe",
        method: "swipe",
        description: "Swipe from (x1,y1) to (x2,y2).",
        read_only: false,
        params: &[
            ("x1", "integer", "", true),
            ("y1", "integer", "", true),
            ("x2", "integer", "", true),
            ("y2", "integer", "", true),
            ("duration_ms", "integer", "Default 300.", false),
            DEVICE,
        ],
    },
    Tool { name: "type_text", method: "text", description: "Type ASCII text into the focused field.", read_only: false, params: &[("text", "string", "Text to type.", true), DEVICE] },
    Tool {
        name: "press_key",
        method: "key",
        description: "Press a key: home, back, enter, recents, power, volume_up, volume_down, delete, tab, or any KEYCODE_ name.",
        read_only: false,
        params: &[("key", "string", "Key name.", true), DEVICE],
    },
    Tool { name: "launch_app", method: "launch", description: "Start an app by package name.", read_only: false, params: &[("package", "string", "e.g. com.android.settings", true), DEVICE] },
    Tool { name: "stop_app", method: "stop", description: "Force-stop an app.", read_only: false, params: &[("package", "string", "Package name.", true), DEVICE] },
    Tool { name: "shell", method: "shell", description: "Run an adb shell command on the phone and return its output.", read_only: false, params: &[("command", "string", "Shell command.", true), DEVICE] },
    Tool { name: "install_apk", method: "install", description: "Install an APK file from this computer.", read_only: false, params: &[("path", "string", "Absolute path to the .apk.", true), DEVICE] },
];

fn tool_list(read_only: bool) -> Value {
    let tools: Vec<Value> = TOOLS
        .iter()
        .filter(|t| t.read_only || !read_only)
        .map(|t| {
            let mut props = serde_json::Map::new();
            let mut required = Vec::new();
            for (name, ty, desc, req) in t.params {
                props.insert(name.to_string(), json!({ "type": ty, "description": desc }));
                if *req {
                    required.push(*name);
                }
            }
            json!({
                "name": t.name,
                "description": t.description,
                "inputSchema": { "type": "object", "properties": props, "required": required },
                "annotations": { "readOnlyHint": t.read_only },
            })
        })
        .collect();
    json!({ "tools": tools })
}

fn text(s: impl Into<String>) -> Value {
    json!({ "type": "text", "text": s.into() })
}

async fn call_tool(client: &mut Option<Client>, read_only: bool, name: &str, args: Value) -> Value {
    let Some(tool) = TOOLS.iter().find(|t| t.name == name && (t.read_only || !read_only)) else {
        return json!({ "content": [text(format!("Unknown tool: {name}"))], "isError": true });
    };
    if client.is_none() {
        match crate::connect().await {
            Ok(c) => *client = Some(c),
            Err(e) => return json!({ "content": [text(format!("{e:#}"))], "isError": true }),
        }
    }
    let params = if args.is_object() { args } else { json!({}) };
    let result = client.as_mut().unwrap().call(tool.method, params).await;
    match result {
        Ok(v) if tool.method == "screenshot" => json!({
            "content": [{ "type": "image", "data": v["png_base64"], "mimeType": "image/png" }]
        }),
        Ok(Value::Bool(true)) => json!({ "content": [text("ok")] }),
        Ok(Value::String(s)) => json!({ "content": [text(s)] }),
        Ok(v) if tool.method == "shell" => json!({
            "content": [text(v["output"].as_str().unwrap_or(""))],
            "isError": v["code"].as_i64().unwrap_or(0) != 0,
        }),
        Ok(v) => json!({ "content": [text(serde_json::to_string_pretty(&v).unwrap_or_default())] }),
        Err(e) => {
            // The app may have restarted; reconnect on the next call.
            *client = None;
            json!({ "content": [text(format!("{e:#}"))], "isError": true })
        }
    }
}

pub async fn serve(read_only: bool) -> Result<()> {
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut out = tokio::io::stdout();
    let mut client: Option<Client> = None;
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(msg) = serde_json::from_str::<Value>(&line) else {
            write(&mut out, json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": "Parse error" } })).await?;
            continue;
        };
        let Some(id) = msg.get("id").cloned() else { continue }; // notifications need no reply
        let method = msg["method"].as_str().unwrap_or("");
        let result = match method {
            "initialize" => Ok(json!({
                "protocolVersion": msg["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18"),
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "pairspan", "version": env!("CARGO_PKG_VERSION") },
                "instructions": "Controls Android phones paired with the Pairspan app. Use ui_elements to find targets, then tap_element or tap. Take a screenshot when the UI tree is not enough."
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(tool_list(read_only)),
            "tools/call" => {
                let name = msg["params"]["name"].as_str().unwrap_or("");
                Ok(call_tool(&mut client, read_only, name, msg["params"]["arguments"].clone()).await)
            }
            _ => Err(json!({ "code": -32601, "message": format!("Method not found: {method}") })),
        };
        let reply = match result {
            Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
            Err(e) => json!({ "jsonrpc": "2.0", "id": id, "error": e }),
        };
        write(&mut out, reply).await?;
    }
    Ok(())
}

async fn write(out: &mut tokio::io::Stdout, v: Value) -> Result<()> {
    let mut b = serde_json::to_vec(&v)?;
    b.push(b'\n');
    out.write_all(&b).await?;
    out.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_hides_write_tools() {
        let all = tool_list(false)["tools"].as_array().unwrap().len();
        let ro = tool_list(true);
        let names: Vec<&str> = ro["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"screenshot") && !names.contains(&"tap") && !names.contains(&"shell"));
        assert!(names.len() < all);
    }

    #[test]
    fn schemas_list_required_params() {
        let v = tool_list(false);
        let tap = v["tools"].as_array().unwrap().iter().find(|t| t["name"] == "tap").unwrap();
        assert_eq!(tap["inputSchema"]["required"], json!(["x", "y"]));
    }
}
