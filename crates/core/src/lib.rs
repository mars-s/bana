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
pub use graph::{Graph, Settings, SystemNodes};
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
