use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;

use super::*;

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bana-ffi-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    dir.join("graph.bana")
}

fn j(s: &str) -> Value {
    serde_json::from_str(s).unwrap_or(Value::Null)
}

fn page(e: &Engine) -> Value {
    j(&e.rpc("page".into(), String::new()))
}

#[test]
fn creates_edits_saves_and_reopens() {
    let path = tmp("roundtrip");
    let e = Engine::open(path.display().to_string(), false, Some("2026-10-10".into())).unwrap();
    let p = page(&e);
    assert_eq!(p["kind"], "day");
    assert!(p["title"].as_str().unwrap().starts_with("Today"));
    // The day page starts with one empty line to type in, or none: add one either way.
    let added = j(&e.run("node.add".into(), format!(r#"{{"parent": {}, "name": "Milk"}}"#, p["target"])));
    let id = added["result"]["id"].as_u64().unwrap();
    let r = j(&e.run("node.setName".into(), format!(r#"{{"node": {id}, "name": "Milk and eggs", "caret": 13}}"#)));
    assert!(r.get("error").is_none(), "{r}");
    e.save().unwrap();
    drop(e);
    let again = Engine::open(path.display().to_string(), false, Some("2026-10-10".into())).unwrap();
    let rows = page(&again)["rows"]["items"].clone();
    assert!(rows.as_array().unwrap().iter().any(|r| r["name"] == "Milk and eggs"), "{rows}");
}

#[test]
fn file_is_the_desktop_format() {
    let path = tmp("format");
    let e = Engine::open(path.display().to_string(), true, Some("2026-10-10".into())).unwrap();
    e.save().unwrap();
    let g = bana_core::Graph::load(&path).unwrap();
    assert!(g.iter().count() > 10);
    assert!(bana_core::store::Store::is_ours(&std::fs::read(&path).unwrap()));
}

#[test]
fn snapshots_have_the_web_shapes() {
    let e = Engine::open(tmp("shapes").display().to_string(), true, Some("2026-10-10".into())).unwrap();
    let p = page(&e);
    for key in ["zoom", "target", "title", "crumbs", "rows", "focus", "caret", "tags", "canBack", "version"] {
        assert!(p.get(key).is_some(), "page lacks {key}");
    }
    let s = j(&e.rpc("sidebar".into(), "{}".into()));
    assert!(s["workspaces"].is_array() && s["schema"].is_u64());
    let f = j(&e.rpc("find".into(), r#"{"query": "a"}"#.into()));
    assert!(f.is_array());
}

#[test]
fn listeners_hear_changes_only() {
    struct L(AtomicU64);
    impl ChangeListener for L {
        fn changed(&self, version: u64) {
            self.0.store(version, Ordering::SeqCst);
        }
    }
    let e = Engine::open(tmp("listen").display().to_string(), true, Some("2026-10-10".into())).unwrap();
    let l = Arc::new(L(AtomicU64::new(0)));
    e.subscribe(l.clone());
    let _ = e.rpc("page".into(), "{}".into());
    assert_eq!(l.0.load(Ordering::SeqCst), 0, "a read is not a change");
    let _ = e.run("nav.previousDay".into(), "{}".into());
    assert_eq!(l.0.load(Ordering::SeqCst), e.version());
}

#[test]
fn hostile_input_answers_errors() {
    let e = Engine::open(tmp("hostile").display().to_string(), false, None).unwrap();
    assert!(j(&e.rpc("page".into(), "{not json".into()))["error"].is_string());
    assert!(j(&e.rpc("page".into(), "[1,2]".into()))["error"].is_string());
    assert!(j(&e.rpc("nope".into(), "{}".into()))["error"].is_string());
    assert!(j(&e.run("no.such".into(), "{}".into()))["error"].is_string());
    assert!(j(&e.run("node.setName".into(), r#"{"node": 999999999, "name": "x"}"#.into()))["error"].is_string());
    assert!(Engine::open(tmp("bad-date").display().to_string(), false, Some("tomorrowish".into())).is_err());
}

#[test]
fn autosave_writes_without_an_explicit_save() {
    let path = tmp("autosave");
    let e = Engine::open(path.display().to_string(), false, Some("2026-10-10".into())).unwrap();
    let _ = e.run("node.quickAdd".into(), r#"{"name": "Written by itself"}"#.into());
    let written = (0..40).any(|_| {
        std::thread::sleep(Duration::from_millis(100));
        path.exists() && bana_core::Graph::load(&path).is_ok_and(|g| g.iter().any(|n| n.name == "Written by itself"))
    });
    assert!(written, "autosave did not write the change within 4 s");
}

/// A keystroke on a phone is a rename plus a page read: it stays well under a frame budget's worth
/// of engine time on a 20k-node graph with a 1000-line page open (release builds are faster still).
#[test]
fn keystroke_cost_on_a_big_graph() {
    let path = tmp("big");
    let mut g = bana_core::sample::large(20_000).unwrap();
    g.save(&path).unwrap();
    let e = Engine::open(path.display().to_string(), false, Some("2026-10-10".into())).unwrap();
    let _ = e.run("nav.home".into(), String::new());
    let _ = e.run("view.expandAll".into(), String::new());
    let p = page(&e);
    let id = p["rows"]["items"][3]["id"].as_u64().unwrap();
    let t = std::time::Instant::now();
    for i in 0..20 {
        let _ = e.run("node.setName".into(), format!(r#"{{"node": {id}, "name": "typed {i}", "caret": 5}}"#));
        let _ = e.rpc("page".into(), r#"{"count": 2000}"#.into());
    }
    let each = t.elapsed() / 20;
    eprintln!("keystroke (rename + page of {} rows): {each:?}", p["rows"]["total"]);
    assert!(each < Duration::from_millis(250), "{each:?}");
}

#[test]
fn sync_starts_off_and_answers_status() {
    let dir = std::env::temp_dir().join(format!("bana-ffi-sync-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let e = Engine::open(dir.join("g.bana").display().to_string(), true, Some("2026-10-10".into())).unwrap();
    assert!(e.rpc("sync.status".into(), String::new()).contains("\"off\""), "no sync before start");
    e.start_sync(dir.join("private/sync.json").display().to_string());
    assert!(e.rpc("sync.status".into(), String::new()).contains("\"off\""));
    assert!(e.sync_now(10).contains("\"off\""));
    // A bad relay address is refused, and nothing is written.
    assert!(e.rpc("sync.setup".into(), r#"{"relay":"http://example.com"}"#.into()).contains("error"));
    assert!(!dir.join("private/sync.json").exists());
    assert!(e.rpc("sync.join".into(), r#"{"code":"nonsense"}"#.into()).contains("not a Bana sync code"));
    let _ = std::fs::remove_dir_all(&dir);
}
