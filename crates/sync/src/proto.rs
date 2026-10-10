//! The relay protocol: JSON text frames over one WebSocket per device (`/v1/rooms/<room>`).
//! The Worker in `cloudflare/sync/` speaks the other side; `docs/sync.md` describes it.
//!
//! Binary payloads (`data`, `name`, `vv`) are base64 of sealed bytes ([`crate::crypto`]): the
//! relay stores and forwards them without being able to read them. A blob too big for one frame
//! goes as `n` parts (`i` = 0..n) of one message `id`.

use serde::{Deserialize, Serialize};

/// The biggest sealed part a device sends (the relay takes frames up to 1 MiB).
pub const PART: usize = 256 * 1024;

/// Device to relay.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "lowercase")]
pub enum Up {
    /// The answer to the challenge: the device id (public key), its signature of
    /// [`crate::crypto::hello_message`], what it already has (`cursor`), a one-time invite secret
    /// when joining, and its name (sealed).
    Hello {
        device: String,
        sig: String,
        cursor: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        invite: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
    /// Registers an invite: the hash of its secret (valid for a day, once).
    Invite { hash: String },
    /// A part of an encrypted update.
    Push { id: String, i: u32, n: u32, data: String },
    /// Everything up to `seq` is safely in this copy; `vv`: its Loro version (sealed).
    Ack {
        seq: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        vv: Option<String>,
    },
    /// A part of an encrypted snapshot holding everything up to `upto`: once whole, the relay
    /// drops the updates it replaces (only when every device has acknowledged `upto`).
    Snap { upto: u64, id: String, i: u32, n: u32, data: String },
    /// The next page of the backlog.
    Pull { after: u64 },
    /// Removes a device from the room for good.
    Revoke { device: String },
}

/// A paired device as the relay lists it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Device {
    pub device: String,
    #[serde(default)]
    pub name: Option<String>,
    /// The last update it acknowledged.
    #[serde(default)]
    pub cursor: u64,
    /// Its Loro version as last acknowledged (sealed).
    #[serde(default)]
    pub vv: Option<String>,
    /// When it was last connected (ms since 1970).
    #[serde(default)]
    pub seen: u64,
    #[serde(default)]
    pub revoked: bool,
}

/// Relay to device.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "lowercase")]
pub enum Down {
    Challenge {
        nonce: String,
    },
    /// Signed in. `head`: the newest update's number; `snapshot`: the stored snapshot, if any.
    Welcome {
        head: u64,
        devices: Vec<Device>,
        #[serde(default)]
        snapshot: Option<SnapInfo>,
    },
    /// A part of the stored snapshot (sealed by device `from`).
    Snapshot {
        upto: u64,
        from: String,
        i: u32,
        n: u32,
        data: String,
    },
    Update {
        seq: u64,
        from: String,
        id: String,
        i: u32,
        n: u32,
        data: String,
    },
    /// The end of a page of backlog (`upto`: its last update). `upto < head`: send a pull.
    Synced {
        head: u64,
        upto: u64,
    },
    /// A pushed part is stored as update `seq`.
    Ok {
        id: String,
        i: u32,
        seq: u64,
    },
    Invited {
        hash: String,
    },
    Acked {
        device: String,
        seq: u64,
        #[serde(default)]
        vv: Option<String>,
    },
    Devices {
        devices: Vec<Device>,
    },
    /// Updates up to `upto` were replaced by a snapshot.
    Pruned {
        upto: u64,
    },
    Error {
        code: String,
        msg: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapInfo {
    pub upto: u64,
    pub n: u32,
}

/// WebSocket close codes the relay uses.
pub const CLOSE_AUTH: u16 = 4001;
pub const CLOSE_REVOKED: u16 = 4003;
pub const CLOSE_LIMIT: u16 = 4008;

pub fn up(m: &Up) -> String {
    serde_json::to_string(m).unwrap_or_default()
}
