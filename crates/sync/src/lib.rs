//! Bana sync: paired copies of a workspace (the desktop app, the phone) exchange Loro updates
//! through a relay that only ever sees ciphertext and routing data (`docs/sync.md`).
//!
//! - [`crypto`]: the workspace key (XChaCha20-Poly1305) and device identities (Ed25519).
//! - [`code`]: pairing codes and their QR codes.
//! - [`config`]: a device's sync settings and progress (a private file).
//! - [`proto`]: the relay protocol (the Worker in `cloudflare/sync/` is the other side).
//! - [`client`]: the sync state machine, no I/O; [`net`]: the WebSocket loop and the service the
//!   apps run.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod account;
pub mod client;
pub mod code;
pub mod config;
pub mod crypto;
pub mod id;
pub mod net;
pub mod proto;

pub(crate) use bana_core::base64 as b64;

pub use client::{Client, Replica, State};
pub use code::Code;
pub use config::Config;
pub use net::{Service, SharedReplica};

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Crypto(String),
    #[error("{0}")]
    Config(String),
    #[error("{0}")]
    Graph(String),
    #[error("{0}")]
    Net(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Milliseconds since 1970.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}
