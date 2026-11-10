//! Real gateway + real PC agent + a simulated phone (same protocol as the Android app):
//! pairing, encrypted ADB tunnel, reconnect, local RPC and unpairing.

use futures_util::{SinkExt, StreamExt};
use p256::ecdsa::SigningKey;
use p256::SecretKey;
use pairspan_core::crypto::{self, Sealer};
use pairspan_core::identity::{decode_public, Identity, PeerId};
use pairspan_core::wire::{self, FromGateway, Role, ToGateway, CH_CLOSE, CH_DATA, CH_OPEN, CH_RPC};
use pairspan_core::{Agent, Options};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

async fn start_gateway() -> SocketAddr {
    let cfg = pairspan_gateway::Config { max_connections: 100, ..Default::default() };
    let app = pairspan_gateway::router(Arc::new(pairspan_gateway::hub::Hub::new()), cfg)
        .into_make_service_with_connect_info::<SocketAddr>();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    addr
}

/// Echo server standing in for the phone's adbd on 127.0.0.1:5555.
async fn fake_adbd() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let (mut s, _) = l.accept().await.unwrap();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                loop {
                    match s.read(&mut buf).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if s.write_all(&buf[..n]).await.is_err() {
                                break;
                            }
                        }
                    }
                }
            });
        }
    });
    port
}

#[derive(Debug, PartialEq)]
enum PhoneEvent {
    Paired,
    SessionUp,
    Unpaired,
}

/// Minimal phone: responder side of the handshake, ADB stream bridge, unpair handling.
struct SimPhone {
    key: SigningKey,
    pc: Arc<Mutex<Option<(PeerId, [u8; 33])>>>,
    adbd: u16,
}

impl SimPhone {
    fn spawn(&self, gateway: SocketAddr, pairing: Option<(String, PeerId)>, events: mpsc::UnboundedSender<PhoneEvent>) -> tokio::task::JoinHandle<()> {
        let key = self.key.clone();
        let pc = self.pc.clone();
        let adbd = self.adbd;
        tokio::spawn(async move { run_phone(key, pc, adbd, gateway, pairing, events).await })
    }
}

async fn run_phone(
    key: SigningKey,
    pc: Arc<Mutex<Option<(PeerId, [u8; 33])>>>,
    adbd: u16,
    gateway: SocketAddr,
    pairing: Option<(String, PeerId)>,
    events: mpsc::UnboundedSender<PhoneEvent>,
) {
    let identity = Identity::from_key(key.clone());
    let secret = SecretKey::from(key.as_nonzero_scalar());
    let (ws, _) = tokio_tungstenite::connect_async(format!("ws://{gateway}/ws")).await.unwrap();
    let (mut sink, mut stream) = ws.split();
    let nonce = match next_json(&mut stream).await {
        FromGateway::Challenge { nonce, .. } => {
            use base64::Engine;
            base64::engine::general_purpose::STANDARD.decode(nonce).unwrap()
        }
        m => panic!("{m:?}"),
    };
    let sig = identity.sign_challenge(&nonce);
    let public = identity.public_b64();
    let peers: Vec<PeerId> = pc.lock().unwrap().iter().map(|(id, _)| *id).collect();
    let hello = ToGateway::Hello { role: Role::Phone, public: &public, sig: &sig, peers, token: None };
    sink.send(Message::Text(hello.json().into())).await.unwrap();
    assert!(matches!(next_json(&mut stream).await, FromGateway::Welcome { .. }));

    let (out_tx, mut out_rx) = mpsc::channel::<Message>(256);
    tokio::spawn(async move {
        while let Some(m) = out_rx.recv().await {
            if sink.send(m).await.is_err() {
                break;
            }
        }
    });
    if let Some((secret_code, _)) = &pairing {
        let room = crypto::room_id(secret_code);
        out_tx.send(Message::Text(ToGateway::RoomJoin { room: &room }.json().into())).await.unwrap();
    }

    let sealer: Arc<tokio::sync::Mutex<Option<Sealer>>> = Arc::new(tokio::sync::Mutex::new(None));
    let mut opener = None;
    let mut candidate: Option<(PeerId, [u8; 33])> = None;
    let streams: Arc<Mutex<HashMap<u32, mpsc::Sender<Vec<u8>>>>> = Default::default();

    let send = |sealer: Arc<tokio::sync::Mutex<Option<Sealer>>>, out: mpsc::Sender<Message>, to: PeerId, plain: Vec<u8>| async move {
        let mut g = sealer.lock().await;
        let Some(s) = g.as_mut() else { return };
        let frame = s.seal(&plain).unwrap();
        let _ = out.send(Message::Binary(wire::relay_frame(to, &frame).into())).await;
    };

    while let Some(Ok(msg)) = stream.next().await {
        match msg {
            Message::Text(t) => {
                if let Ok(FromGateway::RoomPeer { id, public, .. }) = serde_json::from_str::<FromGateway>(&t) {
                    let expected = pairing.as_ref().map(|(_, k)| *k);
                    assert_eq!(Some(id), expected, "QR k= must match the room peer");
                    candidate = Some((id, decode_public(&public).unwrap()));
                }
            }
            Message::Binary(frame) => {
                let (src, payload) = wire::parse_relay(&frame).unwrap();
                match payload[0] {
                    crypto::HS_INIT => {
                        let paired = *pc.lock().unwrap();
                        let (peer_pub, psk) = match (candidate, paired) {
                            (Some((id, k)), _) if id == src => (k, Some(crypto::pairing_psk(&pairing.as_ref().unwrap().0))),
                            (_, Some((id, k))) if id == src => (k, None),
                            _ => continue,
                        };
                        let (session, resp) = crypto::respond(&secret, &peer_pub, psk, payload).unwrap();
                        out_tx.send(Message::Binary(wire::relay_frame(src, &resp).into())).await.unwrap();
                        *sealer.lock().await = Some(session.tx);
                        opener = Some(session.rx);
                        if psk.is_some() {
                            *pc.lock().unwrap() = Some((src, peer_pub));
                            candidate = None;
                            out_tx.send(Message::Text(ToGateway::Peers { ids: vec![src] }.json().into())).await.unwrap();
                            let _ = events.send(PhoneEvent::Paired);
                        }
                        let hello = serde_json::json!({"m":"hello","p":{"name":"SimPhone","model":"Sim 1","android":"15"}});
                        send(sealer.clone(), out_tx.clone(), src, wire::channel_frame(CH_RPC, 0, hello.to_string().as_bytes())).await;
                        let _ = events.send(PhoneEvent::SessionUp);
                    }
                    crypto::DATA => {
                        let plain = opener.as_mut().unwrap().open(payload).unwrap();
                        let (ch, sid, body) = wire::parse_channel(&plain).unwrap();
                        match ch {
                            CH_OPEN => {
                                assert_eq!(body, b"adb");
                                let sock = TcpStream::connect(("127.0.0.1", adbd)).await.unwrap();
                                let (mut rd, mut wr) = sock.into_split();
                                let (tx, mut rx) = mpsc::channel::<Vec<u8>>(64);
                                streams.lock().unwrap().insert(sid, tx);
                                tokio::spawn(async move {
                                    while let Some(b) = rx.recv().await {
                                        if wr.write_all(&b).await.is_err() {
                                            break;
                                        }
                                    }
                                });
                                let (sealer, out) = (sealer.clone(), out_tx.clone());
                                tokio::spawn(async move {
                                    let mut buf = vec![0u8; 65536];
                                    loop {
                                        match rd.read(&mut buf).await {
                                            Ok(0) | Err(_) => break,
                                            Ok(n) => send(sealer.clone(), out.clone(), src, wire::channel_frame(CH_DATA, sid, &buf[..n])).await,
                                        }
                                    }
                                    send(sealer, out, src, wire::channel_frame(CH_CLOSE, sid, b"")).await;
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
                            CH_RPC => {
                                let v: serde_json::Value = serde_json::from_slice(body).unwrap();
                                if v["m"] == "unpair" {
                                    *pc.lock().unwrap() = None;
                                    let _ = events.send(PhoneEvent::Unpaired);
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

async fn next_json<S>(stream: &mut S) -> FromGateway
where
    S: futures_util::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    loop {
        if let Some(Ok(Message::Text(t))) = stream.next().await {
            return serde_json::from_str(&t).unwrap();
        }
    }
}

async fn eventually<F: Fn() -> bool>(what: &str, f: F) {
    for _ in 0..100 {
        if f() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("timed out waiting for: {what}");
}

async fn roundtrip(port: u16, payload: &[u8]) -> Vec<u8> {
    let mut s = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    s.write_all(payload).await.unwrap();
    let mut got = vec![0u8; payload.len()];
    tokio::time::timeout(Duration::from_secs(10), s.read_exact(&mut got)).await.expect("tunnel echo timed out").unwrap();
    got
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pair_tunnel_reconnect_rpc_unpair() {
    let gateway = start_gateway().await;
    let adbd = fake_adbd().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("settings.json"),
        serde_json::json!({ "gateway_url": format!("ws://{gateway}/ws"), "automation_enabled": true }).to_string(),
    )
    .unwrap();
    std::env::set_var("PAIRSPAN_ADB_PORT_BASE", "47900");

    let agent = Agent::start(dir.path(), Options { manage_adb: false }).unwrap();
    eventually("agent connects", || agent.status().connected).await;

    // 1. Pairing through a room, with the QR payload the phone would scan.
    let info = agent.start_pairing().await.unwrap();
    let url = url::Url::parse(&info.qr).unwrap();
    let q: HashMap<_, _> = url.query_pairs().into_owned().collect();
    assert_eq!(q["g"], format!("ws://{gateway}/ws"));
    assert_eq!(wire::display_secret(&q["t"]), info.code);
    let pc_id = PeerId::parse(&q["k"]).unwrap();
    assert_eq!(pc_id, agent.id());

    let phone = SimPhone { key: SigningKey::random(&mut rand::rngs::OsRng), pc: Default::default(), adbd };
    let phone_id = Identity::from_key(phone.key.clone()).id();
    let (ev_tx, mut ev_rx) = mpsc::unbounded_channel();
    let task = phone.spawn(gateway, Some((q["t"].clone(), pc_id)), ev_tx.clone());
    let device = agent.wait_paired(Duration::from_secs(10)).await.unwrap();
    assert_eq!(device.id, phone_id);
    assert_eq!(ev_rx.recv().await, Some(PhoneEvent::Paired));
    assert!(agent.status().pairing.is_none(), "room is closed after pairing");
    eventually("phone online with its hello info", || {
        agent.devices().iter().any(|d| d.online && d.device.name == "SimPhone" && d.device.android == "15")
    })
    .await;

    // 2. Encrypted ADB tunnel: small and multi-chunk payloads come back intact and in order.
    assert_eq!(roundtrip(device.adb_port, b"CNXN host::pairspan").await, b"CNXN host::pairspan");
    let big: Vec<u8> = (0..1_500_000u32).map(|i| (i % 251) as u8).collect();
    assert_eq!(roundtrip(device.adb_port, &big).await, big);

    // 3. Phone drops and comes back: new handshake from pinned keys, no pairing secret.
    task.abort();
    eventually("phone offline", || agent.devices().iter().all(|d| !d.online)).await;
    let task = phone.spawn(gateway, None, ev_tx.clone());
    eventually("phone back online", || agent.devices().iter().any(|d| d.online)).await;
    assert_eq!(roundtrip(device.adb_port, b"after reconnect").await, b"after reconnect");

    // 4. Local RPC socket, as used by the CLI and MCP server.
    #[cfg(unix)]
    {
        let sock = dir.path().join("pairspan.sock");
        tokio::spawn({
            let (agent, sock) = (agent.clone(), sock.clone());
            async move { pairspan_core::rpc::serve(agent, &sock).await.unwrap() }
        });
        eventually("socket bound", || sock.exists()).await;
        let mut client = pairspan_core::rpc::Client::connect(&sock).await.unwrap();
        let devices = client.call("devices", serde_json::json!({})).await.unwrap();
        assert_eq!(devices[0]["online"], true);
        assert_eq!(devices[0]["serial"], format!("127.0.0.1:{}", device.adb_port));
        let err = client.call("nope", serde_json::json!({})).await.unwrap_err();
        assert!(err.to_string().contains("unknown method"));
        let err = client.call("tap", serde_json::json!({"device": "no-such"})).await.unwrap_err();
        assert!(err.to_string().contains("no paired phone"));
        assert!(std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&sock).unwrap().permissions()) & 0o077 == 0);
    }

    // 5. Unpair from the PC: the phone is told and the link is cut.
    while ev_rx.try_recv().is_ok() {}
    agent.remove_device(phone_id, true).await.unwrap();
    let ev = tokio::time::timeout(Duration::from_secs(5), ev_rx.recv()).await.unwrap();
    assert_eq!(ev, Some(PhoneEvent::Unpaired));
    assert!(agent.devices().is_empty());
    eventually("tunnel port released", || std::net::TcpListener::bind(("127.0.0.1", device.adb_port)).is_ok()).await;
    task.abort();
}
