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
//!
//! Names are what clients say they are. A **scoped agent token** (`agent-tokens.json` beside
//! it, made by `bana agent-token create`, never by a command) fixes the name and the allow list
//! for whoever holds it: `agents.json` does not apply to it, and a self-named client that takes
//! a token's name is recorded as "NAME (unverified)". Only scoped tokens that list `mirror.*`
//! may run the mirror commands.
//!
//! Undo and redo from an assistant take back only its own last step: when the step on top was
//! made by someone else (the person, another assistant) they are refused.

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
    /// Came with a scoped agent token: the name is checked, not self-reported.
    pub scoped: bool,
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
        let taken = read_tokens(&file.with_file_name(TOKENS)).iter().any(|t| t["name"].as_str() == Some(name.as_str()));
        let name = if taken { format!("{name} (unverified)") } else { name };
        Agent { name, allow, scoped: false }
    }

    /// The scoped agent whose token is `token`, from the tokens file `file`.
    pub fn from_token(token: &str, file: &Path) -> Option<Agent> {
        let t = token.trim();
        let hit = read_tokens(file).into_iter().find(|e| t.len() >= 32 && e["token"].as_str() == Some(t))?;
        Some(from_entry(&hit))
    }

    /// The scoped agent named `name` (its token was checked at the door), from the tokens file.
    pub fn scoped(name: &str, file: &Path) -> Option<Agent> {
        read_tokens(file).iter().find(|e| e["name"].as_str() == Some(name)).map(from_entry)
    }

    /// Whether the person allowed this agent `id` (an id, or a `prefix.*` pattern, matches).
    pub fn allows(&self, id: &str) -> bool {
        self.allow.iter().any(|a| match a.strip_suffix('*') {
            Some(prefix) => id.starts_with(prefix),
            None => id == a,
        })
    }
}

/// The scoped agent tokens file, beside `agents.json` in the data folder.
pub const TOKENS: &str = "agent-tokens.json";

fn from_entry(entry: &Value) -> Agent {
    let allow = entry["allow"].as_array().map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
    Agent { name: clean_name(entry["name"].as_str().unwrap_or_default()), allow, scoped: true }
}

fn read_tokens(file: &Path) -> Vec<Value> {
    let v: Value = std::fs::read(file).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(Value::Null);
    v["tokens"].as_array().cloned().unwrap_or_default()
}

/// Makes (or replaces) the scoped agent token `name` with `allow` in the tokens file `file`
/// (readable by this user only) and returns the new token. For the person (the CLI) only.
pub fn create_token(file: &Path, name: &str, allow: &[String]) -> std::io::Result<String> {
    let name = clean_name(name);
    let token = format!("{}{}", bana_core::random_hex(), bana_core::random_hex());
    let mut all = read_tokens(file);
    all.retain(|e| e["name"].as_str() != Some(name.as_str()));
    all.push(serde_json::json!({"name": name, "token": token, "allow": allow}));
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
    let body = serde_json::to_vec_pretty(&serde_json::json!({"tokens": all})).unwrap_or_default();
    std::io::Write::write_all(&mut opts.open(file)?, &body)?;
    Ok(token)
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
    agent.allows(spec.id)
}

/// Refuses `spec` with `params` for `agent`: a Never rule, or a change to a command node.
pub(crate) fn check(s: &Session, agent: &Agent, spec: &CommandSpec, params: &Value) -> Result<(), CmdError> {
    let refuse = |rule: &str| CmdError::Refused { command: spec.id.to_string(), rule: rule.to_string() };
    if let Some(r) = rule(spec)
        && !allowed(agent, spec)
    {
        return Err(refuse(&r));
    }
    // Undo and redo take back only the agent's own last step.
    let top = match spec.id {
        "edit.undo" => s.graph.undo_by(),
        "edit.redo" => s.graph.redo_by(),
        _ => None,
    };
    if top.is_some_and(|by| by != Some(agent.name.as_str())) {
        return Err(refuse("not its own change"));
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
pub(crate) fn in_command_node(s: &Session, id: NodeId) -> bool {
    let g = &s.graph;
    [id, g.resolve(id)]
        .into_iter()
        .any(|n| std::iter::once(n).chain(g.ancestors(n)).take(256).any(|a| g.get(a).is_some_and(|x| x.kind == bana_core::Kind::Command)))
}

/// The person's hand-edited opt-outs: `agents.json` in the data folder (`~/Bana`).
pub fn config_file() -> std::path::PathBuf {
    bana_core::data_dir().join("agents.json")
}

/// The scoped agent tokens: `agent-tokens.json` in the data folder.
pub fn tokens_file() -> std::path::PathBuf {
    bana_core::data_dir().join(TOKENS)
}
