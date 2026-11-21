//! Real PC agent + a simulated phone (same protocol as the Android app) over the real
//! signalling service and a real WebRTC link: pairing, encrypted ADB tunnel, unpair.
//! Needs internet (Supabase Realtime + STUN): `cargo test -p pairspan-core --test link -- --ignored`.

use bytes::BytesMut;
use p256::ecdsa::SigningKey;
use p256::SecretKey;
use pairspan_core::crypto::{self, Sealer};
use pairspan_core::identity::{decode_public, Identity, PeerId};
use pairspan_core::signal::{Incoming, Signal};
use pairspan_core::wire::{self, CH_CLOSE, CH_DATA, CH_OPEN, CH_RPC};
use pairspan_core::{Agent, Options};
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use webrtc::data_channel::{DataChannel, DataChannelEvent};
use webrtc::peer_connection::{
    PeerConnection, PeerConnectionBuilder, PeerConnectionEventHandler, RTCConfigurationBuilder, RTCIceGatheringState,
    RTCIceServer, RTCSessionDescription,
};

/// Echo server standing in for the phone's adbd.
async fn fake_adbd() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let (mut s, _) = l.accept().await.unwrap();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                while let Ok(n) = s.read(&mut buf).await {
                    if n == 0 || s.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                }
            });
        }
    });
    port
}

struct Answerer {
    gathered: Arc<tokio::sync::Notify>,
    channel: mpsc::UnboundedSender<Arc<dyn DataChannel>>,
}

#[async_trait::async_trait]
impl PeerConnectionEventHandler for Answerer {
    async fn on_ice_gathering_state_change(&self, state: RTCIceGatheringState) {
        if state == RTCIceGatheringState::Complete {
            self.gathered.notify_one();
        }
    }
    async fn on_data_channel(&self, dc: Arc<dyn DataChannel>) {
        let _ = self.channel.send(dc);
    }
}

async fn answer(offer: &serde_json::Value, channel: mpsc::UnboundedSender<Arc<dyn DataChannel>>) -> (Arc<dyn PeerConnection>, serde_json::Value) {
    let gathered = Arc::new(tokio::sync::Notify::new());
    let pc = PeerConnectionBuilder::new()
        .with_configuration(
            RTCConfigurationBuilder::new()
                .with_ice_servers(vec![RTCIceServer { urls: vec!["stun:stun.l.google.com:19302".into()], ..Default::default() }])
                .build(),
        )
        .with_handler(Arc::new(Answerer { gathered: gathered.clone(), channel }))
        .with_udp_addrs(vec!["0.0.0.0:0"])
        .build()
        .await
        .unwrap();
    let pc: Arc<dyn PeerConnection> = Arc::new(pc);
    let offer: RTCSessionDescription = serde_json::from_value(offer.clone()).unwrap();
    pc.set_remote_description(offer).await.unwrap();
    let ans = pc.create_answer(None).await.unwrap();
    pc.set_local_description(ans).await.unwrap();
    let _ = tokio::time::timeout(Duration::from_secs(4), gathered.notified()).await;
    let local = pc.local_description().await.unwrap();
    (pc, serde_json::to_value(local).unwrap())
}

/// The phone side: room hello → link → answer offers → respond to the handshake → bridge ADB.
async fn run_phone(secret_code: String, pc_id: PeerId, adbd: u16, paired: mpsc::UnboundedSender<()>) {
    let key = SigningKey::random(&mut rand::rngs::OsRng);
    let identity = Identity::from_key(key.clone());
    let secret = SecretKey::from(key.as_nonzero_scalar());
    let (signal, mut rx) = Signal::connect().await.unwrap();
    let room = wire::room_name(&crypto::room_id(&secret_code));
    signal.join(&room, "phone");
    let hello = json!({ "t": "phone", "pub": identity.public_b64() });

    let mut pc_key: Option<[u8; 33]> = None;
    let mut link = String::new();
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    let (dc_tx, mut dc_rx) = mpsc::unbounded_channel::<Arc<dyn DataChannel>>();
    let mut rtc: Option<Arc<dyn PeerConnection>> = None;

    loop {
        tokio::select! {
            _ = tick.tick(), if pc_key.is_none() => signal.broadcast(&room, hello.clone()),
            m = rx.recv() => match m.unwrap() {
                Incoming::Broadcast { payload, .. } if payload["t"] == "pc" && pc_key.is_none() => {
                    let k = decode_public(payload["pub"].as_str().unwrap()).unwrap();
                    assert_eq!(PeerId::from_pubkey(&k), pc_id, "the PC named over USB must answer");
                    pc_key = Some(k);
                    link = wire::link_name(&k, identity.public());
                    signal.join(&link, "phone");
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    signal.broadcast(&link, json!({ "t": "want" }));
                }
                Incoming::Broadcast { payload, .. } if payload["t"] == "offer" => {
                    let (pc, sdp) = answer(&payload["sdp"], dc_tx.clone()).await;
                    rtc = Some(pc);
                    signal.broadcast(&link, json!({ "t": "answer", "g": payload["g"], "sdp": sdp }));
                }
                _ => {}
            },
            dc = dc_rx.recv() => {
                let dc = dc.unwrap();
                let psk = crypto::pairing_psk(&secret_code);
                tokio::spawn(serve_channel(dc, secret.clone(), pc_key.unwrap(), psk, adbd, paired.clone()));
            }
        }
        let _ = &rtc;
    }
}

async fn serve_channel(dc: Arc<dyn DataChannel>, secret: SecretKey, pc_key: [u8; 33], psk: [u8; 32], adbd: u16, paired: mpsc::UnboundedSender<()>) {
    let sealer: Arc<tokio::sync::Mutex<Option<Sealer>>> = Default::default();
    let mut opener = None;
    let streams: Arc<Mutex<HashMap<u32, mpsc::Sender<Vec<u8>>>>> = Default::default();
    let send = |sealer: Arc<tokio::sync::Mutex<Option<Sealer>>>, dc: Arc<dyn DataChannel>, plain: Vec<u8>| async move {
        let mut g = sealer.lock().await;
        let frame = g.as_mut().unwrap().seal(&plain).unwrap();
        let _ = dc.send(BytesMut::from(&frame[..])).await;
    };
    while let Some(ev) = dc.poll().await {
        let DataChannelEvent::OnMessage(m) = ev else { continue };
        let payload = &m.data[..];
        match payload[0] {
            crypto::HS_INIT => {
                let (session, resp) = crypto::respond(&secret, &pc_key, Some(psk), payload).unwrap();
                dc.send(BytesMut::from(&resp[..])).await.unwrap();
                *sealer.lock().await = Some(session.tx);
                opener = Some(session.rx);
                let hello = json!({"m":"hello","p":{"name":"SimPhone","model":"Sim 1","android":"15"}});
                send(sealer.clone(), dc.clone(), wire::channel_frame(CH_RPC, 0, hello.to_string().as_bytes())).await;
                let _ = paired.send(());
            }
            crypto::DATA => {
                let plain = opener.as_mut().unwrap().open(payload).unwrap();
                let (ch, sid, body) = wire::parse_channel(&plain).unwrap();
                match ch {
                    CH_OPEN => {
                        assert_eq!(body, b"adb");
                        let (mut rd, mut wr) = TcpStream::connect(("127.0.0.1", adbd)).await.unwrap().into_split();
                        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(64);
                        streams.lock().unwrap().insert(sid, tx);
                        tokio::spawn(async move {
                            while let Some(b) = rx.recv().await {
                                if wr.write_all(&b).await.is_err() {
                                    break;
                                }
                            }
                        });
                        let (sealer, dc) = (sealer.clone(), dc.clone());
                        tokio::spawn(async move {
                            let mut buf = vec![0u8; wire::MAX_CHUNK];
                            while let Ok(n) = rd.read(&mut buf).await {
                                if n == 0 {
                                    break;
                                }
                                send(sealer.clone(), dc.clone(), wire::channel_frame(CH_DATA, sid, &buf[..n])).await;
                            }
                            send(sealer, dc, wire::channel_frame(CH_CLOSE, sid, b"")).await;
                        });
                    }
                    CH_DATA => {
                        let tx = streams.lock().unwrap().get(&sid).cloned();
                        if let Some(tx) = tx {
                            tx.send(body.to_vec()).await.unwrap();
                        }
                    }
                    CH_CLOSE => {
                        streams.lock().unwrap().remove(&sid);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "needs internet: Supabase Realtime and STUN"]
async fn pair_and_tunnel_over_supabase_and_webrtc() {
    let dir = tempfile::tempdir().unwrap();
    let agent = Agent::start(dir.path(), Options { manage_adb: false, watch_usb: false }).unwrap();
    for _ in 0..100 {
        if agent.status().connected {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(agent.status().connected, "signalling did not connect");

    let adbd = fake_adbd().await;
    let info = agent.start_pairing().await.unwrap();
    let (paired_tx, mut paired_rx) = mpsc::unbounded_channel();
    tokio::spawn(run_phone(info.code.clone(), agent.id(), adbd, paired_tx));
    let device = agent.wait_paired(Duration::from_secs(40)).await.expect("pairing");
    paired_rx.recv().await.unwrap();

    // Tunnel: bytes through the local adb port reach the phone's "adbd" and come back, in 16 KiB+ chunks.
    let payload: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
    let mut s = TcpStream::connect(("127.0.0.1", device.adb_port)).await.unwrap();
    s.write_all(&payload).await.unwrap();
    let mut got = vec![0u8; payload.len()];
    tokio::time::timeout(Duration::from_secs(20), s.read_exact(&mut got)).await.expect("tunnel echo timed out").unwrap();
    assert_eq!(got, payload);
    assert!(agent.status().devices[0].online);

    // The switch takes the link down.
    agent.set_enabled(false).unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert!(!agent.status().devices[0].online);
    agent.remove_device(device.id, false).await.unwrap();
    assert!(agent.status().devices.is_empty());
}
