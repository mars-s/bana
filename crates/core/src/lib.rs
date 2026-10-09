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
pub mod query;
pub mod sample;
pub mod view;

pub use date::Date;
pub use graph::{Graph, Settings, SystemNodes};
pub use model::*;
pub use query::Query;
pub use view::{GroupBy, Sort, SortBy, SystemField, ViewDef, ViewKind};

/// The product name shown to people. Rename the product here.
pub const PRODUCT_NAME: &str = "TanaCraft";

/// Where the graph lives unless a file is given: `~/TanaCraft/graph.json`.
pub fn default_graph_path() -> std::path::PathBuf {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(std::path::PathBuf::from).unwrap_or_default();
    home.join(PRODUCT_NAME).join("graph.json")
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
