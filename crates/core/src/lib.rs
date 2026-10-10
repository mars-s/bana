//! TanaCraft core: the node graph.
//!
//! Every bullet is a [`Node`]. Nodes own their children; a [`Kind::Reference`] node mirrors another
//! node. Supertags ([`Kind::Tag`]) carry template fields ([`Kind::Field`]) and can extend each
//! other. Day nodes live under Daily notes, grouped by year and ISO week. Deleting moves a node to
//! the Trash, where it can be restored. Edits inside [`Graph::begin`]/[`Graph::commit`] are undoable.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod date;
pub mod export;
pub mod format;
pub mod graph;
pub mod model;
pub mod opml;
pub mod query;
pub mod sample;
pub mod store;
pub mod view;

pub use date::Date;
pub use graph::{Graph, GraphEvent, Settings, SystemNodes};
/// A copy's version (what it has of every peer's edits): sync sends what the other side lacks.
pub use loro::VersionVector;
pub use model::*;
pub use query::Query;
pub use view::{GroupBy, Sort, SortBy, SystemField, ViewDef, ViewKind};

/// The product name shown to people. Rename the product here.
pub const PRODUCT_NAME: &str = "TanaCraft";

/// Where the graph lives unless a file is given: `~/TanaCraft/graph.tanacraft` (a Loro document,
/// see [`store`]).
pub fn default_graph_path() -> std::path::PathBuf {
    data_dir().join("graph.tanacraft")
}

/// The app's data folder: `$TANACRAFT_HOME`, else `~/TanaCraft`. It holds the default graph and
/// the install's local API token.
pub fn data_dir() -> std::path::PathBuf {
    if let Some(d) = std::env::var_os("TANACRAFT_HOME").filter(|d| !d.is_empty()) {
        return std::path::PathBuf::from(d);
    }
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(std::path::PathBuf::from).unwrap_or_default();
    home.join(PRODUCT_NAME)
}

/// The install's local API token (`<data dir>/token`, readable by this user only), made on first
/// use. Local tools (MCP, the CLI, scripts, the tunnel connector) send it to the running app.
pub fn local_token() -> std::io::Result<String> {
    let path = data_dir().join("token");
    if let Ok(t) = std::fs::read_to_string(&path)
        && t.trim().len() >= 32
    {
        return Ok(t.trim().to_string());
    }
    std::fs::create_dir_all(data_dir())?;
    let token = random_hex();
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
    std::io::Write::write_all(&mut opts.open(&path)?, token.as_bytes())?;
    Ok(token)
}

/// 128 random bits as hex (std's per-process random hash keys, from the OS).
pub fn random_hex() -> String {
    use std::hash::{BuildHasher, Hasher};
    let part = || {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or_default());
        h.write_u32(std::process::id());
        h.finish()
    };
    format!("{:016x}{:016x}", part(), part())
}

/// Where a running app (desktop or `serve`) says how to reach its engine for `graph`: a JSON file
/// `{"port", "token", "pid"}` beside the graph. MCP and the CLI use it to work on the live app
/// instead of the file.
pub fn live_file(graph: &std::path::Path) -> std::path::PathBuf {
    let mut name = graph.file_name().unwrap_or_default().to_os_string();
    name.push(".live");
    graph.with_file_name(name)
}

/// Where files attached to nodes are kept for `graph`: a folder beside it (`graph.tanacraft.files`).
pub fn files_dir(graph: &std::path::Path) -> std::path::PathBuf {
    let mut name = graph.file_name().unwrap_or_default().to_os_string();
    name.push(".files");
    graph.with_file_name(name)
}

/// Standard base64 (RFC 4648, with padding): attached files travel as text through the API.
pub mod base64 {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    pub fn encode(data: &[u8]) -> String {
        let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
        for chunk in data.chunks(3) {
            let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
            let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
            for i in 0..4 {
                if i <= chunk.len() {
                    out.push(ABC[(n >> (18 - 6 * i) & 63) as usize] as char);
                } else {
                    out.push('=');
                }
            }
        }
        out
    }

    pub fn decode(text: &str) -> Option<Vec<u8>> {
        let clean: Vec<u8> = text.bytes().filter(|c| !c.is_ascii_whitespace()).collect();
        if !clean.len().is_multiple_of(4) {
            return None;
        }
        let val = |c: u8| ABC.iter().position(|x| *x == c).map(|p| p as u32);
        let mut out = Vec::with_capacity(clean.len() / 4 * 3);
        for q in clean.chunks(4) {
            let pad = q.iter().rev().take_while(|c| **c == b'=').count();
            if pad > 2 {
                return None;
            }
            let mut n = 0u32;
            for (i, c) in q.iter().enumerate() {
                n |= if *c == b'=' { 0 } else { val(*c)? } << (18 - 6 * i);
            }
            out.extend_from_slice(&[(n >> 16) as u8, (n >> 8) as u8, n as u8][..3 - pad]);
        }
        Some(out)
    }
}

/// A ZIP archive of `files` (`(path, bytes)`), stored without compression: Markdown exports are
/// small and every system opens it (UC-08.13).
pub mod zip {
    fn crc32(data: &[u8]) -> u32 {
        let mut crc = !0u32;
        for b in data {
            crc ^= u32::from(*b);
            for _ in 0..8 {
                crc = if crc & 1 == 1 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
            }
        }
        !crc
    }

    pub fn store(files: &[(String, Vec<u8>)]) -> Vec<u8> {
        let (mut out, mut central) = (Vec::new(), Vec::new());
        let le16 = |v: &mut Vec<u8>, x: u16| v.extend_from_slice(&x.to_le_bytes());
        let le32 = |v: &mut Vec<u8>, x: u32| v.extend_from_slice(&x.to_le_bytes());
        // ponytail: sizes are 32-bit (no ZIP64); an export over 4 GB would need it.
        let mut count = 0u16;
        for (name, data) in files.iter().take(usize::from(u16::MAX)) {
            let (crc, size, offset) = (crc32(data), data.len() as u32, out.len() as u32);
            let name = name.as_bytes();
            // Local header: version 2.0, flag 0x800 (UTF-8 names), stored, no time.
            le32(&mut out, 0x0403_4b50);
            for x in [20u16, 0x800, 0, 0, 0x21] {
                le16(&mut out, x);
            }
            for x in [crc, size, size] {
                le32(&mut out, x);
            }
            le16(&mut out, name.len() as u16);
            le16(&mut out, 0);
            out.extend_from_slice(name);
            out.extend_from_slice(data);
            le32(&mut central, 0x0201_4b50);
            for x in [20u16, 20, 0x800, 0, 0, 0x21] {
                le16(&mut central, x);
            }
            for x in [crc, size, size] {
                le32(&mut central, x);
            }
            for x in [name.len() as u16, 0, 0, 0, 0] {
                le16(&mut central, x);
            }
            le32(&mut central, 0);
            le32(&mut central, offset);
            central.extend_from_slice(name);
            count += 1;
        }
        let (at, len) = (out.len() as u32, central.len() as u32);
        out.extend_from_slice(&central);
        le32(&mut out, 0x0605_4b50);
        for x in [0u16, 0, count, count] {
            le16(&mut out, x);
        }
        le32(&mut out, len);
        le32(&mut out, at);
        le16(&mut out, 0);
        out
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn crc_matches_the_standard_check_value() {
            assert_eq!(super::crc32(b"123456789"), 0xCBF4_3926);
        }
    }
}

/// Loads `path`; when it does not exist yet but an older `graph.json` sits beside it (versions
/// before the Loro store), that one is loaded instead (and saved to `path` from then on). `None`
/// when there is nothing to load.
pub fn load_or_migrate(path: &std::path::Path) -> Result<Option<Graph>> {
    if path.exists() {
        return Graph::load(path).map(Some);
    }
    let old = path.with_file_name("graph.json");
    if path.extension().is_some_and(|e| e == "tanacraft") && old.exists() {
        return Graph::load(&old).map(Some);
    }
    Ok(None)
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("no node {0}")]
    NoNode(NodeId),
    #[error("{0}")]
    Invalid(String),
    #[error("file error: {0}")]
    Io(String),
    #[error("bad graph file: {0}")]
    Format(String),
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests;
