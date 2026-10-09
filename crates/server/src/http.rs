//! A small HTTP server (std only) so the web UI runs in a normal browser:
//!
//! - `POST /rpc` with `{"method": ..., "params": {...}}` (or an array of them, run in order) answers
//!   with [`Host::handle`]'s JSON (an array for an array).
//! - `GET /events?since=N` waits up to 25 s for the version to pass N, then answers `{"version"}`.
//! - Anything else is a file from the web UI's build folder (`index.html` for `/`).
//!
//! It listens on 127.0.0.1 only and refuses requests from pages on other origins, so a web site
//! open in the same browser cannot reach the notes.

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
}

impl Shared {
    pub fn new(host: Host) -> Arc<Shared> {
        Arc::new(Shared { host: Mutex::new(host), changed: Condvar::new() })
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

/// Serves until the process ends. `web` is the built web UI (`web/dist`).
pub fn serve(shared: Arc<Shared>, port: u16, web: PathBuf) -> Result<(), String> {
    let listener = TcpListener::bind(("127.0.0.1", port)).map_err(|e| format!("port {port}: {e}"))?;
    shared.spawn_autosave();
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
                _ => {}
            }
        }
    }
    if len > MAX_BODY {
        return Err("request too large".into());
    }
    let mut body = vec![0; len];
    r.read_exact(&mut body).map_err(|e| e.to_string())?;
    Ok(Some(Request { method, path, query, origin, host, body }))
}

/// Pages on this machine's own server (any port: the Vite dev server too) and the Tauri app.
pub(crate) fn origin_ok(origin: Option<&str>) -> bool {
    let Some(o) = origin else { return true };
    let host = o.split("://").nth(1).unwrap_or(o);
    let host = host.rsplit_once(':').map(|(h, _)| h).unwrap_or(host);
    matches!(host, "127.0.0.1" | "localhost" | "[::1]" | "tauri.localhost") || o.starts_with("tauri://")
}

fn respond(mut s: &TcpStream, status: &str, ctype: &str, origin: Option<&str>, body: &[u8]) -> Result<(), String> {
    let cors = match origin.filter(|o| origin_ok(Some(o))) {
        Some(o) => format!("Access-Control-Allow-Origin: {o}\r\nAccess-Control-Allow-Headers: content-type\r\nVary: Origin\r\n"),
        None => String::new(),
    };
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\n{cors}Connection: close\r\n\r\n",
        body.len()
    );
    s.write_all(head.as_bytes()).and_then(|_| s.write_all(body)).map_err(|e| e.to_string())
}

fn connection(shared: &Shared, stream: TcpStream, web: &Path) -> Result<(), String> {
    let Some(req) = read_request(&stream)? else { return Ok(()) };
    let origin = req.origin.as_deref();
    let host_ok = req.host.as_deref().is_none_or(|h| origin_ok(Some(&format!("http://{h}"))));
    if !origin_ok(origin) || !host_ok {
        return respond(&stream, "403 Forbidden", "text/plain", None, b"other origins may not use this server");
    }
    match (req.method.as_str(), req.path.as_str()) {
        ("OPTIONS", _) => respond(&stream, "204 No Content", "text/plain", origin, b""),
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
