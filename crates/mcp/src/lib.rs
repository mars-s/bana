//! TanaCraft's MCP server (Model Context Protocol, JSON-RPC 2.0, one message per line on stdio).
//!
//! Two backends: [`Backend::Live`] drives the running app (desktop or `tanacraft-cli serve`) over
//! its localhost HTTP API, with the token from the live file beside the graph, so the open window
//! shows every change at once; [`Backend::Local`] edits the graph file in-process (saved after
//! every change), only when no app has it open. [`Backend::open`] picks.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::io::{BufRead, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};
use tanacraft_commands::Session;
use tanacraft_core::PRODUCT_NAME;

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
}

impl Backend {
    /// The running app that has `graph` open (its live file answers), else the file itself.
    pub fn open(graph: &Path) -> Result<Backend, String> {
        if let Some(live) = Backend::find_live(graph) {
            return Ok(live);
        }
        let g = tanacraft_core::load_or_migrate(graph).map_err(|e| e.to_string())?.unwrap_or_default();
        if let Some(dir) = graph.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let mut s = Session::new(g);
        s.path = Some(graph.to_path_buf());
        Ok(Backend::Local(Box::new(s)))
    }

    /// The running app announced beside `graph`, if it answers.
    pub fn find_live(graph: &Path) -> Option<Backend> {
        let v: Value = serde_json::from_slice(&std::fs::read(tanacraft_core::live_file(graph)).ok()?).ok()?;
        let port = u16::try_from(v.get("port")?.as_u64()?).ok()?;
        let token = v.get("token")?.as_str()?.to_string();
        let mut b = Backend::Live { port, token };
        b.post(&json!({"method": "version"})).ok()?;
        Some(b)
    }

    /// `host:port` (127.0.0.1 only) with a token (`--token`, or the live file of the default graph).
    pub fn connect(addr: &str, token: Option<String>) -> Result<Backend, String> {
        let port = addr.rsplit(':').next().and_then(|p| p.parse::<u16>().ok()).ok_or(format!("connect {addr}: no port"))?;
        let token = match token {
            Some(t) => t,
            None => {
                let v: Value = std::fs::read(tanacraft_core::live_file(&tanacraft_core::default_graph_path()))
                    .ok()
                    .and_then(|b| serde_json::from_slice(&b).ok())
                    .ok_or("no token: pass --token (it is in the live file beside the graph)")?;
                v["token"].as_str().unwrap_or_default().to_string()
            }
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

    /// Runs a command by id.
    pub fn run(&mut self, command: &str, params: Value) -> Result<Value, String> {
        match self {
            Backend::Local(s) => {
                let mutates = s.registry.get(command).is_some_and(|c| c.mutates)
                    || command.starts_with("nav.")
                    || command == "edit.undo"
                    || command == "edit.redo";
                let r = s.run(command, &params).map_err(|e| e.to_string())?;
                if mutates && s.path.is_some() && s.graph.is_dirty() {
                    s.run("file.save", &json!({})).map_err(|e| format!("saved nothing: {e}"))?;
                }
                Ok(r)
            }
            Backend::Live { .. } => {
                let v = self.post(&json!({"method": "run", "params": {"command": command, "params": params}}))?;
                match v.get("error") {
                    Some(e) => Err(e.as_str().unwrap_or("error").to_string()),
                    None => Ok(v.get("result").cloned().unwrap_or(Value::Null)),
                }
            }
        }
    }

    pub fn commands(&mut self) -> Result<Value, String> {
        match self {
            Backend::Local(s) => Ok(s.registry.describe()),
            Backend::Live { .. } => self.post(&json!({"method": "commands", "params": {}})),
        }
    }
}

/// Tool definitions (MCP `tools/list`).
pub fn tools() -> Value {
    let node = json!({"type": ["integer", "string"], "description": "node id"});
    json!([
        {"name": "list_commands", "description": "Every command: id, label, shortcut, params, whether it changes the graph.", "inputSchema": {"type": "object", "properties": {}}},
        {"name": "execute", "description": "Run a command by id with its params (see list_commands).", "inputSchema": {"type": "object", "properties": {"command": {"type": "string"}, "params": {"type": "object"}}, "required": ["command"]}},
        {"name": "view", "description": "The page currently open: title and visible bullets (> marks the focused one).", "inputSchema": {"type": "object", "properties": {}}},
        {"name": "inspect", "description": "A node and its subtree as JSON (ids, tags, fields, children). Default: the open page.", "inputSchema": {"type": "object", "properties": {"node": node}}},
        {"name": "search", "description": "Find nodes whose text contains the query.", "inputSchema": {"type": "object", "properties": {"query": {"type": "string"}}, "required": ["query"]}},
        {"name": "add_node", "description": "Add a node under parent (default: the open page). Optional supertag.", "inputSchema": {"type": "object", "properties": {"name": {"type": "string"}, "parent": node, "tag": {"type": "string"}}, "required": ["name"]}},
        {"name": "export_markdown", "description": "A node and its subtree as a Markdown outline. Default: the open page.", "inputSchema": {"type": "object", "properties": {"node": node}}},
    ])
}

fn call_tool(b: &mut Backend, name: &str, args: &Value) -> Result<Value, String> {
    let arg = |k: &str| args.get(k).cloned().unwrap_or(Value::Null);
    match name {
        "list_commands" => b.commands(),
        "execute" => {
            let id = args.get("command").and_then(Value::as_str).ok_or("missing `command`")?;
            b.run(id, args.get("params").cloned().filter(|v| !v.is_null()).unwrap_or(json!({})))
        }
        "view" => b.run("graph.view", json!({})),
        "inspect" => b.run("graph.inspect", json!({"node": arg("node")})),
        "search" => b.run("graph.search", json!({"query": arg("query")})),
        "export_markdown" => b.run("export.markdown", json!({"node": arg("node")})),
        "add_node" => {
            let r = b.run("node.add", json!({"name": arg("name"), "parent": arg("parent")}))?;
            if let Some(tag) = args.get("tag").and_then(Value::as_str) {
                b.run("tag.add", json!({"node": r["id"], "tag": tag}))?;
            }
            Ok(r)
        }
        other => Err(format!("unknown tool `{other}`")),
    }
}

/// An MCP server bound to one backend.
pub struct Server {
    pub backend: Backend,
}

fn error(id: Value, code: i64, msg: impl Into<String>) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": msg.into()}})
}

impl Server {
    pub fn new(backend: Backend) -> Server {
        Server { backend }
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
                let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or(PROTOCOL_VERSION);
                let version = if SUPPORTED.contains(&asked) { asked } else { PROTOCOL_VERSION };
                json!({
                    "protocolVersion": version,
                    "capabilities": {"tools": {}, "resources": {}},
                    "serverInfo": {"name": PRODUCT_NAME.to_lowercase(), "title": PRODUCT_NAME, "version": env!("CARGO_PKG_VERSION")},
                    "instructions": INSTRUCTIONS,
                })
            }
            "ping" => json!({}),
            "tools/list" => json!({"tools": tools()}),
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                match call_tool(&mut self.backend, name, &args) {
                    Ok(v) => {
                        json!({"content": [{"type": "text", "text": serde_json::to_string_pretty(&v).unwrap_or_default()}], "structuredContent": wrap(v), "isError": false})
                    }
                    Err(e) => json!({"content": [{"type": "text", "text": e}], "isError": true}),
                }
            }
            "resources/list" => json!({"resources": [
                {"uri": "tanacraft://commands", "name": "commands", "description": "Every command (id, label, shortcut, params)", "mimeType": "application/json"},
                {"uri": "tanacraft://page", "name": "page", "description": "The open page as a Markdown outline", "mimeType": "text/markdown"},
            ]}),
            "resources/read" => {
                let uri = params.get("uri").and_then(Value::as_str).unwrap_or_default();
                let (mime, text) = match uri {
                    "tanacraft://commands" => ("application/json", self.backend.commands().map(|v| v.to_string())),
                    "tanacraft://page" => (
                        "text/markdown",
                        self.backend.run("export.markdown", json!({})).map(|v| v["markdown"].as_str().unwrap_or_default().to_string()),
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
    use tanacraft_core::{Date, sample};

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
        let dir = std::env::temp_dir().join(format!("tanacraft-live-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("graph.tanacraft");
        let today = Date::new(2026, 10, 10).unwrap();
        let mut session = Session::with_today(sample::sample(today).unwrap(), today);
        session.path = Some(file.clone());
        let shared = tanacraft_server::http::Shared::new(tanacraft_server::Host::new(session));
        let listener = tanacraft_server::http::bind(0).unwrap();
        let port = listener.local_addr().unwrap().port();
        tanacraft_server::http::announce(&shared, port, None).unwrap();
        let s2 = std::sync::Arc::clone(&shared);
        std::thread::spawn(move || tanacraft_server::http::serve_on(s2, listener, dir.clone()));
        let mut b = Backend::open(&file).unwrap();
        assert!(matches!(b, Backend::Live { .. }));
        let r = b.run("node.add", json!({"name": "From MCP"})).unwrap();
        let id = tanacraft_core::NodeId(r["id"].as_u64().unwrap());
        assert_eq!(shared.host.lock().unwrap().session.graph.name(id), "From MCP");
        // A wrong token is refused.
        let mut bad = Backend::Live { port, token: "nope".into() };
        assert!(bad.run("graph.view", json!({})).is_err());
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
        let page = call(&mut s, json!({"jsonrpc": "2.0", "id": 6, "method": "resources/read", "params": {"uri": "tanacraft://page"}}));
        assert!(page["result"]["contents"][0]["text"].as_str().unwrap().contains("Community garden"));
        let unknown = call(&mut s, json!({"jsonrpc": "2.0", "id": 7, "method": "nope"}));
        assert_eq!(unknown["error"]["code"], -32601);
        assert_eq!(s.handle_line("{not json").unwrap()["error"]["code"], -32700);
    }
}
