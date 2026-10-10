//! The sync service the apps run: a thread holding one WebSocket to the relay, reconnecting with
//! backoff, plus the calls behind Settings > Sync (`sync.*` requests, see [`Service::handle`]).
//! Offline, edits pile up in the graph as usual; the next connection sends them.

use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tungstenite::Message;
use tungstenite::stream::MaybeTlsStream;

use crate::client::{Client, Event, Replica, State};
use crate::config::Config;
use crate::proto::{CLOSE_AUTH, CLOSE_REVOKED};
use crate::{Error, Result, now_ms};

/// What the service syncs: lends the graph (as a [`Replica`]) for one call, under its lock.
pub trait SharedReplica: Send + Sync + 'static {
    fn with(&self, f: &mut dyn FnMut(&mut dyn Replica));
    /// Called with no lock held after each step: the place to tell views about merged edits.
    fn settled(&self) {}
}

const READ_TICK: Duration = Duration::from_millis(150);
/// Local edits are pushed at most this often: typing makes one relay write per tick, not per key
/// (the free plan allows 100,000 stored rows a day).
const POLL_EVERY: Duration = Duration::from_millis(750);
const PING_EVERY: Duration = Duration::from_secs(25);
/// No frame (not even a pong) for this long: the connection is dead.
const SILENT_FOR: Duration = Duration::from_secs(70);
const MAX_BACKOFF: Duration = Duration::from_secs(60);

#[derive(Default)]
pub(crate) struct St {
    pub(crate) client: Option<Client>,
    /// Frames from requests (invite, revoke) for the worker to send.
    outbox: Vec<String>,
    /// Bumped to stop the current worker (leave, a new setup).
    generation: u64,
    /// Skip the backoff wait and connect now.
    wake: bool,
    /// Edits not on the relay yet (from the worker's last look).
    pending: bool,
    /// The last connection problem.
    net_error: Option<String>,
    /// Bana ID: the sign-in in progress, the last outcome ("ok" or what went wrong), the new
    /// recovery code until the page has shown it, and whether this passkey can unlock (PRF).
    pub(crate) flow: Option<crate::account::Flow>,
    pub(crate) id_note: Option<String>,
    pub(crate) recovery: Option<String>,
    pub(crate) id_prf: bool,
}

pub(crate) struct Inner {
    pub(crate) path: PathBuf,
    replica: Arc<dyn SharedReplica>,
    st: Mutex<St>,
    pub(crate) cv: Condvar,
}

#[derive(Clone)]
pub struct Service {
    pub(crate) inner: Arc<Inner>,
}

fn net(e: impl std::fmt::Display) -> Error {
    Error::Net(e.to_string())
}

impl Service {
    /// The service for a graph whose sync settings live at `path`; starts syncing when the file
    /// is there (a device set up before).
    pub fn new(path: PathBuf, replica: Arc<dyn SharedReplica>) -> Service {
        let s = Service { inner: Arc::new(Inner { path, replica, st: Mutex::new(St::default()), cv: Condvar::new() }) };
        match Config::load(&s.inner.path) {
            Ok(Some(cfg)) => {
                if let Err(e) = s.start(cfg) {
                    eprintln!("sync: {e}");
                }
            }
            Ok(None) => {}
            Err(e) => eprintln!("sync: {e}"),
        }
        s
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, St> {
        self.inner.st.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn start(&self, cfg: Config) -> Result<()> {
        let client = Client::new(cfg)?;
        let generation = {
            let mut st = self.lock();
            st.generation += 1;
            st.client = Some(client);
            st.outbox.clear();
            st.net_error = None;
            st.generation
        };
        self.inner.cv.notify_all();
        let inner = Arc::clone(&self.inner);
        std::thread::Builder::new().name("bana-sync".into()).spawn(move || worker(inner, generation)).map_err(net)?;
        Ok(())
    }

    /// Starts syncing this workspace through `relay` as its first device.
    pub fn setup(&self, relay: &str, name: &str) -> Result<Value> {
        if self.lock().client.is_some() {
            return Err(Error::Config("sync is already set up on this device".into()));
        }
        let cfg = Config::create(relay, name)?;
        cfg.save(&self.inner.path)?;
        self.start(cfg)?;
        Ok(self.status())
    }

    /// Joins the workspace of a pairing code: this copy's notes are replaced by it on the first sync.
    pub fn join(&self, code: &str, name: &str) -> Result<Value> {
        if self.lock().client.is_some() {
            return Err(Error::Config("sync is already set up on this device; turn it off first".into()));
        }
        let code = crate::code::Code::decode(code)?;
        let cfg = Config::join(&code, name)?;
        cfg.save(&self.inner.path)?;
        self.start(cfg)?;
        Ok(self.status())
    }

    /// Stops syncing on this device and forgets its settings (the notes stay).
    pub fn leave(&self) -> Result<Value> {
        {
            let mut st = self.lock();
            st.generation += 1;
            st.client = None;
            st.outbox.clear();
        }
        self.inner.cv.notify_all();
        match std::fs::remove_file(&self.inner.path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(Error::Config(e.to_string())),
        }
        Ok(self.status())
    }

    /// A pairing code (and its QR code) for another device; waits for the relay to take it.
    pub fn invite(&self, wait: Duration) -> Result<Value> {
        let mut st = self.lock();
        let Some(c) = st.client.as_mut() else { return Err(Error::Config("set up sync first".into())) };
        if !c.ready() {
            return Err(Error::Net("not connected to the relay; try again when sync shows Synced".into()));
        }
        let (code, hash, frame) = c.invite()?;
        st.outbox.push(frame);
        let deadline = Instant::now() + wait;
        loop {
            if let Some(c) = st.client.as_mut()
                && let Some(i) = c.events.iter().position(|e| *e == Event::Invited(hash.clone()))
            {
                c.events.remove(i);
                break;
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(Error::Net("the relay did not answer; try again".into()));
            }
            st = self.inner.cv.wait_timeout(st, left).map(|(g, _)| g).unwrap_or_else(|p| p.into_inner().0);
        }
        drop(st);
        Ok(json!({"code": code.encode(), "qr": code.qr_svg()?, "expires": "in 24 hours, once"}))
    }

    pub fn revoke(&self, device: &str) -> Result<Value> {
        let mut st = self.lock();
        let Some(c) = st.client.as_ref() else { return Err(Error::Config("set up sync first".into())) };
        if device == c.device_id() {
            return Err(Error::Config("this is this device: turn sync off here instead".into()));
        }
        if !c.devices.contains_key(device) {
            return Err(Error::Config("no such device".into()));
        }
        if !c.ready() {
            return Err(Error::Net("not connected to the relay".into()));
        }
        let frame = c.revoke(device);
        st.outbox.push(frame);
        Ok(json!({"ok": true}))
    }

    pub fn devices(&self) -> Value {
        let st = self.lock();
        let list: Vec<&crate::client::DeviceInfo> = st.client.as_ref().map(|c| c.devices.values().collect()).unwrap_or_default();
        json!({"devices": list})
    }

    pub fn status(&self) -> Value {
        let st = self.lock();
        let Some(c) = st.client.as_ref() else { return json!({"state": "off"}) };
        let state = if c.state == State::Synced && st.pending { State::Syncing } else { c.state };
        let message = match state {
            State::Error => c.problem.clone(),
            State::Offline => st.net_error.clone().or_else(|| c.problem.clone()),
            _ => c.problem.clone(),
        };
        json!({
            "state": state,
            "message": message,
            "relay": c.cfg.relay,
            "name": c.cfg.name,
            "device": c.device_id(),
            "devices": c.devices.values().filter(|d| !d.revoked).count(),
            "lastSynced": c.last_synced,
            "pending": st.pending,
            "joining": c.cfg.joining,
        })
    }

    /// Connects now (no backoff) and waits up to `wait` for everything to be on the relay and
    /// everything on the relay to be here (the phone's foreground and background refresh).
    pub fn sync_now(&self, wait: Duration) -> Value {
        let start = now_ms();
        let deadline = Instant::now() + wait;
        let mut st = self.lock();
        st.wake = true;
        self.inner.cv.notify_all();
        loop {
            let done = st
                .client
                .as_ref()
                .is_none_or(|c| c.state == State::Error || (c.state == State::Synced && !st.pending && c.last_synced.is_some_and(|t| t >= start)));
            let left = deadline.saturating_duration_since(Instant::now());
            if done || left.is_zero() {
                break;
            }
            st = self.inner.cv.wait_timeout(st, left).map(|(g, _)| g).unwrap_or_else(|p| p.into_inner().0);
        }
        drop(st);
        self.status()
    }

    /// `sync.*` requests from the apps: status, setup {relay, name}, join {code, name}, invite,
    /// devices, revoke {device}, leave, now {wait ms}.
    pub fn handle(&self, method: &str, params: &Value) -> Value {
        let s = |k: &str| params.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
        let name = || Some(s("name")).filter(|n| !n.trim().is_empty()).unwrap_or_else(default_name);
        let r = match method {
            "sync.status" => Ok(self.status()),
            "sync.setup" => self.setup(&s("relay"), &name()),
            "sync.join" => self.join(&s("code"), &name()),
            "sync.invite" => self.invite(Duration::from_secs(10)),
            "sync.devices" => Ok(self.devices()),
            "sync.revoke" => self.revoke(&s("device")),
            "sync.leave" => self.leave(),
            m if m.starts_with("id.") => self.id_handle(m, params),
            "sync.now" => Ok(self.sync_now(Duration::from_millis(params.get("wait").and_then(Value::as_u64).unwrap_or(10_000).min(60_000)))),
            other => Err(Error::Config(format!("unknown method `{other}`"))),
        };
        match r {
            Ok(v) => v,
            Err(e) => json!({"error": e.to_string()}),
        }
    }
}

/// This computer's name (for the devices list).
pub fn default_name() -> String {
    std::process::Command::new("scutil")
        .args(["--get", "ComputerName"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .or_else(|| std::process::Command::new("hostname").output().ok().and_then(|o| String::from_utf8(o.stdout).ok()))
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "Computer".into())
}

type Socket = tungstenite::WebSocket<MaybeTlsStream<TcpStream>>;

fn connect(url: &str) -> Result<(Socket, TcpStream)> {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    let host = rest.split('/').next().unwrap_or_default();
    let tls = url.starts_with("wss://");
    let addr = if host.rsplit_once(':').is_some_and(|(_, p)| p.chars().all(|c| c.is_ascii_digit())) && !host.ends_with(']') {
        host.to_string()
    } else {
        format!("{host}:{}", if tls { 443 } else { 80 })
    };
    let mut last = Error::Net(format!("cannot resolve {host}"));
    for a in addr.to_socket_addrs().map_err(|e| Error::Net(format!("{host}: {e}")))? {
        match TcpStream::connect_timeout(&a, Duration::from_secs(10)) {
            Ok(tcp) => {
                tcp.set_read_timeout(Some(Duration::from_secs(15))).map_err(net)?;
                tcp.set_write_timeout(Some(Duration::from_secs(15))).map_err(net)?;
                tcp.set_nodelay(true).map_err(net)?;
                let handle = tcp.try_clone().map_err(net)?;
                let (ws, _) = tungstenite::client_tls(url, tcp).map_err(|e| Error::Net(format!("relay: {e}")))?;
                return Ok((ws, handle));
            }
            Err(e) => last = Error::Net(format!("{host}: {e}")),
        }
    }
    Err(last)
}

/// One turn of the client: a frame handled, or a poll.
type Step<'a> = dyn FnMut(&mut Client, &mut dyn Replica) -> Result<Vec<String>> + 'a;

impl Inner {
    fn lock(&self) -> MutexGuard<'_, St> {
        self.st.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Runs `f` on the client (under the service lock) and the graph (under its own), then saves
    /// the client's progress and wakes anyone waiting.
    fn step(&self, generation: u64, f: &mut Step<'_>) -> Option<Vec<String>> {
        let mut st = self.lock();
        if st.generation != generation {
            return None;
        }
        let c = st.client.as_mut()?;
        let mut out = Ok(Vec::new());
        let mut pending = false;
        self.replica.with(&mut |r| {
            out = f(c, r);
            pending = c.pending(r);
        });
        st.pending = pending;
        let c = st.client.as_mut()?;
        let out = match out {
            Ok(o) => o,
            Err(e) => {
                c.problem = Some(e.to_string());
                Vec::new()
            }
        };
        if c.dirty {
            c.dirty = false;
            if let Err(e) = c.cfg.save(&self.path) {
                c.problem = Some(e.to_string());
            }
        }
        drop(st);
        self.cv.notify_all();
        self.replica.settled();
        Some(out)
    }

    /// Waits `d` unless woken (a sync request) or stopped.
    fn sleep(&self, generation: u64, d: Duration) {
        let deadline = Instant::now() + d;
        let mut st = self.lock();
        while st.generation == generation && !st.wake {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            st = self.cv.wait_timeout(st, left).map(|(g, _)| g).unwrap_or_else(|p| p.into_inner().0);
        }
        st.wake = false;
    }

    fn alive(&self, generation: u64) -> bool {
        let st = self.lock();
        st.generation == generation && st.client.as_ref().is_some_and(|c| c.state != State::Error)
    }
}

fn worker(inner: Arc<Inner>, generation: u64) {
    let mut backoff = Duration::from_secs(1);
    while inner.alive(generation) {
        let url = match inner.lock().client.as_ref() {
            Some(c) => c.cfg.socket_url(),
            None => return,
        };
        match connect(&url) {
            Ok((ws, tcp)) => {
                backoff = Duration::from_secs(1);
                inner.lock().net_error = None;
                session(&inner, generation, ws, &tcp);
            }
            Err(e) => {
                let mut st = inner.lock();
                st.net_error = Some(e.to_string());
                if let Some(c) = st.client.as_mut() {
                    c.closed();
                }
                drop(st);
                inner.cv.notify_all();
            }
        }
        if !inner.alive(generation) {
            break;
        }
        // Jitter (0 to 25%) so devices that lost the relay together do not all come back at once.
        let jitter = Duration::from_millis(now_ms() % (backoff.as_millis() as u64 / 4 + 1));
        inner.sleep(generation, backoff + jitter);
        backoff = (backoff * 2).min(MAX_BACKOFF);
    }
}

fn session(inner: &Arc<Inner>, generation: u64, mut ws: Socket, tcp: &TcpStream) {
    let _ = tcp.set_read_timeout(Some(READ_TICK));
    if inner
        .step(generation, &mut |c, _| {
            c.opened();
            Ok(Vec::new())
        })
        .is_none()
    {
        return;
    }
    let mut last_frame = Instant::now();
    let mut last_ping = Instant::now();
    let mut last_poll = Instant::now();
    let send = |ws: &mut Socket, frames: Vec<String>| -> bool { frames.into_iter().all(|f| ws.send(Message::text(f)).is_ok()) };
    loop {
        match ws.read() {
            Ok(Message::Text(t)) => {
                last_frame = Instant::now();
                let text = t.to_string();
                let Some(out) = inner.step(generation, &mut |c, r| c.handle(&text, r, now_ms())) else { break };
                if !send(&mut ws, out) {
                    break;
                }
            }
            Ok(Message::Close(frame)) => {
                if let Some(f) = frame {
                    let code = u16::from(f.code);
                    if code == CLOSE_REVOKED || code == CLOSE_AUTH {
                        let mut st = inner.lock();
                        if let Some(c) = st.client.as_mut()
                            && c.state != State::Error
                        {
                            c.state = State::Error;
                            c.problem = Some(if code == CLOSE_REVOKED {
                                "This device was removed from sync.".into()
                            } else {
                                format!("The relay refused this device: {}", f.reason)
                            });
                        }
                    }
                }
                break;
            }
            Ok(_) => last_frame = Instant::now(),
            Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
            Err(_) => break,
        }
        if last_frame.elapsed() > SILENT_FOR {
            break;
        }
        let outbox = {
            let mut st = inner.lock();
            if st.generation != generation {
                break;
            }
            std::mem::take(&mut st.outbox)
        };
        if !send(&mut ws, outbox) {
            break;
        }
        let ready = inner.lock().client.as_ref().is_some_and(Client::ready);
        if ready && last_poll.elapsed() >= POLL_EVERY {
            last_poll = Instant::now();
            let Some(out) = inner.step(generation, &mut |c, r| c.poll(r, now_ms())) else { break };
            if !send(&mut ws, out) {
                break;
            }
        }
        if last_ping.elapsed() > PING_EVERY {
            last_ping = Instant::now();
            if ws.send(Message::text("ping")).is_err() {
                break;
            }
        }
    }
    let _ = ws.close(None);
    let _ = ws.flush();
    let _ = inner.step(generation, &mut |c, _| {
        c.closed();
        Ok(Vec::new())
    });
}
