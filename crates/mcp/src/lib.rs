//! Bana's MCP server (Model Context Protocol, JSON-RPC 2.0, one message per line on stdio).
//!
//! Two backends: [`Backend::Live`] drives the running app (desktop or `bana serve`) over
//! its localhost HTTP API, with the token from the live file beside the graph, so the open window
//! shows every change at once; [`Backend::Local`] edits the graph file in-process (saved after
//! every change), only when no app has it open. [`Backend::open`] picks.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::io::{BufRead, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::time::Duration;

use bana_commands::Session;
use bana_commands::agents::Agent;
use bana_core::PRODUCT_NAME;
use serde_json::{Value, json};

pub const PROTOCOL_VERSION: &str = "2025-06-18";
const SUPPORTED: &[&str] = &[PROTOCOL_VERSION, "2025-03-26", "2024-11-05"];

const INSTRUCTIONS: &str = "An outliner graph: every bullet is a node with an id. Supertags (#task) add fields to \
nodes; references mirror nodes; day nodes live under Daily notes. Every action is a command: list them with \
list_commands, run them with execute. Typical flow: execute nav.today, add_node {name}, execute tag.add {node, tag:\"task\"}, \
execute field.set {node, field:\"Due\", value:\"2026-10-12\"}, view to check, export_markdown to read a subtree.";

pub enum Backend {
    Local(Box<Session>),
    /// A running engine at 127.0.0.1:`port`.
    Live {
        port: u16,
        token: String,
    },
    /// The engine in this process (the app's own `/mcp` endpoint): `(method, params)` in, the
    /// host's JSON answer out.
    Engine(EngineFn),
}

/// See [`Backend::Engine`].
pub type EngineFn = Box<dyn FnMut(&str, Value) -> Value + Send>;

impl Backend {
    /// The running app that has `graph` open (its live file answers), else the file itself.
    pub fn open(graph: &Path) -> Result<Backend, String> {
        if let Some(live) = Backend::find_live(graph) {
            return Ok(live);
        }
        let g = bana_core::load_or_migrate(graph).map_err(|e| e.to_string())?.unwrap_or_default();
        if let Some(dir) = graph.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let mut s = Session::new(g);
        s.path = Some(graph.to_path_buf());
        Ok(Backend::Local(Box::new(s)))
    }

    /// The running app announced beside `graph`, if it answers to the install's token.
    pub fn find_live(graph: &Path) -> Option<Backend> {
        Backend::find_live_with(graph, bana_core::local_token().ok()?)
    }

    pub fn find_live_with(graph: &Path, token: String) -> Option<Backend> {
        let v: Value = serde_json::from_slice(&std::fs::read(bana_core::live_file(graph)).ok()?).ok()?;
        let port = u16::try_from(v.get("port")?.as_u64()?).ok()?;
        let mut b = Backend::Live { port, token };
        b.post(&json!({"method": "version"})).ok()?;
        Some(b)
    }

    /// `host:port` (127.0.0.1 only) with a token (`--token`, else the install's local token).
    pub fn connect(addr: &str, token: Option<String>) -> Result<Backend, String> {
        let port = addr.rsplit(':').next().and_then(|p| p.parse::<u16>().ok()).ok_or(format!("connect {addr}: no port"))?;
        let token = match token {
            Some(t) => t,
            None => bana_core::local_token().map_err(|e| format!("no local token: {e}"))?,
        };
        let mut b = Backend::Live { port, token };
        b.post(&json!({"method": "version"}))?;
        Ok(b)
    }

    /// One request to the live engine (its own window, so the person's page and caret stay put).
    fn post(&mut self, req: &Value) -> Result<Value, String> {
        let Backend::Live { port, token } = self else { return Err("not connected".into()) };
        let mut req = req.clone();
        req["params"]["window"] = json!("mcp");
        let body = req.to_string();
        let mut s = TcpStream::connect(("127.0.0.1", *port)).map_err(|e| format!("the app on port {port}: {e}"))?;
        s.set_read_timeout(Some(Duration::from_secs(60))).map_err(|e| e.to_string())?;
        write!(
            s,
            "POST /rpc HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nAuthorization: Bearer {token}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        s.read_to_end(&mut out).map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&out);
        let (head, body) = text.split_once("\r\n\r\n").ok_or("bad reply from the app")?;
        if !head.starts_with("HTTP/1.1 200") {
            return Err(format!("the app refused: {}", head.lines().next().unwrap_or_default()));
        }
        serde_json::from_str(body).map_err(|e| format!("bad reply: {e}"))
    }

    /// Runs a command by id for the assistant `agent` (its MCP client name): the engine checks it
    /// against the agent policy and marks what it changes with the name (`bana_commands::agents`).
    pub fn run(&mut self, agent: &str, command: &str, params: Value) -> Result<Value, String> {
        match self {
            Backend::Local(s) => {
                let mutates = s.registry.get(command).is_some_and(|c| c.mutates)
                    || command.starts_with("nav.")
                    || command == "edit.undo"
                    || command == "edit.redo";
                s.agent = Some(Agent::load(agent, &bana_commands::agents::config_file()));
                let r = s.run(command, &params);
                s.agent = None;
                let r = r.map_err(|e| e.to_string())?;
                if mutates && s.path.is_some() && s.graph.is_dirty() {
                    s.run("file.save", &json!({})).map_err(|e| format!("saved nothing: {e}"))?;
                }
                Ok(r)
            }
            Backend::Engine(f) => {
                let v = f("run", json!({"command": command, "params": params, "window": "mcp", "agent": agent}));
                match v.get("error") {
                    Some(e) => Err(e.as_str().unwrap_or("error").to_string()),
                    None => Ok(v.get("result").cloned().unwrap_or(Value::Null)),
                }
            }
            Backend::Live { .. } => {
                let v = self.post(&json!({"method": "run", "params": {"command": command, "params": params, "agent": agent}}))?;
                match v.get("error") {
                    Some(e) => Err(e.as_str().unwrap_or("error").to_string()),
                    None => Ok(v.get("result").cloned().unwrap_or(Value::Null)),
                }
            }
        }
    }

    /// Where this server works, for the person starting it (DIFFS N89).
    pub fn describe(&self) -> String {
        match self {
            Backend::Local(s) => match &s.path {
                Some(p) => format!("the file {} (no app has it open)", p.display()),
                None => "a graph in memory".into(),
            },
            Backend::Live { port, .. } => format!("the running app at 127.0.0.1:{port}"),
            Backend::Engine(_) => "the app's own engine".into(),
        }
    }

    pub fn commands(&mut self) -> Result<Value, String> {
        match self {
            Backend::Local(s) => Ok(s.registry.describe()),
            Backend::Live { .. } => self.post(&json!({"method": "commands", "params": {}})),
            Backend::Engine(f) => Ok(f("commands", json!({"window": "mcp"}))),
        }
    }
}

/// Tool definitions (MCP `tools/list`). Each has a `title` naming the app, so a client with
/// several servers tells them apart (DIFFS N89); the names stay as connectors already use them.
pub fn tools() -> Value {
    let node = json!({"type": ["integer", "string"], "description": "a node id (from search, inspect or add_node)"});
    let open = "The assistant has a page of its own in the app (separate from the person's windows): open a page there with execute nav.open {node} or nav.today.";
    json!([
        {"name": "list_commands", "title": "Bana: list commands", "description": "Every Bana command: id, label, shortcut, params, whether it changes the graph. Run one with execute.", "inputSchema": {"type": "object", "properties": {}}},
        {"name": "execute", "title": "Bana: run a command", "description": format!("Run a Bana command by id with its params (see list_commands). Commands without a node act on the assistant's page. {open}"), "inputSchema": {"type": "object", "properties": {"command": {"type": "string", "description": "a command id, e.g. tag.add"}, "params": {"type": "object"}}, "required": ["command"]}},
        {"name": "view", "title": "Bana: view the page", "description": format!("The assistant's page in Bana: title and visible bullets with ids (> marks the focused one). {open}"), "inputSchema": {"type": "object", "properties": {}}},
        {"name": "inspect", "title": "Bana: inspect a node", "description": "A Bana node and its subtree as JSON (ids, tags, fields, children). Without node: the assistant's page.", "inputSchema": {"type": "object", "properties": {"node": node}}},
        {"name": "search", "title": "Bana: search notes", "description": "Bana nodes whose text contains the query (case-insensitive), with ids.", "inputSchema": {"type": "object", "properties": {"query": {"type": "string", "minLength": 1}}, "required": ["query"]}},
        {"name": "add_node", "title": "Bana: add a node", "description": "Add a Bana node. parent: a node id, or a day in words (\"today\", \"tomorrow\", \"next friday\", \"2026-10-20\"); without it, the assistant's page. tag: an existing supertag (set create_tag to make a new one).", "inputSchema": {"type": "object", "properties": {"name": {"type": "string"}, "parent": {"type": ["integer", "string"], "description": "a node id or a day in words"}, "tag": {"type": "string"}, "create_tag": {"type": "boolean", "description": "make the supertag if there is none by that name"}}, "required": ["name"]}},
        {"name": "export_markdown", "title": "Bana: read as Markdown", "description": "A Bana node and its subtree as a Markdown outline. Without node: the assistant's page.", "inputSchema": {"type": "object", "properties": {"node": node}}},
    ])
}

/// What an `execute` that failed on its params should have sent: the command's params line.
fn params_hint(b: &mut Backend, id: &str) -> String {
    let Ok(list) = b.commands() else { return String::new() };
    let found = list.as_array().and_then(|a| a.iter().find(|c| c["id"] == id).cloned());
    match found {
        Some(c) => format!(" ({id} takes {})", c["params"].as_str().unwrap_or("{}")),
        None => format!(" (no command `{id}`; list_commands shows them all)"),
    }
}

fn call_tool(b: &mut Backend, agent: &str, name: &str, args: &Value) -> Result<Value, String> {
    let arg = |k: &str| args.get(k).cloned().unwrap_or(Value::Null);
    match name {
        "list_commands" => b.commands(),
        "execute" => {
            let id = args.get("command").and_then(Value::as_str).ok_or("missing `command`: a command id from list_commands")?;
            let params = args.get("params").cloned().filter(|v| !v.is_null()).unwrap_or(json!({}));
            b.run(agent, id, params).map_err(|e| if e.starts_with("refused:") { e } else { format!("{e}{}", params_hint(b, id)) })
        }
        "view" => b.run(agent, "graph.view", json!({})),
        "inspect" => b.run(agent, "graph.inspect", json!({"node": arg("node")})),
        "search" => {
            let q = args.get("query").and_then(Value::as_str).unwrap_or_default();
            if q.trim().is_empty() {
                return Err("`query` is empty: give a word or phrase to look for".into());
            }
            b.run(agent, "graph.search", json!({"query": q}))
        }
        "export_markdown" => b.run(agent, "export.markdown", json!({"node": arg("node")})),
        "add_node" => {
            // A typo must not make a supertag (DIFFS N89): only an existing one, unless asked.
            let tag = args.get("tag").and_then(Value::as_str).map(|t| t.trim().trim_start_matches('#').to_string()).filter(|t| !t.is_empty());
            if let Some(t) = &tag
                && args.get("create_tag").and_then(Value::as_bool) != Some(true)
            {
                let tags = b.run(agent, "graph.tags", json!({}))?;
                let known = tags.as_array().is_some_and(|a| a.iter().any(|x| x["name"].as_str().is_some_and(|n| n.eq_ignore_ascii_case(t))));
                if !known {
                    return Err(format!("no supertag #{t}; pass create_tag: true to make it (execute graph.tags lists them)"));
                }
            }
            let r = b.run(agent, "node.add", json!({"name": arg("name"), "parent": arg("parent")}))?;
            if let Some(t) = tag {
                b.run(agent, "tag.add", json!({"node": r["id"], "tag": t}))?;
            }
            Ok(r)
        }
        other => Err(format!("unknown tool `{other}`")),
    }
}

/// An MCP server bound to one backend.
pub struct Server {
    pub backend: Backend,
    /// Who is asking: a connector's header (`X-Bana-Agent`), else the client's name from
    /// `initialize` (`clientInfo`). Recorded on what it changes; "assistant" when unknown.
    pub client: Option<String>,
}

fn error(id: Value, code: i64, msg: impl Into<String>) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": msg.into()}})
}

impl Server {
    pub fn new(backend: Backend) -> Server {
        Server { backend, client: None }
    }

    /// Serves newline-delimited JSON-RPC until `input` closes. stdout is the protocol stream.
    pub fn serve(&mut self, input: impl BufRead, mut output: impl Write) -> std::io::Result<()> {
        for line in input.lines() {
            if let Some(reply) = self.handle_line(&line?) {
                writeln!(output, "{reply}")?;
                output.flush()?;
            }
        }
        Ok(())
    }

    /// One message in, at most one reply out (none for notifications).
    pub fn handle_line(&mut self, line: &str) -> Option<Value> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }
        let msg: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => return Some(error(Value::Null, -32700, format!("parse error: {e}"))),
        };
        let method = msg.get("method").and_then(Value::as_str)?;
        let id = msg.get("id").cloned()?; // notifications get no reply
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        let result = match method {
            "initialize" => {
                if self.client.is_none() {
                    let info = &params["clientInfo"];
                    self.client = info["name"].as_str().or(info["title"].as_str()).map(bana_commands::agents::clean_name);
                }
                let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or(PROTOCOL_VERSION);
                let version = if SUPPORTED.contains(&asked) { asked } else { PROTOCOL_VERSION };
                json!({
                    "protocolVersion": version,
                    "capabilities": {"tools": {}, "resources": {}},
                    "serverInfo": {"name": PRODUCT_NAME.to_lowercase(), "title": PRODUCT_NAME, "version": env!("CARGO_PKG_VERSION")},
                    "instructions": format!("{INSTRUCTIONS} This server works on {}.", self.backend.describe()),
                })
            }
            "ping" => json!({}),
            "tools/list" => json!({"tools": tools()}),
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
                // An unknown tool is a protocol error, not a tool result (MCP spec).
                if !tools().as_array().is_some_and(|t| t.iter().any(|x| x["name"] == name)) {
                    return Some(error(id, -32602, format!("unknown tool `{name}`")));
                }
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                let agent = self.client.clone().unwrap_or_else(|| "assistant".into());
                match call_tool(&mut self.backend, &agent, name, &args) {
                    Ok(v) => {
                        json!({"content": [{"type": "text", "text": serde_json::to_string_pretty(&v).unwrap_or_default()}], "structuredContent": wrap(v), "isError": false})
                    }
                    Err(e) => json!({"content": [{"type": "text", "text": e}], "isError": true}),
                }
            }
            "resources/list" => json!({"resources": [
                {"uri": "bana://commands", "name": "commands", "description": "Every command (id, label, shortcut, params)", "mimeType": "application/json"},
                {"uri": "bana://page", "name": "page", "description": "The open page as a Markdown outline", "mimeType": "text/markdown"},
            ]}),
            "resources/read" => {
                let uri = params.get("uri").and_then(Value::as_str).unwrap_or_default();
                let (mime, text) = match uri {
                    "bana://commands" => ("application/json", self.backend.commands().map(|v| v.to_string())),
                    "bana://page" => (
                        "text/markdown",
                        self.backend.run("assistant", "export.markdown", json!({})).map(|v| v["markdown"].as_str().unwrap_or_default().to_string()),
                    ),
                    _ => return Some(error(id, -32002, format!("resource not found: {uri}"))),
                };
                match text {
                    Ok(t) => json!({"contents": [{"uri": uri, "mimeType": mime, "text": t}]}),
                    Err(e) => return Some(error(id, -32603, e)),
                }
            }
            other => return Some(error(id, -32601, format!("method not found: {other}"))),
        };
        Some(json!({"jsonrpc": "2.0", "id": id, "result": result}))
    }
}

/// `structuredContent` must be an object.
fn wrap(v: Value) -> Value {
    if v.is_object() { v } else { json!({"result": v}) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bana_core::{Date, sample};

    fn server() -> Server {
        let today = Date::new(2026, 10, 10).unwrap();
        Server::new(Backend::Local(Box::new(Session::with_today(sample::sample(today).unwrap(), today))))
    }

    fn call(s: &mut Server, msg: Value) -> Value {
        s.handle_line(&msg.to_string()).unwrap()
    }

    #[test]
    fn a_running_engine_is_found_through_its_live_file_and_edited_live() {
        // UC-10.11: the app announces port and token beside the graph; MCP works on it.
        let dir = std::env::temp_dir().join(format!("bana-live-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("graph.bana");
        let today = Date::new(2026, 10, 10).unwrap();
        let mut session = Session::with_today(sample::sample(today).unwrap(), today);
        session.path = Some(file.clone());
        let shared = bana_server::http::Shared::with_token(bana_server::Host::new(session), "t".repeat(32));
        let listener = bana_server::http::bind(0).unwrap();
        let port = listener.local_addr().unwrap().port();
        bana_server::http::announce(&shared, port, None).unwrap();
        let s2 = std::sync::Arc::clone(&shared);
        std::thread::spawn(move || bana_server::http::serve_on(s2, listener, dir.clone()));
        let mut b = Backend::find_live_with(&file, "t".repeat(32)).unwrap();
        let r = b.run("test", "node.add", json!({"name": "From MCP"})).unwrap();
        let id = bana_core::NodeId(r["id"].as_u64().unwrap());
        assert_eq!(shared.host.lock().unwrap().session.graph.name(id), "From MCP");
        // A wrong token is refused.
        let mut bad = Backend::Live { port, token: "nope".into() };
        assert!(bad.run("test", "graph.view", json!({})).is_err());
    }

    #[test]
    fn lifecycle_tools_and_resources() {
        let mut s = server();
        let init = call(&mut s, json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-03-26"}}));
        assert_eq!(init["result"]["protocolVersion"], "2025-03-26");
        assert!(s.handle_line(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string()).is_none());
        let tools = call(&mut s, json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}));
        assert!(tools["result"]["tools"].as_array().unwrap().len() >= 6);

        let add = call(
            &mut s,
            json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "add_node", "arguments": {"name": "Buy seeds", "tag": "task"}}}),
        );
        assert_eq!(add["result"]["isError"], false, "{add}");
        let id = add["result"]["structuredContent"]["id"].clone();
        let md = call(
            &mut s,
            json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "export_markdown", "arguments": {"node": id}}}),
        );
        assert!(md["result"]["content"][0]["text"].as_str().unwrap().contains("[ ] Buy seeds #task"));

        let bad = call(
            &mut s,
            json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "execute", "arguments": {"command": "no.such"}}}),
        );
        assert_eq!(bad["result"]["isError"], true);
        let page = call(&mut s, json!({"jsonrpc": "2.0", "id": 6, "method": "resources/read", "params": {"uri": "bana://page"}}));
        assert!(page["result"]["contents"][0]["text"].as_str().unwrap().contains("Community garden"));
        // DIFFS N89: a typo does not make a supertag; "today" is a parent; an empty search and an
        // unknown tool are refused; a failed execute says what the command takes.
        let typo = call(
            &mut s,
            json!({"jsonrpc": "2.0", "id": 8, "method": "tools/call", "params": {"name": "add_node", "arguments": {"name": "x", "tag": "tsak"}}}),
        );
        assert_eq!(typo["result"]["isError"], true);
        assert!(typo["result"]["content"][0]["text"].as_str().unwrap().contains("create_tag"));
        let made = call(
            &mut s,
            json!({"jsonrpc": "2.0", "id": 9, "method": "tools/call", "params": {"name": "add_node", "arguments": {"name": "y", "tag": "errand", "create_tag": true, "parent": "today"}}}),
        );
        assert_eq!(made["result"]["isError"], false, "{made}");
        let empty =
            call(&mut s, json!({"jsonrpc": "2.0", "id": 10, "method": "tools/call", "params": {"name": "search", "arguments": {"query": ""}}}));
        assert_eq!(empty["result"]["isError"], true);
        let nope = call(&mut s, json!({"jsonrpc": "2.0", "id": 11, "method": "tools/call", "params": {"name": "nope", "arguments": {}}}));
        assert_eq!(nope["error"]["code"], -32602);
        let hint = call(
            &mut s,
            json!({"jsonrpc": "2.0", "id": 12, "method": "tools/call", "params": {"name": "execute", "arguments": {"command": "node.merge", "params": {}}}}),
        );
        assert!(hint["result"]["content"][0]["text"].as_str().unwrap().contains("node.merge takes"), "{hint}");
        assert!(init["result"]["instructions"].as_str().unwrap().contains("works on"));
        let unknown = call(&mut s, json!({"jsonrpc": "2.0", "id": 7, "method": "nope"}));
        assert_eq!(unknown["error"]["code"], -32601);
        assert_eq!(s.handle_line("{not json").unwrap()["error"]["code"], -32700);
    }

    #[test]
    fn never_commands_are_refused_through_mcp_and_the_file_stays_byte_identical() {
        let dir = std::env::temp_dir().join(format!("bana-mcp-never-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("graph.bana");
        let _ = std::fs::remove_file(&file);
        let today = Date::new(2026, 10, 10).unwrap();
        let mut session = Session::with_today(sample::sample(today).unwrap(), today);
        session.path = Some(file.clone());
        let a = session.run("node.add", &json!({"name": "Signed contract"})).unwrap()["id"].as_u64().unwrap();
        session.run("node.toggleLock", &json!({"node": a})).unwrap();
        session.run("commandNode.create", &json!({"name": "Tidy", "steps": ["tag.add {\"tag\": \"task\"}"]})).unwrap();
        session.run("file.save", &json!({})).unwrap();
        let bytes = std::fs::read(&file).unwrap();
        let mut s = Server::new(Backend::Local(Box::new(session)));
        call(&mut s, json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"clientInfo": {"name": "claude-code", "version": "1"}}}));
        let exec = |s: &mut Server, command: &str, params: Value| {
            call(
                s,
                json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "execute", "arguments": {"command": command, "params": params}}}),
            )
        };
        for (command, params) in [
            ("node.hardDelete", json!({"node": a})),
            ("trash.empty", json!({})),
            ("node.toggleLock", json!({"node": a})),
            ("file.save", json!({"path": dir.join("other.bana").to_string_lossy()})),
            ("file.open", json!({"path": file.to_string_lossy()})),
            ("file.read", json!({"node": a})),
            ("import.folder", json!({"path": dir.to_string_lossy()})),
            ("import.file", json!({"path": file.to_string_lossy()})),
            ("settings.set", json!({"key": "time_12h", "value": true})),
            ("settings.get", json!({})),
            ("workspace.create", json!({"name": "Planted"})),
            ("workspace.switch", json!({"name": "Planted"})),
            ("commandNode.create", json!({"name": "Plant", "steps": ["trash.empty"]})),
            ("commandNode.on", json!({"command": "Tidy", "event": "added", "tag": "task"})),
            ("commandNode.run", json!({"command": "Tidy", "node": a})),
            ("view.deleteColumn", json!({"node": a, "field": "Due"})),
        ] {
            let r = exec(&mut s, command, params);
            assert_eq!(r["result"]["isError"], true, "{command}: {r}");
            let text = r["result"]["content"][0]["text"].as_str().unwrap();
            assert!(text.starts_with(&format!("refused: assistants may not run `{command}` (rule `")), "{text}");
        }
        // A locked node refuses an assistant's rename and field edit.
        assert_eq!(exec(&mut s, "node.setName", json!({"node": a, "name": "x"}))["result"]["isError"], true);
        assert_eq!(exec(&mut s, "field.add", json!({"node": a, "field": "Due", "value": "today"}))["result"]["isError"], true);
        assert_eq!(std::fs::read(&file).unwrap(), bytes, "nothing reached the file");
        assert!(!dir.join("other.bana").exists());
        // Normal edits work, and carry the client's name.
        let add = call(
            &mut s,
            json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "add_node", "arguments": {"name": "From Claude", "tag": "task"}}}),
        );
        assert_eq!(add["result"]["isError"], false, "{add}");
        let id = bana_core::NodeId(add["result"]["structuredContent"]["id"].as_u64().unwrap());
        assert_ne!(std::fs::read(&file).unwrap(), bytes);
        let by = match &s.backend {
            Backend::Local(session) => session.graph.get(id).and_then(|n| n.by.clone()),
            _ => None,
        };
        assert_eq!(by.as_deref(), Some("claude-code"));
        assert_eq!(bana_core::Graph::load(&file).unwrap().get(id).unwrap().by.clone(), Some("claude-code".into()), "kept in the file");
    }
}
