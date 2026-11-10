use base64::{engine::general_purpose::STANDARD as B64, Engine};
use futures_util::{SinkExt, StreamExt};
use p256::ecdsa::{signature::Signer, Signature, SigningKey};
use pairspan_gateway::{hub::Hub, proto::PeerId, router, Config};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn start(cfg: Config) -> SocketAddr {
    let cfg = Config { max_connections: 100, ..cfg };
    let app = router(Arc::new(Hub::new()), cfg).into_make_service_with_connect_info::<SocketAddr>();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    addr
}

struct Client {
    ws: Ws,
    key: SigningKey,
}

impl Client {
    fn id(key: &SigningKey) -> PeerId {
        PeerId::from_pubkey(key.verifying_key().to_encoded_point(true).as_bytes())
    }

    async fn connect(addr: SocketAddr, key: SigningKey, role: &str, peers: &[PeerId], token: Option<&str>) -> (Self, Value) {
        let (ws, _) = connect_async(format!("ws://{addr}/ws")).await.unwrap();
        let mut c = Client { ws, key };
        let challenge = c.json().await;
        assert_eq!(challenge["t"], "challenge");
        assert_eq!(challenge["v"], 1);
        let nonce = B64.decode(challenge["nonce"].as_str().unwrap()).unwrap();
        let mut msg = b"pairspan/v1/auth".to_vec();
        msg.extend_from_slice(&nonce);
        let sig: Signature = c.key.sign(&msg);
        let hello = json!({
            "t": "hello",
            "role": role,
            "pub": B64.encode(c.key.verifying_key().to_encoded_point(true).as_bytes()),
            "sig": B64.encode(sig.to_der().as_bytes()),
            "peers": peers.iter().map(|p| p.to_string()).collect::<Vec<_>>(),
            "token": token,
        });
        c.send(hello).await;
        let reply = c.next().await;
        (c, reply)
    }

    async fn send(&mut self, v: Value) {
        self.ws.send(Message::Text(v.to_string().into())).await.unwrap();
    }

    async fn send_bin(&mut self, dest: PeerId, payload: &[u8]) {
        let mut f = vec![0x01];
        f.extend_from_slice(&dest.0);
        f.extend_from_slice(payload);
        self.ws.send(Message::Binary(f.into())).await.unwrap();
    }

    /// Next text (as JSON), binary (as {"bin": [src hex, payload]}) or close ({"close": code}).
    async fn next(&mut self) -> Value {
        loop {
            let msg = tokio::time::timeout(Duration::from_secs(3), self.ws.next())
                .await
                .expect("timed out waiting for a message");
            match msg {
                Some(Ok(Message::Text(t))) => return serde_json::from_str(&t).unwrap(),
                Some(Ok(Message::Binary(b))) => {
                    assert_eq!(b[0], 0x01);
                    let src = PeerId(b[1..17].try_into().unwrap());
                    return json!({ "bin": [src.to_string(), String::from_utf8_lossy(&b[17..])] });
                }
                Some(Ok(Message::Close(frame))) => return json!({ "close": frame.map(|f| u16::from(f.code)) }),
                Some(Ok(_)) => continue,
                _ => return json!({ "close": null }),
            }
        }
    }

    async fn json(&mut self) -> Value {
        self.next().await
    }

    async fn expect_silence(&mut self) {
        let r = tokio::time::timeout(Duration::from_millis(300), self.ws.next()).await;
        assert!(r.is_err(), "expected no message, got {r:?}");
    }
}

fn key() -> SigningKey {
    SigningKey::random(&mut rand::rngs::OsRng)
}

#[tokio::test]
async fn rejects_bad_signature() {
    let addr = start(Config::default()).await;
    let (ws, _) = connect_async(format!("ws://{addr}/ws")).await.unwrap();
    let mut c = Client { ws, key: key() };
    let _ = c.json().await;
    let other = key();
    let sig: Signature = other.sign(b"pairspan/v1/auth wrong nonce");
    c.send(json!({
        "t": "hello", "role": "pc",
        "pub": B64.encode(c.key.verifying_key().to_encoded_point(true).as_bytes()),
        "sig": B64.encode(sig.to_bytes()),
    }))
    .await;
    assert_eq!(c.next().await, json!({ "close": 4001 }));
}

#[tokio::test]
async fn enforces_gateway_token() {
    let addr = start(Config { token: Some("s3cret".into()), ..Default::default() }).await;
    let (mut bad, reply) = Client::connect(addr, key(), "pc", &[], Some("nope")).await;
    assert_eq!(reply, json!({ "close": 4003 }));
    let _ = bad.ws.close(None).await;
    let (_, reply) = Client::connect(addr, key(), "pc", &[], Some("s3cret")).await;
    assert_eq!(reply["t"], "welcome");
}

#[tokio::test]
async fn pairing_room_links_peers_temporarily() {
    let addr = start(Config::default()).await;
    let (pc_key, phone_key) = (key(), key());
    let (pc_id, phone_id) = (Client::id(&pc_key), Client::id(&phone_key));

    let (mut pc, w) = Client::connect(addr, pc_key, "pc", &[], None).await;
    assert_eq!(w["id"], pc_id.to_string());
    let (mut phone, _) = Client::connect(addr, phone_key, "phone", &[], None).await;

    // Unlinked peers cannot reach each other.
    phone.send_bin(pc_id, b"too early").await;
    pc.expect_silence().await;

    let room = "ab".repeat(32);
    pc.send(json!({ "t": "room_open", "room": room })).await;
    assert_eq!(pc.json().await, json!({ "t": "room_opened", "room": room, "expires_in": 300 }));

    // A phone cannot open rooms and a PC cannot join them.
    phone.send(json!({ "t": "room_open", "room": room })).await;
    assert_eq!(phone.json().await["code"], "forbidden");

    phone.send(json!({ "t": "room_join", "room": room })).await;
    let to_pc = pc.json().await;
    assert_eq!(to_pc["t"], "room_peer");
    assert_eq!(to_pc["id"], phone_id.to_string());
    assert_eq!(to_pc["role"], "phone");
    let to_phone = phone.json().await;
    assert_eq!(to_phone["id"], pc_id.to_string());
    assert_eq!(to_phone["role"], "pc");

    // Room is single use.
    phone.send(json!({ "t": "room_join", "room": room })).await;
    assert_eq!(phone.json().await["code"], "room_not_found");

    // Temporary link carries the handshake both ways, with the source id rewritten in.
    phone.send_bin(pc_id, b"hs-init").await;
    assert_eq!(pc.next().await, json!({ "bin": [phone_id.to_string(), "hs-init"] }));
    pc.send_bin(phone_id, b"hs-resp").await;
    assert_eq!(phone.next().await, json!({ "bin": [pc_id.to_string(), "hs-resp"] }));
}

#[tokio::test]
async fn mutual_trust_presence_and_revocation() {
    let addr = start(Config::default()).await;
    let (pc_key, phone_key, stranger_key) = (key(), key(), key());
    let (pc_id, phone_id) = (Client::id(&pc_key), Client::id(&phone_key));

    let (mut pc, w) = Client::connect(addr, pc_key, "pc", &[phone_id], None).await;
    assert_eq!(w["online"], json!([]));

    let (mut phone, w) = Client::connect(addr, phone_key, "phone", &[pc_id], None).await;
    assert_eq!(w["online"], json!([pc_id.to_string()]));
    assert_eq!(pc.json().await, json!({ "t": "presence", "id": phone_id.to_string(), "online": true }));

    pc.send_bin(phone_id, b"adb data").await;
    assert_eq!(phone.next().await, json!({ "bin": [pc_id.to_string(), "adb data"] }));

    // A stranger that trusts the phone still cannot reach it (trust must be mutual).
    let (mut stranger, _) = Client::connect(addr, stranger_key, "pc", &[phone_id], None).await;
    stranger.send_bin(phone_id, b"spoof").await;
    phone.expect_silence().await;

    // Unpair on the PC: both sides learn immediately and routing stops.
    pc.send(json!({ "t": "peers", "ids": [] })).await;
    assert_eq!(pc.json().await, json!({ "t": "presence", "id": phone_id.to_string(), "online": false }));
    assert_eq!(phone.json().await, json!({ "t": "presence", "id": pc_id.to_string(), "online": false }));
    phone.send_bin(pc_id, b"after revoke").await;
    pc.expect_silence().await;

    // Disconnect of a linked peer is announced.
    pc.send(json!({ "t": "peers", "ids": [phone_id.to_string()] })).await;
    assert_eq!(pc.json().await["online"], true);
    assert_eq!(phone.json().await["online"], true);
    drop(pc);
    assert_eq!(phone.json().await, json!({ "t": "presence", "id": pc_id.to_string(), "online": false }));
}

#[tokio::test]
async fn newer_connection_replaces_older() {
    let addr = start(Config::default()).await;
    let k = key();
    let (mut first, _) = Client::connect(addr, k.clone(), "phone", &[], None).await;
    let (_second, w) = Client::connect(addr, k, "phone", &[], None).await;
    assert_eq!(w["t"], "welcome");
    assert_eq!(first.next().await, json!({ "close": 4000 }));
}

#[tokio::test]
async fn health_and_metrics() {
    let addr = start(Config::default()).await;
    let (_c, _) = Client::connect(addr, key(), "pc", &[], None).await;
    let body = http_get(addr, "/health").await;
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["protocol"], 1);
    assert_eq!(v["connections"], 1);
    let m = http_get(addr, "/metrics").await;
    assert!(m.contains("pairspan_connections 1"));
}

async fn http_get(addr: SocketAddr, path: &str) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut s = TcpStream::connect(addr).await.unwrap();
    s.write_all(format!("GET {path} HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n").as_bytes())
        .await
        .unwrap();
    let mut buf = String::new();
    s.read_to_string(&mut buf).await.unwrap();
    buf.split("\r\n\r\n").nth(1).unwrap_or_default().to_string()
}

/// `cargo test --release -p pairspan-gateway -- --ignored --nocapture throughput`
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore]
async fn throughput() {
    let addr = start(Config::default()).await;
    let (a_key, b_key) = (key(), key());
    let (a_id, b_id) = (Client::id(&a_key), Client::id(&b_key));
    let (a, _) = Client::connect(addr, a_key, "pc", &[b_id], None).await;
    let (mut b, _) = Client::connect(addr, b_key, "phone", &[a_id], None).await;
    let mut a = a;
    let _ = a.json().await; // presence

    for size in [1024usize, 16 * 1024, 64 * 1024] {
        let frames = (256 * 1024 * 1024 / size).min(100_000);
        let mut f = vec![0x01];
        f.extend_from_slice(&b_id.0);
        f.extend(std::iter::repeat_n(0xAB, size));
        let frame: bytes::Bytes = f.into();
        let start = std::time::Instant::now();
        let recv = async {
            let mut got = 0;
            while got < frames {
                if let Some(Ok(Message::Binary(_))) = b.ws.next().await { got += 1; }
            }
        };
        let send = async {
            for _ in 0..frames { a.ws.feed(Message::Binary(frame.clone())).await.unwrap(); }
            a.ws.flush().await.unwrap();
        };
        tokio::join!(recv, send);
        let secs = start.elapsed().as_secs_f64();
        println!(
            "{:>6} B frames: {:>8.0} frames/s  {:>7.1} MB/s",
            size, frames as f64 / secs, (frames * size) as f64 / secs / 1e6
        );
    }
}
