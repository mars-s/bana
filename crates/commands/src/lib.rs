//! Bana commands: the editing [`Session`] and the command registry.
//!
//! **Everything is a command.** Each user-visible action is a [`CommandSpec`] with a stable id
//! (`node.indent`, `tag.add`), a label, a menu location, a default shortcut, a params description
//! and a `run` function. The UI, keyboard shortcuts, the command palette, the CLI and the MCP server
//! all call [`Session::run`].
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod cmds;
mod session;
mod templates;

use std::collections::HashMap;

use serde_json::{Value, json};

pub use bana_core as core;
pub use session::{NavState, Row, Session};

#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum CmdError {
    #[error("unknown command `{0}`")]
    Unknown(String),
    #[error("bad parameters: {0}")]
    Params(String),
    #[error("not available: {0}")]
    Disabled(String),
    #[error("{0}")]
    Failed(String),
}

impl From<bana_core::Error> for CmdError {
    fn from(e: bana_core::Error) -> Self {
        CmdError::Failed(e.to_string())
    }
}

pub type CmdResult = Result<Value, CmdError>;

/// A registered command.
#[derive(Clone)]
pub struct CommandSpec {
    pub id: &'static str,
    pub label: &'static str,
    /// Where it lives in the UI: `Node`, `Supertag`, `Navigate`, `File`...
    pub location: &'static str,
    /// Default shortcut in a portable form (`Mod+Shift+Up`, `Tab`). `Mod` = Ctrl, or Cmd on macOS.
    /// Several shortcuts are separated by ` / `.
    pub shortcut: &'static str,
    /// Parameter description for agents (`{"node": id, "name": string}`).
    pub params: &'static str,
    /// Changes the graph (undoable).
    pub mutates: bool,
    pub run: fn(&mut Session, &Value) -> CmdResult,
}

impl CommandSpec {
    pub const fn new(id: &'static str, label: &'static str, location: &'static str, run: fn(&mut Session, &Value) -> CmdResult) -> Self {
        CommandSpec { id, label, location, shortcut: "", params: "{}", mutates: true, run }
    }
    pub const fn key(mut self, s: &'static str) -> Self {
        self.shortcut = s;
        self
    }
    pub const fn params(mut self, p: &'static str) -> Self {
        self.params = p;
        self
    }
    /// Doesn't change the graph (navigation, view, export).
    pub const fn pure(mut self) -> Self {
        self.mutates = false;
        self
    }
}

/// All commands by id.
pub struct Registry {
    specs: Vec<CommandSpec>,
    by_id: HashMap<&'static str, usize>,
}

impl Registry {
    pub fn new(specs: Vec<CommandSpec>) -> Self {
        let by_id = specs.iter().enumerate().map(|(i, s)| (s.id, i)).collect();
        Registry { specs, by_id }
    }
    pub fn get(&self, id: &str) -> Option<&CommandSpec> {
        self.by_id.get(id).and_then(|i| self.specs.get(*i))
    }
    pub fn all(&self) -> &[CommandSpec] {
        &self.specs
    }
    /// Command list as JSON (for agents and `commands`).
    pub fn describe(&self) -> Value {
        Value::Array(
            self.specs
                .iter()
                .map(|s| json!({"id": s.id, "label": s.label, "location": s.location, "shortcut": s.shortcut, "params": s.params, "mutates": s.mutates}))
                .collect(),
        )
    }
    /// The command bound to a shortcut (`Mod+Shift+Up`), if any.
    pub fn by_shortcut(&self, key: &str) -> Option<&CommandSpec> {
        self.specs.iter().find(|s| !s.shortcut.is_empty() && s.shortcut.split(" / ").any(|k| k.eq_ignore_ascii_case(key)))
    }
    /// Commands whose label or id contains every word of `query` (command palette).
    pub fn search(&self, query: &str) -> Vec<&CommandSpec> {
        let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        self.specs
            .iter()
            .filter(|s| {
                let hay = format!("{} {} {}", s.label, s.id, s.location).to_lowercase();
                words.iter().all(|w| hay.contains(w.as_str()))
            })
            .collect()
    }
}

/// Parameter helpers. Params come from people and agents: never trust them.
pub mod p {
    use bana_core::NodeId;
    use serde_json::Value;

    use crate::CmdError;

    pub fn str<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
        v.get(k).and_then(Value::as_str)
    }
    pub fn req_str<'a>(v: &'a Value, k: &str) -> Result<&'a str, CmdError> {
        str(v, k).ok_or_else(|| CmdError::Params(format!("`{k}` (string) is required")))
    }
    pub fn usize(v: &Value, k: &str) -> Option<usize> {
        v.get(k).and_then(Value::as_u64).map(|x| x.min(usize::MAX as u64) as usize)
    }
    pub fn bool(v: &Value, k: &str) -> Option<bool> {
        v.get(k).and_then(Value::as_bool)
    }
    /// A node id given as a number or a string (`42`, `"42"`, `"n42"`).
    pub fn node(v: &Value, k: &str) -> Result<Option<NodeId>, CmdError> {
        match v.get(k) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::Number(n)) => n.as_u64().map(|x| Some(NodeId(x))).ok_or_else(|| CmdError::Params(format!("`{k}` must be a node id"))),
            Some(Value::String(s)) => s.parse().map(Some).map_err(CmdError::Params),
            Some(_) => Err(CmdError::Params(format!("`{k}` must be a node id"))),
        }
    }
}

#[cfg(test)]
mod tests;
