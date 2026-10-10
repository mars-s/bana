//! Bana ID keys (docs/bana-id.md): the workspace key is kept on the relay only wrapped, once per
//! passkey (by a key derived from the passkey's PRF output, which never leaves the devices) and
//! once by the printable recovery code. The relay never sees an unwrapped key.
//!
//! - [`prf_wrapping_key`]: a passkey's PRF output to a wrapping key.
//! - [`RecoveryCode`]: 125 random bits as 25 letters and digits; it gives a wrapping key and,
//!   separately, the sign-in proof the relay keeps a hash of.
//! - [`wrap`] / [`unwrap`]: XChaCha20-Poly1305 bound to the account and the kind of wrap.
//! - [`Ephemeral`]: the sign-in page on the relay hands the PRF output back masked with an X25519
//!   shared secret, so the relay only carries ciphertext between the page and the app.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use sha2::{Digest, Sha256};

use crate::crypto::{WorkspaceKey, random};
use crate::{Error, Result, b64};

const WRAP_VERSION: u8 = 1;
const NONCE: usize = 24;
/// Crockford's base32: no I, L, O or U, so a code read aloud or retyped survives.
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const CODE_LEN: usize = 25;

fn sha256(parts: &[&[u8]]) -> [u8; 32] {
    let mut h = Sha256::new();
    for p in parts {
        h.update(p);
    }
    h.finalize().into()
}

/// The salt every Bana sign-in asks the passkey's PRF to evaluate (the page sends the same).
pub fn prf_salt() -> [u8; 32] {
    sha256(&[b"bana-id/prf/v1"])
}

/// A wrapping key: whoever has it can unwrap the workspace key it wrapped.
#[derive(Clone, PartialEq, Eq)]
pub struct WrappingKey([u8; 32]);

impl std::fmt::Debug for WrappingKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("WrappingKey(..)")
    }
}

/// The wrapping key for one passkey, from its PRF output.
pub fn prf_wrapping_key(prf: &[u8]) -> Result<WrappingKey> {
    if prf.len() < 32 {
        return Err(Error::Crypto("the passkey's PRF output is too short".into()));
    }
    Ok(WrappingKey(sha256(&[b"bana-id/prf-wrap/v1\0", prf])))
}

/// What a wrap is for; bound into it, so a recovery wrap cannot pass for a passkey wrap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WrapKind {
    Passkey,
    Recovery,
}

fn aad(account: &str, kind: WrapKind) -> Vec<u8> {
    let mut a = b"bana-id/wrap/v1\0".to_vec();
    a.extend_from_slice(account.as_bytes());
    a.push(0);
    a.push(match kind {
        WrapKind::Passkey => b'p',
        WrapKind::Recovery => b'r',
    });
    a
}

/// The workspace key wrapped for `account` (base64, what the relay stores).
pub fn wrap(by: &WrappingKey, account: &str, kind: WrapKind, key: &WorkspaceKey) -> Result<String> {
    let nonce: [u8; NONCE] = random()?;
    let raw = b64::decode(&key.to_b64()).ok_or_else(|| Error::Crypto("bad workspace key".into()))?;
    let ct = XChaCha20Poly1305::new(&by.0.into())
        .encrypt(XNonce::from_slice(&nonce), Payload { msg: &raw, aad: &aad(account, kind) })
        .map_err(|_| Error::Crypto("could not wrap the key".into()))?;
    let mut out = vec![WRAP_VERSION];
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    Ok(b64::encode(&out))
}

/// Unwraps what [`wrap`] made; another key, account or kind, or any change, fails.
pub fn unwrap(by: &WrappingKey, account: &str, kind: WrapKind, blob: &str) -> Result<WorkspaceKey> {
    let bad = || Error::Crypto("the key could not be unlocked: wrong passkey or recovery code, or the stored copy was changed".into());
    let b = b64::decode(blob.trim()).ok_or_else(bad)?;
    if b.len() < 1 + NONCE + 16 || b[0] != WRAP_VERSION {
        return Err(bad());
    }
    let plain = XChaCha20Poly1305::new(&by.0.into())
        .decrypt(XNonce::from_slice(&b[1..1 + NONCE]), Payload { msg: &b[1 + NONCE..], aad: &aad(account, kind) })
        .map_err(|_| bad())?;
    WorkspaceKey::from_bytes(&plain).map_err(|_| bad())
}

/// The printable recovery code. Shown once; the relay keeps only a hash of its sign-in proof.
#[derive(Clone, PartialEq, Eq)]
pub struct RecoveryCode(String);

impl std::fmt::Debug for RecoveryCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RecoveryCode(..)")
    }
}

impl RecoveryCode {
    pub fn generate() -> Result<RecoveryCode> {
        let bytes: [u8; CODE_LEN] = random()?;
        Ok(RecoveryCode(bytes.iter().map(|b| ALPHABET[(*b & 31) as usize] as char).collect()))
    }

    /// Reads a code as typed: any case, spaces and dashes ignored, O read as 0, I and L as 1.
    pub fn parse(text: &str) -> Result<RecoveryCode> {
        let mut out = String::new();
        for c in text.chars().filter(|c| !c.is_whitespace() && *c != '-') {
            let c = match c.to_ascii_uppercase() {
                'O' => '0',
                'I' | 'L' => '1',
                c => c,
            };
            if !c.is_ascii() || !ALPHABET.contains(&(c as u8)) {
                return Err(Error::Config("a recovery code has only letters and digits".into()));
            }
            out.push(c);
        }
        if out.len() != CODE_LEN {
            return Err(Error::Config(format!("a recovery code has {CODE_LEN} letters and digits")));
        }
        Ok(RecoveryCode(out))
    }

    /// For showing and printing: five groups of five.
    pub fn display(&self) -> String {
        self.0.as_bytes().chunks(5).map(|c| String::from_utf8_lossy(c).into_owned()).collect::<Vec<_>>().join("-")
    }

    pub fn wrapping_key(&self) -> WrappingKey {
        WrappingKey(sha256(&[b"bana-id/recovery-wrap/v1\0", self.0.as_bytes()]))
    }

    /// What the device shows the relay to sign in with the code (base64). The relay stores a
    /// hash of it; neither tells anything about the wrapping key.
    pub fn proof(&self) -> String {
        b64::encode(&sha256(&[b"bana-id/recovery-auth/v1\0", self.0.as_bytes()]))
    }
}

/// The app's half of the hand-back from the sign-in page: an X25519 key pair made for one sign-in.
pub struct Ephemeral(x25519_dalek::StaticSecret);

impl Ephemeral {
    pub fn generate() -> Result<Ephemeral> {
        Ok(Ephemeral(x25519_dalek::StaticSecret::from(random::<32>()?)))
    }

    /// The public key, base64url (it goes in the page's address).
    pub fn public(&self) -> String {
        b64url(x25519_dalek::PublicKey::from(&self.0).as_bytes())
    }

    /// The PRF output the page masked with the secret shared with `page_public` (base64url).
    pub fn unmask(&self, page_public: &str, masked: &str) -> Result<Vec<u8>> {
        let bad = || Error::Crypto("the sign-in page's answer is malformed".into());
        let pk: [u8; 32] = from_b64url(page_public).ok_or_else(bad)?.try_into().map_err(|_| bad())?;
        let masked = from_b64url(masked).ok_or_else(bad)?;
        if masked.len() != 32 {
            return Err(bad());
        }
        let pk = x25519_dalek::PublicKey::from(pk);
        let shared = self.0.diffie_hellman(&pk);
        let me = x25519_dalek::PublicKey::from(&self.0);
        let pad = sha256(&[b"bana-id/transport/v1\0", shared.as_bytes(), me.as_bytes(), pk.as_bytes()]);
        Ok(masked.iter().zip(pad).map(|(a, b)| a ^ b).collect())
    }

    /// What the page does (for tests): masks `prf` for the app whose public key is `app_public`.
    pub fn mask_for(&self, app_public: &str, prf: &[u8; 32]) -> Result<String> {
        let pk: [u8; 32] = from_b64url(app_public).and_then(|b| b.try_into().ok()).ok_or_else(|| Error::Crypto("bad key".into()))?;
        let pk = x25519_dalek::PublicKey::from(pk);
        let shared = self.0.diffie_hellman(&pk);
        let me = x25519_dalek::PublicKey::from(&self.0);
        let pad = sha256(&[b"bana-id/transport/v1\0", shared.as_bytes(), pk.as_bytes(), me.as_bytes()]);
        Ok(b64url(&prf.iter().zip(pad).map(|(a, b)| a ^ b).collect::<Vec<u8>>()))
    }
}

/// Base64url without padding (WebAuthn's encoding).
pub fn b64url(b: &[u8]) -> String {
    b64::encode(b).trim_end_matches('=').replace('+', "-").replace('/', "_")
}

pub fn from_b64url(s: &str) -> Option<Vec<u8>> {
    let mut t = s.trim().replace('-', "+").replace('_', "/");
    while !t.len().is_multiple_of(4) {
        t.push('=');
    }
    b64::decode(&t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_prf_wrapped_key_unwraps_only_with_that_passkey() {
        let key = WorkspaceKey::generate().unwrap();
        let k = prf_wrapping_key(&[7u8; 32]).unwrap();
        let blob = wrap(&k, "acct", WrapKind::Passkey, &key).unwrap();
        assert_eq!(unwrap(&k, "acct", WrapKind::Passkey, &blob).unwrap(), key);
        let other = prf_wrapping_key(&[8u8; 32]).unwrap();
        assert!(unwrap(&other, "acct", WrapKind::Passkey, &blob).is_err(), "another passkey fails");
        assert!(unwrap(&k, "other", WrapKind::Passkey, &blob).is_err(), "the account is bound");
        assert!(unwrap(&k, "acct", WrapKind::Recovery, &blob).is_err(), "the kind is bound");
        assert!(prf_wrapping_key(&[1u8; 8]).is_err());
        assert!(!format!("{k:?}").contains('7'));
    }

    #[test]
    fn a_recovery_code_wraps_and_reads_back_as_typed() {
        let key = WorkspaceKey::generate().unwrap();
        let code = RecoveryCode::generate().unwrap();
        let shown = code.display();
        assert_eq!(shown.len(), 29);
        let blob = wrap(&code.wrapping_key(), "acct", WrapKind::Recovery, &key).unwrap();
        // Typed back in lower case with spaces instead of dashes.
        let typed = RecoveryCode::parse(&shown.to_lowercase().replace('-', " ")).unwrap();
        assert_eq!(unwrap(&typed.wrapping_key(), "acct", WrapKind::Recovery, &blob).unwrap(), key);
        let wrong = RecoveryCode::generate().unwrap();
        assert!(unwrap(&wrong.wrapping_key(), "acct", WrapKind::Recovery, &blob).is_err(), "a wrong code fails");
        assert_ne!(code.proof(), wrong.proof());
        assert_ne!(code.proof(), b64::encode(&code.wrapping_key().0), "the proof is not the key");
        assert!(RecoveryCode::parse("too short").is_err());
        assert!(RecoveryCode::parse(&format!("{shown}U")).is_err());
    }

    #[test]
    fn a_tampered_blob_fails() {
        let key = WorkspaceKey::generate().unwrap();
        let k = prf_wrapping_key(&[3u8; 32]).unwrap();
        let blob = wrap(&k, "acct", WrapKind::Passkey, &key).unwrap();
        let mut raw = b64::decode(&blob).unwrap();
        for i in [0, 5, raw.len() - 1] {
            raw[i] ^= 1;
            assert!(unwrap(&k, "acct", WrapKind::Passkey, &b64::encode(&raw)).is_err(), "byte {i} changed");
            raw[i] ^= 1;
        }
        assert!(unwrap(&k, "acct", WrapKind::Passkey, "not base64!").is_err());
        assert!(unwrap(&k, "acct", WrapKind::Passkey, &b64::encode(&raw[..20])).is_err(), "truncated");
    }

    #[test]
    fn the_page_hands_the_prf_output_back_masked() {
        let app = Ephemeral::generate().unwrap();
        let page = Ephemeral::generate().unwrap();
        let prf = [42u8; 32];
        let masked = page.mask_for(&app.public(), &prf).unwrap();
        assert_ne!(from_b64url(&masked).unwrap(), prf.to_vec());
        assert_eq!(app.unmask(&page.public(), &masked).unwrap(), prf.to_vec());
        let other = Ephemeral::generate().unwrap();
        assert_ne!(other.unmask(&page.public(), &masked).unwrap(), prf.to_vec());
        assert_eq!(from_b64url(&b64url(&[251, 255, 1])).unwrap(), vec![251, 255, 1]);
    }
}
