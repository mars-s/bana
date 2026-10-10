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
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(std::path::PathBuf::from).unwrap_or_default();
    home.join(PRODUCT_NAME).join("graph.tanacraft")
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
