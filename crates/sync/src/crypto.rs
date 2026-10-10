//! The keys: one symmetric key per synced workspace (it encrypts everything the relay keeps) and
//! one Ed25519 identity per device (it signs in to the relay).
//!
//! - Data: XChaCha20-Poly1305 with a random 24-byte nonce per message. The additional data binds
//!   each message to its room, its kind (update, snapshot, acknowledgement, device name) and the
//!   device that sent it, so the relay can neither read, change, relabel nor replay one as another.
//! - The room id is a hash of the key: unguessable, and it tells the relay nothing about the key.
//! - Keys never appear in `Debug` output, logs or URLs.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};

use crate::{Error, Result, b64};

/// What a sealed message is; part of the additional data, so one kind cannot pass for another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Update,
    Snapshot,
    Ack,
    Name,
}

impl Kind {
    fn tag(self) -> u8 {
        match self {
            Kind::Update => b'u',
            Kind::Snapshot => b's',
            Kind::Ack => b'a',
            Kind::Name => b'n',
        }
    }
}

const SEALED_VERSION: u8 = 1;
const NONCE: usize = 24;

/// Random bytes from the operating system.
pub fn random<const N: usize>() -> Result<[u8; N]> {
    let mut b = [0u8; N];
    getrandom::getrandom(&mut b).map_err(|e| Error::Crypto(format!("no randomness: {e}")))?;
    Ok(b)
}

fn sha256(parts: &[&[u8]]) -> [u8; 32] {
    let mut h = Sha256::new();
    for p in parts {
        h.update(p);
    }
    h.finalize().into()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The workspace key: whoever has it can read and write the synced notes.
#[derive(Clone, PartialEq, Eq)]
pub struct WorkspaceKey([u8; 32]);

impl std::fmt::Debug for WorkspaceKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("WorkspaceKey(..)")
    }
}

impl WorkspaceKey {
    pub fn generate() -> Result<WorkspaceKey> {
        Ok(WorkspaceKey(random()?))
    }

    pub fn from_bytes(b: &[u8]) -> Result<WorkspaceKey> {
        Ok(WorkspaceKey(b.try_into().map_err(|_| Error::Crypto("a workspace key is 32 bytes".into()))?))
    }

    pub fn to_b64(&self) -> String {
        b64::encode(&self.0)
    }

    pub fn from_b64(s: &str) -> Result<WorkspaceKey> {
        WorkspaceKey::from_bytes(&b64::decode(s).ok_or_else(|| Error::Crypto("bad workspace key".into()))?)
    }

    /// The room on the relay: 128 bits of a hash of the key, as hex.
    pub fn room(&self) -> String {
        hex(&sha256(&[b"tanacraft-sync/room/v1\0", &self.0])[..16])
    }

    fn cipher(&self) -> XChaCha20Poly1305 {
        let k = sha256(&[b"tanacraft-sync/data/v1\0", &self.0]);
        XChaCha20Poly1305::new(&k.into())
    }

    fn aad(room: &str, kind: Kind, sender: &str) -> Vec<u8> {
        let mut a = b"tanacraft-sync/v1\0".to_vec();
        a.extend_from_slice(room.as_bytes());
        a.push(0);
        a.push(kind.tag());
        a.extend_from_slice(sender.as_bytes());
        a
    }

    /// Encrypts `plain` as a message of `kind` from device `sender` in `room`.
    pub fn seal(&self, room: &str, kind: Kind, sender: &str, plain: &[u8]) -> Result<Vec<u8>> {
        let nonce: [u8; NONCE] = random()?;
        let aad = WorkspaceKey::aad(room, kind, sender);
        let ct = self
            .cipher()
            .encrypt(XNonce::from_slice(&nonce), Payload { msg: plain, aad: &aad })
            .map_err(|_| Error::Crypto("could not encrypt".into()))?;
        let mut out = Vec::with_capacity(1 + NONCE + ct.len());
        out.push(SEALED_VERSION);
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ct);
        Ok(out)
    }

    /// Decrypts a message sealed by [`WorkspaceKey::seal`]; any change to it, another key, room,
    /// kind or sender fails.
    pub fn open(&self, room: &str, kind: Kind, sender: &str, sealed: &[u8]) -> Result<Vec<u8>> {
        let bad = || Error::Crypto("a message could not be decrypted: wrong key, or changed on the way".into());
        if sealed.len() < 1 + NONCE + 16 || sealed[0] != SEALED_VERSION {
            return Err(bad());
        }
        let aad = WorkspaceKey::aad(room, kind, sender);
        self.cipher().decrypt(XNonce::from_slice(&sealed[1..1 + NONCE]), Payload { msg: &sealed[1 + NONCE..], aad: &aad }).map_err(|_| bad())
    }
}

/// A device's identity: an Ed25519 key pair; the public half (base64) is the device id.
#[derive(Clone)]
pub struct DeviceKey(SigningKey);

impl std::fmt::Debug for DeviceKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DeviceKey({})", self.id())
    }
}

impl DeviceKey {
    pub fn generate() -> Result<DeviceKey> {
        Ok(DeviceKey(SigningKey::from_bytes(&random()?)))
    }

    pub fn from_b64(seed: &str) -> Result<DeviceKey> {
        let b = b64::decode(seed).ok_or_else(|| Error::Crypto("bad device key".into()))?;
        let seed: [u8; 32] = b.as_slice().try_into().map_err(|_| Error::Crypto("a device key is 32 bytes".into()))?;
        Ok(DeviceKey(SigningKey::from_bytes(&seed)))
    }

    /// The secret half, for the config file.
    pub fn to_b64(&self) -> String {
        b64::encode(self.0.as_bytes())
    }

    /// The device id: the public key, base64.
    pub fn id(&self) -> String {
        b64::encode(self.0.verifying_key().as_bytes())
    }

    /// The answer to the relay's challenge `nonce` for `room`.
    pub fn sign_hello(&self, room: &str, nonce: &str) -> String {
        b64::encode(&self.0.sign(hello_message(room, nonce).as_bytes()).to_bytes())
    }
}

/// What a device signs to sign in: the room and the relay's one-time challenge.
pub fn hello_message(room: &str, nonce: &str) -> String {
    format!("tanacraft-sync/hello/v1:{room}:{nonce}")
}

/// Checks a hello signature (the relay does the same in JavaScript; the fake relay in tests uses this).
pub fn verify_hello(device: &str, room: &str, nonce: &str, sig: &str) -> bool {
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};
    let (Some(pk), Some(sig)) = (b64::decode(device), b64::decode(sig)) else { return false };
    let (Ok(pk), Ok(sig)) = (<[u8; 32]>::try_from(pk.as_slice()), <[u8; 64]>::try_from(sig.as_slice())) else { return false };
    let Ok(vk) = VerifyingKey::from_bytes(&pk) else { return false };
    vk.verify(hello_message(room, nonce).as_bytes(), &Signature::from_bytes(&sig)).is_ok()
}

/// What the relay keeps of an invite: a hash of its one-time secret.
pub fn invite_hash(secret: &[u8]) -> String {
    b64::encode(&sha256(&[secret]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealed_messages_open_only_as_they_were_sealed() {
        let k = WorkspaceKey::generate().unwrap();
        let room = k.room();
        assert_eq!(room.len(), 32);
        let s = k.seal(&room, Kind::Update, "me", b"hello").unwrap();
        assert_eq!(k.open(&room, Kind::Update, "me", &s).unwrap(), b"hello");
        assert!(k.open(&room, Kind::Snapshot, "me", &s).is_err(), "kind is bound");
        assert!(k.open(&room, Kind::Update, "you", &s).is_err(), "sender is bound");
        assert!(k.open("00", Kind::Update, "me", &s).is_err(), "room is bound");
        let mut t = s.clone();
        let last = t.len() - 1;
        t[last] ^= 1;
        assert!(k.open(&room, Kind::Update, "me", &t).is_err(), "tampering is caught");
        let other = WorkspaceKey::generate().unwrap();
        assert!(other.open(&room, Kind::Update, "me", &s).is_err(), "another key fails");
        assert!(!format!("{k:?}").contains(&k.to_b64()), "Debug hides the key");
    }

    #[test]
    fn hello_signatures_verify() {
        let d = DeviceKey::generate().unwrap();
        let sig = d.sign_hello("room", "nonce");
        assert!(verify_hello(&d.id(), "room", "nonce", &sig));
        assert!(!verify_hello(&d.id(), "room", "other", &sig));
        let again = DeviceKey::from_b64(&d.to_b64()).unwrap();
        assert_eq!(again.id(), d.id());
    }
}
