//! Pairspan v3 gateway. See docs/protocol-v3.md.

pub mod hub;
pub mod metrics;
pub mod proto;
mod session;

use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{ConnectInfo, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use hub::Hub;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_FRAME: usize = (1 << 20) + proto::HEADER_LEN;

#[derive(Debug, Clone, Default)]
pub struct Config {
    /// Shared secret clients must present in `hello.token`; open gateway when `None`.
    pub token: Option<String>,
    /// Read the client address from `X-Forwarded-For` (only behind a trusted reverse proxy).
    pub trust_proxy: bool,
    pub max_connections: usize,
    /// Browser origins allowed to open a socket. Native clients send no Origin and are always allowed.
    pub allowed_origins: Vec<String>,
    pub docs_root: Option<PathBuf>,
}

impl Config {
    pub fn from_env() -> Self {
        let var = |k: &str| std::env::var(k).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        Self {
            token: var("GATEWAY_TOKEN"),
            trust_proxy: var("TRUST_PROXY").as_deref() == Some("1"),
            max_connections: var("MAX_CONNECTIONS").and_then(|v| v.parse().ok()).unwrap_or(10_000),
            allowed_origins: var("ALLOWED_ORIGINS")
                .map(|v| v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
                .unwrap_or_default(),
            docs_root: var("DOCS_ROOT").map(PathBuf::from),
        }
    }
}

#[derive(Clone)]
struct AppState {
    hub: Arc<Hub>,
    cfg: Arc<Config>,
}

pub fn router(hub: Arc<Hub>, cfg: Config) -> Router {
    let docs = cfg.docs_root.clone();
    let state = AppState { hub, cfg: Arc::new(cfg) };
    let router = Router::new()
        .route("/ws", get(ws_upgrade))
        .route("/health", get(health))
        .route("/metrics", get(metrics))
        .with_state(state);
    match docs {
        Some(root) => router.fallback_service(tower_http::services::ServeDir::new(root)),
        None => router,
    }
}

/// Periodically drops expired rooms, temporary links and rate-limit windows.
pub fn spawn_sweeper(hub: Arc<Hub>) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(30));
        loop {
            tick.tick().await;
            hub.sweep();
        }
    });
}

fn client_ip(cfg: &Config, headers: &HeaderMap, addr: SocketAddr) -> IpAddr {
    if cfg.trust_proxy {
        if let Some(ip) = headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').next())
            .and_then(|v| v.trim().parse().ok())
        {
            return ip;
        }
    }
    addr.ip()
}

async fn ws_upgrade(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Response {
    if let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
        if !state.cfg.allowed_origins.iter().any(|o| o == origin) {
            return StatusCode::FORBIDDEN.into_response();
        }
    }
    if state.hub.online() >= state.cfg.max_connections {
        return (StatusCode::SERVICE_UNAVAILABLE, "gateway full").into_response();
    }
    let ip = client_ip(&state.cfg, &headers, addr);
    ws.max_message_size(MAX_FRAME)
        .max_frame_size(MAX_FRAME)
        .on_upgrade(move |socket| session::run(socket, state.hub, state.cfg, ip))
}

async fn health(State(state): State<AppState>) -> impl IntoResponse {
    Json(serde_json::json!({
        "ok": true,
        "service": "pairspan-gateway",
        "version": VERSION,
        "protocol": proto::PROTOCOL,
        "connections": state.hub.online(),
        "rooms": state.hub.open_rooms(),
    }))
}

async fn metrics(State(state): State<AppState>) -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        state.hub.metrics.render(state.hub.online(), state.hub.open_rooms()),
    )
}
