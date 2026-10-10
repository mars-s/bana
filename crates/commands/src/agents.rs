//! What assistants may do (agent permission tiers). Commands that come from an assistant (MCP on
//! stdio, `POST /mcp`, the remote connector) run with [`Session::agent`] set; [`Session::run`]
//! checks them here first. The person's own windows never set it, so the app is unaffected.
//!
//! Never run for assistants: the [`NEVER`] list, any command flagged
//! [`destructive`](crate::CommandSpec::destructive), and any change to a command node or its
//! steps (an assistant could plant a trigger). The only way out is `agents.json` in the data
//! folder, edited by hand (no command writes it):
//!
//! ```json
//! { "allow": ["trash.empty"], "clients": { "my-gateway": ["node.toggleLock"] } }
//! ```

use std::path::Path;

use bana_core::NodeId;
use serde_json::Value;

use crate::{CmdError, CommandSpec, Session};

/// Command ids (or `prefix.*`) never run for assistants, and the rule each one falls under.
pub const NEVER: &[&str] = &[
    "node.hardDelete",
    "trash.empty",
    "node.toggleLock",
    "file.*",
    "import.folder",
    "import.file",
    "settings.*",
    "workspace.*",
    "commandNode.create",
    "commandNode.on",
    "commandNode.button",
    "commandNode.run",
];

/// An assistant running commands: its name (recorded on what it changes) and the commands the
/// person allowed it by hand.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Agent {
    pub name: String,
    pub allow: Vec<String>,
}

impl Agent {
    /// `name` (cleaned up: one line, at most 64 chars, "assistant" when empty) with what
    /// `agents.json` at `file` allows it: `allow` for every assistant plus its own `clients` entry.
    pub fn load(name: &str, file: &Path) -> Agent {
        let name = clean_name(name);
        let cfg: Value = std::fs::read(file).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(Value::Null);
        let list = |v: &Value| v.as_array().map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect::<Vec<_>>()).unwrap_or_default();
        let mut allow = list(&cfg["allow"]);
        allow.extend(list(&cfg["clients"][name.as_str()]));
        Agent { name, allow }
    }
}

/// An MCP client's name as recorded: one line, no control characters, at most 64 chars.
pub fn clean_name(name: &str) -> String {
    let n: String = name.chars().filter(|c| !c.is_control()).take(64).collect::<String>().trim().to_string();
    if n.is_empty() { "assistant".into() } else { n }
}

/// The rule that keeps `spec` from assistants, if any (before the person's allow list).
pub fn rule(spec: &CommandSpec) -> Option<String> {
    let hit = NEVER.iter().find(|r| match r.strip_suffix('*') {
        Some(prefix) => spec.id.starts_with(prefix),
        None => spec.id == **r,
    });
    match hit {
        Some(r) => Some(r.to_string()),
        None => spec.destructive.then(|| "destructive".to_string()),
    }
}

fn allowed(agent: &Agent, spec: &CommandSpec) -> bool {
    agent.allow.iter().any(|a| match a.strip_suffix('*') {
        Some(prefix) => spec.id.starts_with(prefix),
        None => spec.id == a,
    })
}

/// Refuses `spec` with `params` for `agent`: a Never rule, or a change to a command node.
pub(crate) fn check(s: &Session, agent: &Agent, spec: &CommandSpec, params: &Value) -> Result<(), CmdError> {
    let refuse = |rule: &str| CmdError::Refused { command: spec.id.to_string(), rule: rule.to_string() };
    if let Some(r) = rule(spec)
        && !allowed(agent, spec)
    {
        return Err(refuse(&r));
    }
    if spec.mutates && !allowed(agent, spec) {
        let mut hit: Vec<NodeId> = Vec::new();
        if spec.params.contains("\"node\"") {
            hit.extend(s.targets(params).unwrap_or_default());
        }
        for k in ["node", "parent", "to", "target", "into", "after", "before"] {
            if let Ok(Some(n)) = crate::p::node(params, k) {
                hit.push(n);
            }
        }
        if hit.into_iter().any(|n| in_command_node(s, n)) {
            return Err(refuse("command nodes"));
        }
    }
    Ok(())
}

/// `id` (or what it references) is a command node or one of its steps.
fn in_command_node(s: &Session, id: NodeId) -> bool {
    let g = &s.graph;
    [id, g.resolve(id)]
        .into_iter()
        .any(|n| std::iter::once(n).chain(g.ancestors(n)).take(256).any(|a| g.get(a).is_some_and(|x| x.kind == bana_core::Kind::Command)))
}

/// The person's hand-edited opt-outs: `agents.json` in the data folder (`~/Bana`).
pub fn config_file() -> std::path::PathBuf {
    bana_core::data_dir().join("agents.json")
}
