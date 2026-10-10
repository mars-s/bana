//! An in-process fake of the relay (the Durable Object in `cloudflare/sync/room.ts`): the same
//! protocol and rules (sign-in by signed challenge, invites, backlog pages, live broadcast only to
//! devices that caught up, acknowledgements, snapshot pruning, revoking), with no network, so the
//! tests can drive several engines step by step, offline for weeks, against a tampering relay.

use std::collections::{BTreeMap, VecDeque};

use bana_core::base64 as b64;
use bana_sync::crypto::{invite_hash, verify_hello};
use bana_sync::proto::{CLOSE_AUTH, CLOSE_REVOKED, Device, Down, SnapInfo, Up};

/// Rows per backlog page (small, so tests page).
pub const PAGE: usize = 16;

/// A snapshot being uploaded: (upto, parts).
type PendingSnap = (u64, Vec<Option<String>>);

pub struct Row {
    pub seq: u64,
    pub from: String,
    pub id: String,
    pub i: u32,
    pub n: u32,
    pub data: String,
}

struct Conn {
    device: Option<String>,
    nonce: String,
    live: bool,
    out: VecDeque<String>,
    closed: Option<u16>,
}

pub struct Relay {
    pub room: String,
    pub devices: Vec<Device>,
    pub rows: Vec<Row>,
    pub head: u64,
    /// (upto, from, parts)
    pub snapshot: Option<(u64, String, Vec<String>)>,
    pending: BTreeMap<(String, String), PendingSnap>,
    invites: BTreeMap<String, u64>,
    conns: BTreeMap<u32, Conn>,
    next: u32,
    pub now: u64,
    /// Flip a bit in the next stored update (a relay that changes data).
    pub tamper: bool,
}

fn send(c: &mut Conn, m: &Down) {
    c.out.push_back(serde_json::to_string(m).unwrap());
}

impl Relay {
    pub fn new(room: &str) -> Relay {
        Relay {
            room: room.into(),
            devices: Vec::new(),
            rows: Vec::new(),
            head: 0,
            snapshot: None,
            pending: BTreeMap::new(),
            invites: BTreeMap::new(),
            conns: BTreeMap::new(),
            next: 1,
            now: 1_000,
            tamper: false,
        }
    }

    pub fn connect(&mut self) -> u32 {
        let id = self.next;
        self.next += 1;
        let nonce = b64::encode(&id.to_le_bytes().repeat(8));
        let mut c = Conn { device: None, nonce: nonce.clone(), live: false, out: VecDeque::new(), closed: None };
        send(&mut c, &Down::Challenge { nonce });
        self.conns.insert(id, c);
        id
    }

    pub fn disconnect(&mut self, id: u32) {
        self.conns.remove(&id);
    }

    /// Frames for connection `id`, and its close code once the relay closed it.
    pub fn take(&mut self, id: u32) -> (Vec<String>, Option<u16>) {
        match self.conns.get_mut(&id) {
            Some(c) => {
                let out = c.out.drain(..).collect();
                let closed = c.closed;
                if closed.is_some() {
                    self.conns.remove(&id);
                }
                (out, closed)
            }
            None => (Vec::new(), Some(1006)),
        }
    }

    fn close(&mut self, id: u32, code: u16, err: &str, msg: &str) {
        if let Some(c) = self.conns.get_mut(&id) {
            send(c, &Down::Error { code: err.into(), msg: msg.into() });
            c.closed = Some(code);
        }
    }

    fn broadcast(&mut self, except: Option<u32>, live_only: bool, m: &Down) {
        for (id, c) in self.conns.iter_mut() {
            if Some(*id) != except && c.device.is_some() && c.closed.is_none() && (c.live || !live_only) {
                send(c, m);
            }
        }
    }

    fn page(&mut self, id: u32, after: u64) {
        let mut after = after;
        let mut frames = Vec::new();
        if let Some((upto, from, parts)) = &self.snapshot
            && after < *upto
        {
            for (i, p) in parts.iter().enumerate() {
                frames.push(Down::Snapshot { upto: *upto, from: from.clone(), i: i as u32, n: parts.len() as u32, data: p.clone() });
            }
            after = *upto;
        }
        let mut upto = after;
        for r in self.rows.iter().filter(|r| r.seq > after).take(PAGE) {
            frames.push(Down::Update { seq: r.seq, from: r.from.clone(), id: r.id.clone(), i: r.i, n: r.n, data: r.data.clone() });
            upto = r.seq;
        }
        frames.push(Down::Synced { head: self.head, upto });
        let head = self.head;
        if let Some(c) = self.conns.get_mut(&id) {
            for f in &frames {
                send(c, f);
            }
            c.live = upto >= head;
        }
    }

    fn device_list(&self) -> Vec<Device> {
        self.devices.clone()
    }

    pub fn recv(&mut self, id: u32, text: &str) {
        let Some(conn) = self.conns.get(&id) else { return };
        if conn.closed.is_some() {
            return;
        }
        let Ok(msg) = serde_json::from_str::<Up>(text) else {
            return self.close(id, 1003, "bad-message", "not a message");
        };
        let me = conn.device.clone();
        match (msg, me) {
            (Up::Hello { device, sig, cursor, invite, name }, None) => {
                let nonce = conn.nonce.clone();
                if !verify_hello(&device, &self.room, &nonce, &sig) {
                    return self.close(id, CLOSE_AUTH, "bad-signature", "the signature does not match");
                }
                let known = self.devices.iter().position(|d| d.device == device);
                let mut joined = false;
                match known {
                    Some(i) if self.devices[i].revoked => return self.close(id, CLOSE_REVOKED, "revoked", "this device was removed"),
                    Some(_) => {}
                    None => {
                        let valid = invite
                            .as_deref()
                            .and_then(b64::decode)
                            .map(|s| invite_hash(&s))
                            .and_then(|h| self.invites.remove(&h))
                            .is_some_and(|exp| exp > self.now);
                        if invite.is_some() && !valid {
                            return self.close(id, CLOSE_AUTH, "bad-invite", "the invite is not valid (used, expired or unknown)");
                        }
                        if !valid && !self.devices.is_empty() {
                            return self.close(id, CLOSE_AUTH, "not-member", "this device is not in the room");
                        }
                        self.devices.push(Device { device: device.clone(), name: None, cursor: 0, vv: None, seen: 0, revoked: false });
                        joined = true;
                    }
                }
                let now = self.now;
                if let Some(d) = self.devices.iter_mut().find(|d| d.device == device) {
                    d.seen = now;
                    if name.is_some() {
                        d.name = name;
                    }
                }
                if let Some(c) = self.conns.get_mut(&id) {
                    c.device = Some(device);
                }
                let welcome = Down::Welcome {
                    head: self.head,
                    devices: self.device_list(),
                    snapshot: self.snapshot.as_ref().map(|(upto, _, parts)| SnapInfo { upto: *upto, n: parts.len() as u32 }),
                };
                if let Some(c) = self.conns.get_mut(&id) {
                    send(c, &welcome);
                }
                self.page(id, cursor);
                if joined {
                    let d = Down::Devices { devices: self.device_list() };
                    self.broadcast(Some(id), false, &d);
                }
            }
            (_, None) => self.close(id, CLOSE_AUTH, "not-signed-in", "sign in first"),
            (Up::Hello { .. }, Some(_)) => {}
            (Up::Invite { hash }, Some(_)) => {
                self.invites.insert(hash.clone(), self.now + 24 * 3600 * 1000);
                if let Some(c) = self.conns.get_mut(&id) {
                    send(c, &Down::Invited { hash });
                }
            }
            (Up::Push { id: mid, i, n, mut data }, Some(dev)) => {
                if self.tamper {
                    self.tamper = false;
                    let mut b = b64::decode(&data).unwrap();
                    let last = b.len() - 1;
                    b[last] ^= 0x40;
                    data = b64::encode(&b);
                }
                self.head += 1;
                let seq = self.head;
                self.rows.push(Row { seq, from: dev.clone(), id: mid.clone(), i, n, data: data.clone() });
                if let Some(c) = self.conns.get_mut(&id) {
                    send(c, &Down::Ok { id: mid.clone(), i, seq });
                }
                self.broadcast(Some(id), true, &Down::Update { seq, from: dev, id: mid, i, n, data });
            }
            (Up::Ack { seq, vv }, Some(dev)) => {
                let head = self.head;
                if let Some(d) = self.devices.iter_mut().find(|d| d.device == dev) {
                    d.cursor = d.cursor.max(seq.min(head));
                    if vv.is_some() {
                        d.vv = vv.clone();
                    }
                    let m = Down::Acked { device: dev.clone(), seq: d.cursor, vv };
                    self.broadcast(Some(id), false, &m);
                }
            }
            (Up::Pull { after }, Some(_)) => self.page(id, after),
            (Up::Snap { upto, id: sid, i, n, data }, Some(dev)) => {
                let min = self.devices.iter().filter(|d| !d.revoked).map(|d| d.cursor).min().unwrap_or(0);
                let have = self.snapshot.as_ref().map_or(0, |s| s.0);
                if upto > min || upto <= have || upto > self.head {
                    if let Some(c) = self.conns.get_mut(&id) {
                        send(c, &Down::Error { code: "snapshot".into(), msg: "not every device has that yet".into() });
                    }
                    return;
                }
                let e = self.pending.entry((dev.clone(), sid.clone())).or_insert_with(|| (upto, vec![None; n as usize]));
                if let Some(slot) = e.1.get_mut(i as usize) {
                    *slot = Some(data);
                }
                if e.1.iter().all(Option::is_some) {
                    let (upto, parts) = self.pending.remove(&(dev.clone(), sid)).unwrap();
                    self.snapshot = Some((upto, dev, parts.into_iter().flatten().collect()));
                    self.rows.retain(|r| r.seq > upto);
                    self.broadcast(None, false, &Down::Pruned { upto });
                }
            }
            (Up::Revoke { device }, Some(_)) => {
                if let Some(d) = self.devices.iter_mut().find(|d| d.device == device) {
                    d.revoked = true;
                }
                let theirs: Vec<u32> = self.conns.iter().filter(|(_, c)| c.device.as_deref() == Some(&device)).map(|(k, _)| *k).collect();
                for c in theirs {
                    self.close(c, CLOSE_REVOKED, "revoked", "this device was removed");
                }
                let d = Down::Devices { devices: self.device_list() };
                self.broadcast(None, false, &d);
            }
        }
    }
}
