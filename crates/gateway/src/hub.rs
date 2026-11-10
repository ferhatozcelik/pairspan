//! Shared gateway state: who is online, who trusts whom, pairing rooms and relaying.
//!
//! Locking rules: DashMap guards are never held across an `.await`; handles are cloned out first.

use crate::metrics::Metrics;
use crate::proto::{PeerId, Role, RoomId, ServerMsg, FRAME_RELAY, HEADER_LEN};
use bytes::{Bytes, BytesMut};
use dashmap::DashMap;
use std::collections::HashSet;
use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Notify};

pub const ROOM_TTL: Duration = Duration::from_secs(300);
pub const TEMP_LINK_TTL: Duration = Duration::from_secs(600);
pub const MAX_PEERS: usize = 64;
pub const MAX_ROOMS_PER_PC: usize = 4;
pub const JOIN_WINDOW: Duration = Duration::from_secs(300);
pub const MAX_JOINS_PER_WINDOW: u32 = 20;
pub const SLOW_CONSUMER: Duration = Duration::from_secs(10);
pub const SEND_QUEUE: usize = 256;

#[derive(Debug)]
pub enum Out {
    Text(String),
    Binary(Bytes),
    Close(u16, &'static str),
}

/// Cheap-to-clone handle to one live connection.
#[derive(Clone)]
pub struct Conn {
    pub conn_id: u64,
    pub role: Role,
    pub pubkey: Arc<str>,
    pub tx: mpsc::Sender<Out>,
    pub peers: Arc<RwLock<HashSet<PeerId>>>,
    pub kill: Arc<Notify>,
}

impl Conn {
    fn trusts(&self, other: &PeerId) -> bool {
        self.peers.read().unwrap().contains(other)
    }

    /// Control messages are rare; a peer whose queue is full is too slow and gets dropped.
    pub fn push(&self, msg: Out) {
        if self.tx.try_send(msg).is_err() {
            self.kill.notify_one();
        }
    }

    fn close(&self, code: u16, reason: &'static str) {
        let _ = self.tx.try_send(Out::Close(code, reason));
        self.kill.notify_one();
    }
}

struct Room {
    owner: PeerId,
    expires: Instant,
}

#[derive(Debug, PartialEq, Eq)]
pub enum JoinError {
    NotFound,
    RateLimited,
}

pub struct Hub {
    conns: DashMap<PeerId, Conn>,
    rooms: DashMap<RoomId, Room>,
    temp_links: DashMap<(PeerId, PeerId), Instant>,
    join_attempts: DashMap<IpAddr, (u32, Instant)>,
    next_conn: AtomicU64,
    pub metrics: Metrics,
}

fn link_key(a: PeerId, b: PeerId) -> (PeerId, PeerId) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

pub fn parse_peer_list(ids: &[String]) -> Option<HashSet<PeerId>> {
    if ids.len() > MAX_PEERS {
        return None;
    }
    ids.iter().map(|s| PeerId::parse_hex(s)).collect()
}

impl Default for Hub {
    fn default() -> Self {
        Self::new()
    }
}

impl Hub {
    pub fn new() -> Self {
        Self {
            conns: DashMap::new(),
            rooms: DashMap::new(),
            temp_links: DashMap::new(),
            join_attempts: DashMap::new(),
            next_conn: AtomicU64::new(1),
            metrics: Metrics::default(),
        }
    }

    pub fn next_conn_id(&self) -> u64 {
        self.next_conn.fetch_add(1, Ordering::Relaxed)
    }

    pub fn online(&self) -> usize {
        self.conns.len()
    }

    pub fn open_rooms(&self) -> usize {
        self.rooms.len()
    }

    fn get(&self, id: &PeerId) -> Option<Conn> {
        self.conns.get(id).map(|c| c.clone())
    }

    /// Adds a connection, replacing (and closing) any older one with the same peer id.
    /// Returns the peers that are online and trust `id` back.
    pub fn register(&self, id: PeerId, conn: Conn) -> Vec<PeerId> {
        let peers: Vec<PeerId> = conn.peers.read().unwrap().iter().copied().collect();
        if let Some(old) = self.conns.insert(id, conn) {
            old.close(4000, "replaced");
        }
        let online: Vec<PeerId> = peers
            .into_iter()
            .filter(|p| self.get(p).is_some_and(|c| c.trusts(&id)))
            .collect();
        for p in &online {
            if let Some(c) = self.get(p) {
                c.push(Out::Text(ServerMsg::Presence { id, online: true }.json()));
            }
        }
        online
    }

    /// Removes the connection only if it is still the current one for `id`.
    pub fn unregister(&self, id: PeerId, conn_id: u64) {
        let Some((_, conn)) = self.conns.remove_if(&id, |_, c| c.conn_id == conn_id) else {
            return;
        };
        let peers: Vec<PeerId> = conn.peers.read().unwrap().iter().copied().collect();
        for p in peers {
            if let Some(c) = self.get(&p) {
                if c.trusts(&id) {
                    c.push(Out::Text(ServerMsg::Presence { id, online: false }.json()));
                }
            }
        }
    }

    /// Replaces the trust list and tells both ends about links that came up or went down.
    pub fn set_peers(&self, id: PeerId, conn: &Conn, new: HashSet<PeerId>) {
        let old = std::mem::replace(&mut *conn.peers.write().unwrap(), new.clone());
        let notify = |p: &PeerId, online: bool| {
            if let Some(c) = self.get(p) {
                if c.trusts(&id) {
                    c.push(Out::Text(ServerMsg::Presence { id, online }.json()));
                    conn.push(Out::Text(ServerMsg::Presence { id: *p, online }.json()));
                }
            }
        };
        for p in new.difference(&old) {
            notify(p, true);
        }
        for p in old.difference(&new) {
            notify(p, false);
        }
    }

    pub fn open_room(&self, room: RoomId, owner: PeerId) {
        self.rooms.insert(
            room,
            Room {
                owner,
                expires: Instant::now() + ROOM_TTL,
            },
        );
        self.metrics.rooms_opened.fetch_add(1, Ordering::Relaxed);
    }

    pub fn close_room(&self, room: &RoomId, owner: PeerId) {
        self.rooms.remove_if(room, |_, r| r.owner == owner);
    }

    fn allow_join(&self, ip: IpAddr) -> bool {
        let now = Instant::now();
        let mut entry = self.join_attempts.entry(ip).or_insert((0, now));
        if now.duration_since(entry.1) > JOIN_WINDOW {
            *entry = (0, now);
        }
        entry.0 += 1;
        entry.0 <= MAX_JOINS_PER_WINDOW
    }

    /// Consumes the room, links the two peers temporarily and introduces them to each other.
    pub fn join_room(
        &self,
        room: RoomId,
        joiner: PeerId,
        joiner_conn: &Conn,
        ip: IpAddr,
    ) -> Result<(), JoinError> {
        if !self.allow_join(ip) {
            return Err(JoinError::RateLimited);
        }
        let now = Instant::now();
        let (_, entry) = self
            .rooms
            .remove_if(&room, |_, r| r.expires > now)
            .ok_or(JoinError::NotFound)?;
        let owner = self.get(&entry.owner).ok_or(JoinError::NotFound)?;
        if entry.owner == joiner {
            return Err(JoinError::NotFound);
        }
        self.temp_links
            .insert(link_key(entry.owner, joiner), now + TEMP_LINK_TTL);
        let room_hex = room.to_string();
        owner.push(Out::Text(
            ServerMsg::RoomPeer {
                room: room_hex.clone(),
                id: joiner,
                pubkey: &joiner_conn.pubkey,
                role: joiner_conn.role,
            }
            .json(),
        ));
        joiner_conn.push(Out::Text(
            ServerMsg::RoomPeer {
                room: room_hex,
                id: entry.owner,
                pubkey: &owner.pubkey,
                role: owner.role,
            }
            .json(),
        ));
        self.metrics.rooms_joined.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn linked(&self, src: PeerId, src_conn: &Conn, dest: PeerId, dest_conn: &Conn) -> bool {
        if src_conn.trusts(&dest) && dest_conn.trusts(&src) {
            return true;
        }
        self.temp_links
            .get(&link_key(src, dest))
            .is_some_and(|exp| *exp > Instant::now())
    }

    /// Routes one binary frame. `frame` is `0x01 | dest (16) | payload`; the receiver gets the source id instead.
    pub async fn relay(&self, src: PeerId, src_conn: &Conn, frame: Bytes) {
        let mut dest = [0u8; 16];
        dest.copy_from_slice(&frame[1..HEADER_LEN]);
        let dest = PeerId(dest);
        let Some(dest_conn) = self.get(&dest) else {
            self.metrics.frames_dropped.fetch_add(1, Ordering::Relaxed);
            return;
        };
        if dest == src || !self.linked(src, src_conn, dest, &dest_conn) {
            self.metrics.frames_dropped.fetch_add(1, Ordering::Relaxed);
            return;
        }
        let len = frame.len() as u64;
        // Rewrite the header in place when we own the only reference; copy otherwise.
        let mut buf = frame
            .try_into_mut()
            .unwrap_or_else(|shared| BytesMut::from(&shared[..]));
        buf[0] = FRAME_RELAY;
        buf[1..HEADER_LEN].copy_from_slice(&src.0);
        match dest_conn
            .tx
            .send_timeout(Out::Binary(buf.freeze()), SLOW_CONSUMER)
            .await
        {
            Ok(()) => {
                self.metrics.frames_relayed.fetch_add(1, Ordering::Relaxed);
                self.metrics.bytes_relayed.fetch_add(len, Ordering::Relaxed);
            }
            Err(mpsc::error::SendTimeoutError::Timeout(_)) => {
                self.metrics.slow_consumers.fetch_add(1, Ordering::Relaxed);
                dest_conn.close(4008, "slow consumer");
            }
            Err(mpsc::error::SendTimeoutError::Closed(_)) => {
                self.metrics.frames_dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Drops expired rooms, temporary links and rate-limit windows.
    pub fn sweep(&self) {
        let now = Instant::now();
        self.rooms.retain(|_, r| r.expires > now);
        self.temp_links.retain(|_, exp| *exp > now);
        self.join_attempts
            .retain(|_, (_, start)| now.duration_since(*start) <= JOIN_WINDOW);
    }
}
