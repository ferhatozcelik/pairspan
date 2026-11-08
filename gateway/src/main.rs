use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio::sync::broadcast;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DeviceRole {
    Pc,
    Phone,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivePairing {
    pub pair_key: String, // "{pc_id}::{hardware_serial}"
    pub pc_id: String,
    pub pc_os: String, // "macos" | "windows" | "linux"
    pub hardware_serial: String,
    pub phone_model: String,
    pub pair_token: String,
    pub canonical_endpoint: String, // Always "127.0.0.1:44755"
    pub phone_wlan_ip: Option<String>,
    pub deduplicated_count: usize,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LifecycleControlMessage {
    /// Sent by Desktop (macOS/Win/Linux) or Android Phone on startup or network reconnect
    RegisterSession {
        role: DeviceRole,
        pc_id: String,
        #[serde(default)]
        pc_os: String,
        hardware_serial: String,
        #[serde(default)]
        phone_model: String,
        pair_token: String,
        phone_wlan_ip: Option<String>,
    },
    /// Keep-alive heartbeat so the connection never drops silently
    Heartbeat {
        pair_key: String,
        pair_token: String,
    },
    /// Sent when user removes the phone on PC OR removes the PC on the phone
    PairRevoke {
        pc_id: String,
        hardware_serial: String,
        pair_token: String,
        initiator: DeviceRole,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GatewayEvent {
    SessionBound {
        pair_key: String,
        canonical_endpoint: String,
        deduplicated_previous: bool,
        deduplicated_count: usize,
    },
    SessionRejected {
        code: String,
        message: String,
    },
    PairingRevoked {
        pair_key: String,
        initiator: DeviceRole,
        reason: String,
    },
    HeartbeatAck {
        pair_key: String,
        timestamp: String,
    },
}

pub struct GatewayState {
    /// Active pairings keyed by `{pc_id}::{hardware_serial}` (guarantees single device identity)
    pub active_pairings: HashMap<String, ActivePairing>,
    /// Explicitly revoked tokens (cannot auto-reconnect once removed by PC or Phone)
    pub revoked_tokens: HashSet<String>,
    /// Explicitly revoked pair keys
    pub revoked_pair_keys: HashSet<String>,
    /// Broadcast channel for instant peer notifications (e.g., mutual unpair kill-switch)
    pub tx: broadcast::Sender<GatewayEvent>,
}

impl GatewayState {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self {
            active_pairings: HashMap::new(),
            revoked_tokens: HashSet::new(),
            revoked_pair_keys: HashSet::new(),
            tx,
        }
    }

    pub fn process_message(&mut self, msg: LifecycleControlMessage) -> GatewayEvent {
        match msg {
            LifecycleControlMessage::RegisterSession {
                role: _,
                pc_id,
                pc_os,
                hardware_serial,
                phone_model,
                pair_token,
                phone_wlan_ip,
            } => {
                let pair_key = format!("{}::{}", pc_id, hardware_serial);

                // Strict Lifecycle Check: If user removed the pairing on PC or Phone, reject reconnect
                if self.revoked_tokens.contains(&pair_token)
                    || self.revoked_pair_keys.contains(&pair_key)
                {
                    return GatewayEvent::SessionRejected {
                        code: "REVOKED".to_string(),
                        message: "Pairing was removed on PC or Phone. Auto-reconnect is permanently disabled until re-paired via USB Setup.".to_string(),
                    };
                }

                // Single-Identity Deduplication: Replace any stale Wi-Fi session for the same hardware serial
                let existing = self.active_pairings.get(&pair_key).cloned();
                let deduplicated_previous = existing.is_some();
                let deduplicated_count = existing
                    .as_ref()
                    .map(|e| e.deduplicated_count + 1)
                    .unwrap_or(0);

                let token = if pair_token.is_empty() {
                    format!("ps_{}", Uuid::new_v4().simple())
                } else {
                    pair_token
                };

                let record = ActivePairing {
                    pair_key: pair_key.clone(),
                    pc_id,
                    pc_os: if pc_os.is_empty() {
                        existing.as_ref().map(|e| e.pc_os.clone()).unwrap_or_else(|| "desktop".to_string())
                    } else {
                        pc_os
                    },
                    hardware_serial,
                    phone_model: if phone_model.is_empty() {
                        existing.as_ref().map(|e| e.phone_model.clone()).unwrap_or_else(|| "Android Phone".to_string())
                    } else {
                        phone_model
                    },
                    pair_token: token,
                    canonical_endpoint: "127.0.0.1:44755".to_string(),
                    phone_wlan_ip: phone_wlan_ip.or_else(|| existing.and_then(|e| e.phone_wlan_ip)),
                    deduplicated_count,
                    updated_at: chrono::Utc::now().to_rfc3339(),
                };

                self.active_pairings.insert(pair_key.clone(), record);

                GatewayEvent::SessionBound {
                    pair_key,
                    canonical_endpoint: "127.0.0.1:44755".to_string(),
                    deduplicated_previous,
                    deduplicated_count,
                }
            }
            LifecycleControlMessage::Heartbeat {
                pair_key,
                pair_token,
            } => {
                if self.revoked_tokens.contains(&pair_token)
                    || self.revoked_pair_keys.contains(&pair_key)
                {
                    return GatewayEvent::SessionRejected {
                        code: "REVOKED".to_string(),
                        message: "Pairing revoked.".to_string(),
                    };
                }
                if let Some(p) = self.active_pairings.get_mut(&pair_key) {
                    p.updated_at = chrono::Utc::now().to_rfc3339();
                }
                GatewayEvent::HeartbeatAck {
                    pair_key,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                }
            }
            LifecycleControlMessage::PairRevoke {
                pc_id,
                hardware_serial,
                pair_token,
                initiator,
            } => {
                let pair_key = format!("{}::{}", pc_id, hardware_serial);
                if !pair_token.is_empty() {
                    self.revoked_tokens.insert(pair_token);
                }
                self.revoked_pair_keys.insert(pair_key.clone());
                self.active_pairings.remove(&pair_key);

                let evt = GatewayEvent::PairingRevoked {
                    pair_key,
                    initiator,
                    reason: "User explicitly removed pairing on PC or Phone. Auto-reconnect permanently terminated.".to_string(),
                };
                let _ = self.tx.send(evt.clone());
                evt
            }
        }
    }
}

type SharedGateway = Arc<Mutex<GatewayState>>;

#[derive(Serialize)]
struct HealthResponse {
    service: &'static str,
    engine: &'static str,
    version: &'static str,
    active_pairings: usize,
    revoked_tokens: usize,
}

async fn health_handler(State(state): State<SharedGateway>) -> Json<HealthResponse> {
    let guard = state.lock().unwrap();
    Json(HealthResponse {
        service: "pairspan-gateway-api",
        engine: "rust-axum-tokio",
        version: "1.1.0",
        active_pairings: guard.active_pairings.len(),
        revoked_tokens: guard.revoked_tokens.len(),
    })
}

async fn list_pairings_handler(State(state): State<SharedGateway>) -> Json<Vec<ActivePairing>> {
    let guard = state.lock().unwrap();
    Json(guard.active_pairings.values().cloned().collect())
}

async fn control_api_handler(
    State(state): State<SharedGateway>,
    Json(payload): Json<LifecycleControlMessage>,
) -> (StatusCode, Json<GatewayEvent>) {
    let mut guard = state.lock().unwrap();
    let event = guard.process_message(payload);
    let status = match &event {
        GatewayEvent::SessionRejected { .. } => StatusCode::FORBIDDEN,
        _ => StatusCode::OK,
    };
    (status, Json(event))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<SharedGateway>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws_socket(socket, state))
}

async fn handle_ws_socket(socket: WebSocket, state: SharedGateway) {
    let (mut sender, mut receiver) = socket.split();
    let mut bcast_rx = {
        let guard = state.lock().unwrap();
        guard.tx.subscribe()
    };

    loop {
        tokio::select! {
            Some(Ok(msg)) = receiver.next() => {
                if let Message::Text(text) = msg {
                    if let Ok(parsed) = serde_json::from_str::<LifecycleControlMessage>(&text) {
                        let reply = {
                            let mut guard = state.lock().unwrap();
                            guard.process_message(parsed)
                        };
                        if let Ok(json) = serde_json::to_string(&reply) {
                            let _ = sender.send(Message::Text(json)).await;
                        }
                    }
                }
            }
            Ok(bcast_evt) = bcast_rx.recv() => {
                if let Ok(json) = serde_json::to_string(&bcast_evt) {
                    if sender.send(Message::Text(json)).await.is_err() {
                        break;
                    }
                }
            }
            else => break,
        }
    }
}

pub fn build_router(state: SharedGateway) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/api/v1/pairings", get(list_pairings_handler))
        .route("/api/v1/lifecycle", post(control_api_handler))
        .route("/ws", get(ws_handler))
        .with_state(state)
}

#[tokio::main]
async fn main() {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(44756);

    let state = Arc::new(Mutex::new(GatewayState::new()));
    let app = build_router(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("[Pairspan Rust API & Gateway] Listening on http://{} and ws://{}/ws", addr, addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lifecycle_single_identity_and_mutual_revocation() {
        let mut gw = GatewayState::new();

        // 1. Initial USB/QR registration binds single identity
        let evt1 = gw.process_message(LifecycleControlMessage::RegisterSession {
            role: DeviceRole::Pc,
            pc_id: "macbook-pro".to_string(),
            pc_os: "macos".to_string(),
            hardware_serial: "38291FDJH0019A".to_string(),
            phone_model: "Pixel 8 Pro".to_string(),
            pair_token: "ps_token_123".to_string(),
            phone_wlan_ip: Some("192.168.1.42".to_string()),
        });
        assert!(matches!(
            evt1,
            GatewayEvent::SessionBound {
                deduplicated_previous: false,
                ..
            }
        ));

        // 2. Reconnect after Wi-Fi drop / IP change deduplicates without creating a second device ID
        let evt2 = gw.process_message(LifecycleControlMessage::RegisterSession {
            role: DeviceRole::Phone,
            pc_id: "macbook-pro".to_string(),
            pc_os: "macos".to_string(),
            hardware_serial: "38291FDJH0019A".to_string(),
            phone_model: "Pixel 8 Pro".to_string(),
            pair_token: "ps_token_123".to_string(),
            phone_wlan_ip: Some("192.168.1.99".to_string()),
        });
        assert!(matches!(
            evt2,
            GatewayEvent::SessionBound {
                deduplicated_previous: true,
                deduplicated_count: 1,
                ..
            }
        ));
        assert_eq!(gw.active_pairings.len(), 1);

        // 3. User removes phone on PC (or PC on Phone) -> permanent revocation
        let evt3 = gw.process_message(LifecycleControlMessage::PairRevoke {
            pc_id: "macbook-pro".to_string(),
            hardware_serial: "38291FDJH0019A".to_string(),
            pair_token: "ps_token_123".to_string(),
            initiator: DeviceRole::Pc,
        });
        assert!(matches!(evt3, GatewayEvent::PairingRevoked { .. }));
        assert_eq!(gw.active_pairings.len(), 0);

        // 4. Subsequent auto-reconnect attempt is rejected with REVOKED
        let evt4 = gw.process_message(LifecycleControlMessage::RegisterSession {
            role: DeviceRole::Phone,
            pc_id: "macbook-pro".to_string(),
            pc_os: "macos".to_string(),
            hardware_serial: "38291FDJH0019A".to_string(),
            phone_model: "Pixel 8 Pro".to_string(),
            pair_token: "ps_token_123".to_string(),
            phone_wlan_ip: Some("192.168.1.99".to_string()),
        });
        assert!(matches!(
            evt4,
            GatewayEvent::SessionRejected { ref code, .. } if code == "REVOKED"
        ));
    }
}
