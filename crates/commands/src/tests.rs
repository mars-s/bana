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
    assert_eq!(lines(&mut s), ["  - Hello", "> - ", "  -  world"]);
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
    run(&mut s, "node.backspace", json!({}));
    assert_eq!(lines(&mut s), ["> - Hello there"], "first node: nothing to merge into");
}

/// Spec 02 §4: Backspace merges into the previous *sibling*, even when an expanded child of that
/// sibling is the row above; a node with children does not merge; Delete at the end joins the next.
#[test]
fn backspace_and_delete_follow_siblings() {
    let mut s = blank();
    let beta = id(&run(&mut s, "node.add", json!({"name": "Beta"})), "id");
    run(&mut s, "node.add", json!({"name": "child one", "parent": beta.0}));
    run(&mut s, "node.expand", json!({"node": beta.0}));
    let two = id(&run(&mut s, "node.add", json!({"name": "child two"})), "id");
    run(&mut s, "node.backspace", json!({"node": two.0}));
    assert_eq!(lines(&mut s), ["> - Betachild two", "    - child one"]);
    assert_eq!(s.caret, 4);
    run(&mut s, "node.backspace", json!({"node": beta.0}));
    assert_eq!(lines(&mut s)[0], "> - Betachild two", "a node with children does not merge");
    let x = id(&run(&mut s, "node.add", json!({"name": "two"})), "id");
    run(&mut s, "node.add", json!({"name": "three"}));
    run(&mut s, "node.joinNext", json!({"node": x.0}));
    assert_eq!(lines(&mut s)[2], "> - twothree");
    assert_eq!(s.caret, 3);
}

/// Spec 02 §6: moving the first child up crosses into the previous branch; the page top stops.
#[test]
fn move_up_crosses_into_previous_branch() {
    let mut s = blank();
    let a = id(&run(&mut s, "node.add", json!({"name": "A"})), "id");
    let b = id(&run(&mut s, "node.add", json!({"name": "B"})), "id");
    run(&mut s, "node.add", json!({"name": "a1", "parent": a.0}));
    let b1 = id(&run(&mut s, "node.add", json!({"name": "b1", "parent": b.0})), "id");
    run(&mut s, "node.expand", json!({"node": b.0}));
    run(&mut s, "node.moveUp", json!({"node": b1.0}));
    assert_eq!(s.graph.children(a).len(), 2);
    assert_eq!(s.graph.name(s.graph.children(a)[1]), "b1");
    run(&mut s, "node.moveDown", json!({"node": b1.0}));
    assert_eq!(s.graph.children(b), &[b1], "and back down into the next branch");
    assert!(s.run("node.moveUp", &json!({"node": a.0})).is_err(), "first top-level node stays");
}

#[test]
fn enter_at_start_paste_duplicate_heading() {
    let mut s = blank();
    let a = id(&run(&mut s, "node.add", json!({"name": "text"})), "id");
    run(&mut s, "node.enter", json!({"node": a.0, "caret": 0}));
    assert_eq!(lines(&mut s), ["> - ", "  - text"], "caret goes into the new line above");
    run(&mut s, "node.paste", json!({"text": "- one\n  - one a\n- two"}));
    assert_eq!(lines(&mut s), ["  - one", "    - one a", "> - two", "  - text"]);
    run(&mut s, "node.paste", json!({"node": a.0, "text": "x\ny"}));
    assert_eq!(lines(&mut s)[3..], ["  - text".to_string(), "  - x".to_string(), "> - y".to_string()]);
    run(&mut s, "focus.set", json!({"node": a.0}));
    run(&mut s, "node.duplicate", json!({}));
    assert_eq!(s.focus, Some(a), "duplicate keeps the caret in the original");
    run(&mut s, "node.toggleHeading", json!({}));
    assert!(s.graph.get(a).unwrap().heading);
    // Trailing spaces go when a tag is applied.
    run(&mut s, "node.setName", json!({"node": a.0, "name": "Gamma "}));
    run(&mut s, "tag.add", json!({"node": a.0, "tag": "task"}));
    assert_eq!(s.graph.name(a), "Gamma");
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
    assert_eq!(crumbs, ["Library"]);
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
    let week = s.graph.parent(s.zoom).unwrap();
    run(&mut s, "nav.open", json!({"node": week.0}));
    run(&mut s, "nav.calendarStep", json!({"step": 1}));
    assert_eq!(s.graph.name(s.zoom), "Week 43");
    let year = s.graph.parent(s.zoom).unwrap();
    run(&mut s, "nav.open", json!({"node": year.0}));
    run(&mut s, "nav.calendarStep", json!({"step": -1}));
    assert_eq!(s.graph.name(s.zoom), "2025");
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
    assert_eq!(s.graph.name(s.graph.parent(s.zoom).unwrap()), "Sat, 10 Oct", "an untitled page on today");
    assert_eq!(s.graph.name(s.zoom), "");
    let beds = run(&mut s, "node.add", json!({"name": "List"}));
    let list = id(&beds, "id");
    for (n, d) in [("x", true), ("y", false)] {
        let c = id(&run(&mut s, "node.add", json!({"name": n, "parent": list.0})), "id");
        run(&mut s, "node.setDone", json!({"node": c.0, "done": d}));
    }
    run(&mut s, "node.toggleProgress", json!({"node": list.0}));
    assert_eq!(s.graph.progress(list), (1, 2));
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

#[test]
fn search_nodes_and_views_by_command() {
    let mut s = Session::with_today(sample::sample(today()).unwrap(), today());
    let r = run(&mut s, "search.create", json!({"query": "#task AND TODO", "name": "Todo"}));
    let search = id(&r, "id");
    assert_eq!(r["results"], 3);
    // Expanded search nodes list their results as rows.
    assert!(lines(&mut s).iter().any(|l| l.contains("Fill with compost")), "{:?}", lines(&mut s));
    run(&mut s, "search.addClause", json!({"node": search.0, "clause": "compost", "not": true}));
    assert_eq!(run(&mut s, "graph.results", json!({"node": search.0}))["groups"][0]["items"].as_array().unwrap().len(), 1);
    run(&mut s, "search.toggleNot", json!({"node": search.0, "index": 2}));
    run(&mut s, "search.setMode", json!({"node": search.0, "any": true}));
    let q = run(&mut s, "search.removeClause", json!({"node": search.0, "index": 0}));
    assert_eq!(q["query"], "TODO OR compost");
    assert!(s.run("search.removeClause", &json!({"node": search.0, "index": 9})).is_err());
    assert!(s.run("search.setQuery", &json!({"node": search.0, "query": "#nope"})).is_err());

    run(&mut s, "nav.open", json!({"node": search.0}));
    run(&mut s, "view.set", json!({"kind": "board"}));
    assert_eq!(
        s.graph.view_def(search).group,
        Some(tanacraft_core::GroupBy::Field(s.graph.find_field("Status").unwrap())),
        "board picks an options field for lanes"
    );
    run(&mut s, "view.sort", json!({"by": "Due", "desc": true}));
    run(&mut s, "view.group", json!({"by": "none"}));
    run(&mut s, "view.filter", json!({"query": "#task"}));
    run(&mut s, "view.addColumn", json!({"field": "Status"}));
    run(&mut s, "view.removeColumn", json!({"field": "Due"}));
    assert!(!s.graph.view_columns(search).contains(&s.graph.find_field("Due").unwrap()));
    assert!(s.run("view.set", &json!({"kind": "pie"})).is_err());
    assert!(s.run("view.sort", &json!({"by": "nosuchfield"})).is_err());
    assert!(s.run("view.dateField", &json!({"field": "Status"})).is_err());
    run(&mut s, "view.setColumnWidth", json!({"field": "Status", "width": 220}));
    run(&mut s, "view.setColumnWidth", json!({"field": "title", "width": 5000}));
    assert_eq!(s.graph.column_width(search, s.graph.find_field("Status").unwrap()), Some(220.0));
    assert_eq!(s.graph.column_width(search, NodeId(0)), Some(1200.0), "clamped");
    run(&mut s, "view.toggleGroupHidden", json!({"group": "Done"}));
    run(&mut s, "view.group", json!({"by": "Status"}));
    assert!(s.graph.view_groups(search).iter().all(|g| g.label != "Done"), "hidden group");
    run(&mut s, "view.group", json!({"by": "none"}));
    run(&mut s, "view.calendarMode", json!({"mode": "day"}));
    assert_eq!(s.calendar_mode, 2);
    run(&mut s, "view.dateField", json!({"field": "Due"}));
    run(&mut s, "view.month", json!({"month": "2026-12"}));
    assert_eq!(run(&mut s, "view.month", json!({"step": 1}))["month"], "2027-01");
    // View changes are undoable.
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(s.graph.view_def(search).date_field, None);
}

#[test]
fn insert_quick_add_and_formatting() {
    let mut s = blank();
    let a = id(&run(&mut s, "node.add", json!({"name": "Hello world"})), "id");
    run(&mut s, "focus.set", json!({"node": a.0, "caret": 3}));
    run(&mut s, "node.insert", json!({}));
    assert_eq!(lines(&mut s), ["  - Hello world", "> - "]);
    let r = run(&mut s, "node.quickAdd", json!({"name": "Call the plumber", "tag": "task"}));
    assert_eq!(s.zoom, s.graph.sys.library, "quick add stays on the page");
    assert_eq!(s.graph.name(s.graph.parent(id(&r, "id")).unwrap()), "Sat, 10 Oct");
    assert!(s.run("node.quickAdd", &json!({"name": "  "})).is_err());

    run(&mut s, "format.bold", json!({"node": a.0, "start": 6, "end": 11}));
    assert_eq!(s.graph.name(a), "Hello **world**");
    run(&mut s, "format.bold", json!({"node": a.0, "start": 8, "end": 13}));
    assert_eq!(s.graph.name(a), "Hello world");
    // With focus, the text selection is used.
    run(&mut s, "focus.set", json!({"node": a.0, "caret": 5}));
    s.anchor = 0;
    run(&mut s, "format.highlight", json!({}));
    assert_eq!(s.graph.name(a), "==Hello== world");
    assert_eq!((s.anchor, s.caret), (2, 7));
    assert_eq!(s.graph.search("hello world"), vec![a], "search ignores marks");
}

#[test]
fn multi_select_moves_indents_tags_and_deletes_together() {
    let mut s = blank();
    let ids: Vec<NodeId> = ["a", "b", "c", "d"].iter().map(|n| id(&run(&mut s, "node.add", json!({"name": n})), "id")).collect();
    run(&mut s, "focus.set", json!({"node": ids[1].0}));
    run(&mut s, "select.down", json!({}));
    assert_eq!(s.selected(), vec![ids[1]], "first press selects the current node");
    run(&mut s, "select.down", json!({}));
    assert_eq!(s.selected(), vec![ids[1], ids[2]]);
    assert_eq!(s.focus, None, "selecting leaves text editing");
    run(&mut s, "node.indent", json!({}));
    assert_eq!(lines(&mut s), ["  - a", "    - b", "    - c", "  - d"]);
    run(&mut s, "node.outdent", json!({}));
    assert_eq!(lines(&mut s), ["  - a", "  - b", "  - c", "  - d"]);
    run(&mut s, "node.moveDown", json!({}));
    assert_eq!(lines(&mut s), ["  - a", "  - d", "  - b", "  - c"]);
    run(&mut s, "node.moveUp", json!({}));
    run(&mut s, "node.moveUp", json!({}));
    assert_eq!(lines(&mut s), ["  - b", "  - c", "  - a", "  - d"]);
    assert!(s.run("node.moveUp", &json!({})).is_err(), "first already at the top");
    assert_eq!(lines(&mut s), ["  - b", "  - c", "  - a", "  - d"], "a failed move changes nothing");
    run(&mut s, "tag.add", json!({"tag": "chore"}));
    run(&mut s, "node.toggleDone", json!({}));
    assert_eq!(lines(&mut s)[..2], ["  - [ ] b #chore".to_string(), "  - [ ] c #chore".to_string()]);
    // One undo step per command, even for a selection.
    run(&mut s, "edit.undo", json!({}));
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(lines(&mut s)[0], "  - b");
    run(&mut s, "node.delete", json!({}));
    assert_eq!(lines(&mut s), ["  - a", "  - d"]);
    assert_eq!(s.sel, None);
    // Explicit node lists work too (CLI, MCP).
    run(&mut s, "node.toggleDone", json!({"nodes": [ids[0].0, ids[3].0]}));
    assert_eq!(lines(&mut s), ["  - [ ] a", "  - [ ] d"]);
    // Selecting a parent and its child acts on the parent only.
    run(&mut s, "node.indent", json!({"node": ids[3].0}));
    run(&mut s, "node.expand", json!({"node": ids[0].0}));
    run(&mut s, "select.set", json!({"from": ids[0].0, "to": ids[3].0}));
    assert_eq!(s.selected(), vec![ids[0]]);
}

#[test]
fn view_filters_and_add_item() {
    let mut s = Session::with_today(sample::sample(today()).unwrap(), today());
    let open = s.graph.search("Open tasks")[0];
    run(&mut s, "nav.open", json!({"node": open.0}));
    run(&mut s, "view.addFilter", json!({"clause": "Status:\"Not started\""}));
    assert_eq!(s.graph.view_groups(open)[0].items.len(), 2);
    assert_eq!(s.graph.view_filtered_out(open), 1);
    run(&mut s, "view.removeFilter", json!({"index": 0}));
    assert_eq!(s.graph.view_filtered_out(open), 0);
    assert!(s.run("view.removeFilter", &json!({"index": 0})).is_err());
    // "+" on a search: a node in the Library with the query's supertag, opened.
    run(&mut s, "search.addClause", json!({"node": open.0, "clause": "Status:\"In progress\""}));
    let r = run(&mut s, "view.addItem", json!({"node": open.0}));
    let new = id(&r, "id");
    assert_eq!(s.zoom, new);
    assert_eq!(s.graph.parent(new), Some(s.graph.sys.library));
    assert!(s.graph.view_items(open).contains(&new), "the new node matches the search");
    // Supertag page: on today's page, tagged.
    let task = s.graph.find_tag("task").unwrap();
    let r = run(&mut s, "view.addItem", json!({"node": task.0, "name": "From the tag page"}));
    assert_eq!(s.graph.name(s.graph.parent(id(&r, "id")).unwrap()), "Sat, 10 Oct");
    // Board column preset.
    let beds = s.graph.search("Raised beds")[0];
    run(&mut s, "nav.open", json!({"node": beds.0}));
    let r = run(&mut s, "view.addItem", json!({"node": beds.0, "field": "Status", "value": "Done"}));
    assert_eq!(s.focus, Some(id(&r, "id")));
    run(&mut s, "node.setDone", json!({"node": id(&r, "id").0, "done": true}));
    assert_eq!(s.graph.checkbox(id(&r, "id")), Some(true));
}

#[test]
fn configuration_commands() {
    let mut s = Session::with_today(sample::sample(today()).unwrap(), today());
    run(&mut s, "tag.create", json!({"name": "book", "color": "teal"}));
    run(&mut s, "tag.addField", json!({"tag": "book", "field": "Rating", "type": "number"}));
    run(&mut s, "field.setRange", json!({"field": "Rating", "min": 1, "max": 5}));
    run(&mut s, "field.setRequired", json!({"field": "Rating", "on": true}));
    run(&mut s, "field.setHide", json!({"field": "Rating", "mode": "when_empty"}));
    run(&mut s, "field.setDefault", json!({"field": "Rating", "value": "3"}));
    run(&mut s, "tag.addField", json!({"tag": "book", "field": "Shelf", "type": "options"}));
    run(&mut s, "field.addOption", json!({"field": "Shelf", "option": "To read"}));
    run(&mut s, "field.addOption", json!({"field": "Shelf", "option": "Read"}));
    run(&mut s, "tag.setDoneMap", json!({"tag": "book", "field": "Shelf", "checked": "Read", "unchecked": "To read"}));
    run(&mut s, "tag.setCheckbox", json!({"tag": "book", "on": true}));
    run(&mut s, "tag.addTemplateNode", json!({"tag": "book", "name": "Quotes"}));
    run(&mut s, "tag.addView", json!({"tag": "book", "kind": "cards"}));
    let book = s.graph.find_tag("book").unwrap().0;
    run(&mut s, "node.togglePin", json!({"node": book}));
    let b = id(&run(&mut s, "node.add", json!({"name": "Dune"})), "id");
    run(&mut s, "tag.add", json!({"node": b.0, "tag": "book"}));
    let n = s.graph.get(b).unwrap().clone();
    let rating = s.graph.find_field("Rating").unwrap();
    assert_eq!(n.field_value(rating), Some(&tanacraft_core::FieldValue::Number(3.0)));
    assert_eq!(s.graph.name(s.graph.children(b)[0]), "Quotes");
    assert!(s.run("field.set", &json!({"node": b.0, "field": "Rating", "value": "9"})).is_err(), "out of range");
    run(&mut s, "node.toggleDone", json!({"node": b.0}));
    assert_eq!(export_md(&mut s, b), "- [x] Dune #book\n  - Rating:: 3\n  - Shelf:: Read\n  - Quotes\n");
    assert!(s.graph.pinned().contains(&s.graph.find_tag("book").unwrap()));
    assert!(s.run("field.setRange", &json!({"field": "Rating", "min": 9, "max": 1})).is_err());
    assert!(s.run("field.setHide", &json!({"field": "Rating", "mode": "sometimes"})).is_err());
    run(&mut s, "field.setType", json!({"field": "Shelf", "type": "plain"}));
    run(&mut s, "tag.removeField", json!({"tag": "book", "field": "Shelf"}));
    assert_eq!(s.graph.template_fields(s.graph.find_tag("book").unwrap()), vec![rating]);
}

fn export_md(s: &mut Session, n: NodeId) -> String {
    run(s, "export.markdown", json!({"node": n.0}))["markdown"].as_str().unwrap().to_string()
}

#[test]
fn convert_search_sort_children_select_all() {
    let mut s = Session::with_today(sample::sample(today()).unwrap(), today());
    let open = s.graph.search("Open tasks")[0];
    let r = run(&mut s, "search.toPlainList", json!({"node": open.0}));
    assert_eq!(r["references"], 3);
    assert!(s.graph.query(open).is_none());
    assert_eq!(s.graph.children(open).len(), 3);
    let beds = s.graph.search("Raised beds")[0];
    run(&mut s, "node.sortChildren", json!({"node": beds.0, "by": "name"}));
    let names: Vec<&str> = s.graph.children(beds).iter().map(|c| s.graph.name(*c)).collect();
    assert_eq!(names, ["Build four beds", "Fill with compost", "Order cedar boards"]);
    run(&mut s, "node.sortChildren", json!({"node": beds.0, "by": "done"}));
    assert_eq!(s.graph.name(s.graph.children(beds)[2]), "Order cedar boards", "not done first");
    run(&mut s, "nav.open", json!({"node": beds.0}));
    run(&mut s, "select.all", json!({}));
    assert_eq!(s.selected().len(), 3);
}

#[test]
fn system_fields_in_views() {
    let mut s = Session::with_today(sample::sample(today()).unwrap(), today());
    let task = s.graph.find_tag("task").unwrap();
    run(&mut s, "view.group", json!({"node": task.0, "by": "owner"}));
    let labels: Vec<String> = s.graph.view_groups(task).iter().map(|g| g.label.clone()).collect();
    assert!(labels.contains(&"Raised beds".to_string()), "{labels:?}");
    run(&mut s, "view.sort", json!({"node": task.0, "by": "edited", "desc": true}));
    run(&mut s, "view.toggleSystemField", json!({"node": task.0, "field": "Owner node"}));
    let item = s.graph.search("Build four beds")[0];
    assert_eq!(s.graph.display_line(task, item), "Raised beds");
    assert!(s.run("view.toggleSystemField", &json!({"node": task.0, "field": "Shoe size"})).is_err());
}

#[test]
fn move_many_keeps_order() {
    let mut s = blank();
    let ids: Vec<NodeId> = ["a", "b", "c", "d"].iter().map(|n| id(&run(&mut s, "node.add", json!({"name": n})), "id")).collect();
    let home = s.graph.sys.library;
    run(&mut s, "node.moveMany", json!({"nodes": [ids[0].0, ids[1].0], "parent": home.0, "index": 4}));
    assert_eq!(lines(&mut s), ["  - c", "  - d", "  - a", "  - b"]);
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(lines(&mut s), ["  - a", "  - b", "  - c", "  - d"]);
}

#[test]
fn checklist() {
    let mut s = blank();
    let a = id(&run(&mut s, "node.add", json!({"name": ""})), "id");
    run(&mut s, "node.insertChecklist", json!({"node": a.0}));
    assert_eq!(lines(&mut s), ["> - New checklist", "    - [ ] First item", "    - [ ] Second item"]);
    assert_eq!((s.focus, s.caret), (Some(a), 0), "caret at the start of the checklist name");
    assert_eq!(s.graph.progress(a), (0, 2));
}

#[test]
fn fields_hold_several_values() {
    let mut s = blank();
    let n = id(&run(&mut s, "node.add", json!({"name": "Launch"})), "id");
    run(&mut s, "field.add", json!({"node": n.0, "field": "Owners", "type": "plain"}));
    run(&mut s, "field.addValue", json!({"node": n.0, "field": "Owners", "value": "Ada"}));
    run(&mut s, "field.addValue", json!({"node": n.0, "field": "Owners", "value": "Sam"}));
    let v = run(&mut s, "field.addValue", json!({"node": n.0, "field": "Owners", "value": "Sam"}));
    assert_eq!(v["values"], json!(["Ada", "Sam"]), "duplicates are not added twice");
    // Edit the second value, then remove the first: the second moves up.
    run(&mut s, "field.set", json!({"node": n.0, "field": "Owners", "index": 1, "value": "Kim"}));
    let v = run(&mut s, "field.removeValue", json!({"node": n.0, "field": "Owners", "index": 0}));
    assert_eq!(v["values"], json!(["Kim"]));
    run(&mut s, "field.addValue", json!({"node": n.0, "field": "Owners", "value": "Lee"}));
    assert_eq!(export_md(&mut s, n), "- Launch\n  - Owners:: Kim, Lee\n");
    // Search matches any value.
    let lib = s.graph.sys.library.0;
    let found = run(&mut s, "search.create", json!({"query": "Owners:Lee", "parent": lib}));
    assert_eq!(found["results"], json!(1), "{found}");
    run(&mut s, "edit.undo", json!({}));
    // Undo takes back one value at a time.
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(export_md(&mut s, n), "- Launch\n  - Owners:: Kim\n");
}

#[test]
fn a_field_value_can_move_out_to_a_node() {
    let mut s = blank();
    let n = id(&run(&mut s, "node.add", json!({"name": "Trip"})), "id");
    run(&mut s, "field.add", json!({"node": n.0, "field": "Who", "type": "plain", "value": "Ada"}));
    run(&mut s, "field.addValue", json!({"node": n.0, "field": "Who", "value": "Sam"}));
    let v = run(&mut s, "field.valueToNode", json!({"node": n.0, "field": "Who", "index": 0}));
    assert_eq!(v["values"], json!(["Sam"]));
    assert_eq!(s.graph.name(s.graph.children(n)[0]), "Ada");
}

#[test]
fn move_to_top_bottom_target_and_leave_reference() {
    let mut s = blank();
    let lib = s.graph.sys.library;
    let a = id(&run(&mut s, "node.add", json!({"name": "a"})), "id");
    let b = id(&run(&mut s, "node.add", json!({"name": "b"})), "id");
    let c = id(&run(&mut s, "node.add", json!({"name": "c"})), "id");
    run(&mut s, "node.moveToTop", json!({"node": c.0}));
    assert_eq!(s.graph.children(lib), [c, a, b]);
    run(&mut s, "node.moveToBottom", json!({"node": c.0}));
    assert_eq!(s.graph.children(lib), [a, b, c]);
    run(&mut s, "node.setMoveTarget", json!({"node": a.0}));
    run(&mut s, "node.moveToTarget", json!({"node": c.0}));
    assert_eq!(s.graph.children(a), [c]);
    run(&mut s, "node.move", json!({"node": b.0, "parent": a.0, "leaveReference": true}));
    assert_eq!(s.graph.children(a), [c, b]);
    let left = s.graph.children(lib)[1];
    assert_eq!(s.graph.resolve(left), b, "a reference stays where b was");
    assert!(blank().run("node.moveToTarget", &json!({"node": 1})).is_err());
}

#[test]
fn supertag_title_expression_optional_and_pinned_fields() {
    let mut s = blank();
    run(&mut s, "tag.create", json!({"name": "project"}));
    run(&mut s, "tag.addField", json!({"tag": "project", "field": "Budget", "type": "number"}));
    assert!(
        s.run("tag.setTitleExpression", &json!({"tag": "project", "expression": "(${Budget})"})).is_err(),
        "must start with the name placeholder"
    );
    run(&mut s, "tag.setTitleExpression", json!({"tag": "project", "expression": "${name} (budget ${Budget})"}));
    let n = id(&run(&mut s, "node.add", json!({"name": "two"})), "id");
    run(&mut s, "tag.add", json!({"node": n.0, "tag": "project"}));
    assert_eq!(s.graph.title_suffix(n), [(" (budget ".to_string(), false), ("Budget".to_string(), true), (")".to_string(), false)]);
    run(&mut s, "field.set", json!({"node": n.0, "field": "Budget", "value": "1200"}));
    let text: String = s.graph.title_suffix(n).into_iter().map(|p| p.0).collect();
    assert_eq!(text, " (budget 1200)");
    // Optional fields are offered, not added; pinned toggles.
    let v = run(&mut s, "tag.setOptionalField", json!({"tag": "project", "field": "Notes"}));
    assert_eq!(v["on"], json!(true));
    let m = id(&run(&mut s, "node.add", json!({"name": "three"})), "id");
    run(&mut s, "tag.add", json!({"node": m.0, "tag": "project"}));
    let notes = s.graph.find_field("Notes").unwrap();
    assert!(!s.graph.get(m).unwrap().fields.iter().any(|e| e.field == notes));
    run(&mut s, "tag.setPinnedField", json!({"tag": "project", "field": "Budget"}));
    let tag = s.graph.find_tag("project").unwrap();
    assert_eq!(s.graph.tag_def(tag).unwrap().pinned_fields.len(), 1);
    run(&mut s, "tag.setColor", json!({"tag": "project", "color": "#12ab9f"}));
    assert!(s.run("tag.setColor", &json!({"tag": "project", "color": "#12ab9"})).is_err());
    let r = id(&run(&mut s, "tag.addRelated", json!({"tag": "project", "query": "TODO"})), "node");
    assert_eq!(s.graph.related_for(n), [r]);
    run(&mut s, "tag.removeRelated", json!({"tag": "project", "node": r.0}));
    assert!(s.graph.related_for(n).is_empty());
}

#[test]
fn query_groups_nest_and_or_and_not() {
    let mut s = blank();
    let lib = s.graph.sys.library.0;
    for n in ["Gamma one", "Delta two", "Gamma three"] {
        let x = id(&run(&mut s, "node.add", json!({"name": n})), "id");
        run(&mut s, "tag.add", json!({"node": x.0, "tag": "task"}));
    }
    let q = id(&run(&mut s, "search.create", json!({"query": "#task", "parent": lib})), "id");
    // #task AND ( "one" OR NOT "Gamma" )
    run(&mut s, "search.addGroup", json!({"node": q.0, "kind": "or"}));
    run(&mut s, "search.addClause", json!({"node": q.0, "path": [1], "clause": "one"}));
    run(&mut s, "search.addGroup", json!({"node": q.0, "path": [1], "kind": "not"}));
    let v = run(&mut s, "search.addClause", json!({"node": q.0, "path": [1, 1], "clause": "Gamma"}));
    assert_eq!(v["results"], json!(2), "{v}");
    let names: Vec<String> = s.graph.view_items(q).iter().map(|n| s.graph.name(*n).to_string()).collect();
    assert_eq!(names, ["Gamma one", "Delta two"]);
    assert!(s.run("search.addClause", &json!({"node": q.0, "path": [0], "clause": "x"})).is_err(), "path must lead to a group");
}

#[test]
fn option_sources_and_auto_collect() {
    let mut s = blank();
    run(&mut s, "field.create", json!({"name": "Colour", "type": "options"}));
    let list = id(&run(&mut s, "node.add", json!({"name": "Palette"})), "id");
    for c in ["Red", "Blue"] {
        run(&mut s, "node.add", json!({"name": c, "parent": list.0}));
    }
    let v = run(&mut s, "field.addSource", json!({"field": "Colour", "node": list.0}));
    assert_eq!(v["options"], json!(["Red", "Blue"]));
    run(&mut s, "field.setAutoCollect", json!({"field": "Colour", "on": false}));
    let n = id(&run(&mut s, "node.add", json!({"name": "Car"})), "id");
    run(&mut s, "field.add", json!({"node": n.0, "field": "Colour", "value": "Green"}));
    let f = s.graph.find_field("Colour").unwrap();
    assert_eq!(s.graph.field_options(f), ["Red", "Blue"], "Green is stored but not collected");
    run(&mut s, "field.setAutoCollect", json!({"field": "Colour", "on": true}));
    run(&mut s, "field.set", json!({"node": n.0, "field": "Colour", "value": "Teal"}));
    assert_eq!(s.graph.field_options(f), ["Teal", "Red", "Blue"]);
}
