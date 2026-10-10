//! A small HTTP server (std only) so the web UI runs in a normal browser:
//!
//! - `POST /rpc` with `{"method": ..., "params": {...}}` (or an array of them, run in order) answers
//!   with [`Host::handle`]'s JSON (an array for an array).
//! - `GET /events?since=N` waits up to 25 s for the version to pass N, then answers `{"version"}`.
//! - Anything else is a file from the web UI's build folder (`index.html` for `/`).
//!
//! It listens on 127.0.0.1 only and refuses requests from pages on other origins, so a web site
//! open in the same browser cannot reach the notes. A `POST /rpc` that comes from no page (an MCP
//! server, the CLI, a script) must carry the server's token (`Authorization: Bearer <token>`),
//! the install's local token in the data folder (`tanacraft_core::local_token`); [`announce`] writes
//! the port beside the graph file for local tools to find.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use serde_json::{Value, json};

use crate::Host;

/// Requests bigger than this are refused (a paste of a large outline still fits).
const MAX_BODY: usize = 32 * 1024 * 1024;

pub struct Shared {
    pub host: Mutex<Host>,
    pub changed: Condvar,
    /// What a request from outside a browser page must send.
    pub token: String,
}

impl Shared {
    pub fn new(host: Host) -> Arc<Shared> {
        // The install's token (a fresh one only when the data folder cannot be written).
        let token = tanacraft_core::local_token().unwrap_or_else(|_| tanacraft_core::random_hex());
        Shared::with_token(host, token)
    }

    /// With a given token (tests, or an app that keeps its token elsewhere).
    pub fn with_token(host: Host, token: String) -> Arc<Shared> {
        Arc::new(Shared { host: Mutex::new(host), changed: Condvar::new(), token })
    }

    /// Runs one request (or an array of them) and wakes event waiters.
    pub fn handle(&self, req: &Value) -> Value {
        let Ok(mut host) = self.host.lock() else { return json!({"error": "host lock poisoned"}) };
        let one = |host: &mut Host, r: &Value| {
            let method = r.get("method").and_then(Value::as_str).unwrap_or_default();
            let params = r.get("params").cloned().unwrap_or_else(|| json!({}));
            host.handle(method, &params)
        };
        let before = host.version;
        let out = match req {
            Value::Array(reqs) => Value::Array(reqs.iter().take(10_000).map(|r| one(&mut host, r)).collect()),
            r => one(&mut host, r),
        };
        if host.version != before {
            self.changed.notify_all();
        }
        out
    }

    /// Autosaves every 200 ms once a change is a second old.
    pub fn spawn_autosave(self: &Arc<Self>) {
        let me = Arc::clone(self);
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_millis(200));
                if let Ok(mut h) = me.host.lock() {
                    h.autosave(Duration::from_secs(1));
                }
            }
        });
    }
}

/// Binds 127.0.0.1:`port` (0: any free port); the port is `listener.local_addr()`.
pub fn bind(port: u16) -> Result<TcpListener, String> {
    TcpListener::bind(("127.0.0.1", port)).map_err(|e| format!("port {port}: {e}"))
}

/// Writes the live file beside the graph (see [`tanacraft_core::live_file`]) so MCP and the CLI
/// reach this running engine. Readable by this user only. Returns its path (to remove on exit).
/// `graph`: where to announce when the session has no file of its own (an in-memory graph served
/// for a named file, as tests do).
pub fn announce(shared: &Shared, port: u16, graph: Option<&Path>) -> Option<PathBuf> {
    let graph = shared.host.lock().ok()?.session.path.clone().or_else(|| graph.map(Path::to_path_buf))?;
    let path = tanacraft_core::live_file(&graph);
    // The token itself stays in the data folder (`tanacraft_core::local_token`).
    let body = json!({"port": port, "pid": std::process::id()}).to_string();
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
    let mut f = opts.open(&path).ok()?;
    f.write_all(body.as_bytes()).ok()?;
    Some(path)
}

/// Serves until the process ends. `web` is the built web UI (`web/dist`). Call
/// [`Shared::spawn_autosave`] once as well.
pub fn serve(shared: Arc<Shared>, port: u16, web: PathBuf) -> Result<(), String> {
    serve_on(shared, bind(port)?, web)
}

/// [`serve`] on a listener already bound (after [`announce`]).
pub fn serve_on(shared: Arc<Shared>, listener: TcpListener, web: PathBuf) -> Result<(), String> {
    for stream in listener.incoming().flatten() {
        let shared = Arc::clone(&shared);
        let web = web.clone();
        std::thread::spawn(move || {
            if let Err(e) = connection(&shared, stream, &web) {
                eprintln!("http: {e}");
            }
        });
    }
    Ok(())
}

struct Request {
    method: String,
    path: String,
    query: String,
    origin: Option<String>,
    /// The Host header (checked against DNS rebinding).
    host: Option<String>,
    /// `Authorization: Bearer <token>`.
    token: Option<String>,
    body: Vec<u8>,
}

fn read_request(stream: &TcpStream) -> Result<Option<Request>, String> {
    let mut r = BufReader::new(stream);
    let mut line = String::new();
    if r.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
        return Ok(None);
    }
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let target = parts.next().unwrap_or("/").to_string();
    let (path, query) = target.split_once('?').map(|(a, b)| (a.to_string(), b.to_string())).unwrap_or((target, String::new()));
    let mut len = 0usize;
    let mut origin = None;
    let mut host = None;
    let mut token = None;
    for _ in 0..200 {
        let mut h = String::new();
        if r.read_line(&mut h).map_err(|e| e.to_string())? == 0 || h.trim().is_empty() {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            match k.trim().to_ascii_lowercase().as_str() {
                "content-length" => len = v.trim().parse().unwrap_or(0),
                "origin" => origin = Some(v.trim().to_string()),
                "host" => host = Some(v.trim().to_string()),
                "authorization" => token = v.trim().strip_prefix("Bearer ").map(str::to_string),
                _ => {}
            }
        }
    }
    if len > MAX_BODY {
        return Err("request too large".into());
    }
    let mut body = vec![0; len];
    r.read_exact(&mut body).map_err(|e| e.to_string())?;
    Ok(Some(Request { method, path, query, origin, host, token, body }))
}

/// Pages on this machine's own server (any port: the Vite dev server too) and the Tauri app.
pub(crate) fn origin_ok(origin: Option<&str>) -> bool {
    let Some(o) = origin else { return true };
    let host = o.split("://").nth(1).unwrap_or(o);
    let host = host.rsplit_once(':').map(|(h, _)| h).unwrap_or(host);
    matches!(host, "127.0.0.1" | "localhost" | "[::1]" | "tauri.localhost") || o.starts_with("tauri://")
}

fn respond(s: &TcpStream, status: &str, ctype: &str, origin: Option<&str>, body: &[u8]) -> Result<(), String> {
    respond_with(s, status, ctype, origin, "", body)
}

fn respond_with(mut s: &TcpStream, status: &str, ctype: &str, origin: Option<&str>, extra: &str, body: &[u8]) -> Result<(), String> {
    let cors = match origin.filter(|o| origin_ok(Some(o))) {
        Some(o) => format!("Access-Control-Allow-Origin: {o}\r\nAccess-Control-Allow-Headers: content-type, authorization\r\nVary: Origin\r\n"),
        None => String::new(),
    };
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\n{cors}{extra}Connection: close\r\n\r\n",
        body.len()
    );
    s.write_all(head.as_bytes()).and_then(|_| s.write_all(body)).map_err(|e| e.to_string())
}

fn connection(shared: &Arc<Shared>, stream: TcpStream, web: &Path) -> Result<(), String> {
    let Some(req) = read_request(&stream)? else { return Ok(()) };
    let origin = req.origin.as_deref();
    // The MCP endpoint (streamable HTTP): the token is the authority, so it also answers through a
    // tunnel whose Host is not this machine's; pages on other origins are still refused.
    if req.path == "/mcp" {
        if !origin_ok(origin) {
            return respond(&stream, "403 Forbidden", "text/plain", None, b"other origins may not use this server");
        }
        return mcp_http(shared, &stream, &req);
    }
    let host_ok = req.host.as_deref().is_none_or(|h| origin_ok(Some(&format!("http://{h}"))));
    if !origin_ok(origin) || !host_ok {
        return respond(&stream, "403 Forbidden", "text/plain", None, b"other origins may not use this server");
    }
    match (req.method.as_str(), req.path.as_str()) {
        ("OPTIONS", _) => respond(&stream, "204 No Content", "text/plain", origin, b""),
        // Not from a page: only a local tool that read the token may edit (N/A for the browser UI,
        // whose requests carry its origin).
        ("POST", "/rpc") if origin.is_none() && req.token.as_deref() != Some(shared.token.as_str()) => {
            respond(&stream, "401 Unauthorized", "application/json", None, br#"{"error":"send the token from the live file beside the graph"}"#)
        }
        ("POST", "/rpc") => {
            let out = match serde_json::from_slice::<Value>(&req.body) {
                Ok(v) => shared.handle(&v),
                Err(e) => json!({"error": format!("bad json: {e}")}),
            };
            respond(&stream, "200 OK", "application/json", origin, out.to_string().as_bytes())
        }
        ("GET", "/events") => {
            let since: u64 = req.query.split('&').find_map(|kv| kv.strip_prefix("since=")).and_then(|v| v.parse().ok()).unwrap_or(0);
            let version = wait_version(shared, since);
            respond(&stream, "200 OK", "application/json", origin, json!({"version": version}).to_string().as_bytes())
        }
        ("GET" | "HEAD", path) => static_file(&stream, web, path, origin),
        _ => respond(&stream, "405 Method Not Allowed", "text/plain", origin, b""),
    }
}

/// MCP over streamable HTTP (spec 2025-06-18): `POST /mcp` with a JSON-RPC message or batch,
/// answered as JSON; notifications only get 202. No server-sent stream (`GET` is 405), no
/// sessions. The same tools as the stdio server, run in their own window of this engine.
fn mcp_http(shared: &Arc<Shared>, stream: &TcpStream, req: &Request) -> Result<(), String> {
    if req.token.as_deref() != Some(shared.token.as_str()) {
        return respond_with(
            stream,
            "401 Unauthorized",
            "application/json",
            None,
            "WWW-Authenticate: Bearer realm=\"tanacraft\"\r\n",
            br#"{"error":"send the install's local token as a Bearer token"}"#,
        );
    }
    if req.method != "POST" {
        return respond_with(stream, "405 Method Not Allowed", "text/plain", None, "Allow: POST\r\n", b"");
    }
    let msg: Value = match serde_json::from_slice(&req.body) {
        Ok(v) => v,
        Err(e) => {
            let err = json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": format!("parse error: {e}")}});
            return respond(stream, "400 Bad Request", "application/json", None, err.to_string().as_bytes());
        }
    };
    let me = Arc::clone(shared);
    let engine = move |method: &str, params: Value| me.handle(&json!({"method": method, "params": params}));
    let mut server = tanacraft_mcp::Server::new(tanacraft_mcp::Backend::Engine(Box::new(engine)));
    let replies: Vec<Value> = match &msg {
        Value::Array(all) => all.iter().take(1000).filter_map(|m| server.handle_line(&m.to_string())).collect(),
        one => server.handle_line(&one.to_string()).into_iter().collect(),
    };
    if replies.is_empty() {
        return respond(stream, "202 Accepted", "application/json", None, b"");
    }
    let body = if msg.is_array() { Value::Array(replies) } else { replies.into_iter().next().unwrap_or(Value::Null) };
    respond(stream, "200 OK", "application/json", None, body.to_string().as_bytes())
}

fn wait_version(shared: &Shared, since: u64) -> u64 {
    let Ok(guard) = shared.host.lock() else { return since };
    match shared.changed.wait_timeout_while(guard, Duration::from_secs(25), |h| h.version <= since) {
        Ok((h, _)) => h.version,
        Err(_) => since,
    }
}

fn static_file(stream: &TcpStream, web: &Path, path: &str, origin: Option<&str>) -> Result<(), String> {
    let rel = Path::new(path.trim_start_matches('/'));
    // No `..`, no absolute paths: only files inside the build folder.
    if rel.components().any(|c| !matches!(c, Component::Normal(_))) {
        return respond(stream, "404 Not Found", "text/plain", origin, b"not found");
    }
    let mut file = web.join(rel);
    if path == "/" || file.is_dir() {
        file = web.join("index.html");
    }
    match std::fs::read(&file) {
        Ok(bytes) => respond(stream, "200 OK", content_type(&file), origin, &bytes),
        // Client-side routes fall back to the app.
        Err(_) if !path.contains('.') => match std::fs::read(web.join("index.html")) {
            Ok(bytes) => respond(stream, "200 OK", "text/html; charset=utf-8", origin, &bytes),
            Err(_) => respond(stream, "404 Not Found", "text/plain", origin, b"web UI not built: run `pnpm --dir web build`"),
        },
        Err(_) => respond(stream, "404 Not Found", "text/plain", origin, b"not found"),
    }
}

fn content_type(p: &Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()).unwrap_or_default() {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "webp" => "image/webp",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    }
}
