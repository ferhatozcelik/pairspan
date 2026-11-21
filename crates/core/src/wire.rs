//! Framing inside DATA payloads, pair secrets and signalling channel names.

/// Application channels inside DATA plaintext (docs/protocol.md §3.3).
pub const CH_RPC: u8 = 0x00;
pub const CH_OPEN: u8 = 0x01;
pub const CH_DATA: u8 = 0x02;
pub const CH_CLOSE: u8 = 0x03;

/// Largest body we put in one DATA frame: with the 30 bytes of framing and tag a frame stays
/// under the 16 KiB message size every WebRTC stack accepts.
pub const MAX_CHUNK: usize = 16 * 1024 - 64;

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

/// Signalling channel shared by one PC and one phone. Only the two of them know both keys,
/// so nobody else can derive the name.
pub fn link_name(pc: &[u8; 33], phone: &[u8; 33]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(b"pairspan/v2/link");
    h.update(pc);
    h.update(phone);
    format!("l:{}", &crate::identity::to_hex(&h.finalize())[..40])
}

/// Signalling channel where a phone and PC exchange keys during a USB pairing.
pub fn room_name(room: &str) -> String {
    format!("r:{}", &room[..40.min(room.len())])
}
