use serde_json::{Value, json};
use tanacraft_core::{Date, Graph, NodeId, sample};

use crate::Session;

fn today() -> Date {
    Date::new(2026, 10, 10).unwrap()
}

fn blank() -> Session {
    Session::with_today(Graph::new(), today())
}

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn lines(s: &mut Session) -> Vec<String> {
    let v = run(s, "graph.view", json!({}));
    v["lines"].as_array().unwrap().iter().map(|l| l.as_str().unwrap().to_string()).collect()
}

fn id(v: &Value, k: &str) -> NodeId {
    NodeId(v[k].as_u64().unwrap())
}

#[test]
fn registry_ids_and_shortcuts_are_unique() {
    let s = blank();
    let mut ids = std::collections::HashSet::new();
    let mut keys = std::collections::HashSet::new();
    for c in s.registry.all() {
        assert!(ids.insert(c.id), "duplicate id {}", c.id);
        for k in c.shortcut.split(" / ").filter(|k| !k.is_empty()) {
            assert!(keys.insert(k.to_lowercase()), "duplicate shortcut {k} ({})", c.id);
        }
    }
    assert_eq!(s.registry.by_shortcut("Tab").map(|c| c.id), Some("node.indent"));
    assert!(s.registry.search("zoom in").iter().any(|c| c.id == "nav.zoomIn"));
}

#[test]
fn typing_an_outline_with_enter_tab_and_shift_tab() {
    let mut s = blank();
    let a = id(&run(&mut s, "node.add", json!({"name": "Groceries"})), "id");
    run(&mut s, "focus.set", json!({"node": a.0}));
    run(&mut s, "node.enter", json!({}));
    run(&mut s, "node.setName", json!({"name": "Milk"}));
    run(&mut s, "node.indent", json!({}));
    run(&mut s, "node.enter", json!({}));
    run(&mut s, "node.setName", json!({"name": "Bread"}));
    run(&mut s, "node.enter", json!({}));
    run(&mut s, "node.setName", json!({"name": "Call mum"}));
    run(&mut s, "node.outdent", json!({}));
    assert_eq!(lines(&mut s), ["  - Groceries", "    - Milk", "    - Bread", "> - Call mum"]);
    // Outdenting past the top of the view is refused and changes nothing.
    assert!(s.run("node.outdent", &json!({})).is_err());
    // Each command is one undo step.
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(lines(&mut s)[3], ">   - Call mum");
}

#[test]
fn enter_splits_at_the_caret() {
    let mut s = blank();
    let a = id(&run(&mut s, "node.add", json!({"name": "Hello world"})), "id");
    run(&mut s, "node.enter", json!({"node": a.0, "caret": 5}));
    assert_eq!(lines(&mut s), ["  - Hello", "> -  world"]);
    // At the start: an empty node above, focus stays.
    let b = s.focus.unwrap();
    run(&mut s, "node.enter", json!({"node": b.0, "caret": 0}));
    assert_eq!(lines(&mut s), ["  - Hello", "  - ", "> -  world"]);
}

#[test]
fn enter_on_expanded_parent_adds_first_child() {
    let mut s = blank();
    let a = id(&run(&mut s, "node.add", json!({"name": "Parent"})), "id");
    run(&mut s, "node.add", json!({"name": "Child", "parent": a.0}));
    run(&mut s, "node.expand", json!({"node": a.0}));
    run(&mut s, "node.enter", json!({"node": a.0}));
    assert_eq!(lines(&mut s), ["  - Parent", ">   - ", "    - Child"]);
}

#[test]
fn backspace_merges_and_deletes() {
    let mut s = blank();
    let a = id(&run(&mut s, "node.add", json!({"name": "Hello"})), "id");
    let b = id(&run(&mut s, "node.add", json!({"name": " there"})), "id");
    run(&mut s, "node.backspace", json!({"node": b.0}));
    assert_eq!(lines(&mut s), ["> - Hello there"]);
    assert_eq!((s.focus, s.caret), (Some(a), 5));
    run(&mut s, "node.enter", json!({"caret": 11}));
    run(&mut s, "node.backspace", json!({}));
    assert_eq!(lines(&mut s), ["> - Hello there"]);
    assert!(s.run("node.backspace", &json!({})).is_err(), "nothing above the first node");
}

#[test]
fn move_up_down_and_zoom() {
    let mut s = blank();
    let a = id(&run(&mut s, "node.add", json!({"name": "a"})), "id");
    run(&mut s, "node.add", json!({"name": "b"}));
    run(&mut s, "node.moveDown", json!({"node": a.0}));
    assert_eq!(lines(&mut s), ["  - b", "  - a"]);
    run(&mut s, "node.add", json!({"name": "inside a", "parent": a.0}));
    run(&mut s, "nav.zoomIn", json!({"node": a.0}));
    assert_eq!(lines(&mut s), ["  - inside a"]);
    let crumbs: Vec<&str> = s.breadcrumbs().iter().map(|c| s.graph.name(*c)).collect();
    assert_eq!(crumbs, ["Home"]);
    run(&mut s, "nav.zoomOut", json!({}));
    assert_eq!(s.zoom, s.graph.sys.library);
    assert_eq!(s.focus, Some(a));
    run(&mut s, "nav.back", json!({}));
    assert_eq!(s.zoom, a);
    run(&mut s, "nav.forward", json!({}));
    assert_eq!(s.zoom, s.graph.sys.library);
}

#[test]
fn tags_fields_and_checkboxes_by_name() {
    let mut s = blank();
    let n = id(&run(&mut s, "node.add", json!({"name": "Write report"})), "id");
    run(&mut s, "tag.add", json!({"node": n.0, "tag": "#task", "color": "pink"}));
    run(&mut s, "tag.setCheckbox", json!({"tag": "task", "on": true}));
    run(&mut s, "tag.addField", json!({"tag": "task", "field": "Due", "type": "date"}));
    run(&mut s, "field.set", json!({"node": n.0, "field": "Due", "value": "2026-10-20"}));
    assert!(s.run("field.set", &json!({"node": n.0, "field": "Due", "value": "next week"})).is_err());
    run(&mut s, "node.toggleDone", json!({"node": n.0}));
    let md = run(&mut s, "export.markdown", json!({"node": n.0}));
    assert_eq!(md["markdown"], "- [x] Write report #task\n  - Due:: 2026-10-20\n");
    let tags = run(&mut s, "graph.tags", json!({}));
    let task = tags.as_array().unwrap().iter().find(|t| t["name"] == "task").unwrap();
    assert_eq!(task["count"], 1);
    assert_eq!(task["fields"][0]["type"], "date");
    run(&mut s, "field.add", json!({"node": n.0, "field": "Notes"}));
    assert_eq!(s.graph.get(n).unwrap().fields.len(), 2);
}

#[test]
fn daily_notes_navigation() {
    let mut s = blank();
    run(&mut s, "nav.today", json!({}));
    assert_eq!(s.graph.name(s.zoom), "Sat, 10 Oct");
    run(&mut s, "nav.nextDay", json!({}));
    assert_eq!(s.graph.name(s.zoom), "Sun, 11 Oct");
    run(&mut s, "nav.day", json!({"date": "2026-10-12"}));
    let crumbs: Vec<&str> = s.breadcrumbs().iter().map(|c| s.graph.name(*c)).collect();
    assert_eq!(crumbs, ["Daily notes", "2026", "Week 42"]);
    assert!(!s.graph.can_undo(), "opening a day is not an undo step");
}

#[test]
fn delete_restore_and_undo_keep_view_valid() {
    let mut s = Session::with_today(sample::sample(today()).unwrap(), today());
    let garden = s.graph.search("Community garden")[0];
    run(&mut s, "nav.open", json!({"node": garden.0}));
    run(&mut s, "node.delete", json!({"node": garden.0}));
    assert!(s.graph.in_trash(garden));
    run(&mut s, "node.restore", json!({"node": garden.0}));
    assert!(!s.graph.in_trash(garden));
    let x = id(&run(&mut s, "node.add", json!({"name": "temp", "parent": garden.0})), "id");
    run(&mut s, "nav.zoomIn", json!({"node": x.0}));
    run(&mut s, "edit.undo", json!({}));
    assert!(s.graph.get(x).is_none());
    assert!(s.graph.get(s.zoom).is_some(), "zoom falls back to a live node");
}

#[test]
fn references_render_their_target() {
    let mut s = Session::with_today(sample::sample(today()).unwrap(), today());
    run(&mut s, "nav.today", json!({}));
    let l = lines(&mut s);
    assert!(l.iter().any(|x| x.contains("[[Community garden]]")), "{l:?}");
    let r = s.rows().iter().find(|r| s.graph.resolve(r.id) != r.id).unwrap().id;
    run(&mut s, "node.expand", json!({"node": r.0}));
    assert!(lines(&mut s).iter().any(|x| x.contains("Raised beds")));
    // A reference to an ancestor does not expand forever.
    let garden = s.graph.resolve(r);
    let self_ref = id(&run(&mut s, "node.reference", json!({"target": garden.0, "parent": garden.0})), "id");
    run(&mut s, "node.expand", json!({"node": self_ref.0}));
    assert!(s.rows().len() < 100);
}

#[test]
fn duplicate_reference_replace_and_create_new() {
    let mut s = blank();
    let a = id(&run(&mut s, "node.add", json!({"name": "a"})), "id");
    run(&mut s, "node.add", json!({"name": "kid", "parent": a.0}));
    let copy = id(&run(&mut s, "node.duplicate", json!({"node": a.0})), "id");
    assert_ne!(copy, a);
    assert_eq!(s.graph.name(copy), "a");
    assert_eq!(s.graph.children(copy).len(), 1);
    assert_ne!(s.graph.children(copy)[0], s.graph.children(a)[0]);
    let empty = id(&run(&mut s, "node.add", json!({"name": ""})), "id");
    run(&mut s, "focus.set", json!({"node": empty.0}));
    let r = id(&run(&mut s, "node.reference", json!({"target": a.0, "replace": empty.0})), "id");
    assert!(s.graph.get(empty).is_none());
    assert_eq!(s.focus, Some(r));
    assert_eq!(s.graph.index_in_parent(r), Some(2));
    run(&mut s, "node.createNew", json!({}));
    assert_eq!(s.graph.name(s.zoom), "Sat, 10 Oct");
    assert!(s.focus.is_some());
}

#[test]
fn file_save_and_open() {
    let dir = std::env::temp_dir().join(format!("tanacraft-cmd-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("g.json");
    let mut s = Session::with_today(sample::sample(today()).unwrap(), today());
    run(&mut s, "file.save", json!({"path": path.to_str().unwrap()}));
    let mut t = blank();
    run(&mut t, "file.open", json!({"path": path.to_str().unwrap()}));
    assert_eq!(t.graph.len(), s.graph.len());
    assert!(t.run("file.open", &json!({"path": "/nonexistent/x.json"})).is_err());
    std::fs::remove_dir_all(&dir).ok();
}

/// Every command with junk parameters returns (Ok or Err) and never panics or corrupts the graph.
#[test]
fn hostile_params_never_panic() {
    let junk = [
        json!(null),
        json!({}),
        json!({"node": -1, "parent": "x", "index": 1e300, "caret": -5}),
        json!({"node": 999999, "tag": 3, "field": [], "value": {}, "name": 7, "date": "2026-13-45"}),
        json!({"node": "n2", "parent": 2, "tag": "#", "field": "", "type": "nope", "options": "x", "path": ""}),
        json!({"node": 1, "target": 1, "base": 1, "tag": 6, "on": "yes"}),
    ];
    let ids: Vec<&str> = blank().registry.all().iter().map(|c| c.id).filter(|id| !id.starts_with("file.")).collect();
    for params in &junk {
        let mut s = Session::with_today(sample::sample(today()).unwrap(), today());
        s.focus = s.rows().first().map(|r| r.id);
        for id in &ids {
            let _ = s.run(id, params);
            assert!(s.graph.get(s.zoom).is_some(), "{id} broke the view");
        }
        let text = s.graph.to_json().unwrap();
        Graph::from_json(&text).unwrap();
        assert!(s.graph.get(s.graph.sys.root).is_some());
    }
}

#[test]
fn typing_a_word_is_one_undo_step() {
    let mut s = blank();
    let a = id(&run(&mut s, "node.add", json!({"name": ""})), "id");
    run(&mut s, "focus.set", json!({"node": a.0}));
    for w in ["H", "Hi", "Hi!"] {
        run(&mut s, "node.setName", json!({"name": w}));
    }
    run(&mut s, "node.enter", json!({}));
    run(&mut s, "node.setName", json!({"name": "x"}));
    run(&mut s, "edit.undo", json!({}));
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(s.graph.name(a), "Hi!");
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(s.graph.name(a), "");
}
