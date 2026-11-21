//! The PC agent: a signalling connection (Supabase Realtime), a direct WebRTC link per phone
//! carrying the encrypted session, and a local TCP port per phone that `adb connect` uses
//! (bytes are tunnelled to the phone's adbd). Phones are set up once over USB (`usb.rs`).

use crate::crypto::{self, Initiator, Opener, Sealer};
use crate::identity::{decode_public, Identity, PeerId};
use crate::signal::{Incoming, Signal};
use crate::store::{Device, Settings, Store};
use crate::wire::{self, CH_CLOSE, CH_DATA, CH_OPEN, CH_RPC, MAX_CHUNK};
use anyhow::{anyhow, bail, Context, Result};
use bytes::{Bytes, BytesMut};
use p256::SecretKey;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc, Notify};
use tokio::task::JoinHandle;
use webrtc::data_channel::{DataChannel, DataChannelEvent};
use webrtc::peer_connection::PeerConnection;

const PAIR_TTL: Duration = Duration::from_secs(120);
const OPEN_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_LOG: usize = 400;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    /// Devices, connection or setup state changed; UIs re-read `status()`.
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

/// A phone being set up over USB right now (see `usb.rs`).
#[derive(Debug, Clone, Serialize)]
pub struct Setup {
    pub serial: String,
    pub model: String,
    /// `install`, `pair`, `adb`, `done` or `failed`.
    pub step: &'static str,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub version: &'static str,
    pub id: PeerId,
    pub enabled: bool,
    /// Connected to the signalling service.
    pub connected: bool,
    pub devices: Vec<DeviceStatus>,
    pub setup: Option<Setup>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PairingInfo {
    pub code: String,
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
    ready: AtomicBool,
    /// The current WebRTC attempt; answers and events from older ones are ignored.
    generation: AtomicU64,
    pc: Mutex<Option<Arc<dyn PeerConnection>>>,
    dc: Mutex<Option<Arc<dyn DataChannel>>>,
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
            ready: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            pc: Mutex::new(None),
            dc: Mutex::new(None),
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
    signal: Mutex<Option<Arc<Signal>>>,
    /// Signalling channel name → the phone on the other end (paired phones and a pairing in progress).
    links: Mutex<HashMap<String, (PeerId, [u8; 33])>>,
    pairing: Mutex<Option<Pairing>>,
    listeners: Mutex<HashMap<PeerId, JoinHandle<()>>>,
    connected: AtomicBool,
    reconnect: Notify,
    logs: Mutex<VecDeque<String>>,
    events: broadcast::Sender<Event>,
    pub(crate) setup: Mutex<Option<Setup>>,
    /// USB serials already handled while plugged in, so a phone is set up once per plug-in.
    pub(crate) usb_seen: Mutex<HashSet<String>>,
    /// Run `adb connect/disconnect` automatically when a phone comes and goes.
    manage_adb: bool,
}

pub struct Options {
    pub manage_adb: bool,
    /// Watch USB and set phones up automatically (the desktop app); the CLI leaves it off.
    pub watch_usb: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { manage_adb: true, watch_usb: true }
    }
}

fn now_string() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    secs.to_string()
}

/// The name people gave the computer ("Ferhat's MacBook Pro"); `hostname` can be an IP on macOS.
fn host_name() -> String {
    let run = |cmd: &str, args: &[&str]| {
        std::process::Command::new(cmd)
            .args(args)
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().trim_end_matches(".local").to_string())
            .filter(|s| !s.is_empty())
    };
    (if cfg!(target_os = "macos") { run("scutil", &["--get", "ComputerName"]) } else { None })
        .or_else(|| run("hostname", &[]))
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
            signal: Mutex::new(None),
            links: Mutex::new(HashMap::new()),
            pairing: Mutex::new(None),
            listeners: Mutex::new(HashMap::new()),
            connected: AtomicBool::new(false),
            reconnect: Notify::new(),
            logs: Mutex::new(VecDeque::new()),
            events,
            setup: Mutex::new(None),
            usb_seen: Mutex::new(HashSet::new()),
            manage_adb: options.manage_adb,
        });
        let devices: Vec<Device> = agent.store.lock().unwrap().devices.clone();
        for d in &devices {
            agent.spawn_listener(d);
            agent.add_link(d.id, &d.public);
        }
        tokio::spawn(agent.clone().signal_loop());
        if options.watch_usb {
            tokio::spawn(agent.clone().usb_loop());
        }
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

    pub(crate) fn changed(&self) {
        let _ = self.events.send(Event::Changed);
    }

    pub fn settings(&self) -> Settings {
        self.store.lock().unwrap().settings.clone()
    }

    pub fn update_settings(&self, settings: Settings) -> Result<()> {
        let toggled = {
            let mut store = self.store.lock().unwrap();
            let toggled = store.settings.enabled != settings.enabled;
            store.settings = settings;
            store.save_settings()?;
            toggled
        };
        if toggled {
            self.reconnect.notify_one();
        }
        self.changed();
        Ok(())
    }

    pub fn set_enabled(&self, enabled: bool) -> Result<()> {
        self.update_settings(Settings { enabled })
    }

    pub fn enabled(&self) -> bool {
        self.store.lock().unwrap().settings.enabled
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
            enabled: store.settings.enabled,
            connected: self.connected.load(Ordering::Relaxed),
            devices,
            setup: self.setup.lock().unwrap().clone(),
        }
    }

    pub fn devices(&self) -> Vec<DeviceStatus> {
        self.status().devices
    }

    pub(crate) fn device_by_usb(&self, serial: &str) -> Option<Device> {
        self.store.lock().unwrap().devices.iter().find(|d| d.usb_serial == serial).cloned()
    }

    // ---------------------------------------------------------------- signalling

    fn signal(&self) -> Option<Arc<Signal>> {
        self.signal.lock().unwrap().clone()
    }

    fn add_link(&self, id: PeerId, public_b64: &str) {
        let Ok(key) = decode_public(public_b64) else { return };
        let name = wire::link_name(self.identity.public(), &key);
        self.links.lock().unwrap().insert(name.clone(), (id, key));
        if let Some(s) = self.signal() {
            s.join(&name, "pc");
        }
    }

    fn remove_link(&self, id: PeerId) {
        let mut links = self.links.lock().unwrap();
        let names: Vec<String> = links.iter().filter(|(_, (p, _))| *p == id).map(|(n, _)| n.clone()).collect();
        for n in names {
            links.remove(&n);
            if let Some(s) = self.signal() {
                s.leave(&n);
            }
        }
    }

    async fn signal_loop(self: Arc<Self>) {
        let mut backoff = 1u64;
        loop {
            if !self.enabled() {
                self.shutdown_links().await;
                self.reconnect.notified().await;
                continue;
            }
            match Signal::connect().await {
                Ok((signal, mut rx)) => {
                    backoff = 1;
                    *self.signal.lock().unwrap() = Some(signal.clone());
                    self.connected.store(true, Ordering::Relaxed);
                    self.log("Connected to the signalling service");
                    self.changed();
                    let names: Vec<String> = self.links.lock().unwrap().keys().cloned().collect();
                    for n in names {
                        signal.join(&n, "pc");
                    }
                    let room = self.pairing.lock().unwrap().as_ref().map(|p| wire::room_name(&p.room));
                    if let Some(room) = room {
                        signal.join(&room, "pc");
                    }
                    loop {
                        tokio::select! {
                            m = rx.recv() => match m {
                                Some(m) => self.on_signal(m).await,
                                None => { self.log("Signalling connection lost"); break }
                            },
                            _ = self.reconnect.notified() => break,
                        }
                    }
                }
                Err(e) => self.log(format!("Signalling: {e:#}")),
            }
            // Established phone links are direct and survive a signalling outage.
            *self.signal.lock().unwrap() = None;
            self.connected.store(false, Ordering::Relaxed);
            self.changed();
            if self.enabled() {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(backoff)) => {}
                    _ = self.reconnect.notified() => {}
                }
                backoff = (backoff * 2).min(30);
            }
        }
    }

    async fn shutdown_links(&self) {
        let ids: Vec<PeerId> = self.peers.lock().unwrap().keys().copied().collect();
        for id in ids {
            self.peer_down(id).await;
        }
        self.changed();
    }

    async fn on_signal(&self, msg: Incoming) {
        match msg {
            Incoming::Presence { topic, key, online: true } if key == "phone" => {
                let link = self.links.lock().unwrap().get(&topic).copied();
                if let Some((id, _)) = link {
                    let up = self.existing_peer(&id).is_some_and(|p| p.ready.load(Ordering::Relaxed));
                    if !up {
                        self.connect_phone(id, topic).await;
                    }
                }
            }
            Incoming::Broadcast { topic, payload } => match payload["t"].as_str() {
                Some("want") => {
                    let link = self.links.lock().unwrap().get(&topic).copied();
                    if let Some((id, _)) = link {
                        self.connect_phone(id, topic).await;
                    }
                }
                Some("answer") => {
                    let link = self.links.lock().unwrap().get(&topic).copied();
                    let Some((id, _)) = link else { return };
                    let Some(peer) = self.existing_peer(&id) else { return };
                    if payload["g"].as_u64() != Some(peer.generation.load(Ordering::Relaxed)) {
                        return;
                    }
                    let pc = peer.pc.lock().unwrap().clone();
                    if let Some(pc) = pc {
                        if let Err(e) = crate::rtc::accept_answer(&pc, &payload["sdp"]).await {
                            self.log(format!("Phone {}: {e:#}", id.short()));
                        }
                    }
                }
                Some("phone") => self.on_room_phone(&topic, &payload),
                _ => {}
            },
            Incoming::Presence { .. } => {}
        }
    }

    /// Opens (or reopens) the direct link to a phone: a fresh offer replaces any older attempt.
    async fn connect_phone(&self, id: PeerId, topic: String) {
        let Some(agent) = self.me.upgrade() else { return };
        let peer = self.peer(id);
        if peer.ready.load(Ordering::Relaxed) {
            // The phone asked again, so its side of the old link is gone.
            self.peer_down(id).await;
        }
        let generation = peer.generation.fetch_add(1, Ordering::Relaxed) + 1;
        let offer = match crate::rtc::offer().await {
            Ok(o) => o,
            Err(e) => {
                self.log(format!("Phone {}: {e:#}", id.short()));
                return;
            }
        };
        if peer.generation.load(Ordering::Relaxed) != generation {
            let _ = offer.pc.close().await;
            return;
        }
        self.close_rtc(&peer);
        *peer.pc.lock().unwrap() = Some(offer.pc.clone());
        if let Some(s) = self.signal() {
            s.broadcast(&topic, json!({ "t": "offer", "g": generation, "sdp": offer.sdp }));
        }
        let (dc, dead) = (offer.dc, offer.dead);
        tokio::spawn(async move { agent.run_channel(peer, generation, dc, dead).await });
    }

    async fn run_channel(self: Arc<Self>, peer: Arc<Peer>, generation: u64, dc: Arc<dyn DataChannel>, dead: Arc<Notify>) {
        let current = || peer.generation.load(Ordering::Relaxed) == generation;
        let opened = tokio::time::timeout(OPEN_TIMEOUT, async {
            loop {
                match dc.poll().await {
                    Some(DataChannelEvent::OnOpen) => return true,
                    Some(DataChannelEvent::OnClose) | None => return false,
                    _ => {}
                }
            }
        })
        .await
        .unwrap_or(false);
        if !opened || !current() {
            if current() {
                self.log(format!("Phone {}: no direct connection (network blocks peer-to-peer?)", peer.id.short()));
                self.close_rtc(&peer);
            }
            return;
        }
        *peer.dc.lock().unwrap() = Some(dc.clone());
        self.link_up(peer.id).await;
        loop {
            tokio::select! {
                ev = dc.poll() => match ev {
                    Some(DataChannelEvent::OnMessage(m)) => self.on_frame(peer.id, &m.data).await,
                    Some(DataChannelEvent::OnClose) | None => break,
                    _ => {}
                },
                _ = dead.notified() => break,
            }
            if !current() {
                return;
            }
        }
        if current() {
            self.peer_down(peer.id).await;
        }
    }

    fn close_rtc(&self, peer: &Peer) {
        peer.dc.lock().unwrap().take();
        if let Some(pc) = peer.pc.lock().unwrap().take() {
            tokio::spawn(async move {
                let _ = pc.close().await;
            });
        }
    }

    async fn send_frame(&self, peer: &Peer, frame: &[u8]) -> Result<()> {
        let dc = peer.dc.lock().unwrap().clone().ok_or_else(|| anyhow!("phone is not connected"))?;
        dc.send(BytesMut::from(frame)).await.map_err(|e| anyhow!("link closed: {e}"))
    }

    // ---------------------------------------------------------------- sessions

    fn peer(&self, id: PeerId) -> Arc<Peer> {
        self.peers.lock().unwrap().entry(id).or_insert_with(|| Arc::new(Peer::new(id))).clone()
    }

    fn existing_peer(&self, id: &PeerId) -> Option<Arc<Peer>> {
        self.peers.lock().unwrap().get(id).cloned()
    }

    /// The data channel is open: run the handshake (bound to the pair secret while pairing).
    async fn link_up(&self, id: PeerId) {
        let pairing = {
            let pairing = self.pairing.lock().unwrap();
            pairing.as_ref().and_then(|p| p.phone.filter(|(pid, _)| *pid == id).map(|(_, key)| (key, crypto::pairing_psk(&p.secret))))
        };
        let result = match pairing {
            Some((key, psk)) => self.handshake(id, &key, Some(psk)).await,
            None => {
                let Some(device) = self.store.lock().unwrap().device(&id).cloned() else { return };
                let Ok(key) = decode_public(&device.public) else { return };
                self.handshake(id, &key, None).await
            }
        };
        if let Err(e) = result {
            self.log(format!("Phone {}: handshake failed to start: {e}", id.short()));
        }
    }

    async fn peer_down(&self, id: PeerId) {
        let Some(peer) = self.existing_peer(&id) else { return };
        peer.generation.fetch_add(1, Ordering::Relaxed);
        self.close_rtc(&peer);
        let was_ready = peer.ready.swap(false, Ordering::Relaxed);
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
        self.send_frame(&peer, &frame).await
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
                        self.link_up(src).await;
                    }
                    None => {}
                }
            }
            crypto::REKEY => self.link_up(src).await,
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

        let hello = json!({ "m": "hello", "p": { "name": self.host_name, "os": std::env::consts::OS, "version": env!("CARGO_PKG_VERSION") } });
        let _ = self.send_channel(&peer, CH_RPC, 0, hello.to_string().as_bytes()).await;

        let device = self.store.lock().unwrap().device(&src).cloned();
        if let Some(d) = device {
            self.log(format!("{} is online (direct, end-to-end encrypted)", d.name));
            let agent = self.me.upgrade();
            tokio::spawn(async move {
                if let Some(a) = agent {
                    a.reconnect_adb(d.serial()).await;
                }
            });
        }
        self.changed();
    }

    /// adb keeps a dropped TCP device listed as "offline" and answers a new `connect` with
    /// "already connected", so drop the stale entry first.
    pub(crate) async fn reconnect_adb(&self, serial: String) {
        if !self.manage_adb {
            return;
        }
        let _ = crate::adb::run(None, &["disconnect", &serial], Duration::from_secs(5)).await;
        let _ = crate::adb::run(None, &["connect", &serial], Duration::from_secs(10)).await;
    }

    async fn on_plain(&self, peer: &Arc<Peer>, plain: &[u8]) {
        let Some((channel, stream, body)) = wire::parse_channel(plain) else { return };
        match channel {
            CH_RPC => {
                let Ok(msg) = serde_json::from_slice::<Value>(body) else { return };
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

    fn on_phone_hello(&self, id: PeerId, p: &Value) {
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
        let mut tx = peer.tx.lock().await;
        let sealer = tx.as_mut().ok_or_else(|| anyhow!("phone is not connected"))?;
        let frame = sealer.seal(&plain)?;
        self.send_frame(peer, &frame).await
    }

    // ---------------------------------------------------------------- pairing

    /// Opens a pairing room; `usb.rs` hands the secret to the phone over the cable.
    pub async fn start_pairing(&self) -> Result<PairingInfo> {
        let signal = self.signal().context("Not connected to the signalling service. Check the internet connection.")?;
        let secret = wire::new_pair_secret();
        let room = crypto::room_id(&secret);
        let old = self.pairing.lock().unwrap().replace(Pairing {
            secret: secret.clone(),
            room: room.clone(),
            expires: Instant::now() + PAIR_TTL,
            phone: None,
        });
        if let Some(old) = old {
            signal.leave(&wire::room_name(&old.room));
        }
        signal.join(&wire::room_name(&room), "pc");
        self.changed();
        Ok(PairingInfo { code: secret, expires_in: PAIR_TTL.as_secs() })
    }

    pub async fn cancel_pairing(&self) {
        let old = self.pairing.lock().unwrap().take();
        if let Some(old) = old {
            if let Some(s) = self.signal() {
                s.leave(&wire::room_name(&old.room));
            }
            if let Some((id, _)) = old.phone {
                if self.store.lock().unwrap().device(&id).is_none() {
                    self.remove_link(id);
                }
            }
            self.changed();
        }
    }

    /// A phone in the pairing room introduced itself: answer with our key and start linking.
    fn on_room_phone(&self, topic: &str, payload: &Value) {
        let Some(public) = payload["pub"].as_str() else { return };
        let Ok(key) = decode_public(public) else { return };
        let id = PeerId::from_pubkey(&key);
        {
            let mut pairing = self.pairing.lock().unwrap();
            let Some(p) = pairing.as_mut().filter(|p| wire::room_name(&p.room) == topic && p.expires > Instant::now()) else { return };
            if p.phone.is_some_and(|(pid, _)| pid != id) {
                return;
            }
            p.phone = Some((id, key));
        }
        if let Some(s) = self.signal() {
            s.broadcast(topic, json!({ "t": "pc", "pub": self.identity.public_b64() }));
        }
        let name = wire::link_name(self.identity.public(), &key);
        if !self.links.lock().unwrap().contains_key(&name) {
            self.log(format!("Pairing: phone {} answered, connecting…", id.short()));
            self.add_link(id, public);
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
                usb_serial: String::new(),
            };
            store.devices.push(device.clone());
            store.save_devices()?;
            device
        };
        let room = self.pairing.lock().unwrap().take().map(|p| p.room);
        if let (Some(room), Some(s)) = (room, self.signal()) {
            s.leave(&wire::room_name(&room));
        }
        self.spawn_listener(&device);
        self.log(format!("Paired with phone {}", id.short()));
        let _ = self.events.send(Event::Paired { device: device.clone() });
        Ok(())
    }

    pub(crate) fn set_usb_serial(&self, id: PeerId, serial: &str) {
        let mut store = self.store.lock().unwrap();
        if let Some(d) = store.devices.iter_mut().find(|d| d.id == id) {
            d.usb_serial = serial.to_string();
            let _ = store.save_devices();
        }
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

    /// Unpairs a phone; it is told first when it is online.
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
        self.remove_link(id);
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
