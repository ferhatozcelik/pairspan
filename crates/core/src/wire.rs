//! Gateway control messages (client side) and the framing inside relay / DATA payloads.

use crate::identity::PeerId;
use serde::{Deserialize, Serialize};

pub const FRAME_RELAY: u8 = 0x01;

/// Application channels inside DATA plaintext (docs/protocol.md §3.3).
pub const CH_RPC: u8 = 0x00;
pub const CH_OPEN: u8 = 0x01;
pub const CH_DATA: u8 = 0x02;
pub const CH_CLOSE: u8 = 0x03;

/// Largest body we put in one DATA frame; keeps relay frames far below the 1 MiB gateway limit.
pub const MAX_CHUNK: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Pc,
    Phone,
}

#[derive(Debug, Serialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ToGateway<'a> {
    Hello {
        role: Role,
        #[serde(rename = "pub")]
        public: &'a str,
        sig: &'a str,
        peers: Vec<PeerId>,
        #[serde(skip_serializing_if = "Option::is_none")]
        token: Option<&'a str>,
    },
    Peers {
        ids: Vec<PeerId>,
    },
    RoomOpen {
        room: &'a str,
    },
    RoomClose {
        room: &'a str,
    },
    RoomJoin {
        room: &'a str,
    },
}

impl ToGateway<'_> {
    pub fn json(&self) -> String {
        serde_json::to_string(self).expect("serializable")
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum FromGateway {
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
        public: String,
        role: Role,
    },
    Error {
        code: String,
        msg: String,
    },
}

/// `0x01 | peer id (16) | payload`
pub fn relay_frame(peer: PeerId, payload: &[u8]) -> Vec<u8> {
    let mut f = Vec::with_capacity(17 + payload.len());
    f.push(FRAME_RELAY);
    f.extend_from_slice(&peer.0);
    f.extend_from_slice(payload);
    f
}

pub fn parse_relay(frame: &[u8]) -> Option<(PeerId, &[u8])> {
    if frame.len() < 18 || frame[0] != FRAME_RELAY {
        return None;
    }
    Some((PeerId(frame[1..17].try_into().ok()?), &frame[17..]))
}

/// `channel (1) | stream id (4) | body`
pub fn channel_frame(channel: u8, stream: u32, body: &[u8]) -> Vec<u8> {
    let mut f = Vec::with_capacity(5 + body.len());
    f.push(channel);
    f.extend_from_slice(&stream.to_be_bytes());
    f.extend_from_slice(body);
    f
}

pub fn parse_channel(plain: &[u8]) -> Option<(u8, u32, &[u8])> {
    if plain.len() < 5 {
        return None;
    }
    Some((plain[0], u32::from_be_bytes(plain[1..5].try_into().ok()?), &plain[5..]))
}

/// Pair secret: 20 characters without look-alikes (about 99 bits).
pub const SECRET_ALPHABET: &[u8] = b"23456789ABCDEFGHJKMNPQRSTUVWXYZ";

pub fn new_pair_secret() -> String {
    use rand::Rng;
    let mut rng = rand::rngs::OsRng;
    (0..20)
        .map(|_| SECRET_ALPHABET[rng.gen_range(0..SECRET_ALPHABET.len())] as char)
        .collect()
}

pub fn display_secret(secret: &str) -> String {
    secret
        .as_bytes()
        .chunks(4)
        .map(|c| std::str::from_utf8(c).unwrap())
        .collect::<Vec<_>>()
        .join("-")
}

pub fn normalize_secret(input: &str) -> String {
    input.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_uppercase()).collect()
}

/// `pairspan://pair?g=<gateway>&t=<secret>&k=<pc peer id>`
pub fn qr_payload(gateway: &str, secret: &str, pc: PeerId) -> String {
    let g: String = url::form_urlencoded::byte_serialize(gateway.as_bytes()).collect();
    format!("pairspan://pair?g={g}&t={secret}&k={pc}")
}

pub fn qr_svg(payload: &str) -> String {
    match qrcode::QrCode::with_error_correction_level(payload, qrcode::EcLevel::M) {
        Ok(code) => code
            .render::<qrcode::render::svg::Color>()
            .min_dimensions(240, 240)
            .quiet_zone(true)
            .dark_color(qrcode::render::svg::Color("#161616"))
            .light_color(qrcode::render::svg::Color("#fffef7"))
            .build(),
        Err(_) => String::new(),
    }
}

/// Terminal rendering for `pairspan pair`.
pub fn qr_text(payload: &str) -> String {
    match qrcode::QrCode::with_error_correction_level(payload, qrcode::EcLevel::L) {
        Ok(code) => code
            .render::<qrcode::render::unicode::Dense1x2>()
            .dark_color(qrcode::render::unicode::Dense1x2::Light)
            .light_color(qrcode::render::unicode::Dense1x2::Dark)
            .quiet_zone(true)
            .build(),
        Err(_) => String::new(),
    }
}
