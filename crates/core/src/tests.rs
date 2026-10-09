use crate::*;

fn graph_with(names: &[&str]) -> (Graph, Vec<NodeId>) {
    let mut g = Graph::new();
    let home = g.sys.library;
    let ids = names.iter().map(|n| g.create(home, None, n).unwrap()).collect();
    (g, ids)
}

fn names(g: &Graph, parent: NodeId) -> Vec<String> {
    g.children(parent).iter().map(|c| g.name(*c).to_string()).collect()
}

#[test]
fn create_and_order() {
    let (mut g, ids) = graph_with(&["a", "b", "c"]);
    let home = g.sys.library;
    g.create(home, Some(1), "x").unwrap();
    g.create(home, Some(99), "z").unwrap();
    assert_eq!(names(&g, home), ["a", "x", "b", "c", "z"]);
    assert_eq!(g.parent(ids[0]), Some(home));
    assert!(g.create(NodeId(9999), None, "nope").is_err());
}

#[test]
fn names_are_one_line() {
    let (mut g, ids) = graph_with(&["a"]);
    g.set_name(ids[0], "one\ntwo\r\nthree").unwrap();
    assert_eq!(g.name(ids[0]), "one two three");
}

#[test]
fn indent_outdent_move() {
    let (mut g, ids) = graph_with(&["a", "b", "c"]);
    let home = g.sys.library;
    assert!(g.indent(ids[0]).is_err(), "first child cannot indent");
    g.indent(ids[1]).unwrap();
    assert_eq!(names(&g, home), ["a", "c"]);
    assert_eq!(names(&g, ids[0]), ["b"]);
    assert!(g.is_expanded(ids[0]));
    g.outdent(ids[1]).unwrap();
    assert_eq!(names(&g, home), ["a", "b", "c"]);
    assert!(g.outdent(ids[0]).is_err(), "cannot outdent out of Home");
    g.move_sibling(ids[2], true).unwrap();
    assert_eq!(names(&g, home), ["a", "c", "b"]);
    assert!(g.move_sibling(ids[0], true).is_err());
    g.move_sibling(ids[0], false).unwrap();
    assert_eq!(names(&g, home), ["c", "a", "b"]);
}

#[test]
fn move_refuses_cycles_and_system_nodes() {
    let (mut g, ids) = graph_with(&["a"]);
    let child = g.create(ids[0], None, "child").unwrap();
    assert!(g.move_to(ids[0], child, None).is_err());
    assert!(g.move_to(ids[0], ids[0], None).is_err());
    assert!(g.move_to(g.sys.library, ids[0], None).is_err());
    assert!(g.trash(g.sys.journal).is_err());
}

#[test]
fn move_down_within_parent_lands_at_index() {
    let (mut g, ids) = graph_with(&["a", "b", "c", "d"]);
    let home = g.sys.library;
    g.move_to(ids[0], home, Some(3)).unwrap();
    assert_eq!(names(&g, home), ["b", "c", "a", "d"]);
}

#[test]
fn trash_and_restore() {
    let (mut g, ids) = graph_with(&["a", "b", "c"]);
    let home = g.sys.library;
    let kid = g.create(ids[1], None, "kid").unwrap();
    g.trash(ids[1]).unwrap();
    assert_eq!(names(&g, home), ["a", "c"]);
    assert!(g.in_trash(kid));
    assert!(g.search("kid").is_empty());
    assert_eq!(g.restore(ids[1]).unwrap(), home);
    assert_eq!(names(&g, home), ["a", "b", "c"]);
    g.trash(ids[2]).unwrap();
    assert_eq!(g.empty_trash().unwrap(), 1);
    assert!(g.get(ids[2]).is_none());
}

#[test]
fn restore_falls_back_to_home() {
    let (mut g, ids) = graph_with(&["a"]);
    let kid = g.create(ids[0], None, "kid").unwrap();
    g.trash(kid).unwrap();
    g.trash(ids[0]).unwrap();
    assert_eq!(g.restore(kid).unwrap(), g.sys.library);
}

#[test]
fn merge_into_previous() {
    let (mut g, ids) = graph_with(&["Hello ", "world"]);
    let kid = g.create(ids[1], None, "kid").unwrap();
    let caret = g.merge_into(ids[1], ids[0]).unwrap();
    assert_eq!(caret, 6);
    assert_eq!(g.name(ids[0]), "Hello world");
    assert_eq!(g.parent(kid), Some(ids[0]));
    assert!(g.get(ids[1]).is_none());
    assert!(g.merge_into(ids[0], kid).is_err());
}

#[test]
fn references_mirror_their_target() {
    let (mut g, ids) = graph_with(&["target", "holder"]);
    g.create(ids[0], None, "child of target").unwrap();
    let r = g.create_reference(ids[1], None, ids[0]).unwrap();
    assert_eq!(g.name(r), "target");
    assert_eq!(names(&g, r), ["child of target"]);
    g.set_name(r, "renamed").unwrap();
    assert_eq!(g.name(ids[0]), "renamed");
    // A child created under a reference belongs to the target.
    let c = g.create(r, None, "via ref").unwrap();
    assert_eq!(g.parent(c), Some(ids[0]));
    assert_eq!(g.references_to(ids[0]), vec![r]);
    // Deleting the reference leaves the original alone.
    g.trash(r).unwrap();
    assert_eq!(g.name(ids[0]), "renamed");
    // Reference to a reference points at the real node.
    let r2 = g.create_reference(ids[1], None, r).unwrap();
    assert_eq!(g.resolve(r2), ids[0]);
}

#[test]
fn supertags_fields_and_inheritance() {
    let (mut g, ids) = graph_with(&["buy milk"]);
    let status = g.create_field("Status", FieldType::Options).unwrap();
    g.field_def_mut(status).unwrap().default = Some(FieldValue::Choice("Todo".into()));
    let due = g.create_field("Due", FieldType::Date).unwrap();
    let base = g.create_tag("item", "grey").unwrap();
    g.tag_add_field(base, due).unwrap();
    let task = g.create_tag("#task", "pink").unwrap();
    g.tag_add_field(task, status).unwrap();
    g.tag_extend(task, base).unwrap();
    g.set_tag_checkbox(task, true).unwrap();
    assert!(g.create_tag("Task", "red").is_err(), "names are unique, case-insensitive");
    assert!(g.tag_extend(base, task).is_err(), "no cycles");
    assert_eq!(g.template_fields(task), vec![due, status]);

    g.add_tag(ids[0], task).unwrap();
    let n = g.get(ids[0]).unwrap();
    assert_eq!(n.done, Some(false), "checkbox supertag");
    assert_eq!(n.field_value(status), Some(&FieldValue::Choice("Todo".into())));
    assert_eq!(n.fields.len(), 2);

    g.set_field_text(ids[0], due, "2026-10-12").unwrap();
    assert_eq!(g.get(ids[0]).unwrap().field_value(due), Some(&FieldValue::Date(Date::new(2026, 10, 12).unwrap())));
    assert!(g.set_field_text(ids[0], due, "soon").is_err());
    g.set_field_text(ids[0], status, "Waiting").unwrap();
    assert!(g.field_def(status).unwrap().options.contains(&"Waiting".to_string()), "options collect new values");

    // Adding a field to the base template reaches nodes tagged with the child supertag.
    let notes = g.create_field("Notes", FieldType::Plain).unwrap();
    g.tag_add_field(base, notes).unwrap();
    assert_eq!(g.get(ids[0]).unwrap().fields.len(), 3);

    g.remove_tag(ids[0], task).unwrap();
    let n = g.get(ids[0]).unwrap();
    assert!(n.tags.is_empty());
    // Filled fields stay, empty template fields go.
    assert_eq!(n.fields.iter().map(|f| f.field).collect::<Vec<_>>(), vec![due, status]);
    assert_eq!(g.find_tag("TASK"), Some(task));
}

#[test]
fn field_value_parsing() {
    assert_eq!(FieldValue::parse(FieldType::Number, " 4.5 "), Ok(FieldValue::Number(4.5)));
    assert!(FieldValue::parse(FieldType::Number, "NaN").is_err());
    assert_eq!(FieldValue::parse(FieldType::Checkbox, "Yes"), Ok(FieldValue::Checkbox(true)));
    assert!(FieldValue::parse(FieldType::Checkbox, "maybe").is_err());
    assert!(FieldValue::parse(FieldType::Email, "nope").is_err());
    assert_eq!(FieldType::parse("URL"), Some(FieldType::Url));
}

#[test]
fn options_from_supertag_links_nodes() {
    let mut g = Graph::new();
    let person = g.create_tag("person", "green").unwrap();
    let owner = g.create_field("Owner", FieldType::OptionsFromTag).unwrap();
    g.field_def_mut(owner).unwrap().source_tag = Some(person);
    let n = g.create(g.sys.library, None, "project").unwrap();
    g.set_field_text(n, owner, "Sam").unwrap();
    let sam = g.tagged(person)[0];
    assert_eq!(g.name(sam), "Sam");
    assert_eq!(g.get(n).unwrap().field_value(owner), Some(&FieldValue::Node(sam)));
    let m = g.create(g.sys.library, None, "other").unwrap();
    g.set_field_text(m, owner, "sam").unwrap();
    assert_eq!(g.tagged(person).len(), 1, "existing node is reused");
    assert_eq!(g.references_to(sam).len(), 2);
}

#[test]
fn daily_notes() {
    let mut g = Graph::new();
    let d = Date::new(2026, 10, 10).unwrap();
    let day = g.day(d).unwrap();
    assert_eq!(g.name(day), "Sat, 10 Oct");
    assert_eq!(g.day(d).unwrap(), day, "found, not duplicated");
    let crumbs: Vec<&str> = g.ancestors(day).iter().map(|a| g.name(*a)).collect();
    assert_eq!(crumbs, ["Workspace", "Daily notes", "2026", "Week 41"]);
    assert!(g.get(day).unwrap().tags.contains(&g.sys.day_tag));
    // Days are kept in date order inside a week.
    let earlier = g.day(Date::new(2026, 10, 5).unwrap()).unwrap();
    assert_eq!(g.children(g.parent(day).unwrap()), &[earlier, day]);
    // A late-December day can belong to next year's week 1.
    let dec = g.day(Date::new(2024, 12, 30).unwrap()).unwrap();
    let crumbs: Vec<&str> = g.ancestors(dec).iter().map(|a| g.name(*a)).collect();
    assert_eq!(crumbs, ["Workspace", "Daily notes", "2025", "Week 1"]);
    assert_eq!(names(&g, g.sys.journal), ["2025", "2026"]);
}

#[test]
fn undo_redo_restores_exact_state() {
    let (mut g, ids) = graph_with(&["a", "b"]);
    let before = g.to_json().unwrap();
    g.begin();
    g.indent(ids[1]).unwrap();
    g.set_name(ids[0], "A!").unwrap();
    let c = g.create(ids[0], None, "new").unwrap();
    g.commit("indent");
    let after = g.to_json().unwrap();
    assert_eq!(g.undo().as_deref(), Some("indent"));
    assert!(g.get(c).is_none());
    // Expanded state is view state, not undone; compare the nodes only.
    let strip = |s: &str| serde_json::from_str::<serde_json::Value>(s).unwrap()["nodes"].clone();
    assert_eq!(strip(&g.to_json().unwrap()), strip(&before));
    assert_eq!(g.redo().as_deref(), Some("indent"));
    assert_eq!(strip(&g.to_json().unwrap()), strip(&after));
    assert!(!g.can_redo());
}

#[test]
fn abort_rolls_back_and_empty_steps_vanish() {
    let (mut g, ids) = graph_with(&["a"]);
    g.begin();
    g.set_name(ids[0], "changed").unwrap();
    g.abort();
    assert_eq!(g.name(ids[0]), "a");
    g.begin();
    g.commit("nothing");
    assert!(!g.can_undo());
}

#[test]
fn json_round_trip_and_repair() {
    let g = sample::sample(Date::new(2026, 10, 10).unwrap()).unwrap();
    let text = g.to_json().unwrap();
    let back = Graph::from_json(&text).unwrap();
    assert_eq!(back.to_json().unwrap(), text);

    // Dangling child ids and duplicate ownership are repaired, not fatal.
    let mut v: serde_json::Value = serde_json::from_str(&text).unwrap();
    let nodes = v["nodes"].as_array_mut().unwrap();
    let lib = nodes.iter_mut().find(|n| n["id"] == 2).unwrap();
    lib["children"].as_array_mut().unwrap().push(serde_json::json!(424242));
    let repaired = Graph::from_json(&v.to_string()).unwrap();
    assert!(!repaired.children(repaired.sys.library).contains(&NodeId(424242)));

    assert!(Graph::from_json("{}").is_err());
    assert!(Graph::from_json("not json").is_err());
}

#[test]
fn save_and_load_file() {
    let dir = std::env::temp_dir().join(format!("tanacraft-core-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("graph.json");
    let mut g = sample::sample(Date::new(2026, 1, 1).unwrap()).unwrap();
    g.save(&path).unwrap();
    assert!(!g.is_dirty());
    let back = Graph::load(&path).unwrap();
    assert_eq!(back.len(), g.len());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn markdown_and_json_export() {
    let g = sample::sample(Date::new(2026, 10, 10).unwrap()).unwrap();
    let garden = g.search("Community garden")[0];
    let md = export::to_markdown(&g, garden, false);
    assert!(md.starts_with("- Community garden #project\n"), "{md}");
    assert!(md.contains("  - Status:: In progress\n"), "{md}");
    assert!(md.contains("  - Owner:: Robin Ash\n"), "{md}");
    assert!(md.contains("    - [x] Order cedar boards #task\n"), "{md}");
    let day = g.find_day(Date::new(2026, 10, 10).unwrap()).unwrap();
    let md = export::to_markdown(&g, day, true);
    assert!(md.contains("- [[Community garden]] #project\n"), "{md}");
    let j = export::to_json_tree(&g, garden);
    assert_eq!(j["fields"]["Status"], "In progress");
    assert_eq!(j["tags"][0], "project");
}
