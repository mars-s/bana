//! Pairing codes: what the desktop shows (as a QR code and as text) and the phone reads. A code
//! carries the relay's address, the workspace key, a one-time invite secret and who invited, so
//! it is as secret as the notes: it is shown on screen, never sent anywhere or logged.

use serde::{Deserialize, Serialize};

use crate::{Error, Result, b64};

const PREFIX: &str = "bana-sync:";

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Code {
    pub v: u32,
    pub relay: String,
    /// The workspace key (base64).
    pub key: String,
    /// The one-time invite secret (base64); the relay knows only its hash.
    pub invite: String,
    /// The inviting device's id and name.
    pub by: String,
    #[serde(default)]
    pub by_name: String,
}

impl std::fmt::Debug for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Code(relay {}, by {})", self.relay, self.by_name)
    }
}

impl Code {
    pub fn encode(&self) -> String {
        format!("{PREFIX}{}", b64::encode(serde_json::to_string(self).unwrap_or_default().as_bytes()))
    }

    /// Reads a code as pasted (spaces and line breaks from copying are ignored).
    pub fn decode(text: &str) -> Result<Code> {
        let bad = || Error::Config("this is not a Bana sync code".into());
        let t: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        let body = t.strip_prefix(PREFIX).ok_or_else(bad)?;
        let json = b64::decode(body).ok_or_else(bad)?;
        let c: Code = serde_json::from_slice(&json).map_err(|_| bad())?;
        if c.v != 1 {
            return Err(Error::Config("this sync code is from a newer version of the app".into()));
        }
        crate::config::check_relay(&c.relay)?;
        crate::crypto::WorkspaceKey::from_b64(&c.key)?;
        Ok(c)
    }

    /// The code as a QR code (SVG).
    pub fn qr_svg(&self) -> Result<String> {
        use qrcode::render::svg;
        let qr = qrcode::QrCode::with_error_correction_level(self.encode().as_bytes(), qrcode::EcLevel::L)
            .map_err(|e| Error::Config(format!("QR code: {e}")))?;
        Ok(qr
            .render::<svg::Color<'_>>()
            .min_dimensions(240, 240)
            .quiet_zone(true)
            .dark_color(svg::Color("#000000"))
            .light_color(svg::Color("#ffffff"))
            .build())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_round_trip_and_reject_junk() {
        let key = crate::crypto::WorkspaceKey::generate().unwrap();
        let c = Code {
            v: 1,
            relay: "https://r.example.dev".into(),
            key: key.to_b64(),
            invite: b64::encode(&[7; 32]),
            by: "dev".into(),
            by_name: "Mac".into(),
        };
        let text = c.encode();
        assert_eq!(Code::decode(&text).unwrap(), c);
        // Pasted with line breaks and spaces.
        let wrapped: String = text.chars().enumerate().flat_map(|(i, ch)| if i % 40 == 39 { vec![ch, '\n', ' '] } else { vec![ch] }).collect();
        assert_eq!(Code::decode(&wrapped).unwrap(), c);
        assert!(Code::decode("hello").is_err());
        assert!(Code::decode("bana-sync:!!!").is_err());
        assert!(c.qr_svg().unwrap().starts_with("<?xml"));
        assert!(!format!("{c:?}").contains(&c.key));
    }
}
