//! Wire types for protocol v3 (see docs/protocol-v3.md).

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use p256::ecdsa::{signature::Verifier, Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

pub const PROTOCOL: u8 = 3;
pub const AUTH_CONTEXT: &[u8] = b"pairspan/v3/auth";
pub const FRAME_RELAY: u8 = 0x01;
pub const HEADER_LEN: usize = 17;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PeerId(pub [u8; 16]);

impl PeerId {
    pub fn from_pubkey(compressed: &[u8]) -> Self {
        let digest = Sha256::digest(compressed);
        let mut id = [0u8; 16];
        id.copy_from_slice(&digest[..16]);
        PeerId(id)
    }

    pub fn parse_hex(s: &str) -> Option<Self> {
        if s.len() != 32 {
            return None;
        }
        let mut id = [0u8; 16];
        for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
            id[i] = (hex_val(chunk[0])? << 4) | hex_val(chunk[1])?;
        }
        Some(PeerId(id))
    }
}

fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    }
}

impl fmt::Display for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for b in self.0 {
            write!(f, "{b:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Short form keeps logs readable and avoids printing full identities.
        write!(f, "{}", &self.to_string()[..8])
    }
}

impl Serialize for PeerId {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

/// Room ids are SHA-256 digests (64 hex chars) of the pair secret; the gateway never sees the secret.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct RoomId(pub [u8; 32]);

impl RoomId {
    pub fn parse_hex(s: &str) -> Option<Self> {
        if s.len() != 64 {
            return None;
        }
        let mut id = [0u8; 32];
        for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
            id[i] = (hex_val(chunk[0])? << 4) | hex_val(chunk[1])?;
        }
        Some(RoomId(id))
    }
}

impl fmt::Display for RoomId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for b in self.0 {
            write!(f, "{b:02x}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Pc,
    Phone,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ClientMsg {
    Hello {
        role: Role,
        #[serde(rename = "pub")]
        pubkey: String,
        sig: String,
        #[serde(default)]
        peers: Vec<String>,
        #[serde(default)]
        token: Option<String>,
    },
    Peers {
        ids: Vec<String>,
    },
    RoomOpen {
        room: String,
    },
    RoomClose {
        room: String,
    },
    RoomJoin {
        room: String,
    },
}

#[derive(Debug, Serialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ServerMsg<'a> {
    Challenge {
        v: u8,
        nonce: String,
    },
    Welcome {
        id: PeerId,
        online: Vec<PeerId>,
    },
    Presence {
        id: PeerId,
        online: bool,
    },
    RoomOpened {
        room: String,
        expires_in: u64,
    },
    RoomPeer {
        room: String,
        id: PeerId,
        #[serde(rename = "pub")]
        pubkey: &'a str,
        role: Role,
    },
    Error {
        code: &'a str,
        msg: &'a str,
    },
}

impl ServerMsg<'_> {
    pub fn json(&self) -> String {
        serde_json::to_string(self).expect("server messages always serialize")
    }
}

/// Verifies the hello signature and returns the normalized (compressed) key in base64 and the peer id.
pub fn verify_hello(pubkey_b64: &str, sig_b64: &str, nonce: &[u8]) -> Option<(String, PeerId)> {
    let pub_bytes = B64.decode(pubkey_b64).ok()?;
    let sig_bytes = B64.decode(sig_b64).ok()?;
    let key = VerifyingKey::from_sec1_bytes(&pub_bytes).ok()?;
    let sig = Signature::from_der(&sig_bytes)
        .or_else(|_| Signature::from_slice(&sig_bytes))
        .ok()?;
    let mut msg = Vec::with_capacity(AUTH_CONTEXT.len() + nonce.len());
    msg.extend_from_slice(AUTH_CONTEXT);
    msg.extend_from_slice(nonce);
    key.verify(&msg, &sig).ok()?;
    let compressed = key.to_encoded_point(true);
    let compressed = compressed.as_bytes();
    Some((B64.encode(compressed), PeerId::from_pubkey(compressed)))
}

pub fn encode_nonce(nonce: &[u8]) -> String {
    B64.encode(nonce)
}
