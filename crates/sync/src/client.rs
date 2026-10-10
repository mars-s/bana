//! The sync client as a state machine with no I/O: [`Client::handle`] takes one frame from the
//! relay, [`Client::poll`] looks for local changes; both answer the frames to send. The network
//! loop ([`crate::net`]) and the in-process fake relay of the tests drive the same code.
//!
//! What it keeps track of:
//! - `cursor`: the last relay update this copy has safely taken in (merged and written to disk).
//!   It is acknowledged to the relay, which drops old updates only once every device is past them.
//! - `sent`: the Loro version the relay has from this copy; a push sends what came after it.
//! - every other device's acknowledged Loro version (sealed, so the relay cannot read it): this
//!   copy compacts its history only up to what all of them have (`Graph::compact_history`).

use std::collections::BTreeMap;

use serde::Serialize;
use tanacraft_core::{Graph, VersionVector};

use crate::config::Config;
use crate::crypto::{DeviceKey, Kind, WorkspaceKey, invite_hash, random};
use crate::proto::{Device, Down, PART, Up, up};
use crate::{Error, Result, b64};

/// The copy of the notes a client syncs: the graph, behind whatever lock guards it.
pub trait Replica {
    fn version(&mut self) -> Result<VersionVector>;
    fn updates_since(&mut self, since: &VersionVector) -> Result<Vec<u8>>;
    fn snapshot(&mut self) -> Result<Vec<u8>>;
    /// Merges `parts` in, makes them durable and tells the views (the caret stays put).
    fn import(&mut self, parts: &[Vec<u8>]) -> Result<()>;
    /// Replaces the notes with the workspace's (a device joining a synced workspace).
    fn replace(&mut self, parts: &[Vec<u8>]) -> Result<()>;
    /// Drops history every copy in `acked` has.
    fn compact(&mut self, acked: &[VersionVector]) -> Result<bool>;
}

fn core(e: tanacraft_core::Error) -> Error {
    Error::Graph(e.to_string())
}

impl Replica for Graph {
    fn version(&mut self) -> Result<VersionVector> {
        Graph::version(self).map_err(core)
    }
    fn updates_since(&mut self, since: &VersionVector) -> Result<Vec<u8>> {
        Graph::updates_since(self, since).map_err(core)
    }
    fn snapshot(&mut self) -> Result<Vec<u8>> {
        self.to_snapshot().map_err(core)
    }
    fn import(&mut self, parts: &[Vec<u8>]) -> Result<()> {
        self.merge_all(parts).map_err(core)
    }
    fn replace(&mut self, parts: &[Vec<u8>]) -> Result<()> {
        *self = Graph::from_parts(parts).map_err(core)?;
        Ok(())
    }
    fn compact(&mut self, acked: &[VersionVector]) -> Result<bool> {
        self.compact_history(acked).map_err(core)
    }
}

/// Update rows the relay keeps before a device replaces them with a snapshot.
pub const PRUNE_ROWS: u64 = 256;
/// The least time between two snapshots from this device, and between two compactions.
pub const PRUNE_EVERY_MS: u64 = 10 * 60 * 1000;
pub const COMPACT_EVERY_MS: u64 = 60 * 60 * 1000;
/// A changed version is acknowledged at most this often (a new cursor at once).
const ACK_EVERY_MS: u64 = 5_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    /// Not connected (no network, relay down, or not started).
    Offline,
    /// Connected and signing in, or catching up.
    Syncing,
    /// Up to date with the relay.
    Synced,
    /// Stopped: this device was removed, the code was wrong, or the key does not match.
    Error,
}

#[derive(Debug, Clone, PartialEq)]
enum Phase {
    Closed,
    Challenged,
    /// Signed in, reading the backlog.
    CatchingUp,
    Ready,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeviceInfo {
    pub device: String,
    pub name: String,
    pub cursor: u64,
    pub seen: u64,
    pub revoked: bool,
    pub me: bool,
    #[serde(skip)]
    pub vv: Option<VersionVector>,
}

/// Something the service waits for.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Invited(String),
    Fatal(String),
}

/// The relay's snapshot as it arrives: (upto, from, parts).
type Snap = (u64, String, Vec<Option<Vec<u8>>>);

struct Partial {
    first: u64,
    parts: Vec<Option<Vec<u8>>>,
}

struct InFlight {
    id: String,
    vv: VersionVector,
    n: u32,
}

pub struct Client {
    pub cfg: Config,
    key: WorkspaceKey,
    me: DeviceKey,
    my_id: String,
    phase: Phase,
    pub state: State,
    /// What went wrong last (shown with the state); cleared by a clean sync.
    pub problem: Option<String>,
    pub head: u64,
    snapshot_upto: u64,
    /// The relay's snapshot, as it arrives (parts) and once read.
    snap: Option<Snap>,
    partials: BTreeMap<(String, String), Partial>,
    /// Payloads read during the backlog, merged at the end of each page.
    batch: Vec<Vec<u8>>,
    batch_max: u64,
    /// A joining device's whole backlog (it replaces the local notes at the end).
    joined: Vec<Vec<u8>>,
    unreadable: u64,
    in_flight: Option<InFlight>,
    acked_seq: u64,
    acked_vv: Option<VersionVector>,
    /// The relay number of this copy's last push, once stored.
    last_push_seq: u64,
    /// A device that acknowledged past that push but lacks some of it (an update lost or changed
    /// on the way) gets everything it lacks once more; its version then, so it is sent once.
    repaired: BTreeMap<String, VersionVector>,
    last_ack: u64,
    last_prune: u64,
    last_compact: u64,
    snap_out: bool,
    pub devices: BTreeMap<String, DeviceInfo>,
    /// The invite being registered (hash).
    pub events: Vec<Event>,
    /// The config changed (cursor, sent version): the caller saves it.
    pub dirty: bool,
    /// When the copy last finished catching up (ms since 1970).
    pub last_synced: Option<u64>,
}

impl Client {
    pub fn new(cfg: Config) -> Result<Client> {
        let key = cfg.workspace_key()?;
        let me = cfg.device_key()?;
        let my_id = me.id();
        Ok(Client {
            cfg,
            key,
            me,
            my_id,
            phase: Phase::Closed,
            state: State::Offline,
            problem: None,
            head: 0,
            snapshot_upto: 0,
            snap: None,
            partials: BTreeMap::new(),
            batch: Vec::new(),
            batch_max: 0,
            joined: Vec::new(),
            unreadable: 0,
            in_flight: None,
            acked_seq: 0,
            acked_vv: None,
            last_push_seq: 0,
            repaired: BTreeMap::new(),
            last_ack: 0,
            last_prune: 0,
            last_compact: 0,
            snap_out: false,
            devices: BTreeMap::new(),
            events: Vec::new(),
            dirty: false,
            last_synced: None,
        })
    }

    pub fn device_id(&self) -> &str {
        &self.my_id
    }

    pub fn ready(&self) -> bool {
        self.phase == Phase::Ready
    }

    /// The connection opened.
    pub fn opened(&mut self) {
        self.reset();
        if self.state != State::Error {
            self.state = State::Syncing;
        }
    }

    /// The connection closed (or failed).
    pub fn closed(&mut self) {
        self.reset();
        if self.state != State::Error {
            self.state = State::Offline;
        }
    }

    fn reset(&mut self) {
        self.phase = Phase::Closed;
        self.snap = None;
        self.partials.clear();
        self.batch.clear();
        self.joined.clear();
        self.in_flight = None;
        self.snap_out = false;
        self.acked_seq = 0;
        self.acked_vv = None;
    }

    fn fatal(&mut self, msg: &str) {
        self.state = State::Error;
        self.problem = Some(msg.to_string());
        self.events.push(Event::Fatal(msg.to_string()));
    }

    fn seal(&self, kind: Kind, plain: &[u8]) -> Result<String> {
        Ok(b64::encode(&self.key.seal(&self.cfg.room, kind, &self.my_id, plain)?))
    }

    fn open(&self, kind: Kind, from: &str, data: &[u8]) -> Result<Vec<u8>> {
        self.key.open(&self.cfg.room, kind, from, data)
    }

    fn device_info(&self, d: &Device) -> DeviceInfo {
        let name = d
            .name
            .as_deref()
            .and_then(b64::decode)
            .and_then(|s| self.open(Kind::Name, &d.device, &s).ok())
            .and_then(|b| String::from_utf8(b).ok())
            .unwrap_or_else(|| "Unnamed device".into());
        let vv =
            d.vv.as_deref().and_then(b64::decode).and_then(|s| self.open(Kind::Ack, &d.device, &s).ok()).and_then(|b| VersionVector::decode(&b).ok());
        DeviceInfo { device: d.device.clone(), name, cursor: d.cursor, seen: d.seen, revoked: d.revoked, me: d.device == self.my_id, vv }
    }

    /// The highest cursor this copy may claim: the last update read, but never past a message
    /// still missing parts.
    fn advance(&mut self, seq: u64) {
        let limit = self.partials.values().map(|p| p.first.saturating_sub(1)).min().unwrap_or(u64::MAX);
        let to = seq.min(limit);
        if to > self.cfg.cursor {
            self.cfg.cursor = to;
            self.dirty = true;
        }
    }

    /// One frame from the relay. Answers the frames to send back.
    pub fn handle(&mut self, text: &str, r: &mut dyn Replica, now: u64) -> Result<Vec<String>> {
        let Ok(msg) = serde_json::from_str::<Down>(text) else { return Ok(Vec::new()) };
        let mut out = Vec::new();
        match msg {
            Down::Challenge { nonce } => {
                let name = self.seal(Kind::Name, self.cfg.name.as_bytes())?;
                let invite = if self.cfg.joining { self.cfg.invite.clone() } else { None };
                out.push(up(&Up::Hello {
                    device: self.my_id.clone(),
                    sig: self.me.sign_hello(&self.cfg.room, &nonce),
                    cursor: self.cfg.cursor,
                    invite,
                    name: Some(name),
                }));
                self.phase = Phase::Challenged;
            }
            Down::Welcome { head, devices, snapshot } => {
                self.head = head;
                self.snapshot_upto = snapshot.map_or(0, |s| s.upto);
                self.devices = devices.iter().map(|d| (d.device.clone(), self.device_info(d))).collect();
                self.phase = Phase::CatchingUp;
                // Signed in: the invite is spent.
                if self.cfg.invite.take().is_some() {
                    self.dirty = true;
                }
            }
            Down::Snapshot { upto, from, i, n, data } => {
                let s = self.snap.get_or_insert_with(|| (upto, from.clone(), vec![None; n.clamp(1, 4096) as usize]));
                if let (Some(slot), Some(bytes)) = (s.2.get_mut(i as usize), b64::decode(&data)) {
                    *slot = Some(bytes);
                }
                if s.2.iter().all(Option::is_some) {
                    let (upto, from, parts) = self.snap.take().unwrap_or_default();
                    let sealed: Vec<u8> = parts.into_iter().flatten().flatten().collect();
                    match self.open(Kind::Snapshot, &from, &sealed) {
                        Ok(plain) => self.batch.push(plain),
                        Err(_) => self.unreadable += 1,
                    }
                    self.batch_max = self.batch_max.max(upto);
                }
            }
            Down::Update { seq, from, id, i, n, data } => {
                self.head = self.head.max(seq);
                let Some(bytes) = b64::decode(&data) else { return Ok(out) };
                let whole = if n <= 1 {
                    Some(bytes)
                } else {
                    let p = self
                        .partials
                        .entry((from.clone(), id.clone()))
                        .or_insert_with(|| Partial { first: seq, parts: vec![None; n.min(4096) as usize] });
                    if let Some(slot) = p.parts.get_mut(i as usize) {
                        *slot = Some(bytes);
                    }
                    if p.parts.iter().all(Option::is_some) {
                        self.partials.remove(&(from.clone(), id)).map(|p| p.parts.into_iter().flatten().flatten().collect())
                    } else {
                        None
                    }
                };
                let plain = match whole.map(|w| self.open(Kind::Update, &from, &w)) {
                    Some(Ok(p)) => Some(p),
                    Some(Err(_)) => {
                        self.unreadable += 1;
                        None
                    }
                    None => None,
                };
                if self.phase == Phase::Ready {
                    if let Some(p) = plain {
                        self.import(r, vec![p])?;
                    }
                    self.advance(seq);
                    self.note_unreadable();
                } else {
                    self.batch.extend(plain);
                    self.batch_max = self.batch_max.max(seq);
                }
            }
            Down::Synced { head, upto } => {
                self.head = self.head.max(head);
                let batch = std::mem::take(&mut self.batch);
                if self.cfg.joining {
                    self.joined.extend(batch);
                } else if !batch.is_empty() {
                    self.import(r, batch)?;
                }
                let to = upto.max(self.batch_max);
                self.batch_max = 0;
                if upto < head {
                    self.advance_if_not_joining(to);
                    out.push(up(&Up::Pull { after: upto }));
                } else {
                    if self.cfg.joining && !self.finish_join(r, to)? {
                        return Ok(out);
                    }
                    self.advance_if_not_joining(to);
                    self.phase = Phase::Ready;
                    self.state = State::Synced;
                    self.last_synced = Some(now);
                    self.note_unreadable();
                    out.extend(self.poll(r, now)?);
                }
            }
            Down::Ok { id, i, seq } => {
                self.advance(seq);
                if let Some(f) = &self.in_flight
                    && f.id == id
                    && i + 1 == f.n
                    && let Some(f) = self.in_flight.take()
                {
                    self.last_push_seq = seq;
                    let mut sent = self.cfg.sent_version();
                    sent.merge(&f.vv);
                    self.cfg.set_sent_version(&sent);
                    self.dirty = true;
                    if self.phase == Phase::Ready {
                        self.state = State::Synced;
                        self.last_synced = Some(now);
                    }
                }
            }
            Down::Acked { device, seq, vv } => {
                let d = Device { device: device.clone(), name: None, cursor: seq, vv, seen: now, revoked: false };
                let info = self.device_info(&d);
                if let Some(e) = self.devices.get_mut(&device) {
                    e.cursor = seq;
                    if info.vv.is_some() {
                        e.vv = info.vv;
                    }
                }
            }
            Down::Devices { devices } => {
                self.devices = devices.iter().map(|d| (d.device.clone(), self.device_info(d))).collect();
            }
            Down::Pruned { upto } => {
                self.snapshot_upto = self.snapshot_upto.max(upto);
                self.snap_out = false;
            }
            Down::Invited { hash } => self.events.push(Event::Invited(hash)),
            Down::Error { code, msg } => match code.as_str() {
                "revoked" => self.fatal("This device was removed from sync. Set it up again to sync."),
                "not-member" | "bad-invite" => self.fatal(&format!("The relay did not let this device in: {msg}")),
                "snapshot" => self.snap_out = false,
                _ => self.problem = Some(msg),
            },
        }
        Ok(out)
    }

    fn advance_if_not_joining(&mut self, to: u64) {
        if !self.cfg.joining {
            self.advance(to);
        }
    }

    /// The end of a joining device's first sync: its notes become the workspace's.
    fn finish_join(&mut self, r: &mut dyn Replica, to: u64) -> Result<bool> {
        let parts = std::mem::take(&mut self.joined);
        if parts.is_empty() {
            let why = if self.unreadable > 0 {
                "the workspace key does not match (the code may be damaged)"
            } else {
                "the workspace has nothing on the relay yet"
            };
            self.fatal(&format!("Could not join: {why}"));
            return Ok(false);
        }
        r.replace(&parts)?;
        self.cfg.joining = false;
        self.cfg.set_sent_version(&r.version()?);
        self.dirty = true;
        self.advance(to);
        Ok(true)
    }

    fn note_unreadable(&mut self) {
        if self.unreadable > 0 {
            self.problem = Some(format!("{} update(s) could not be decrypted and were skipped (wrong key, or changed on the way)", self.unreadable));
        }
    }

    /// Merges payloads, and counts what came in as already on the relay (never sent back).
    fn import(&mut self, r: &mut dyn Replica, parts: Vec<Vec<u8>>) -> Result<()> {
        let before = r.version()?;
        if let Err(e) = r.import(&parts) {
            self.problem = Some(format!("an update could not be merged: {e}"));
            return Ok(());
        }
        let after = r.version()?;
        let mut sent = self.cfg.sent_version();
        let mut changed = false;
        for (peer, end) in after.iter() {
            let had = before.get(peer).copied().unwrap_or(0);
            // Only peers this copy had pushed everything of: anything else (its own unsent edits)
            // still needs sending.
            if *end > had && sent.get(peer).copied().unwrap_or(0) >= had {
                sent.insert(*peer, *end);
                changed = true;
            }
        }
        if changed {
            self.cfg.set_sent_version(&sent);
            self.dirty = true;
        }
        Ok(())
    }

    /// Local work: push new edits, acknowledge, compact, and replace old updates with a snapshot.
    pub fn poll(&mut self, r: &mut dyn Replica, now: u64) -> Result<Vec<String>> {
        let mut out = Vec::new();
        if self.phase != Phase::Ready {
            return Ok(out);
        }
        let vv = r.version()?;
        if self.in_flight.is_none() {
            let sent = self.cfg.sent_version();
            let lacking = self.lacking(&sent);
            if !sent.includes_vv(&vv) || lacking.is_some() {
                let base = lacking.unwrap_or(sent);
                let bytes = r.updates_since(&base)?;
                let sealed = self.key.seal(&self.cfg.room, Kind::Update, &self.my_id, &bytes)?;
                let id = b64::encode(&random::<9>()?);
                let parts: Vec<&[u8]> = sealed.chunks(PART).collect();
                let n = parts.len() as u32;
                for (i, p) in parts.into_iter().enumerate() {
                    out.push(up(&Up::Push { id: id.clone(), i: i as u32, n, data: b64::encode(p) }));
                }
                self.in_flight = Some(InFlight { id, vv: vv.clone(), n });
                self.state = State::Syncing;
            }
        }
        let cursor = self.cfg.cursor;
        let vv_new = self.acked_vv.as_ref() != Some(&vv);
        if cursor > self.acked_seq || (vv_new && now.saturating_sub(self.last_ack) >= ACK_EVERY_MS) {
            let sealed = self.seal(Kind::Ack, &vv.encode())?;
            out.push(up(&Up::Ack { seq: cursor, vv: Some(sealed) }));
            self.acked_seq = cursor;
            self.acked_vv = Some(vv.clone());
            self.last_ack = now;
            if let Some(me) = self.devices.get_mut(&self.my_id) {
                me.cursor = cursor;
            }
        }
        if now.saturating_sub(self.last_compact) >= COMPACT_EVERY_MS {
            self.last_compact = now;
            self.compact(r, &vv)?;
        }
        out.extend(self.prune(r, now)?);
        Ok(out)
    }

    /// The version of another device that acknowledged this copy's last push but does not have
    /// all of it (what it lacks is then pushed again, once per version it reports).
    fn lacking(&mut self, sent: &VersionVector) -> Option<VersionVector> {
        let d = self.devices.values().find(|d| {
            !d.me
                && !d.revoked
                && d.cursor >= self.last_push_seq
                && self.last_push_seq > 0
                && d.vv.as_ref().is_some_and(|v| !v.includes_vv(sent) && self.repaired.get(&d.device) != Some(v))
        })?;
        let (id, vv) = (d.device.clone(), d.vv.clone()?);
        self.repaired.insert(id, vv.clone());
        Some(vv)
    }

    /// Compacts history up to what every other paired device has acknowledged, and only when this
    /// copy has everything they acknowledged (then anything still to come was made after it).
    fn compact(&mut self, r: &mut dyn Replica, vv: &VersionVector) -> Result<bool> {
        let others: Vec<&DeviceInfo> = self.devices.values().filter(|d| !d.me && !d.revoked).collect();
        if others.is_empty() {
            return Ok(false);
        }
        let mut acked = Vec::new();
        for d in others {
            match &d.vv {
                Some(v) if vv.includes_vv(v) => acked.push(v.clone()),
                _ => return Ok(false),
            }
        }
        acked.push(self.cfg.sent_version());
        r.compact(&acked)
    }

    /// Once the relay holds many updates every device has, this device sends a snapshot of
    /// everything so the relay can drop them.
    fn prune(&mut self, r: &mut dyn Replica, now: u64) -> Result<Vec<String>> {
        let mut out = Vec::new();
        if self.snap_out || self.in_flight.is_some() || now.saturating_sub(self.last_prune) < PRUNE_EVERY_MS {
            return Ok(out);
        }
        let upto = self.devices.values().filter(|d| !d.revoked).map(|d| if d.me { self.cfg.cursor } else { d.cursor }).min().unwrap_or(0);
        if upto <= self.snapshot_upto || upto - self.snapshot_upto < PRUNE_ROWS || upto > self.cfg.cursor {
            return Ok(out);
        }
        self.last_prune = now;
        let sealed = self.key.seal(&self.cfg.room, Kind::Snapshot, &self.my_id, &r.snapshot()?)?;
        let id = b64::encode(&random::<9>()?);
        let parts: Vec<&[u8]> = sealed.chunks(PART).collect();
        let n = parts.len() as u32;
        for (i, p) in parts.into_iter().enumerate() {
            out.push(up(&Up::Snap { upto, id: id.clone(), i: i as u32, n, data: b64::encode(p) }));
        }
        self.snap_out = true;
        Ok(out)
    }

    /// A new invite: the code to show, and the frame registering it with the relay.
    pub fn invite(&mut self) -> Result<(crate::code::Code, String, String)> {
        let secret = random::<32>()?;
        let hash = invite_hash(&secret);
        let code = crate::code::Code {
            v: 1,
            relay: self.cfg.relay.clone(),
            key: self.key.to_b64(),
            invite: b64::encode(&secret),
            by: self.my_id.clone(),
            by_name: self.cfg.name.clone(),
        };
        Ok((code, hash.clone(), up(&Up::Invite { hash })))
    }

    pub fn revoke(&self, device: &str) -> String {
        up(&Up::Revoke { device: device.to_string() })
    }

    /// Whether this copy has edits the relay does not have yet.
    pub fn pending(&self, r: &mut dyn Replica) -> bool {
        self.in_flight.is_some() || r.version().is_ok_and(|vv| !self.cfg.sent_version().includes_vv(&vv))
    }
}
