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

#[test]
fn inbox_capture_and_workspaces() {
    let mut s = blank();
    let v = run(&mut s, "node.capture", json!({"name": "Call the plumber", "tag": "task"}));
    let inbox = id(&v, "inbox");
    assert_eq!(s.graph.name(inbox), "Inbox");
    assert!(s.graph.is_system(inbox));
    run(&mut s, "nav.inbox", json!({}));
    assert_eq!(s.zoom, inbox);
    run(&mut s, "workspace.rename", json!({"name": "Home"}));
    // DIFFS N6: a second workspace sits beside the first in the same graph; nothing switches and
    // no file is written.
    let w = run(&mut s, "workspace.create", json!({"name": "Work Projects"}));
    let ws = id(&w, "id");
    let lib = id(&w, "library");
    assert_eq!(s.graph.name(s.graph.sys.root), "Home");
    assert_eq!(s.zoom, inbox, "creating does not leave the page");
    assert_eq!(s.graph.search("plumber").len(), 1);
    let list = run(&mut s, "workspace.list", json!({}));
    let names: Vec<&str> = list["workspaces"].as_array().unwrap().iter().map(|w| w["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Home", "Work Projects"]);
    let page = id(&run(&mut s, "node.add", json!({"name": "Q4 plan", "parent": lib.0})), "id");
    assert_eq!(s.graph.workspace_of(page), ws);
    assert_eq!(s.graph.workspace_of(inbox), s.graph.sys.root);
    run(&mut s, "workspace.switch", json!({"name": "Work Projects"}));
    assert_eq!(s.zoom, ws);
    assert!(s.run("workspace.switch", &json!({"name": "nope"})).is_err());
    assert!(s.run("node.delete", &json!({"node": ws.0})).is_err(), "a workspace is built in");
    run(&mut s, "workspace.rename", json!({"name": "Work", "workspace": "Work Projects"}));
    assert_eq!(s.graph.name(ws), "Work");
    run(&mut s, "edit.undo", json!({}));
    run(&mut s, "edit.undo", json!({}));
    run(&mut s, "edit.undo", json!({}));
    assert!(s.graph.get(ws).is_none(), "creating a workspace undoes");
}

#[test]
fn clone_reference_makes_an_independent_copy() {
    let mut s = blank();
    let a = id(&run(&mut s, "node.add", json!({"name": "Recipe"})), "id");
    run(&mut s, "node.add", json!({"name": "Flour", "parent": a.0}));
    let other = id(&run(&mut s, "node.add", json!({"name": "Plan"})), "id");
    let r = id(&run(&mut s, "node.reference", json!({"target": a.0, "parent": other.0})), "id");
    let c = id(&run(&mut s, "node.cloneReference", json!({"node": r.0})), "id");
    assert_ne!(c, a);
    assert_eq!(s.graph.children(other), [c]);
    assert_eq!(s.graph.name(s.graph.children(c)[0]), "Flour");
    assert!(s.graph.get(r).is_none());
    assert!(s.run("node.cloneReference", &json!({"node": a.0})).is_err());
}

#[test]
fn insert_date_links_the_day_at_the_caret() {
    let mut s = blank();
    let n = id(&run(&mut s, "node.add", json!({"name": "Due "})), "id");
    run(&mut s, "focus.set", json!({"node": n.0, "caret": 4}));
    run(&mut s, "edit.insertDate", json!({"date": "2026-10-12"}));
    let day = s.graph.find_day(Date::new(2026, 10, 12).unwrap()).unwrap();
    assert_eq!(s.graph.name(n), format!("Due [[id:{}]]", day.0));
    assert_eq!(s.graph.references_to(day), [n], "the day lists the node in its references");
    run(&mut s, "edit.insertTime", json!({"node": n.0, "caret": 0}));
    assert!(s.graph.name(n).chars().nth(2) == Some(':'), "{}", s.graph.name(n));
}

#[test]
fn convert_to_supertag_and_hard_delete() {
    let mut s = blank();
    let n = id(&run(&mut s, "node.add", json!({"name": "Recipe"})), "id");
    run(&mut s, "node.add", json!({"name": "Ingredients", "parent": n.0}));
    let v = run(&mut s, "node.convertToSupertag", json!({"node": n.0}));
    let tag = id(&v, "tag");
    assert!(s.graph.tag_def(tag).is_some());
    assert_eq!(s.graph.parent(tag), Some(s.graph.sys.schema));
    let lib = s.graph.sys.library;
    assert_eq!(s.graph.resolve(s.graph.children(lib)[0]), tag, "a reference stays");
    let x = id(&run(&mut s, "node.add", json!({"name": "Pancakes"})), "id");
    run(&mut s, "tag.add", json!({"node": x.0, "tag": "Recipe"}));
    assert_eq!(s.graph.name(s.graph.children(x)[0]), "Ingredients", "children became template content");
    assert!(s.run("node.convertToSupertag", &json!({"node": lib.0})).is_err());
    // Hard delete removes the node and every reference to it.
    let v = run(&mut s, "node.hardDelete", json!({"node": x.0}));
    assert_eq!(v["references_removed"], json!(0));
    let r = run(&mut s, "node.hardDelete", json!({"node": tag.0}));
    assert_eq!(r["references_removed"], json!(1));
    assert!(s.graph.in_trash(tag));
}

#[test]
fn paste_as_code_wraps_in_backticks() {
    let mut s = blank();
    let n = id(&run(&mut s, "node.add", json!({"name": "Run "})), "id");
    run(&mut s, "edit.pasteAsCode", json!({"node": n.0, "text": "cargo xtask ci\n"}));
    assert_eq!(s.graph.name(n), "Run `cargo xtask ci `");
}

#[test]
fn page_icon_and_banner() {
    let mut s = blank();
    let n = id(&run(&mut s, "node.add", json!({"name": "Garden"})), "id");
    run(&mut s, "node.setIcon", json!({"node": n.0, "icon": "🌱"}));
    run(&mut s, "node.setBanner", json!({"node": n.0, "color": "green"}));
    let node = s.graph.get(n).unwrap();
    assert_eq!((node.icon.as_deref(), node.banner.as_deref()), (Some("🌱"), Some("green")));
    assert!(s.run("node.setBanner", &json!({"node": n.0, "color": "plaid"})).is_err());
    run(&mut s, "node.setIcon", json!({"node": n.0, "icon": ""}));
    assert_eq!(s.graph.get(n).unwrap().icon, None);
    assert_eq!(s.graph.get(n).unwrap().banner.as_deref(), Some("green"), "the banner stays");
}

#[test]
fn import_markdown_builds_nodes() {
    let mut s = blank();
    let md = "# Groceries\n- Milk\n  - oat\n- [x] Bread #shop\n\n# Garden\n* Water the beds\n";
    let v = run(&mut s, "import.markdown", json!({"text": md}));
    assert_eq!(v["created"], json!(6));
    let lib = s.graph.sys.library;
    let kids: Vec<String> = s.graph.children(lib).iter().map(|n| s.graph.name(*n).to_string()).collect();
    assert_eq!(kids, ["Groceries", "Garden"]);
    let groceries = s.graph.children(lib)[0];
    assert!(s.graph.get(groceries).unwrap().heading);
    let milk = s.graph.children(groceries)[0];
    assert_eq!(s.graph.name(s.graph.children(milk)[0]), "oat");
    let bread = s.graph.children(groceries)[1];
    assert_eq!((s.graph.name(bread), s.graph.checkbox(bread)), ("Bread", Some(true)));
    assert!(s.graph.find_tag("shop").is_some());
}

#[test]
fn calendar_day_length() {
    let mut s = blank();
    let n = id(&run(&mut s, "node.add", json!({"name": "Plan"})), "id");
    run(&mut s, "view.set", json!({"node": n.0, "kind": "calendar"}));
    run(&mut s, "view.dayLength", json!({"node": n.0, "start": 8, "end": 18}));
    assert_eq!(s.graph.view_def(n).day_hours, Some((8, 18)));
    assert!(s.run("view.dayLength", &json!({"node": n.0, "start": 18, "end": 8})).is_err());
    run(&mut s, "view.dayLength", json!({"node": n.0}));
    assert_eq!(s.graph.view_def(n).day_hours, None);
}

#[test]
fn user_fields_offer_known_people() {
    let mut s = blank();
    run(&mut s, "field.create", json!({"name": "Assignee", "type": "user"}));
    let a = id(&run(&mut s, "node.add", json!({"name": "Fix fence"})), "id");
    run(&mut s, "field.add", json!({"node": a.0, "field": "Assignee", "value": "Robin"}));
    let names = s.graph.user_names();
    assert_eq!(names.len(), 2, "{names:?}");
    assert_eq!(names[1], "Robin");
}

#[test]
fn moving_the_first_top_level_node_up_stays_on_the_page() {
    // DIFFS N5.
    let mut s = blank();
    let page = id(&run(&mut s, "node.add", json!({"name": "Page"})), "id");
    run(&mut s, "node.add", json!({"name": "Before"}));
    run(&mut s, "nav.open", json!({"node": page.0}));
    let a = id(&run(&mut s, "node.add", json!({"name": "a", "parent": page.0})), "id");
    let b = id(&run(&mut s, "node.add", json!({"name": "b", "parent": page.0})), "id");
    run(&mut s, "focus.set", json!({"node": a.0}));
    assert!(s.run("node.moveUp", &json!({})).is_err());
    assert_eq!(s.graph.children(page), [a, b]);
    run(&mut s, "focus.set", json!({"node": b.0}));
    assert!(s.run("node.moveDown", &json!({})).is_err());
    assert_eq!(s.graph.children(page), [a, b]);
    run(&mut s, "node.moveUp", json!({}));
    assert_eq!(s.graph.children(page), [b, a]);
}

#[test]
fn enter_at_the_end_of_a_heading_makes_its_first_child() {
    // DIFFS N9.
    let mut s = blank();
    let h = id(&run(&mut s, "node.add", json!({"name": "Head"})), "id");
    run(&mut s, "node.toggleHeading", json!({"node": h.0}));
    run(&mut s, "focus.set", json!({"node": h.0, "caret": 4}));
    run(&mut s, "node.enter", json!({}));
    run(&mut s, "node.setName", json!({"name": "kid"}));
    assert_eq!(s.graph.children(h).len(), 1);
    assert_eq!(s.graph.name(s.graph.children(h)[0]), "kid");
}

#[test]
fn table_header_menu_moves_calculates_hides_and_deletes_columns() {
    // Spec 04 §2 header context menu (`163-table-header-menu.jpg`).
    let mut s = blank();
    let list = id(&run(&mut s, "node.add", json!({"name": "Costs"})), "id");
    for (name, n) in [("a", "3"), ("b", "5"), ("c", "")] {
        let item = id(&run(&mut s, "node.add", json!({"name": name, "parent": list.0})), "id");
        run(&mut s, "field.add", json!({"node": item.0, "field": "Price", "type": "number", "value": n}));
        run(&mut s, "field.add", json!({"node": item.0, "field": "Shop", "value": "Corner"}));
    }
    run(&mut s, "view.set", json!({"node": list.0, "kind": "table"}));
    run(&mut s, "view.addColumn", json!({"node": list.0, "field": "Price"}));
    run(&mut s, "view.addColumn", json!({"node": list.0, "field": "Shop"}));
    let price = s.graph.find_field("Price").unwrap();
    let shop = s.graph.find_field("Shop").unwrap();
    run(&mut s, "view.moveColumn", json!({"node": list.0, "field": "Shop", "direction": "left"}));
    assert_eq!(s.graph.view_columns(list), [shop, price]);
    assert!(s.run("view.moveColumn", &json!({"node": list.0, "field": "Shop", "direction": "left"})).is_err());
    for (calc, want) in [
        ("sum", "8"),
        ("average", "4"),
        ("median", "4"),
        ("min", "3"),
        ("max", "5"),
        ("count_all", "3"),
        ("count_values", "2"),
        ("count_unique", "2"),
    ] {
        let r = run(&mut s, "view.calculate", json!({"node": list.0, "field": "Price", "calc": calc}));
        assert_eq!(r["result"], want, "{calc}");
    }
    assert_eq!(s.graph.view_def(list).calcs, [(price, "count_unique".to_string())]);
    run(&mut s, "view.hideOtherColumns", json!({"node": list.0, "field": "Price"}));
    assert_eq!(s.graph.view_columns(list), [price]);
    let r = run(&mut s, "view.deleteColumn", json!({"node": list.0, "field": "Price"}));
    assert_eq!(r["values_removed"], 3);
    assert!(s.graph.children(list).iter().all(|c| s.graph.get(*c).unwrap().fields.iter().all(|e| e.field != price)));
}

#[test]
fn settings_change_time_format_week_start_and_survive_save() {
    let mut s = blank();
    assert_eq!(run(&mut s, "settings.get", json!({}))["time_format"], "24h");
    run(&mut s, "settings.set", json!({"key": "time_format", "value": "12h"}));
    run(&mut s, "settings.set", json!({"key": "week_start", "value": "sunday"}));
    run(&mut s, "settings.set", json!({"key": "show_recents", "value": false}));
    let all = run(&mut s, "settings.set", json!({"key": "url_embeds", "value": true}));
    assert_eq!(all["url_embeds"], true);
    assert_eq!(all["warn_referenced"], false);
    assert_eq!(s.graph.fmt_time(14 * 60 + 5), "2:05 PM");
    assert_eq!(s.graph.fmt_time(0), "12:00 AM");
    let sat = Date::new(2026, 10, 10).unwrap();
    assert_eq!(s.graph.week_start(sat), Date::new(2026, 10, 4).unwrap(), "a Sunday");
    assert!(!s.graph.settings.shows("recents"));
    assert!(s.graph.is_dirty());
    let back = Graph::from_json(&s.graph.to_json().unwrap()).unwrap();
    assert_eq!(back.settings, s.graph.settings);
    assert!(s.run("settings.set", &json!({"key": "time_format", "value": "13h"})).is_err());
    assert!(s.run("settings.set", &json!({"key": "nope", "value": true})).is_err());
    run(&mut s, "settings.set", json!({"key": "week_start", "value": "monday"}));
    assert_eq!(s.graph.week_start(sat), Date::new(2026, 10, 5).unwrap());
}

#[test]
fn building_blocks_are_promoted_or_hidden_in_menus() {
    // Spec 03 §5 Advanced options, building block.
    let mut s = blank();
    run(&mut s, "tag.create", json!({"name": "alpha"}));
    run(&mut s, "tag.create", json!({"name": "meeting"}));
    run(&mut s, "tag.setBaseType", json!({"tag": "meeting", "base": "meeting"}));
    run(&mut s, "tag.create", json!({"name": "agenda item"}));
    run(&mut s, "tag.create", json!({"name": "internal"}));
    let n = id(&run(&mut s, "node.add", json!({"name": "Standup"})), "id");
    run(&mut s, "tag.add", json!({"node": n.0, "tag": "meeting"}));
    run(&mut s, "tag.setBuildingBlock", json!({"tag": "agenda item", "promoteFor": ["meeting"]}));
    run(&mut s, "tag.setBuildingBlock", json!({"tag": "internal", "hide": true}));
    let names: Vec<String> = s.graph.menu_tags(n).iter().map(|t| s.graph.name(*t).to_string()).collect();
    assert_eq!(names.first().map(String::as_str), Some("agenda item"));
    assert!(!names.contains(&"internal".to_string()));
    assert!(s.graph.fuzzy("internal", 5).is_empty(), "hidden from Search and open");
    assert!(s.run("tag.setBuildingBlock", &json!({"tag": "alpha", "promoteFor": ["spaceship"]})).is_err());
}

#[test]
fn context_notes_live_on_the_reference_not_the_target() {
    // Spec 01 §3: Alt+Cmd+Down, notes for this context.
    let mut s = blank();
    let a = id(&run(&mut s, "node.add", json!({"name": "Recipe"})), "id");
    let b = id(&run(&mut s, "node.add", json!({"name": "Plan"})), "id");
    let r = id(&run(&mut s, "node.reference", json!({"target": a.0, "parent": b.0})), "id");
    run(&mut s, "node.contextNote", json!({"node": r.0, "text": "for Sunday lunch"}));
    assert_eq!(s.graph.get(r).unwrap().description, "for Sunday lunch");
    assert_eq!(s.graph.get(a).unwrap().description, "", "the original is untouched");
    assert_eq!(run(&mut s, "node.contextNote", json!({"node": r.0}))["note"], "for Sunday lunch");
    assert!(s.run("node.contextNote", &json!({"node": a.0, "text": "x"})).is_err());
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(s.graph.get(r).unwrap().description, "");
}

#[test]
fn opml_import_reads_checkboxes_tags_and_notes() {
    let mut s = blank();
    let r = run(
        &mut s,
        "import.opml",
        json!({"text": "<opml><body><outline text=\"Trip\" _note=\"in May\"><outline text=\"[x] Book train #task\"/></outline></body></opml>"}),
    );
    assert_eq!(r["created"], 2);
    let trip = s.graph.search("Trip")[0];
    assert_eq!(s.graph.get(trip).unwrap().description, "in May");
    let book = s.graph.children(trip)[0];
    assert_eq!(s.graph.name(book), "Book train");
    assert_eq!(s.graph.checkbox(book), Some(true));
    assert_eq!(s.graph.get(book).unwrap().tags.len(), 1);
    let out = run(&mut s, "export.opml", json!({"node": trip.0}));
    assert!(out["opml"].as_str().unwrap().contains("_note=\"in May\""));
}

#[test]
fn each_workspace_has_its_own_schema_and_trash() {
    // Spec 01 §7.
    let mut s = blank();
    let ws = id(&run(&mut s, "workspace.create", json!({"name": "Lab"})), "id");
    let lib = s.graph.workspace_library(ws);
    let note = id(&run(&mut s, "node.add", json!({"name": "Sample 1", "parent": lib.0})), "id");
    run(&mut s, "tag.add", json!({"node": note.0, "tag": "specimen"}));
    let tag = s.graph.find_tag("specimen").unwrap();
    assert_eq!(s.graph.parent(tag), Some(s.graph.workspace_schema(ws)));
    assert_eq!(s.graph.workspace_of(tag), ws);
    run(&mut s, "node.delete", json!({"node": note.0}));
    assert_eq!(s.graph.parent(note), Some(s.graph.workspace_trash(ws)));
    assert!(s.graph.in_trash(note));
    assert!(s.graph.search("Sample").is_empty());
    // Content dropped on the workspace itself lands in its Library.
    let other = id(&run(&mut s, "node.add", json!({"name": "Loose", "parent": ws.0})), "id");
    assert_eq!(s.graph.parent(other), Some(lib));
    for part in [lib, s.graph.workspace_schema(ws), s.graph.workspace_trash(ws)] {
        assert!(s.graph.is_system(part));
    }
    run(&mut s, "nav.open", json!({"node": lib.0}));
    assert_eq!(run(&mut s, "trash.empty", json!({}))["deleted"], 1);
    assert!(s.graph.get(note).is_none());
    // The main workspace's trash is untouched by that.
    let home = s.graph.sys.library;
    let main_note = id(&run(&mut s, "node.add", json!({"name": "Home note", "parent": home.0})), "id");
    run(&mut s, "node.delete", json!({"node": main_note.0}));
    assert_eq!(s.graph.parent(main_note), Some(s.graph.sys.trash));
}

#[test]
fn command_nodes_run_their_steps_as_one_undo_step() {
    // Spec 01 §10: command nodes.
    let mut s = blank();
    let c = id(
        &run(
            &mut s,
            "commandNode.create",
            json!({"name": "Make it a task", "steps": ["tag.add {\"tag\": \"task\"}", "node.toggleDone", "field.add {\"field\": \"Owner\", \"value\": \"Ada\"}"]}),
        ),
        "id",
    );
    assert_eq!(s.graph.parent(c), Some(s.graph.sys.schema));
    let n = id(&run(&mut s, "node.add", json!({"name": "Fix the gate"})), "id");
    let r = run(&mut s, "commandNode.run", json!({"command": "Make it a task", "node": n.0}));
    assert_eq!(r["ran"].as_array().unwrap().len(), 3);
    let node = s.graph.get(n).unwrap();
    assert_eq!(node.tags.len(), 1);
    assert_eq!(s.graph.checkbox(n), Some(false));
    assert_eq!(node.field_values(s.graph.find_field("Owner").unwrap()).len(), 1);
    run(&mut s, "edit.undo", json!({}));
    assert!(s.graph.get(n).unwrap().tags.is_empty(), "one undo step for the whole run");
    // A bad step rolls the whole run back.
    let bad = id(&run(&mut s, "commandNode.create", json!({"name": "Broken", "steps": ["tag.add {\"tag\": \"x\"}", "nope.nope"]})), "id");
    assert!(s.run("commandNode.run", &json!({"command": bad.0, "node": n.0})).is_err());
    assert!(s.graph.get(n).unwrap().tags.is_empty());
    let nested = id(&run(&mut s, "commandNode.create", json!({"name": "Loop", "steps": ["commandNode.run {\"command\": \"Loop\"}"]})), "id");
    assert!(s.run("commandNode.run", &json!({"command": nested.0, "node": n.0})).is_err(), "no nesting");
}

#[test]
fn context_fields_live_on_the_reference() {
    let mut s = blank();
    let a = id(&run(&mut s, "node.add", json!({"name": "Ada"})), "id");
    let b = id(&run(&mut s, "node.add", json!({"name": "Team"})), "id");
    let r = id(&run(&mut s, "node.reference", json!({"target": a.0, "parent": b.0})), "id");
    run(&mut s, "node.contextField", json!({"node": r.0, "field": "Role", "value": "Lead"}));
    let role = s.graph.find_field("Role").unwrap();
    assert_eq!(s.graph.get(r).unwrap().field_values(role).len(), 1);
    assert!(s.graph.get(a).unwrap().fields.is_empty(), "the person is untouched");
    run(&mut s, "node.contextField", json!({"node": r.0, "field": "Role", "value": ""}));
    assert!(s.graph.get(r).unwrap().fields.is_empty());
    assert!(s.run("node.contextField", &json!({"node": a.0, "field": "Role", "value": "x"})).is_err());
}

#[test]
fn held_search_results_wait_for_run_once() {
    // Spec 05 §2: results do not update while editing until Run once / Done.
    let mut s = blank();
    run(&mut s, "node.add", json!({"name": "apple pie"}));
    let q = id(&run(&mut s, "search.create", json!({"query": "apple"})), "id");
    run(&mut s, "search.hold", json!({"node": q.0}));
    run(&mut s, "node.add", json!({"name": "apple tart"}));
    run(&mut s, "search.addClause", json!({"node": q.0, "clause": "pie"}));
    assert_eq!(s.graph.view_items(q).len(), 1, "held");
    run(&mut s, "search.setQuery", json!({"node": q.0, "query": "apple"}));
    run(&mut s, "search.hold", json!({"node": q.0}));
    assert_eq!(s.graph.view_items(q).len(), 2, "Run once refreshes");
    run(&mut s, "search.hold", json!({"on": false}));
    run(&mut s, "node.add", json!({"name": "apple jam"}));
    assert_eq!(s.graph.view_items(q).len(), 3, "live again");
}
