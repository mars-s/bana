//! A device's sync settings and progress, kept in a file only this user can read (beside the
//! graph on the desktop, in the app's private Library on the phone; never in a shared folder).
//! It holds the workspace key: treat it like a password.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::crypto::{DeviceKey, WorkspaceKey};
use crate::{Error, Result, b64};

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub version: u32,
    /// The relay's address as typed (`https://...`); the WebSocket goes to [`Config::socket_url`].
    pub relay: String,
    pub room: String,
    /// The workspace key (base64).
    pub key: String,
    /// This device's secret identity key (base64).
    pub device: String,
    /// This device's name as shown in the devices list.
    pub name: String,
    /// The last relay update this copy has safely taken in.
    #[serde(default)]
    pub cursor: u64,
    /// The Loro version the relay is known to have from this copy (encoded, base64): the next push
    /// sends what came after it.
    #[serde(default)]
    pub sent: String,
    /// Joining: the copy's notes are replaced by the workspace's on the first sync.
    #[serde(default)]
    pub joining: bool,
    /// The one-time invite secret a joining device signs in with (base64), cleared once used.
    #[serde(default)]
    pub invite: Option<String>,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("relay", &self.relay)
            .field("room", &self.room)
            .field("name", &self.name)
            .field("cursor", &self.cursor)
            .finish()
    }
}

impl Config {
    /// A new synced workspace on this device (the first device).
    pub fn create(relay: &str, name: &str) -> Result<Config> {
        let relay = check_relay(relay)?;
        let key = WorkspaceKey::generate()?;
        Ok(Config {
            version: 1,
            relay,
            room: key.room(),
            key: key.to_b64(),
            device: DeviceKey::generate()?.to_b64(),
            name: clean_name(name),
            cursor: 0,
            sent: String::new(),
            joining: false,
            invite: None,
        })
    }

    /// Joining from a pairing code (see [`crate::code`]).
    pub fn join(code: &crate::code::Code, name: &str) -> Result<Config> {
        let key = WorkspaceKey::from_b64(&code.key)?;
        Ok(Config {
            version: 1,
            relay: check_relay(&code.relay)?,
            room: key.room(),
            key: key.to_b64(),
            device: DeviceKey::generate()?.to_b64(),
            name: clean_name(name),
            cursor: 0,
            sent: String::new(),
            joining: true,
            invite: Some(code.invite.clone()),
        })
    }

    pub fn workspace_key(&self) -> Result<WorkspaceKey> {
        WorkspaceKey::from_b64(&self.key)
    }

    pub fn device_key(&self) -> Result<DeviceKey> {
        DeviceKey::from_b64(&self.device)
    }

    pub fn sent_version(&self) -> bana_core::VersionVector {
        b64::decode(&self.sent).and_then(|b| bana_core::VersionVector::decode(&b).ok()).unwrap_or_default()
    }

    pub fn set_sent_version(&mut self, vv: &bana_core::VersionVector) {
        self.sent = b64::encode(&vv.encode());
    }

    /// `wss://<relay>/v1/rooms/<room>` (`ws://` only for this machine).
    pub fn socket_url(&self) -> String {
        let base = self.relay.trim_end_matches('/');
        let base = base
            .strip_prefix("https://")
            .map(|r| format!("wss://{r}"))
            .or_else(|| base.strip_prefix("http://").map(|r| format!("ws://{r}")))
            .unwrap_or_else(|| base.to_string());
        format!("{base}/v1/rooms/{}", self.room)
    }

    pub fn load(path: &Path) -> Result<Option<Config>> {
        match std::fs::read(path) {
            Ok(b) => serde_json::from_slice(&b).map(Some).map_err(|e| Error::Config(format!("{}: {e}", path.display()))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(Error::Config(format!("{}: {e}", path.display()))),
        }
    }

    /// Writes the file atomically, readable by this user only.
    pub fn save(&self, path: &Path) -> Result<()> {
        let io = |e: std::io::Error| Error::Config(format!("{}: {e}", path.display()));
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(io)?;
        }
        let body = serde_json::to_vec_pretty(self).map_err(|e| Error::Config(e.to_string()))?;
        let tmp = path.with_extension("tmp");
        {
            use std::io::Write;
            let mut opts = std::fs::OpenOptions::new();
            opts.write(true).create(true).truncate(true);
            #[cfg(unix)]
            std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
            let mut f = opts.open(&tmp).map_err(io)?;
            f.write_all(&body).map_err(io)?;
            f.sync_all().map_err(io)?;
        }
        std::fs::rename(&tmp, path).map_err(io)
    }
}

fn clean_name(name: &str) -> String {
    let n: String = name.trim().chars().filter(|c| !c.is_control()).take(64).collect();
    if n.is_empty() { "Device".into() } else { n }
}

/// The relay's address: `https://host[:port][/path]`, or `http://` for this machine only (a
/// relay run with `wrangler dev`). Everything else is refused.
pub fn check_relay(url: &str) -> Result<String> {
    let u = url.trim().trim_end_matches('/');
    let bad = || Error::Config(format!("`{u}` is not a relay address: use https://... (http:// only for 127.0.0.1 or localhost)"));
    let (scheme, rest) = u.split_once("://").ok_or_else(bad)?;
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let name = host.rsplit_once(':').map_or(host, |(h, _)| h);
    if name.is_empty() || rest.contains(['?', '#', ' ', '@']) {
        return Err(bad());
    }
    match scheme {
        "https" => Ok(u.to_string()),
        "http" if matches!(name, "127.0.0.1" | "localhost" | "[::1]") => Ok(u.to_string()),
        _ => Err(bad()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relay_addresses_are_checked() {
        assert_eq!(check_relay("https://sync.example.workers.dev/").unwrap(), "https://sync.example.workers.dev");
        assert!(check_relay("http://127.0.0.1:8790").is_ok());
        assert!(check_relay("http://example.com").is_err(), "plain http only locally");
        assert!(check_relay("ftp://x").is_err());
        assert!(check_relay("https://x?key=1").is_err());
        let c = Config::create("https://r.example.dev", "Mac").unwrap();
        assert_eq!(c.socket_url(), format!("wss://r.example.dev/v1/rooms/{}", c.room));
        assert!(!c.socket_url().contains(&c.key), "the key is never in the URL");
        assert!(!format!("{c:?}").contains(&c.key), "nor in Debug output");
    }
}
