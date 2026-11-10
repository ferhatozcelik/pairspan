//! Long-term P-256 identity of this installation and the peer id derived from it.

use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use p256::ecdsa::{signature::Signer, Signature, SigningKey};
use sha2::{Digest, Sha256};
use std::fmt;
use std::path::Path;

pub const AUTH_CONTEXT: &[u8] = b"pairspan/v1/auth";

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PeerId(pub [u8; 16]);

impl PeerId {
    pub fn from_pubkey(compressed: &[u8]) -> Self {
        let digest = Sha256::digest(compressed);
        let mut id = [0u8; 16];
        id.copy_from_slice(&digest[..16]);
        PeerId(id)
    }

    pub fn parse(s: &str) -> Option<Self> {
        let bytes = from_hex(s)?;
        Some(PeerId(bytes.try_into().ok()?))
    }

    pub fn short(&self) -> String {
        self.to_string()[..8].to_string()
    }
}

impl fmt::Display for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&to_hex(&self.0))
    }
}

impl fmt::Debug for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.short())
    }
}

impl serde::Serialize for PeerId {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> serde::Deserialize<'de> for PeerId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        PeerId::parse(&s).ok_or_else(|| serde::de::Error::custom("invalid peer id"))
    }
}

pub fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn from_hex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

pub struct Identity {
    key: SigningKey,
    public: [u8; 33],
    id: PeerId,
}

impl Identity {
    pub fn from_key(key: SigningKey) -> Self {
        let point = key.verifying_key().to_encoded_point(true);
        let public: [u8; 33] = point.as_bytes().try_into().expect("compressed point is 33 bytes");
        let id = PeerId::from_pubkey(&public);
        Self { key, public, id }
    }

    pub fn generate() -> Self {
        Self::from_key(SigningKey::random(&mut rand::rngs::OsRng))
    }

    /// Loads the key from `path` (32-byte scalar, hex) or creates it with owner-only permissions.
    pub fn load_or_create(path: &Path) -> Result<Self> {
        if path.exists() {
            let hex = std::fs::read_to_string(path).context("read identity key")?;
            let bytes = from_hex(hex.trim()).context("identity key is not hex")?;
            let key = SigningKey::from_slice(&bytes).context("identity key is invalid")?;
            return Ok(Self::from_key(key));
        }
        let identity = Self::generate();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        crate::store::write_private(path, to_hex(&identity.key.to_bytes()).as_bytes())?;
        Ok(identity)
    }

    pub fn id(&self) -> PeerId {
        self.id
    }

    pub fn public(&self) -> &[u8; 33] {
        &self.public
    }

    pub fn public_b64(&self) -> String {
        B64.encode(self.public)
    }

    pub fn signing_key(&self) -> &SigningKey {
        &self.key
    }

    /// Signature for the gateway challenge: ECDSA-P256-SHA256 over `"pairspan/v1/auth" || nonce`, DER encoded.
    pub fn sign_challenge(&self, nonce: &[u8]) -> String {
        let mut msg = Vec::with_capacity(AUTH_CONTEXT.len() + nonce.len());
        msg.extend_from_slice(AUTH_CONTEXT);
        msg.extend_from_slice(nonce);
        let sig: Signature = self.key.sign(&msg);
        B64.encode(sig.to_der().as_bytes())
    }
}

/// Decodes a base64 compressed public key and checks it is a valid P-256 point.
pub fn decode_public(b64: &str) -> Result<[u8; 33]> {
    let bytes = B64.decode(b64).context("public key is not base64")?;
    let key = p256::PublicKey::from_sec1_bytes(&bytes).context("public key is not a P-256 point")?;
    let point = p256::elliptic_curve::sec1::ToEncodedPoint::to_encoded_point(&key, true);
    match point.as_bytes().try_into() {
        Ok(b) => Ok(b),
        Err(_) => bail!("unexpected public key length"),
    }
}
