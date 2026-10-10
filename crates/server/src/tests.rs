use serde_json::json;
use tanacraft_commands::Session;
use tanacraft_core::{Date, sample};

use super::*;

fn host() -> Host {
    let today = Date::new(2026, 10, 10).unwrap();
    let mut s = Session::with_today(sample::sample(today).unwrap(), today);
    s.run("nav.today", &json!({})).unwrap();
    Host::new(s)
}

#[test]
fn page_lists_rows_and_typing_reaches_the_graph() {
    let mut h = host();
    let p = h.handle("page", &json!({}));
    assert!(p["title"].as_str().unwrap().starts_with("Today"));
    let rows = p["rows"]["items"].as_array().unwrap();
    assert!(!rows.is_empty());
    let first = rows.iter().find(|r| r["k"] == "node").unwrap()["id"].as_u64().unwrap();
    let r = h.handle("run", &json!({"command": "node.setName", "params": {"node": first, "name": "typed", "caret": 5}}));
    assert!(r.get("error").is_none(), "{r}");
    assert_eq!(h.session.graph.name(tanacraft_core::NodeId(first)), "typed");
}

#[test]
fn a_logged_edit_sent_twice_runs_once() {
    // A reload replays the write-ahead log; the request in flight (or the unload beacon) may
    // arrive too. The same (client, seq) runs once; a new page's numbers are its own.
    let mut h = host();
    let parent = h.session.zoom.0;
    let add = |seq: u64, client: &str| json!({"command": "node.add", "params": {"parent": parent, "name": "once"}, "wal": [client, seq]});
    let count = |h: &Host| h.session.graph.children(tanacraft_core::NodeId(parent)).iter().filter(|c| h.session.graph.name(**c) == "once").count();
    assert!(h.handle("run", &add(1, "a")).get("dup").is_none());
    assert_eq!(h.handle("run", &add(1, "a"))["dup"], json!(true));
    h.handle("run", &add(2, "a"));
    assert_eq!(h.handle("run", &add(1, "a"))["dup"], json!(true), "an older one late");
    h.handle("run", &add(1, "b"));
    assert_eq!(count(&h), 3);
}

#[test]
fn panels_keep_their_own_page() {
    let mut h = host();
    let today = h.session.zoom;
    h.handle("panel.open", &json!({"name": "p2", "node": h.session.graph.sys.library.0}));
    let p2 = h.handle("page", &json!({"panel": "p2"}));
    assert_eq!(p2["zoom"], json!(h.session.graph.sys.library.0));
    let main = h.handle("page", &json!({"panel": MAIN_PANEL}));
    assert_eq!(main["zoom"], json!(today.0));
}

#[test]
fn windows_are_clamped_and_bad_input_is_an_error() {
    let mut h = host();
    let p = h.handle("page", &json!({"start": 1_000_000, "count": 99_999_999}));
    assert_eq!(p["rows"]["items"].as_array().unwrap().len(), 0);
    assert!(h.handle("run", &json!({"command": "nope"}))["error"].is_string());
    assert!(h.handle("bogus", &json!({}))["error"].is_string());
    assert!(h.handle("sidebar", &json!({}))["pinned"].is_array());
}

#[test]
fn origins_from_other_sites_are_refused() {
    assert!(http_origin_ok(None));
    assert!(http_origin_ok(Some("http://127.0.0.1:5173")));
    assert!(http_origin_ok(Some("http://localhost:8787")));
    assert!(http_origin_ok(Some("tauri://localhost")));
    assert!(!http_origin_ok(Some("https://evil.example")));
    assert!(!http_origin_ok(Some("null")));
    assert!(!http_origin_ok(Some("http://localhost.evil.example")));
}

fn http_origin_ok(o: Option<&str>) -> bool {
    crate::http::origin_ok(o)
}

#[test]
fn a_field_page_reads_schema_its_supertag_and_itself() {
    let mut h = host();
    let status = h.session.graph.find_field("Status").unwrap();
    h.handle("run", &json!({"command": "nav.open", "params": {"node": status.0}}));
    let p = h.handle("page", &json!({}));
    let names: Vec<&str> = p["crumbs"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Schema", "#task", "Status"]);
    assert_eq!(p["fieldType"], json!("options"));
}

#[test]
fn lines_typed_in_two_windows_rebase_over_each_other() {
    // Both insert into "Volunteer meetup": A at the end, B at the start, from the same text.
    assert_eq!(rebase("Volunteer meetup", "Volunteer meetup AAA", "BBB Volunteer meetup"), ("BBB Volunteer meetup AAA".into(), 4));
    assert_eq!(rebase("Volunteer meetup", "BBB Volunteer meetup", "Volunteer meetup AAA"), ("BBB Volunteer meetup AAA".into(), 0));
    // A deletion before this window's insert shifts it back.
    assert_eq!(rebase("abcdef", "abcdefX", "cdef"), ("cdefX".into(), -2));
    // Both at the same place: theirs first.
    assert_eq!(rebase("ab", "aXb", "aYb"), ("aYXb".into(), 1));
    // Overlapping changes: this window's text wins.
    assert_eq!(rebase("abcdef", "aZf", "abYf"), ("aZf".into(), 0));
}

#[test]
fn each_window_has_its_own_page_and_caret() {
    let mut h = host();
    let rows = h.handle("page", &json!({"window": "A"}))["rows"]["items"].clone();
    let ids: Vec<u64> = rows.as_array().unwrap().iter().filter(|r| r["k"] == "node").map(|r| r["id"].as_u64().unwrap()).collect();
    let (one, two) = (ids[0], ids[1]);
    let name = |h: &Host, n: u64| h.session.graph.name(tanacraft_core::NodeId(n)).to_string();
    let (n1, n2) = (name(&h, one), name(&h, two));
    h.handle("run", &json!({"window": "A", "command": "focus.set", "params": {"node": one, "caret": 0}}));
    h.handle("run", &json!({"window": "B", "command": "focus.set", "params": {"node": two, "caret": 0}}));
    assert_eq!(h.handle("page", &json!({"window": "A"}))["focus"], json!(one));
    assert_eq!(h.handle("page", &json!({"window": "B"}))["focus"], json!(two), "B's caret stays in B's line");
    // Both type in one line at once, each from the text it had.
    h.handle("run", &json!({"window": "A", "command": "node.setName", "params": {"node": one, "name": format!("{n1} AAA"), "base": n1}}));
    h.handle("run", &json!({"window": "B", "command": "node.setName", "params": {"node": one, "name": format!("BBB {n1}"), "base": n1}}));
    assert_eq!(name(&h, one), format!("BBB {n1} AAA"));
    assert_eq!(name(&h, two), n2);
    // Panels B opens are its own too.
    h.handle("panel.open", &json!({"window": "B", "name": "p2", "node": one}));
    h.handle("panel.open", &json!({"window": "A", "name": "p2", "node": two}));
    assert_eq!(h.handle("page", &json!({"window": "B", "panel": "p2"}))["zoom"], json!(one));
    assert_eq!(h.handle("page", &json!({"window": "A", "panel": "p2"}))["zoom"], json!(two));
}

#[test]
fn pickers_show_descriptions_recents_and_the_slash_set() {
    let mut h = host();
    let tags = h.handle("choices", &json!({"mode": "tag", "query": "task"}));
    let t = &tags["items"][0];
    assert_eq!(t["label"], json!("task"));
    assert!(t["workspace"].is_string() && t["description"].is_string());
    // "@" before anything is typed: recently opened nodes, with their paths.
    let lib = h.session.graph.sys.library;
    h.handle("run", &json!({"command": "nav.open", "params": {"node": lib.0}}));
    h.handle("run", &json!({"command": "nav.today", "params": {}}));
    let refs = h.handle("choices", &json!({"mode": "reference", "query": ""}));
    assert!(!refs["items"].as_array().unwrap().is_empty());
    assert!(refs["items"][0]["path"].is_string());
    let slash: Vec<String> = h.handle("choices", &json!({"mode": "slash", "query": ""}))["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["label"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(slash, ["Paste", "Search node", "Field", "Reference", "Heading", "Checkbox", "Checklist", "More commands"]);
}

#[test]
fn a_field_name_typed_after_a_line_stays_below_it_when_committed() {
    // DIFFS N76: "x", Enter, `>`, "Fresh9", reload: the field row stays after "x".
    let mut h = host();
    let page = h.handle("run", &json!({"command": "node.add", "params": {"name": "N76"}}))["result"]["id"].as_u64().unwrap();
    let x = h.handle("run", &json!({"command": "node.add", "params": {"name": "x", "parent": page}}))["result"]["id"].as_u64().unwrap();
    let line = h.handle("run", &json!({"command": "node.add", "params": {"name": "", "parent": page}}))["result"]["id"].as_u64().unwrap();
    h.handle("fieldDraft", &json!({"owner": page, "line": line, "name": "Fresh9"}));
    assert_eq!(h.handle("fieldDraft", &json!({"commit": true}))["ok"], json!(true));
    let g = &h.session.graph;
    let f = g.find_field("Fresh9").unwrap();
    let entry = g.get(tanacraft_core::NodeId(page)).unwrap().fields.iter().find(|e| e.field == f).unwrap();
    assert_eq!(entry.after, Some(tanacraft_core::NodeId(x)));
    assert!(!g.children(tanacraft_core::NodeId(page)).contains(&tanacraft_core::NodeId(line)));
}

#[test]
fn keys_typed_at_the_end_of_a_line_another_window_splits_follow_the_end() {
    // DIFFS N74: B's caret is at the end of "Pick a date"; A presses Enter after "Pick".
    let mut h = host();
    h.handle("page", &json!({"window": "A"}));
    let page = h.session.zoom.0;
    let line = h.handle("run", &json!({"window": "A", "command": "node.add", "params": {"name": "Pick a date", "parent": page}}))["result"]["id"]
        .as_u64()
        .unwrap();
    let name = |h: &Host, n: u64| h.session.graph.name(tanacraft_core::NodeId(n)).to_string();
    h.handle("run", &json!({"window": "B", "command": "focus.set", "params": {"node": line, "caret": 11}}));
    h.handle("run", &json!({"window": "A", "command": "node.enter", "params": {"node": line, "caret": 4}}));
    let second = h.handle("page", &json!({"window": "A"}))["focus"].as_u64().unwrap();
    assert_eq!((name(&h, line), name(&h, second)), ("Pick".to_string(), " a date".to_string()));
    let b = h.handle("page", &json!({"window": "B"}));
    assert_eq!((b["focus"].as_u64(), b["caret"].as_u64()), (Some(second), Some(7)), "B's caret moved with the end");
    // B's keys sent before it saw the split (from the old text) go on at the end too.
    h.handle(
        "run",
        &json!({"window": "B", "command": "node.setName", "params": {"node": line, "name": "Pick a date123", "base": "Pick a date", "caret": 14}}),
    );
    assert_eq!((name(&h, line), name(&h, second)), ("Pick".to_string(), " a date123".to_string()));
    // A caret before the split point stays in the first half.
    h.handle(
        "run",
        &json!({"window": "B", "command": "node.setName", "params": {"node": line, "name": "XPick a date", "base": "Pick a date", "caret": 1}}),
    );
    assert_eq!(name(&h, line), "XPick");
    // Enter at the end of a line moves no text: later typing in it stays there.
    h.handle("run", &json!({"window": "A", "command": "node.enter", "params": {"node": line, "caret": 5}}));
    h.handle("run", &json!({"window": "B", "command": "node.setName", "params": {"node": line, "name": "XPick!", "base": "XPick", "caret": 6}}));
    assert_eq!(name(&h, line), "XPick!");
}
