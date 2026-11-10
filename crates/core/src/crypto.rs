//! End-to-end layer (docs/protocol.md §3): handshake and AES-256-GCM data frames.

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{bail, ensure, Context, Result};
use hkdf::Hkdf;
use p256::elliptic_curve::sec1::ToEncodedPoint;
use p256::{PublicKey, SecretKey};
use sha2::{Digest, Sha256};

pub const HS_INIT: u8 = 0x10;
pub const HS_RESP: u8 = 0x11;
pub const DATA: u8 = 0x20;
pub const REKEY: u8 = 0x2F;

const TAG_LEN: usize = 16;
/// Start a new handshake well before the 64-bit counter or the GCM limits matter.
pub const MAX_FRAMES: u64 = 1 << 32;

/// Pre-shared key for the first (pairing) handshake: binds it to the pair secret the gateway never sees.
pub fn pairing_psk(secret: &str) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"pairspan/v1/psk");
    h.update(secret.as_bytes());
    h.finalize().into()
}

/// Room id the gateway sees for a pair secret.
pub fn room_id(secret: &str) -> String {
    let mut h = Sha256::new();
    h.update(b"pairspan/v1/room");
    h.update(secret.as_bytes());
    crate::identity::to_hex(&h.finalize())
}

fn compressed(public: &PublicKey) -> [u8; 33] {
    public
        .to_encoded_point(true)
        .as_bytes()
        .try_into()
        .expect("compressed point is 33 bytes")
}

fn ecdh(secret: &SecretKey, public: &PublicKey) -> [u8; 32] {
    let shared = p256::ecdh::diffie_hellman(secret.to_nonzero_scalar(), public.as_affine());
    (*shared.raw_secret_bytes()).into()
}

fn parse_public(bytes: &[u8]) -> Result<PublicKey> {
    PublicKey::from_sec1_bytes(bytes).context("invalid P-256 public key")
}

struct Keys {
    i2r: [u8; 32],
    r2i: [u8; 32],
    confirm: [u8; 16],
}

/// `th` and the derived keys; identical on both sides given the same transcript.
fn derive(
    s_i: &[u8; 33],
    s_r: &[u8; 33],
    e_i: &[u8; 33],
    e_r: &[u8; 33],
    dh: [[u8; 32]; 4],
    psk: &[u8; 32],
) -> Keys {
    let mut th = Sha256::new();
    th.update(b"pairspan/v1/hs");
    th.update(s_i);
    th.update(s_r);
    th.update(e_i);
    th.update(e_r);
    let th: [u8; 32] = th.finalize().into();

    let mut ikm = Vec::with_capacity(160);
    for part in &dh {
        ikm.extend_from_slice(part);
    }
    ikm.extend_from_slice(psk);
    let mut okm = [0u8; 96];
    Hkdf::<Sha256>::new(Some(&th), &ikm)
        .expand(b"pairspan/v1/keys", &mut okm)
        .expect("96 bytes is a valid HKDF length");

    let k_confirm: [u8; 32] = okm[64..96].try_into().unwrap();
    let tag = Aes256Gcm::new(&k_confirm.into())
        .encrypt(Nonce::from_slice(&[0u8; 12]), Payload { msg: &[], aad: &th })
        .expect("empty plaintext always encrypts");
    Keys {
        i2r: okm[0..32].try_into().unwrap(),
        r2i: okm[32..64].try_into().unwrap(),
        confirm: tag.as_slice().try_into().unwrap(),
    }
}

/// Initiator side (always the PC). Keep it until `HS_RESP` arrives.
pub struct Initiator {
    s: SecretKey,
    s_pub: [u8; 33],
    peer: PublicKey,
    peer_pub: [u8; 33],
    e: SecretKey,
    e_pub: [u8; 33],
    psk: [u8; 32],
}

impl Initiator {
    /// Returns the initiator state and the `HS_INIT` frame to send.
    pub fn start(static_key: &SecretKey, peer_pub: &[u8; 33], psk: Option<[u8; 32]>) -> Result<(Self, Vec<u8>)> {
        Self::start_with(static_key, peer_pub, psk, SecretKey::random(&mut rand::rngs::OsRng))
    }

    /// Same as `start` with a caller-chosen ephemeral key (test vectors only).
    pub fn start_with(
        static_key: &SecretKey,
        peer_pub: &[u8; 33],
        psk: Option<[u8; 32]>,
        e: SecretKey,
    ) -> Result<(Self, Vec<u8>)> {
        let peer = parse_public(peer_pub)?;
        let e_pub = compressed(&e.public_key());
        let mut frame = Vec::with_capacity(34);
        frame.push(HS_INIT);
        frame.extend_from_slice(&e_pub);
        let me = Self {
            s_pub: compressed(&static_key.public_key()),
            s: static_key.clone(),
            peer,
            peer_pub: *peer_pub,
            e,
            e_pub,
            psk: psk.unwrap_or([0; 32]),
        };
        Ok((me, frame))
    }

    pub fn finish(self, hs_resp: &[u8]) -> Result<Session> {
        ensure!(hs_resp.len() == 1 + 33 + TAG_LEN && hs_resp[0] == HS_RESP, "malformed HS_RESP");
        let e_r_bytes: [u8; 33] = hs_resp[1..34].try_into().unwrap();
        let e_r = parse_public(&e_r_bytes)?;
        let dh = [
            ecdh(&self.e, &e_r),     // ee
            ecdh(&self.e, &self.peer), // es
            ecdh(&self.s, &e_r),     // se
            ecdh(&self.s, &self.peer), // ss
        ];
        let keys = derive(&self.s_pub, &self.peer_pub, &self.e_pub, &e_r_bytes, dh, &self.psk);
        if !bool::from(subtle_eq(&keys.confirm, &hs_resp[34..])) {
            bail!("handshake confirmation failed (wrong key or pair secret)");
        }
        Ok(Session::new(keys.i2r, keys.r2i))
    }
}

/// Responder side (the phone). Returns the session and the `HS_RESP` frame.
pub fn respond(static_key: &SecretKey, peer_pub: &[u8; 33], psk: Option<[u8; 32]>, hs_init: &[u8]) -> Result<(Session, Vec<u8>)> {
    respond_with(static_key, peer_pub, psk, hs_init, SecretKey::random(&mut rand::rngs::OsRng))
}

pub fn respond_with(
    static_key: &SecretKey,
    peer_pub: &[u8; 33],
    psk: Option<[u8; 32]>,
    hs_init: &[u8],
    e: SecretKey,
) -> Result<(Session, Vec<u8>)> {
    ensure!(hs_init.len() == 34 && hs_init[0] == HS_INIT, "malformed HS_INIT");
    let e_i_bytes: [u8; 33] = hs_init[1..34].try_into().unwrap();
    let e_i = parse_public(&e_i_bytes)?;
    let peer = parse_public(peer_pub)?;
    let s_pub = compressed(&static_key.public_key());
    let e_pub = compressed(&e.public_key());
    let dh = [
        ecdh(&e, &e_i),          // ee
        ecdh(static_key, &e_i),  // es
        ecdh(&e, &peer),         // se
        ecdh(static_key, &peer), // ss
    ];
    let keys = derive(peer_pub, &s_pub, &e_i_bytes, &e_pub, dh, &psk.unwrap_or([0; 32]));
    let mut frame = Vec::with_capacity(50);
    frame.push(HS_RESP);
    frame.extend_from_slice(&e_pub);
    frame.extend_from_slice(&keys.confirm);
    Ok((Session::new(keys.r2i, keys.i2r), frame))
}

fn subtle_eq(a: &[u8], b: &[u8]) -> subtle_bool::Choice {
    subtle_bool::ct_eq(a, b)
}

/// Tiny constant-time compare without pulling another dependency in.
mod subtle_bool {
    pub struct Choice(bool);
    impl From<Choice> for bool {
        fn from(c: Choice) -> bool {
            c.0
        }
    }
    pub fn ct_eq(a: &[u8], b: &[u8]) -> Choice {
        if a.len() != b.len() {
            return Choice(false);
        }
        let diff = a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y));
        Choice(std::hint::black_box(diff) == 0)
    }
}

pub struct Session {
    pub tx: Sealer,
    pub rx: Opener,
}

impl Session {
    fn new(tx: [u8; 32], rx: [u8; 32]) -> Self {
        Self {
            tx: Sealer { cipher: Aes256Gcm::new(&tx.into()), counter: 0 },
            rx: Opener { cipher: Aes256Gcm::new(&rx.into()), last: None },
        }
    }
}

fn nonce(counter: u64) -> [u8; 12] {
    let mut n = [0u8; 12];
    n[4..].copy_from_slice(&counter.to_be_bytes());
    n
}

pub struct Sealer {
    cipher: Aes256Gcm,
    counter: u64,
}

impl Sealer {
    /// `DATA = 0x20 | counter (8) | ciphertext || tag`.
    pub fn seal(&mut self, plaintext: &[u8]) -> Result<Vec<u8>> {
        ensure!(self.counter < MAX_FRAMES, "session exhausted, rekey required");
        let counter = self.counter;
        self.counter += 1;
        let mut header = [0u8; 9];
        header[0] = DATA;
        header[1..].copy_from_slice(&counter.to_be_bytes());
        let ct = self
            .cipher
            .encrypt(Nonce::from_slice(&nonce(counter)), Payload { msg: plaintext, aad: &header })
            .map_err(|_| anyhow::anyhow!("encryption failed"))?;
        let mut frame = Vec::with_capacity(9 + ct.len());
        frame.extend_from_slice(&header);
        frame.extend_from_slice(&ct);
        Ok(frame)
    }
}

pub struct Opener {
    cipher: Aes256Gcm,
    last: Option<u64>,
}

impl Opener {
    pub fn open(&mut self, frame: &[u8]) -> Result<Vec<u8>> {
        ensure!(frame.len() >= 9 + TAG_LEN && frame[0] == DATA, "malformed DATA frame");
        let counter = u64::from_be_bytes(frame[1..9].try_into().unwrap());
        if self.last.is_some_and(|last| counter <= last) {
            bail!("replayed or reordered frame");
        }
        let pt = self
            .cipher
            .decrypt(Nonce::from_slice(&nonce(counter)), Payload { msg: &frame[9..], aad: &frame[..9] })
            .map_err(|_| anyhow::anyhow!("decryption failed"))?;
        self.last = Some(counter);
        Ok(pt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(seed: &str) -> SecretKey {
        SecretKey::from_slice(&Sha256::digest(seed.as_bytes())).unwrap()
    }

    #[test]
    fn handshake_and_data_roundtrip() {
        let (pc, phone) = (key("pc"), key("phone"));
        let (pc_pub, phone_pub) = (compressed(&pc.public_key()), compressed(&phone.public_key()));
        let psk = Some(pairing_psk("ABCDEFGHJKMNPQRSTUVW"));

        let (init, hs1) = Initiator::start(&pc, &phone_pub, psk).unwrap();
        let (mut phone_s, hs2) = respond(&phone, &pc_pub, psk, &hs1).unwrap();
        let mut pc_s = init.finish(&hs2).unwrap();

        let f = pc_s.tx.seal(b"hello phone").unwrap();
        assert_eq!(phone_s.rx.open(&f).unwrap(), b"hello phone");
        let g = phone_s.tx.seal(b"hello pc").unwrap();
        assert_eq!(pc_s.rx.open(&g).unwrap(), b"hello pc");
        // Replay is rejected.
        assert!(phone_s.rx.open(&f).is_err());
    }

    #[test]
    fn wrong_pair_secret_fails() {
        let (pc, phone) = (key("pc"), key("phone"));
        let (pc_pub, phone_pub) = (compressed(&pc.public_key()), compressed(&phone.public_key()));
        let (init, hs1) = Initiator::start(&pc, &phone_pub, Some(pairing_psk("AAAA"))).unwrap();
        let (_, hs2) = respond(&phone, &pc_pub, Some(pairing_psk("BBBB")), &hs1).unwrap();
        assert!(init.finish(&hs2).is_err());
    }

    #[test]
    fn substituted_static_key_fails() {
        // A gateway that swaps in its own key for the phone cannot complete the handshake.
        let (pc, phone, mallory) = (key("pc"), key("phone"), key("mallory"));
        let (pc_pub, phone_pub) = (compressed(&pc.public_key()), compressed(&phone.public_key()));
        let (init, hs1) = Initiator::start(&pc, &phone_pub, None).unwrap();
        let (_, hs2) = respond(&mallory, &pc_pub, None, &hs1).unwrap();
        assert!(init.finish(&hs2).is_err());
    }

    /// Writes docs/test-vectors.json, which the Android unit tests replay byte for byte.
    #[test]
    fn test_vectors() {
        let to_hex = crate::identity::to_hex;
        let (s_i, s_r, e_i, e_r) = (key("vector/s_i"), key("vector/s_r"), key("vector/e_i"), key("vector/e_r"));
        let (s_i_pub, s_r_pub) = (compressed(&s_i.public_key()), compressed(&s_r.public_key()));
        let secret = "K7M2-QR9P-X4WZ-H8TN-3ABC".replace('-', "");
        let psk = pairing_psk(&secret);
        let (init, hs_init) = Initiator::start_with(&s_i, &s_r_pub, Some(psk), e_i.clone()).unwrap();
        let (mut resp, hs_resp) = respond_with(&s_r, &s_i_pub, Some(psk), &hs_init, e_r.clone()).unwrap();
        let mut ini = init.finish(&hs_resp).unwrap();
        let data_i2r = ini.tx.seal(b"pairspan").unwrap();
        let data_r2i = resp.tx.seal(b"android").unwrap();
        let v = serde_json::json!({
            "secret": secret,
            "room": room_id(&secret),
            "psk": to_hex(&psk),
            "s_i": to_hex(&s_i.to_bytes()), "s_r": to_hex(&s_r.to_bytes()),
            "e_i": to_hex(&e_i.to_bytes()), "e_r": to_hex(&e_r.to_bytes()),
            "s_i_pub": to_hex(&s_i_pub), "s_r_pub": to_hex(&s_r_pub),
            "e_i_pub": to_hex(&compressed(&e_i.public_key())), "e_r_pub": to_hex(&compressed(&e_r.public_key())),
            "peer_id_s_i": crate::identity::PeerId::from_pubkey(&s_i_pub).to_string(),
            "hs_init": to_hex(&hs_init), "hs_resp": to_hex(&hs_resp),
            "data_i2r_pairspan": to_hex(&data_i2r), "data_r2i_android": to_hex(&data_r2i),
        });
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/test-vectors.json");
        let text = serde_json::to_string_pretty(&v).unwrap() + "\n";
        if std::fs::read_to_string(path).ok().as_deref() != Some(text.as_str()) {
            std::fs::write(path, text).unwrap();
        }
    }
}
