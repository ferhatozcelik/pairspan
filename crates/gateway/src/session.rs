//! One WebSocket connection: authenticate, then pump control messages and relay frames.

use crate::hub::{parse_peer_list, Conn, Hub, JoinError, Out, MAX_ROOMS_PER_PC, ROOM_TTL, SEND_QUEUE};
use crate::proto::{self, ClientMsg, PeerId, Role, RoomId, ServerMsg, FRAME_RELAY, HEADER_LEN};
use crate::Config;
use axum::extract::ws::{CloseFrame, Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use rand::RngCore;
use std::net::IpAddr;
use std::sync::atomic::Ordering;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use subtle::ConstantTimeEq;
use tokio::sync::{mpsc, Notify};
use tokio::time::timeout;

const HELLO_TIMEOUT: Duration = Duration::from_secs(10);
const IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const PING_EVERY: Duration = Duration::from_secs(20);
const MAX_TEXT: usize = 64 * 1024;
/// Applies to JSON control messages only; binary relay frames are paced by backpressure instead.
const MAX_CONTROL_PER_SEC: u32 = 200;

fn close_msg(code: u16, reason: &'static str) -> Message {
    Message::Close(Some(CloseFrame {
        code,
        reason: reason.into(),
    }))
}

fn error(code: &str, msg: &str) -> Out {
    Out::Text(ServerMsg::Error { code, msg }.json())
}

fn token_ok(cfg: &Config, given: Option<&str>) -> bool {
    match cfg.token.as_deref() {
        None => true,
        Some(expected) => given.is_some_and(|g| {
            g.len() == expected.len() && bool::from(g.as_bytes().ct_eq(expected.as_bytes()))
        }),
    }
}

pub async fn run(socket: WebSocket, hub: Arc<Hub>, cfg: Arc<Config>, ip: IpAddr) {
    hub.metrics.connections_total.fetch_add(1, Ordering::Relaxed);
    let (mut sink, mut stream) = socket.split();

    // 1. Challenge → hello.
    let mut nonce = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut nonce);
    let challenge = ServerMsg::Challenge {
        v: proto::PROTOCOL,
        nonce: proto::encode_nonce(&nonce),
    };
    if sink.send(Message::Text(challenge.json().into())).await.is_err() {
        return;
    }
    let hello = match timeout(HELLO_TIMEOUT, stream.next()).await {
        Ok(Some(Ok(Message::Text(text)))) if text.len() <= MAX_TEXT => {
            serde_json::from_str::<ClientMsg>(&text).ok()
        }
        _ => None,
    };
    let Some(ClientMsg::Hello {
        role,
        pubkey,
        sig,
        peers,
        token,
    }) = hello
    else {
        hub.metrics.auth_failures.fetch_add(1, Ordering::Relaxed);
        let _ = sink.send(close_msg(4001, "hello required")).await;
        return;
    };
    if !token_ok(&cfg, token.as_deref()) {
        hub.metrics.auth_failures.fetch_add(1, Ordering::Relaxed);
        let _ = sink.send(close_msg(4003, "unauthorized")).await;
        return;
    }
    let Some((pubkey, id)) = proto::verify_hello(&pubkey, &sig, &nonce) else {
        hub.metrics.auth_failures.fetch_add(1, Ordering::Relaxed);
        let _ = sink.send(close_msg(4001, "bad signature")).await;
        return;
    };
    let Some(peers) = parse_peer_list(&peers) else {
        let _ = sink.send(close_msg(4001, "bad peer list")).await;
        return;
    };

    // 2. Register and start the writer.
    let (tx, mut rx) = mpsc::channel::<Out>(SEND_QUEUE);
    let conn = Conn {
        conn_id: hub.next_conn_id(),
        role,
        pubkey: pubkey.into(),
        tx,
        peers: Arc::new(RwLock::new(peers)),
        kill: Arc::new(Notify::new()),
    };
    let mut writer = tokio::spawn(async move {
        let mut ping = tokio::time::interval(PING_EVERY);
        ping.tick().await;
        loop {
            let msg = tokio::select! {
                out = rx.recv() => match out {
                    Some(Out::Text(t)) => Message::Text(t.into()),
                    Some(Out::Binary(b)) => Message::Binary(b),
                    Some(Out::Close(code, reason)) => {
                        let _ = sink.send(close_msg(code, reason)).await;
                        break;
                    }
                    None => break,
                },
                _ = ping.tick() => Message::Ping(Default::default()),
            };
            if sink.send(msg).await.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    });

    // Welcome goes into the queue before register() can enqueue presence for this connection.
    let online = {
        let tx = conn.tx.clone();
        let online = hub.register(id, conn.clone());
        let count = online.len();
        let _ = tx.try_send(Out::Text(ServerMsg::Welcome { id, online }.json()));
        count
    };
    tracing::info!(peer = ?id, ?role, %ip, online, "connected");

    // 3. Reader loop.
    let mut rooms: Vec<RoomId> = Vec::new();
    let mut window = (Instant::now(), 0u32);
    let reason = loop {
        let next = tokio::select! {
            _ = conn.kill.notified() => break "killed",
            next = timeout(IDLE_TIMEOUT, stream.next()) => next,
        };
        let msg = match next {
            Err(_) => break "idle",
            Ok(None) | Ok(Some(Err(_))) => break "closed",
            Ok(Some(Ok(msg))) => msg,
        };

        match msg {
            Message::Binary(frame) => {
                if frame.len() < HEADER_LEN || frame[0] != FRAME_RELAY {
                    conn.push(Out::Close(4008, "bad frame"));
                    break "bad frame";
                }
                hub.relay(id, &conn, frame).await;
            }
            Message::Text(text) => {
                let now = Instant::now();
                if now.duration_since(window.0) >= Duration::from_secs(1) {
                    window = (now, 0);
                }
                window.1 += 1;
                if window.1 > MAX_CONTROL_PER_SEC {
                    conn.push(Out::Close(4008, "rate limit"));
                    break "rate limit";
                }
                if text.len() > MAX_TEXT {
                    conn.push(Out::Close(4008, "message too large"));
                    break "too large";
                }
                match serde_json::from_str::<ClientMsg>(&text) {
                    Ok(m) => handle_control(&hub, id, &conn, ip, &mut rooms, m),
                    Err(_) => conn.push(error("bad_request", "Unknown or malformed message.")),
                }
            }
            Message::Close(_) => break "closed",
            Message::Ping(_) | Message::Pong(_) => {}
        }
    };

    // 4. Cleanup.
    for room in &rooms {
        hub.close_room(room, id);
    }
    hub.unregister(id, conn.conn_id);
    drop(conn);
    if timeout(Duration::from_secs(2), &mut writer).await.is_err() {
        writer.abort();
    }
    tracing::info!(peer = ?id, reason, "disconnected");
}

fn handle_control(hub: &Hub, id: PeerId, conn: &Conn, ip: IpAddr, rooms: &mut Vec<RoomId>, msg: ClientMsg) {
    match msg {
        ClientMsg::Hello { .. } => conn.push(error("bad_request", "Already authenticated.")),
        ClientMsg::Peers { ids } => match parse_peer_list(&ids) {
            Some(set) => hub.set_peers(id, conn, set),
            None => conn.push(error("bad_request", "Invalid peer list.")),
        },
        ClientMsg::RoomOpen { room } => {
            if conn.role != Role::Pc {
                return conn.push(error("forbidden", "Only a PC can open a room."));
            }
            let Some(room_id) = RoomId::parse_hex(&room) else {
                return conn.push(error("bad_request", "Invalid room id."));
            };
            rooms.retain(|r| *r != room_id);
            if rooms.len() >= MAX_ROOMS_PER_PC {
                let oldest = rooms.remove(0);
                hub.close_room(&oldest, id);
            }
            hub.open_room(room_id, id);
            rooms.push(room_id);
            conn.push(Out::Text(
                ServerMsg::RoomOpened {
                    room,
                    expires_in: ROOM_TTL.as_secs(),
                }
                .json(),
            ));
        }
        ClientMsg::RoomClose { room } => {
            if let Some(room_id) = RoomId::parse_hex(&room) {
                hub.close_room(&room_id, id);
                rooms.retain(|r| *r != room_id);
            }
        }
        ClientMsg::RoomJoin { room } => {
            if conn.role != Role::Phone {
                return conn.push(error("forbidden", "Only a phone can join a room."));
            }
            let Some(room_id) = RoomId::parse_hex(&room) else {
                return conn.push(error("bad_request", "Invalid room id."));
            };
            match hub.join_room(room_id, id, conn, ip) {
                Ok(()) => {}
                Err(JoinError::NotFound) => conn.push(error("room_not_found", "Pairing code not found or expired.")),
                Err(JoinError::RateLimited) => conn.push(error("rate_limited", "Too many pairing attempts. Try again later.")),
            }
        }
    }
}
