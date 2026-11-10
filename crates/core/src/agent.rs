//! The PC agent: one gateway connection, an encrypted session per phone, and a local
//! TCP port per phone that `adb connect` uses (bytes are tunnelled to the phone's adbd).

use crate::crypto::{self, Initiator, Opener, Sealer};
use crate::identity::{decode_public, Identity, PeerId};
use crate::store::{Device, Settings, Store};
use crate::wire::{self, FromGateway, Role, ToGateway, CH_CLOSE, CH_DATA, CH_OPEN, CH_RPC, MAX_CHUNK};
use anyhow::{anyhow, bail, Context, Result};
use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use p256::SecretKey;
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc, Notify};
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;

const PAIR_TTL: Duration = Duration::from_secs(300);
const MAX_LOG: usize = 400;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    /// Devices, connection or pairing state changed; UIs re-read `status()`.
    Changed,
    Log { line: String },
    Paired { device: Device },
    PairingFailed { reason: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct DeviceStatus {
    #[serde(flatten)]
    pub device: Device,
    pub serial: String,
    pub online: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub version: &'static str,
    pub id: PeerId,
    pub gateway: String,
    pub connected: bool,
    pub automation: bool,
    pub devices: Vec<DeviceStatus>,
    pub pairing: Option<PairingInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PairingInfo {
    pub code: String,
    pub qr: String,
    pub qr_svg: String,
    pub expires_in: u64,
}

struct Pairing {
    secret: String,
    room: String,
    expires: Instant,
    phone: Option<(PeerId, [u8; 33])>,
}

/// Per-phone link state. Sends hold the async `tx` lock so frame counters go out in order.
struct Peer {
    id: PeerId,
    tx: tokio::sync::Mutex<Option<Sealer>>,
    rx: Mutex<Option<Opener>>,
    pending: Mutex<Option<Initiator>>,
    streams: Mutex<HashMap<u32, mpsc::Sender<Bytes>>>,
    next_stream: AtomicU32,
    online: AtomicBool,
    ready: AtomicBool,
}

impl Peer {
    fn new(id: PeerId) -> Self {
        Self {
            id,
            tx: tokio::sync::Mutex::new(None),
            rx: Mutex::new(None),
            pending: Mutex::new(None),
            streams: Mutex::new(HashMap::new()),
            next_stream: AtomicU32::new(1),
            online: AtomicBool::new(false),
            ready: AtomicBool::new(false),
        }
    }
}

pub struct Agent {
    me: Weak<Agent>,
    identity: Identity,
    secret: SecretKey,
    host_name: String,
    store: Mutex<Store>,
    peers: Mutex<HashMap<PeerId, Arc<Peer>>>,
    out: Mutex<Option<mpsc::Sender<Message>>>,
    pairing: Mutex<Option<Pairing>>,
    listeners: Mutex<HashMap<PeerId, JoinHandle<()>>>,
    connected: AtomicBool,
    reconnect: Notify,
    logs: Mutex<VecDeque<String>>,
    events: broadcast::Sender<Event>,
    /// Run `adb connect/disconnect` automatically when a phone comes and goes.
    manage_adb: bool,
}

pub struct Options {
    pub manage_adb: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { manage_adb: true }
    }
}

fn now_string() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    secs.to_string()
}

fn host_name() -> String {
    std::process::Command::new("hostname")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().trim_end_matches(".local").to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "PC".to_string())
}

impl Agent {
    pub fn start(dir: &Path, options: Options) -> Result<Arc<Self>> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let identity = Identity::load_or_create(&dir.join("identity.key"))?;
        let secret = SecretKey::from(identity.signing_key().as_nonzero_scalar());
        let store = Store::open(dir)?;
        let (events, _) = broadcast::channel(256);
        let agent = Arc::new_cyclic(|me| Self {
            me: me.clone(),
            identity,
            secret,
            host_name: host_name(),
            store: Mutex::new(store),
            peers: Mutex::new(HashMap::new()),
            out: Mutex::new(None),
            pairing: Mutex::new(None),
            listeners: Mutex::new(HashMap::new()),
            connected: AtomicBool::new(false),
            reconnect: Notify::new(),
            logs: Mutex::new(VecDeque::new()),
            events,
            manage_adb: options.manage_adb,
        });
        let devices: Vec<Device> = agent.store.lock().unwrap().devices.clone();
        for d in devices {
            agent.spawn_listener(&d);
        }
        tokio::spawn(agent.clone().gateway_loop());
        Ok(agent)
    }

    pub fn id(&self) -> PeerId {
        self.identity.id()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.events.subscribe()
    }

    pub fn log(&self, line: impl Into<String>) {
        let line = line.into();
        tracing::info!("{line}");
        {
            let mut logs = self.logs.lock().unwrap();
            if logs.len() >= MAX_LOG {
                logs.pop_front();
            }
            logs.push_back(line.clone());
        }
        let _ = self.events.send(Event::Log { line });
    }

    pub fn logs(&self) -> Vec<String> {
        self.logs.lock().unwrap().iter().cloned().collect()
    }

    fn changed(&self) {
        let _ = self.events.send(Event::Changed);
    }

    pub fn settings(&self) -> Settings {
        self.store.lock().unwrap().settings.clone()
    }

    pub fn update_settings(&self, settings: Settings) -> Result<()> {
        let reconnect = {
            let mut store = self.store.lock().unwrap();
            let reconnect = store.settings.gateway_url != settings.gateway_url
                || store.settings.gateway_token != settings.gateway_token;
            store.settings = settings;
            store.save_settings()?;
            reconnect
        };
        if reconnect {
            self.reconnect.notify_one();
        }
        self.changed();
        Ok(())
    }

    pub fn automation_enabled(&self) -> bool {
        self.store.lock().unwrap().settings.automation_enabled
    }

    pub fn status(&self) -> Status {
        let store = self.store.lock().unwrap();
        let peers = self.peers.lock().unwrap();
        let devices = store
            .devices
            .iter()
            .map(|d| DeviceStatus {
                serial: d.serial(),
                online: peers.get(&d.id).is_some_and(|p| p.ready.load(Ordering::Relaxed)),
                device: d.clone(),
            })
            .collect();
        Status {
            version: env!("CARGO_PKG_VERSION"),
            id: self.identity.id(),
            gateway: store.settings.gateway_url.clone(),
            connected: self.connected.load(Ordering::Relaxed),
            automation: store.settings.automation_enabled,
            devices,
            pairing: self.pairing_info(&store.settings.gateway_url),
        }
    }

    fn pairing_info(&self, gateway: &str) -> Option<PairingInfo> {
        let pairing = self.pairing.lock().unwrap();
        let p = pairing.as_ref().filter(|p| p.expires > Instant::now())?;
        let qr = wire::qr_payload(gateway, &p.secret, self.identity.id());
        Some(PairingInfo {
            code: wire::display_secret(&p.secret),
            qr_svg: wire::qr_svg(&qr),
            qr,
            expires_in: p.expires.saturating_duration_since(Instant::now()).as_secs(),
        })
    }

    pub fn devices(&self) -> Vec<DeviceStatus> {
        self.status().devices
    }

    // ---------------------------------------------------------------- gateway

    fn out(&self) -> Result<mpsc::Sender<Message>> {
        self.out.lock().unwrap().clone().ok_or_else(|| anyhow!("Not connected to the Pairspan gateway."))
    }

    async fn send_text(&self, msg: ToGateway<'_>) -> Result<()> {
        self.out()?.send(Message::Text(msg.json().into())).await.map_err(|_| anyhow!("gateway connection closed"))
    }

    async fn send_relay(&self, peer: PeerId, payload: &[u8]) -> Result<()> {
        self.out()?
            .send(Message::Binary(wire::relay_frame(peer, payload).into()))
            .await
            .map_err(|_| anyhow!("gateway connection closed"))
    }

    async fn gateway_loop(self: Arc<Self>) {
        let mut backoff = 1u64;
        loop {
            let settings = self.settings();
            match self.clone().session(&settings).await {
                Ok(()) => backoff = 1,
                Err(e) => self.log(format!("Gateway: {e:#}")),
            }
            *self.out.lock().unwrap() = None;
            self.connected.store(false, Ordering::Relaxed);
            let ids: Vec<PeerId> = self.peers.lock().unwrap().keys().copied().collect();
            for id in ids {
                self.peer_down(id).await;
            }
            self.changed();
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(backoff)) => {}
                _ = self.reconnect.notified() => {}
            }
            backoff = (backoff * 2).min(30);
        }
    }

    async fn session(self: Arc<Self>, settings: &Settings) -> Result<()> {
        let (ws, _) = tokio::time::timeout(Duration::from_secs(15), tokio_tungstenite::connect_async(&settings.gateway_url))
            .await
            .context("connection timed out")?
            .with_context(|| format!("cannot reach {}", settings.gateway_url))?;
        let (mut sink, mut stream) = ws.split();

        let nonce = match next_control(&mut stream).await? {
            FromGateway::Challenge { nonce, .. } => {
                use base64::Engine;
                base64::engine::general_purpose::STANDARD.decode(nonce).context("bad challenge")?
            }
            other => bail!("expected challenge, got {other:?}"),
        };
        let sig = self.identity.sign_challenge(&nonce);
        let public = self.identity.public_b64();
        let peers: Vec<PeerId> = self.store.lock().unwrap().devices.iter().map(|d| d.id).collect();
        let hello = ToGateway::Hello {
            role: Role::Pc,
            public: &public,
            sig: &sig,
            peers,
            token: settings.gateway_token.as_deref(),
        };
        sink.send(Message::Text(hello.json().into())).await?;
        let online = match next_control(&mut stream).await? {
            FromGateway::Welcome { online, .. } => online,
            FromGateway::Error { msg, .. } => bail!("gateway refused: {msg}"),
            other => bail!("expected welcome, got {other:?}"),
        };

        let (tx, mut rx) = mpsc::channel::<Message>(256);
        *self.out.lock().unwrap() = Some(tx);
        self.connected.store(true, Ordering::Relaxed);
        self.log(format!("Connected to {} as {}", settings.gateway_url, self.identity.id().short()));
        self.changed();

        let writer = tokio::spawn(async move {
            let mut ping = tokio::time::interval(Duration::from_secs(20));
            ping.tick().await;
            loop {
                let msg = tokio::select! {
                    m = rx.recv() => match m { Some(m) => m, None => break },
                    _ = ping.tick() => Message::Ping(Default::default()),
                };
                if sink.send(msg).await.is_err() {
                    break;
                }
            }
            let _ = sink.close().await;
        });

        for id in online {
            self.peer_up(id).await;
        }
        let room = self.pairing.lock().unwrap().as_ref().map(|p| p.room.clone());
        if let Some(room) = room {
            let _ = self.send_text(ToGateway::RoomOpen { room: &room }).await;
        }

        let result = loop {
            let msg = tokio::select! {
                m = stream.next() => m,
                _ = self.reconnect.notified() => break Ok(()),
            };
            match msg {
                Some(Ok(Message::Text(text))) => match serde_json::from_str::<FromGateway>(&text) {
                    Ok(m) => self.on_control(m).await,
                    Err(_) => tracing::debug!("ignoring unknown gateway message"),
                },
                Some(Ok(Message::Binary(frame))) => {
                    if let Some((src, payload)) = wire::parse_relay(&frame) {
                        self.on_frame(src, payload).await;
                    }
                }
                Some(Ok(Message::Close(frame))) => break Err(anyhow!("closed by gateway: {:?}", frame.map(|f| f.reason.to_string()))),
                Some(Ok(_)) => {}
                Some(Err(e)) => break Err(e.into()),
                None => break Err(anyhow!("connection lost")),
            }
        };
        writer.abort();
        result
    }

    async fn on_control(&self, msg: FromGateway) {
        match msg {
            FromGateway::Presence { id, online: true } => self.peer_up(id).await,
            FromGateway::Presence { id, online: false } => self.peer_down(id).await,
            FromGateway::RoomPeer { room, id, public, role } => {
                let psk = {
                    let mut pairing = self.pairing.lock().unwrap();
                    match pairing.as_mut() {
                        Some(p) if p.room == room && role == Role::Phone => match decode_public(&public) {
                            // The gateway names the peer; its id must be the hash of the key it hands us.
                            Ok(key) if PeerId::from_pubkey(&key) == id => {
                                p.phone = Some((id, key));
                                Some((crypto::pairing_psk(&p.secret), key))
                            }
                            _ => None,
                        },
                        _ => None,
                    }
                };
                if let Some((psk, key)) = psk {
                    self.log(format!("Pairing: phone {} joined, verifying…", id.short()));
                    if let Err(e) = self.handshake(id, &key, Some(psk)).await {
                        self.pairing_failed(format!("handshake could not start: {e}"));
                    }
                }
            }
            FromGateway::RoomOpened { .. } => self.changed(),
            FromGateway::Error { code, msg } => self.log(format!("Gateway error {code}: {msg}")),
            FromGateway::Challenge { .. } | FromGateway::Welcome { .. } => {}
        }
    }

    fn peer(&self, id: PeerId) -> Arc<Peer> {
        self.peers.lock().unwrap().entry(id).or_insert_with(|| Arc::new(Peer::new(id))).clone()
    }

    fn existing_peer(&self, id: &PeerId) -> Option<Arc<Peer>> {
        self.peers.lock().unwrap().get(id).cloned()
    }

    async fn peer_up(&self, id: PeerId) {
        let Some(device) = self.store.lock().unwrap().device(&id).cloned() else { return };
        let Ok(key) = decode_public(&device.public) else { return };
        self.peer(id).online.store(true, Ordering::Relaxed);
        if let Err(e) = self.handshake(id, &key, None).await {
            self.log(format!("{}: handshake failed to start: {e}", device.name));
        }
    }

    async fn peer_down(&self, id: PeerId) {
        let Some(peer) = self.existing_peer(&id) else { return };
        let was_ready = peer.ready.swap(false, Ordering::Relaxed);
        peer.online.store(false, Ordering::Relaxed);
        *peer.tx.lock().await = None;
        *peer.rx.lock().unwrap() = None;
        *peer.pending.lock().unwrap() = None;
        peer.streams.lock().unwrap().clear();
        if was_ready {
            let device = self.store.lock().unwrap().device(&id).cloned();
            if let Some(d) = device {
                self.log(format!("{} went offline", d.name));
                if self.manage_adb {
                    let serial = d.serial();
                    tokio::spawn(async move {
                        let _ = crate::adb::run(None, &["disconnect", &serial], Duration::from_secs(5)).await;
                    });
                }
            }
            self.changed();
        }
    }

    async fn handshake(&self, id: PeerId, key: &[u8; 33], psk: Option<[u8; 32]>) -> Result<()> {
        let (init, frame) = Initiator::start(&self.secret, key, psk)?;
        let peer = self.peer(id);
        peer.ready.store(false, Ordering::Relaxed);
        *peer.tx.lock().await = None;
        *peer.pending.lock().unwrap() = Some(init);
        self.send_relay(id, &frame).await
    }

    async fn on_frame(&self, src: PeerId, payload: &[u8]) {
        let Some(&kind) = payload.first() else { return };
        match kind {
            crypto::HS_RESP => self.on_handshake_response(src, payload).await,
            crypto::DATA => {
                let Some(peer) = self.existing_peer(&src) else { return };
                let opened = {
                    let mut rx = peer.rx.lock().unwrap();
                    rx.as_mut().map(|o| o.open(payload))
                };
                match opened {
                    Some(Ok(plain)) => self.on_plain(&peer, &plain).await,
                    Some(Err(e)) => {
                        self.log(format!("Dropping frame from {}: {e}; rekeying", src.short()));
                        self.peer_up(src).await;
                    }
                    None => {}
                }
            }
            crypto::REKEY => self.peer_up(src).await,
            _ => {}
        }
    }

    async fn on_handshake_response(&self, src: PeerId, payload: &[u8]) {
        let Some(peer) = self.existing_peer(&src) else { return };
        let Some(init) = peer.pending.lock().unwrap().take() else { return };
        let session = match init.finish(payload) {
            Ok(s) => s,
            Err(e) => {
                let pairing = self.pairing.lock().unwrap().as_ref().and_then(|p| p.phone).map(|(id, _)| id);
                if pairing == Some(src) {
                    self.pairing_failed(format!("{e}"));
                } else {
                    self.log(format!("Handshake with {} failed: {e}", src.short()));
                }
                return;
            }
        };
        *peer.rx.lock().unwrap() = Some(session.rx);
        *peer.tx.lock().await = Some(session.tx);
        peer.ready.store(true, Ordering::Relaxed);
        peer.online.store(true, Ordering::Relaxed);

        let finishing_pairing = {
            let pairing = self.pairing.lock().unwrap();
            pairing.as_ref().and_then(|p| p.phone).filter(|(id, _)| *id == src)
        };
        if let Some((id, key)) = finishing_pairing {
            if let Err(e) = self.complete_pairing(id, key).await {
                self.pairing_failed(format!("{e:#}"));
                return;
            }
        }

        let hello = serde_json::json!({ "m": "hello", "p": { "name": self.host_name, "os": std::env::consts::OS, "version": env!("CARGO_PKG_VERSION") } });
        let _ = self.send_channel(&peer, CH_RPC, 0, hello.to_string().as_bytes()).await;

        let device = self.store.lock().unwrap().device(&src).cloned();
        if let Some(d) = device {
            self.log(format!("{} is online (encrypted session ready)", d.name));
            if self.manage_adb {
                let serial = d.serial();
                tokio::spawn(async move {
                    // adb keeps a dropped TCP device listed as "offline" and answers a new `connect`
                    // with "already connected", so drop the stale entry first.
                    let _ = crate::adb::run(None, &["disconnect", &serial], Duration::from_secs(5)).await;
                    let _ = crate::adb::run(None, &["connect", &serial], Duration::from_secs(10)).await;
                });
            }
        }
        self.changed();
    }

    async fn on_plain(&self, peer: &Arc<Peer>, plain: &[u8]) {
        let Some((channel, stream, body)) = wire::parse_channel(plain) else { return };
        match channel {
            CH_RPC => {
                let Ok(msg) = serde_json::from_slice::<serde_json::Value>(body) else { return };
                match msg["m"].as_str() {
                    Some("hello") => self.on_phone_hello(peer.id, &msg["p"]),
                    Some("unpair") => {
                        self.log(format!("Phone {} removed this PC", peer.id.short()));
                        let _ = self.remove_device(peer.id, false).await;
                    }
                    _ => {}
                }
            }
            CH_DATA => {
                let sender = peer.streams.lock().unwrap().get(&stream).cloned();
                if let Some(s) = sender {
                    if s.send(Bytes::copy_from_slice(body)).await.is_err() {
                        peer.streams.lock().unwrap().remove(&stream);
                    }
                }
            }
            CH_CLOSE => {
                peer.streams.lock().unwrap().remove(&stream);
            }
            CH_OPEN => {
                // Phones never open streams towards the PC.
                let _ = self.send_channel(peer, CH_CLOSE, stream, b"refused").await;
            }
            _ => {}
        }
    }

    fn on_phone_hello(&self, id: PeerId, p: &serde_json::Value) {
        let mut store = self.store.lock().unwrap();
        let Some(d) = store.devices.iter_mut().find(|d| d.id == id) else { return };
        let text = |k: &str| p[k].as_str().map(|s| s.chars().take(64).collect::<String>());
        if let Some(name) = text("name").filter(|n| !n.is_empty()) {
            d.name = name;
        }
        if let Some(model) = text("model") {
            d.model = model;
        }
        if let Some(android) = text("android") {
            d.android = android;
        }
        let _ = store.save_devices();
        drop(store);
        self.changed();
    }

    async fn send_channel(&self, peer: &Peer, channel: u8, stream: u32, body: &[u8]) -> Result<()> {
        let plain = wire::channel_frame(channel, stream, body);
        let out = self.out()?;
        let mut tx = peer.tx.lock().await;
        let sealer = tx.as_mut().ok_or_else(|| anyhow!("phone is not connected"))?;
        let frame = sealer.seal(&plain)?;
        out.send(Message::Binary(wire::relay_frame(peer.id, &frame).into()))
            .await
            .map_err(|_| anyhow!("gateway connection closed"))
    }

    // ---------------------------------------------------------------- pairing

    pub async fn start_pairing(&self) -> Result<PairingInfo> {
        if !self.connected.load(Ordering::Relaxed) {
            bail!("Not connected to the Pairspan gateway ({}).", self.settings().gateway_url);
        }
        let secret = wire::new_pair_secret();
        let room = crypto::room_id(&secret);
        let old = self.pairing.lock().unwrap().replace(Pairing {
            secret,
            room: room.clone(),
            expires: Instant::now() + PAIR_TTL,
            phone: None,
        });
        if let Some(old) = old {
            let _ = self.send_text(ToGateway::RoomClose { room: &old.room }).await;
        }
        self.send_text(ToGateway::RoomOpen { room: &room }).await?;
        self.log("Pairing: waiting for a phone to scan the code");
        self.changed();
        self.pairing_info(&self.settings().gateway_url).context("pairing expired")
    }

    pub async fn cancel_pairing(&self) {
        let old = self.pairing.lock().unwrap().take();
        if let Some(old) = old {
            let _ = self.send_text(ToGateway::RoomClose { room: &old.room }).await;
            self.changed();
        }
    }

    fn pairing_failed(&self, reason: String) {
        self.pairing.lock().unwrap().take();
        self.log(format!("Pairing failed: {reason}"));
        let _ = self.events.send(Event::PairingFailed { reason });
        self.changed();
    }

    async fn complete_pairing(&self, id: PeerId, key: [u8; 33]) -> Result<()> {
        use base64::Engine;
        let device = {
            let mut store = self.store.lock().unwrap();
            store.devices.retain(|d| d.id != id);
            let device = Device {
                id,
                public: base64::engine::general_purpose::STANDARD.encode(key),
                name: "Android phone".into(),
                model: String::new(),
                android: String::new(),
                adb_port: store.next_adb_port(),
                paired_at: now_string(),
            };
            store.devices.push(device.clone());
            store.save_devices()?;
            device
        };
        let room = self.pairing.lock().unwrap().take().map(|p| p.room);
        if let Some(room) = room {
            let _ = self.send_text(ToGateway::RoomClose { room: &room }).await;
        }
        self.sync_peers().await?;
        self.spawn_listener(&device);
        self.log(format!("Paired with phone {}", id.short()));
        let _ = self.events.send(Event::Paired { device });
        Ok(())
    }

    async fn sync_peers(&self) -> Result<()> {
        let ids: Vec<PeerId> = self.store.lock().unwrap().devices.iter().map(|d| d.id).collect();
        self.send_text(ToGateway::Peers { ids }).await
    }

    /// Waits for the next completed pairing (or failure) for up to `timeout`.
    pub async fn wait_paired(&self, timeout: Duration) -> Result<Device> {
        let mut events = self.subscribe();
        let wait = async {
            loop {
                match events.recv().await {
                    Ok(Event::Paired { device }) => return Ok(device),
                    Ok(Event::PairingFailed { reason }) => bail!("pairing failed: {reason}"),
                    Ok(_) => {}
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(_) => bail!("agent stopped"),
                }
            }
        };
        tokio::time::timeout(timeout, wait).await.context("timed out waiting for the phone")?
    }

    /// Unpairs a phone. The gateway cuts the link at once; the phone is told first when it is online.
    pub async fn remove_device(&self, id: PeerId, notify_phone: bool) -> Result<Device> {
        if notify_phone {
            if let Some(peer) = self.existing_peer(&id) {
                if peer.ready.load(Ordering::Relaxed) {
                    let _ = self.send_channel(&peer, CH_RPC, 0, br#"{"m":"unpair"}"#).await;
                }
            }
        }
        let device = {
            let mut store = self.store.lock().unwrap();
            let pos = store.devices.iter().position(|d| d.id == id).context("unknown device")?;
            let d = store.devices.remove(pos);
            store.save_devices()?;
            d
        };
        let _ = self.sync_peers().await;
        self.peer_down(id).await;
        self.peers.lock().unwrap().remove(&id);
        if let Some(h) = self.listeners.lock().unwrap().remove(&id) {
            h.abort();
        }
        if self.manage_adb {
            let _ = crate::adb::run(None, &["disconnect", &device.serial()], Duration::from_secs(5)).await;
        }
        self.log(format!("Removed {}", device.name));
        self.changed();
        Ok(device)
    }

    // ---------------------------------------------------------------- ADB tunnel

    fn spawn_listener(&self, device: &Device) {
        let Some(agent) = self.me.upgrade() else { return };
        let (id, port, name) = (device.id, device.adb_port, device.name.clone());
        let handle = tokio::spawn(async move {
            let listener = match TcpListener::bind(("127.0.0.1", port)).await {
                Ok(l) => l,
                Err(e) => {
                    agent.log(format!("{name}: cannot listen on 127.0.0.1:{port}: {e}"));
                    return;
                }
            };
            loop {
                let Ok((sock, _)) = listener.accept().await else { continue };
                let agent = agent.clone();
                tokio::spawn(async move { agent.tunnel(id, sock).await });
            }
        });
        if let Some(old) = self.listeners.lock().unwrap().insert(device.id, handle) {
            old.abort();
        }
    }

    /// One local TCP connection (from the adb server) ⇄ one stream to the phone's adbd.
    async fn tunnel(self: Arc<Self>, id: PeerId, sock: TcpStream) {
        let Some(peer) = self.existing_peer(&id).filter(|p| p.ready.load(Ordering::Relaxed)) else {
            return;
        };
        let _ = sock.set_nodelay(true);
        let sid = peer.next_stream.fetch_add(1, Ordering::Relaxed);
        let (to_sock, mut from_phone) = mpsc::channel::<Bytes>(64);
        peer.streams.lock().unwrap().insert(sid, to_sock);
        if self.send_channel(&peer, CH_OPEN, sid, b"adb").await.is_err() {
            peer.streams.lock().unwrap().remove(&sid);
            return;
        }
        let (mut rd, mut wr) = sock.into_split();
        let writer = tokio::spawn(async move {
            while let Some(chunk) = from_phone.recv().await {
                if wr.write_all(&chunk).await.is_err() {
                    break;
                }
            }
            let _ = wr.shutdown().await;
        });
        let mut buf = vec![0u8; MAX_CHUNK];
        loop {
            match rd.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if self.send_channel(&peer, CH_DATA, sid, &buf[..n]).await.is_err() {
                        break;
                    }
                }
            }
            if !peer.streams.lock().unwrap().contains_key(&sid) {
                break;
            }
        }
        let still_open = peer.streams.lock().unwrap().remove(&sid).is_some();
        if still_open {
            let _ = self.send_channel(&peer, CH_CLOSE, sid, b"").await;
        }
        let _ = writer.await;
    }
}

async fn next_control<S>(stream: &mut S) -> Result<FromGateway>
where
    S: futures_util::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    let deadline = tokio::time::sleep(Duration::from_secs(15));
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            _ = &mut deadline => bail!("gateway did not answer"),
            msg = stream.next() => match msg {
                Some(Ok(Message::Text(text))) => return serde_json::from_str(&text).context("unexpected gateway message"),
                Some(Ok(Message::Close(frame))) => bail!("closed by gateway: {}", frame.map(|f| f.reason.to_string()).unwrap_or_default()),
                Some(Ok(_)) => continue,
                Some(Err(e)) => return Err(e.into()),
                None => bail!("connection closed"),
            }
        }
    }
}
