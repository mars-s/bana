//! Two and three engines syncing through the in-process fake relay (`relay/mod.rs`): offline edits
//! on every side, typing in one line at once, moves, deletes against edits, a phone away for 60
//! days, the relay replacing old updates with a snapshot, revoking a device, a relay that tampers
//! with data, and a device with the wrong key.

mod relay;

use relay::Relay;
use tanacraft_core::{Date, Graph, NodeId, sample};
use tanacraft_sync::client::{Client, State};
use tanacraft_sync::{Code, Config};

const DAY: u64 = 24 * 3600 * 1000;

struct Peer {
    name: &'static str,
    g: Graph,
    c: Client,
    conn: Option<u32>,
    online: bool,
}

impl Peer {
    fn text(&self) -> String {
        let mut v: Vec<_> = self.g.iter().map(|n| (n.id, n.name.clone(), n.parent, n.children.clone(), n.tags.clone())).collect();
        v.sort_by_key(|x| x.0);
        format!("{v:?}")
    }
}

fn today() -> Date {
    Date::new(2026, 10, 10).unwrap()
}

/// The first device: sets sync up on its graph.
fn first(name: &'static str, g: Graph) -> (Peer, Relay) {
    let cfg = Config::create("https://relay.test", name).unwrap();
    let relay = Relay::new(&cfg.room);
    (Peer { name, g, c: Client::new(cfg).unwrap(), conn: None, online: true }, relay)
}

/// Another device joins with an invite from `by` (which must be connected).
fn join(name: &'static str, by: &mut Peer, relay: &mut Relay, local: Graph) -> Peer {
    let (code, _hash, frame) = by.c.invite().unwrap();
    relay.recv(by.conn.expect("inviter online"), &frame);
    let code = Code::decode(&code.encode()).unwrap();
    let cfg = Config::join(&code, name).unwrap();
    Peer { name, g: local, c: Client::new(cfg).unwrap(), conn: None, online: true }
}

/// Moves frames until nothing moves. Offline peers stay disconnected.
fn pump(relay: &mut Relay, peers: &mut [&mut Peer]) {
    let now = relay.now;
    for p in peers.iter_mut() {
        match (p.online, p.conn) {
            (true, None) if p.c.state != State::Error => {
                p.conn = Some(relay.connect());
                p.c.opened();
            }
            (false, Some(id)) => {
                relay.disconnect(id);
                p.conn = None;
                p.c.closed();
            }
            _ => {}
        }
    }
    for _round in 0..10_000 {
        let mut moved = false;
        for p in peers.iter_mut() {
            let Some(id) = p.conn else { continue };
            let (frames, closed) = relay.take(id);
            for f in frames {
                moved = true;
                let out = p.c.handle(&f, &mut p.g, now).unwrap_or_else(|e| panic!("{}: {e}", p.name));
                for o in out {
                    relay.recv(id, &o);
                }
            }
            if closed.is_some() {
                p.conn = None;
                p.c.closed();
                continue;
            }
            let out = p.c.poll(&mut p.g, now).unwrap();
            for o in out {
                moved = true;
                relay.recv(id, &o);
            }
        }
        if !moved {
            return;
        }
    }
    panic!("the peers never settled");
}

fn assert_same(peers: &[&Peer]) {
    let t = peers[0].text();
    for p in &peers[1..] {
        assert_eq!(p.text(), t, "{} and {} differ", peers[0].name, p.name);
    }
}

fn find(g: &Graph, name: &str) -> NodeId {
    g.iter().find(|n| n.name == name).map(|n| n.id).unwrap_or_else(|| panic!("no node {name}"))
}

/// A desktop with the sample graph and a phone joined to it, both synced.
fn pair() -> (Peer, Peer, Relay) {
    let (mut mac, mut relay) = first("Mac", sample::sample(today()).unwrap());
    pump(&mut relay, &mut [&mut mac]);
    assert_eq!(mac.c.state, State::Synced);
    let mut phone = join("iPhone", &mut mac, &mut relay, Graph::new());
    pump(&mut relay, &mut [&mut mac, &mut phone]);
    assert_eq!(phone.c.state, State::Synced, "{:?}", phone.c.problem);
    assert_same(&[&mac, &phone]);
    (mac, phone, relay)
}

#[test]
fn a_joining_phone_gets_the_desktops_notes_instead_of_its_own() {
    let mut phone_notes = Graph::new();
    phone_notes.create(phone_notes.sys.library, None, "only on the phone before pairing").unwrap();
    let (mut mac, mut relay) = first("Mac", sample::sample(today()).unwrap());
    pump(&mut relay, &mut [&mut mac]);
    let mut phone = join("iPhone", &mut mac, &mut relay, phone_notes);
    pump(&mut relay, &mut [&mut mac, &mut phone]);
    assert_same(&[&mac, &phone]);
    assert!(!phone.g.iter().any(|n| n.name == "only on the phone before pairing"));
    // Both list both devices, with their names (sealed on the relay).
    for p in [&mac, &phone] {
        let mut names: Vec<&str> = p.c.devices.values().map(|d| d.name.as_str()).collect();
        names.sort();
        assert_eq!(names, ["Mac", "iPhone"]);
    }
    assert!(!relay.devices.iter().any(|d| d.name.as_deref() == Some("Mac")), "the relay never sees names");
    // The invite was used once: it does not let a second device in.
    let mut b = join("Tablet", &mut mac, &mut relay, Graph::new());
    pump(&mut relay, &mut [&mut mac, &mut b]);
    assert_eq!(b.c.state, State::Synced);
    let reused = phone_code_reused(&mut mac, &mut relay);
    assert_eq!(reused.c.state, State::Error, "a used invite is refused");
}

fn phone_code_reused(mac: &mut Peer, relay: &mut Relay) -> Peer {
    let (code, _, frame) = mac.c.invite().unwrap();
    relay.recv(mac.conn.unwrap(), &frame);
    let code = Code::decode(&code.encode()).unwrap();
    let mut a = Peer { name: "A", g: Graph::new(), c: Client::new(Config::join(&code, "A").unwrap()).unwrap(), conn: None, online: true };
    pump(relay, &mut [mac, &mut a]);
    assert_eq!(a.c.state, State::Synced);
    let mut b = Peer { name: "B", g: Graph::new(), c: Client::new(Config::join(&code, "B").unwrap()).unwrap(), conn: None, online: true };
    pump(relay, &mut [mac, &mut b]);
    b
}

#[test]
fn offline_edits_on_both_sides_merge_when_they_reconnect() {
    let (mut mac, mut phone, mut relay) = pair();
    let lib = mac.g.sys.library;
    mac.online = false;
    phone.online = false;
    pump(&mut relay, &mut [&mut mac, &mut phone]);
    mac.g.create(lib, None, "typed on the Mac, offline").unwrap();
    phone.g.create(lib, None, "typed on the phone, offline").unwrap();
    assert_eq!(mac.c.state, State::Offline);
    mac.online = true;
    pump(&mut relay, &mut [&mut mac, &mut phone]);
    phone.online = true;
    pump(&mut relay, &mut [&mut mac, &mut phone]);
    assert_same(&[&mac, &phone]);
    find(&mac.g, "typed on the phone, offline");
    find(&phone.g, "typed on the Mac, offline");
}

#[test]
fn live_typing_in_the_same_line_keeps_both_sides_words() {
    let (mut mac, mut phone, mut relay) = pair();
    let lib = mac.g.sys.library;
    let n = mac.g.create(lib, None, "garden").unwrap();
    pump(&mut relay, &mut [&mut mac, &mut phone]);
    assert_eq!(phone.g.name(n), "garden");
    // Both type before either sees the other (one relay round each way).
    mac.g.set_name(n, "big garden").unwrap();
    phone.g.set_name(n, "garden shed").unwrap();
    pump(&mut relay, &mut [&mut mac, &mut phone]);
    assert_same(&[&mac, &phone]);
    assert_eq!(mac.g.name(n), "big garden shed");
    // Keystroke by keystroke, interleaved.
    for (a, b) in ["!", "!!", "!!!"].iter().zip(["?", "??", "???"]) {
        mac.g.set_name(n, &format!("big garden shed{a}")).unwrap();
        phone.g.set_name(n, &format!("{b}big garden shed")).unwrap();
        pump(&mut relay, &mut [&mut mac, &mut phone]);
        // Each side's next edit starts from what it now shows.
        let shown = mac.g.name(n).to_string();
        assert_eq!(phone.g.name(n), shown);
        mac.g.set_name(n, &shown.replace('!', "")).unwrap();
        mac.g.set_name(n, &shown).unwrap();
    }
    pump(&mut relay, &mut [&mut mac, &mut phone]);
    assert_same(&[&mac, &phone]);
}

#[test]
fn moves_and_deletes_against_edits_agree_on_three_devices() {
    let (mut mac, mut phone, mut relay) = pair();
    let mut pad = join("iPad", &mut mac, &mut relay, Graph::new());
    pump(&mut relay, &mut [&mut mac, &mut phone, &mut pad]);
    assert_same(&[&mac, &phone, &pad]);
    let lib = mac.g.sys.library;
    let x = mac.g.create(lib, None, "x").unwrap();
    let y = mac.g.create(lib, None, "y").unwrap();
    let draft = mac.g.create(lib, None, "draft").unwrap();
    pump(&mut relay, &mut [&mut mac, &mut phone, &mut pad]);
    for p in [&mut mac, &mut phone, &mut pad] {
        p.online = false;
    }
    pump(&mut relay, &mut [&mut mac, &mut phone, &mut pad]);
    // Offline: x under y here, y under x there; the draft deleted on one, edited on another.
    mac.g.move_to(x, y, None).unwrap();
    phone.g.move_to(y, x, None).unwrap();
    phone.g.delete_permanently(draft).unwrap();
    pad.g.set_name(draft, "draft, edited").unwrap();
    pad.g.create(draft, None, "child of the draft").unwrap();
    for p in [&mut mac, &mut phone, &mut pad] {
        p.online = true;
    }
    pump(&mut relay, &mut [&mut mac, &mut phone, &mut pad]);
    pump(&mut relay, &mut [&mut mac, &mut phone, &mut pad]);
    assert_same(&[&mac, &phone, &pad]);
    let g = &mac.g;
    assert!(g.get(x).is_some() && g.get(y).is_some());
    assert!(!(g.parent(x) == Some(y) && g.parent(y) == Some(x)), "no loop");
    assert!(g.get(draft).is_none() && !g.iter().any(|n| n.name == "child of the draft"), "the delete wins");
}

#[test]
fn a_phone_away_for_sixty_days_catches_up_and_nothing_it_needs_is_cut() {
    let (mut mac, mut phone, mut relay) = pair();
    let lib = mac.g.sys.library;
    phone.online = false;
    pump(&mut relay, &mut [&mut mac, &mut phone]);
    let line = mac.g.create(lib, None, "").unwrap();
    for day in 0..60u64 {
        relay.now += DAY;
        let n = mac.g.create(lib, None, &format!("mac day {day}")).unwrap();
        for k in 0..6 {
            mac.g.set_name(line, &format!("typed on day {day}, {k}")).unwrap();
            pump(&mut relay, &mut [&mut mac]);
        }
        mac.g.set_name(n, &format!("mac day {day} done")).unwrap();
        if day % 7 == 0 {
            phone.g.create(phone.g.sys.library, None, &format!("phone day {day}")).unwrap();
        }
        pump(&mut relay, &mut [&mut mac]);
    }
    // The phone never acknowledged any of it: the relay kept every update (no snapshot replaced
    // what the phone needs), and the Mac compacted nothing the phone lacks.
    assert!(relay.rows.len() > 300, "{} rows", relay.rows.len());
    assert!(relay.snapshot.as_ref().is_none_or(|s| s.0 <= phone.c.cfg.cursor));
    phone.online = true;
    relay.now += DAY;
    pump(&mut relay, &mut [&mut mac, &mut phone]);
    pump(&mut relay, &mut [&mut mac, &mut phone]);
    assert_same(&[&mac, &phone]);
    for day in 0..60 {
        find(&phone.g, &format!("mac day {day} done"));
        if day % 7 == 0 {
            find(&mac.g, &format!("phone day {day}"));
        }
    }
    assert_eq!(phone.g.name(line), "typed on day 59, 5");
}

#[test]
fn old_updates_are_replaced_by_a_snapshot_once_every_device_has_them() {
    let (mut mac, mut phone, mut relay) = pair();
    let lib = mac.g.sys.library;
    let n = mac.g.create(lib, None, "").unwrap();
    for i in 0..400 {
        relay.now += 1_000;
        mac.g.set_name(n, &format!("line {i}")).unwrap();
        pump(&mut relay, &mut [&mut mac, &mut phone]);
    }
    let before = relay.rows.len();
    relay.now += 11 * 60 * 1000;
    pump(&mut relay, &mut [&mut mac, &mut phone]);
    let snap = relay.snapshot.as_ref().expect("a snapshot").0;
    assert!(snap >= 300, "snapshot up to {snap}");
    assert!(relay.rows.len() < before / 2, "{} rows left of {before}", relay.rows.len());
    assert!(relay.rows.iter().all(|r| r.seq > snap));
    // Hours later both compact their history (each has what the other acknowledged).
    relay.now += 2 * 3600 * 1000;
    let ops = mac.g.history_len();
    mac.g.set_name(n, "after the snapshot").unwrap();
    pump(&mut relay, &mut [&mut mac, &mut phone]);
    relay.now += 2 * 3600 * 1000;
    pump(&mut relay, &mut [&mut mac, &mut phone]);
    assert!(mac.g.history_len() < ops, "compacted: {} of {ops}", mac.g.history_len());
    // A device that joins now starts from the snapshot, then the updates after it.
    let mut pad = join("iPad", &mut mac, &mut relay, Graph::new());
    pump(&mut relay, &mut [&mut mac, &mut phone, &mut pad]);
    assert_eq!(pad.c.state, State::Synced, "{:?}", pad.c.problem);
    assert_same(&[&mac, &phone, &pad]);
    // And everyone keeps syncing afterwards.
    pad.g.create(lib, None, "from the iPad").unwrap();
    phone.g.set_name(n, "phone was here").unwrap();
    pump(&mut relay, &mut [&mut mac, &mut phone, &mut pad]);
    assert_same(&[&mac, &phone, &pad]);
    find(&mac.g, "from the iPad");
}

#[test]
fn a_revoked_device_is_cut_off() {
    let (mut mac, mut phone, mut relay) = pair();
    let lib = mac.g.sys.library;
    let id = phone.c.device_id().to_string();
    let frame = mac.c.revoke(&id);
    relay.recv(mac.conn.unwrap(), &frame);
    pump(&mut relay, &mut [&mut mac, &mut phone]);
    assert_eq!(phone.c.state, State::Error);
    assert!(phone.c.problem.as_deref().unwrap_or_default().contains("removed"));
    assert!(mac.c.devices[&id].revoked);
    // It cannot come back (it does not even try), and a reconnect by hand is refused.
    phone.g.create(lib, None, "after the revoke").unwrap();
    phone.c.state = State::Offline;
    pump(&mut relay, &mut [&mut mac, &mut phone]);
    assert_eq!(phone.c.state, State::Error);
    assert!(!mac.g.iter().any(|n| n.name == "after the revoke"));
    // The others still sync, and pruning no longer waits for it.
    assert!(relay.devices.iter().filter(|d| !d.revoked).all(|d| d.device != id));
}

#[test]
fn tampered_updates_are_rejected_and_the_rest_still_syncs() {
    let (mut mac, mut phone, mut relay) = pair();
    let lib = mac.g.sys.library;
    phone.online = false;
    pump(&mut relay, &mut [&mut mac, &mut phone]);
    relay.tamper = true;
    mac.g.create(lib, None, "changed on the way").unwrap();
    pump(&mut relay, &mut [&mut mac]);
    mac.online = false;
    phone.online = true;
    pump(&mut relay, &mut [&mut mac, &mut phone]);
    assert!(!phone.g.iter().any(|n| n.name == "changed on the way"), "a changed update is never merged");
    assert!(phone.c.problem.as_deref().unwrap_or_default().contains("could not be decrypted"));
    assert_eq!(phone.c.state, State::Synced, "the rest goes on");
    // The Mac sees the phone acknowledged its push without having it, and sends it again.
    mac.online = true;
    mac.g.create(lib, None, "clean").unwrap();
    pump(&mut relay, &mut [&mut mac, &mut phone]);
    find(&phone.g, "clean");
    find(&phone.g, "changed on the way");
    assert_same(&[&mac, &phone]);
}

#[test]
fn a_device_with_the_wrong_key_reads_nothing_and_writes_nothing() {
    let (mut mac, mut phone, mut relay) = pair();
    let lib = mac.g.sys.library;
    let (code, _, frame) = mac.c.invite().unwrap();
    relay.recv(mac.conn.unwrap(), &frame);
    // The right room and invite, another key (a damaged or forged code).
    let mut cfg = Config::join(&code, "Intruder").unwrap();
    cfg.key = tanacraft_sync::crypto::WorkspaceKey::generate().unwrap().to_b64();
    let mut bad = Peer { name: "Intruder", g: Graph::new(), c: Client::new(cfg).unwrap(), conn: None, online: true };
    pump(&mut relay, &mut [&mut mac, &mut phone, &mut bad]);
    assert_eq!(bad.c.state, State::Error);
    assert!(bad.c.problem.as_deref().unwrap_or_default().contains("key does not match"), "{:?}", bad.c.problem);
    assert_eq!(bad.g.len(), Graph::new().len(), "its notes were not replaced");
    // Even if it pushes, nobody merges it.
    let mut cfg2 = bad.c.cfg.clone();
    cfg2.joining = false;
    let mut bad2 = Peer { name: "Intruder", g: Graph::new(), c: Client::new(cfg2).unwrap(), conn: None, online: true };
    bad2.g.create(bad2.g.sys.library, None, "forged").unwrap();
    pump(&mut relay, &mut [&mut mac, &mut phone, &mut bad2]);
    assert!(!mac.g.iter().any(|n| n.name == "forged"));
    assert!(mac.c.problem.is_some());
    mac.g.create(lib, None, "still fine").unwrap();
    pump(&mut relay, &mut [&mut mac, &mut phone]);
    find(&phone.g, "still fine");
}

#[test]
fn many_random_offline_rounds_on_three_devices_converge() {
    let (mut mac, mut phone, mut relay) = pair();
    let mut pad = join("iPad", &mut mac, &mut relay, Graph::new());
    pump(&mut relay, &mut [&mut mac, &mut phone, &mut pad]);
    let mut seed = 11u64;
    let mut rnd = |n: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % n.max(1) as u64) as usize
    };
    for round in 0..8 {
        relay.now += DAY / 2;
        let online: Vec<bool> = (0..3).map(|_| rnd(3) != 0).collect();
        for (p, on) in [&mut mac, &mut phone, &mut pad].into_iter().zip(&online) {
            p.online = *on;
            for _ in 0..8 {
                let ids: Vec<NodeId> = p.g.iter().map(|n| n.id).filter(|id| !p.g.is_system(*id)).collect();
                let (pick, other) = (ids[rnd(ids.len())], ids[rnd(ids.len())]);
                let _ = match rnd(5) {
                    0 => p.g.set_name(pick, &format!("{} r{round}", p.g.name(pick))),
                    1 => p.g.create(pick, None, "new").map(drop),
                    2 => p.g.move_to(pick, other, None),
                    3 => p.g.trash(pick),
                    _ => p.g.set_description(pick, "note"),
                };
            }
        }
        pump(&mut relay, &mut [&mut mac, &mut phone, &mut pad]);
    }
    for p in [&mut mac, &mut phone, &mut pad] {
        p.online = true;
    }
    pump(&mut relay, &mut [&mut mac, &mut phone, &mut pad]);
    pump(&mut relay, &mut [&mut mac, &mut phone, &mut pad]);
    assert_same(&[&mac, &phone, &pad]);
}

#[test]
fn edits_made_offline_before_a_compaction_still_merge_and_a_new_device_joins_from_a_compacted_snapshot() {
    let (mut mac, mut phone, mut relay) = pair();
    let lib = mac.g.sys.library;
    let n = mac.g.create(lib, None, "shared line").unwrap();
    for i in 0..300 {
        relay.now += 1_000;
        mac.g.set_name(n, &format!("shared line {i}")).unwrap();
        pump(&mut relay, &mut [&mut mac, &mut phone]);
    }
    // The phone goes offline and edits the same line and others.
    phone.online = false;
    pump(&mut relay, &mut [&mut mac, &mut phone]);
    phone.g.set_name(n, &format!("phone: {}", phone.g.name(n))).unwrap();
    let pk = phone.g.create(lib, None, "made offline on the phone").unwrap();
    // Meanwhile the Mac compacts (up to what the phone acknowledged) and keeps typing.
    for i in 0..300 {
        relay.now += 60_000;
        mac.g.set_name(n, &format!("{} mac {i}", phone_free(&mac.g, n))).unwrap();
        pump(&mut relay, &mut [&mut mac]);
    }
    relay.now += 2 * 3600 * 1000;
    pump(&mut relay, &mut [&mut mac]);
    // The phone comes back: its offline edits merge into the compacted copy.
    phone.online = true;
    pump(&mut relay, &mut [&mut mac, &mut phone]);
    pump(&mut relay, &mut [&mut mac, &mut phone]);
    assert_same(&[&mac, &phone]);
    assert!(mac.g.get(pk).is_some());
    assert!(mac.g.name(n).contains("phone: "), "{}", mac.g.name(n));
    // Later the relay's snapshot is taken from a compacted copy; a new device starts from it.
    for i in 0..300 {
        relay.now += 60_000;
        phone.g.set_name(n, &format!("round two {i}")).unwrap();
        pump(&mut relay, &mut [&mut mac, &mut phone]);
    }
    relay.now += 2 * 3600 * 1000;
    pump(&mut relay, &mut [&mut mac, &mut phone]);
    relay.now += 2 * 3600 * 1000;
    pump(&mut relay, &mut [&mut mac, &mut phone]);
    let mut pad = join("iPad", &mut mac, &mut relay, Graph::new());
    pump(&mut relay, &mut [&mut mac, &mut phone, &mut pad]);
    assert_eq!(pad.c.state, State::Synced, "{:?}", pad.c.problem);
    assert_same(&[&mac, &phone, &pad]);
    pad.g.set_name(n, "from the pad").unwrap();
    mac.g.create(lib, None, "mac after the pad").unwrap();
    pump(&mut relay, &mut [&mut mac, &mut phone, &mut pad]);
    assert_same(&[&mac, &phone, &pad]);
    assert!(
        mac.c.problem.is_none() && phone.c.problem.is_none() && pad.c.problem.is_none(),
        "{:?} {:?} {:?}",
        mac.c.problem,
        phone.c.problem,
        pad.c.problem
    );
}

fn phone_free(g: &Graph, n: NodeId) -> String {
    g.name(n).split(" mac ").next().unwrap_or_default().to_string()
}
