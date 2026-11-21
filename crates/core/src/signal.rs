//! Supabase Realtime (Phoenix channels over one WebSocket): presence tells the two ends of a
//! link that the other is around, broadcast carries pairing hellos and WebRTC offers/answers.
//! Nothing secret travels here; the data itself goes peer to peer (see `rtc.rs`).

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

/// The public Pairspan project. Self-hosters point `PAIRSPAN_SUPABASE_URL` / `_KEY` at their own.
const SUPABASE_URL: &str = "https://jcqkznspriizdmsbjvvj.supabase.co";
/// A publishable key: meant to ship in apps; it only reaches public Realtime channels.
const SUPABASE_KEY: &str = "sb_publishable_WFPzgHprFKVKb7CKApBnWQ_fnp4gVcm";

pub fn endpoint() -> String {
    let url = std::env::var("PAIRSPAN_SUPABASE_URL").unwrap_or_else(|_| SUPABASE_URL.into());
    let key = std::env::var("PAIRSPAN_SUPABASE_KEY").unwrap_or_else(|_| SUPABASE_KEY.into());
    let ws = url.trim_end_matches('/').replacen("https://", "wss://", 1).replacen("http://", "ws://", 1);
    format!("{ws}/realtime/v1/websocket?apikey={key}&vsn=1.0.0")
}

#[derive(Debug)]
pub enum Incoming {
    Broadcast { topic: String, payload: Value },
    Presence { topic: String, key: String, online: bool },
}

pub struct Signal {
    out: mpsc::UnboundedSender<Message>,
    next_ref: AtomicU64,
    /// join ref → (topic, presence key); the presence `track` goes out once the join is acknowledged.
    joins: Mutex<HashMap<String, (String, String)>>,
}

impl Signal {
    /// Connects and returns the handle plus the event stream; the stream ends when the socket does.
    pub async fn connect() -> Result<(Arc<Self>, mpsc::UnboundedReceiver<Incoming>)> {
        let (ws, _) = tokio::time::timeout(Duration::from_secs(15), tokio_tungstenite::connect_async(endpoint()))
            .await
            .context("connection timed out")?
            .context("cannot reach the Pairspan signalling service")?;
        let (mut sink, mut stream) = ws.split();
        let (out, mut out_rx) = mpsc::unbounded_channel::<Message>();
        let (events, events_rx) = mpsc::unbounded_channel();
        let signal = Arc::new(Self { out, next_ref: AtomicU64::new(1), joins: Mutex::new(HashMap::new()) });

        tokio::spawn(async move {
            while let Some(m) = out_rx.recv().await {
                if sink.send(m).await.is_err() {
                    break;
                }
            }
            let _ = sink.close().await;
        });

        let me = signal.clone();
        tokio::spawn(async move {
            let mut beat = tokio::time::interval(Duration::from_secs(25));
            loop {
                tokio::select! {
                    _ = beat.tick() => { me.push("phoenix", "heartbeat", json!({})); }
                    m = stream.next() => match m {
                        Some(Ok(Message::Text(text))) => {
                            if let Ok(v) = serde_json::from_str::<Value>(&text) {
                                me.dispatch(v, &events);
                            }
                        }
                        Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                        Some(Ok(_)) => {}
                    },
                }
                if events.is_closed() {
                    break;
                }
            }
            // Dropping the last sender ends the caller's event stream; closing `out` stops the writer.
            drop(events);
            me.joins.lock().unwrap().clear();
            let _ = me.out.send(Message::Close(None));
        });
        Ok((signal, events_rx))
    }

    fn push(&self, topic: &str, event: &str, payload: Value) -> String {
        let r = self.next_ref.fetch_add(1, Ordering::Relaxed).to_string();
        let msg = json!({ "topic": topic, "event": event, "payload": payload, "ref": r });
        let _ = self.out.send(Message::Text(msg.to_string().into()));
        r
    }

    /// Joins `pairspan:<name>` and, once accepted, announces ourselves under `key` in its presence.
    pub fn join(&self, name: &str, key: &str) {
        let topic = format!("realtime:pairspan:{name}");
        let config = json!({ "config": { "broadcast": { "self": false }, "presence": { "key": key }, "private": false } });
        let r = self.push(&topic, "phx_join", config);
        self.joins.lock().unwrap().insert(r, (topic, key.to_string()));
    }

    pub fn leave(&self, name: &str) {
        self.push(&format!("realtime:pairspan:{name}"), "phx_leave", json!({}));
    }

    pub fn broadcast(&self, name: &str, payload: Value) {
        let body = json!({ "type": "broadcast", "event": "sig", "payload": payload });
        self.push(&format!("realtime:pairspan:{name}"), "broadcast", body);
    }

    fn dispatch(&self, v: Value, events: &mpsc::UnboundedSender<Incoming>) {
        let Some(topic) = v["topic"].as_str().and_then(|t| t.strip_prefix("realtime:pairspan:")) else { return };
        let topic = topic.to_string();
        match v["event"].as_str().unwrap_or_default() {
            "phx_reply" => {
                let joined = v["ref"].as_str().and_then(|r| self.joins.lock().unwrap().remove(r));
                if let Some((full, _key)) = joined.filter(|_| v["payload"]["status"] == "ok") {
                    let track = json!({ "type": "presence", "event": "track", "payload": {} });
                    self.push(&full, "presence", track);
                }
            }
            "broadcast" => {
                let _ = events.send(Incoming::Broadcast { topic, payload: v["payload"]["payload"].clone() });
            }
            "presence_state" => {
                for key in v["payload"].as_object().into_iter().flat_map(|o| o.keys()) {
                    let _ = events.send(Incoming::Presence { topic: topic.clone(), key: key.clone(), online: true });
                }
            }
            "presence_diff" => {
                for (field, online) in [("joins", true), ("leaves", false)] {
                    for key in v["payload"][field].as_object().into_iter().flat_map(|o| o.keys()) {
                        let _ = events.send(Incoming::Presence { topic: topic.clone(), key: key.clone(), online });
                    }
                }
            }
            _ => {}
        }
    }
}
