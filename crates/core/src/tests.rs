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
    // Invalid URLs and e-mails are kept and flagged, not refused.
    let bad = FieldValue::parse(FieldType::Email, "nope").unwrap();
    assert!(!bad.is_valid());
    assert!(FieldValue::parse(FieldType::Email, "a@b.org").unwrap().is_valid());
    assert!(!FieldValue::parse(FieldType::Url, "example.org").unwrap().is_valid());
    assert!(FieldValue::parse(FieldType::Url, "https://example.org").unwrap().is_valid());
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
    // Weeks newest first; week nodes carry #week.
    let y2026 = g.children(g.sys.journal)[1];
    assert_eq!(names(&g, y2026), ["Week 41"]);
    g.day(Date::new(2026, 10, 20).unwrap()).unwrap();
    assert_eq!(names(&g, y2026), ["Week 43", "Week 41"]);
    let week = g.children(y2026)[0];
    assert_eq!(g.get(week).unwrap().tags, vec![g.find_tag("week").unwrap()]);
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
    assert!(md.contains("  - Owner:: Robin Ash, Mia Chen\n"), "{md}");
    assert!(md.contains("    - [x] Order cedar boards #task\n"), "{md}");
    let day = g.find_day(Date::new(2026, 10, 10).unwrap()).unwrap();
    let md = export::to_markdown(&g, day, true);
    assert!(md.contains("- [[Community garden]] #project\n"), "{md}");
    let j = export::to_json_tree(&g, garden);
    assert_eq!(j["fields"]["Status"], "In progress");
    assert_eq!(j["fields"]["Owner"], serde_json::json!(["Robin Ash", "Mia Chen"]), "several values export as a list");
    assert_eq!(j["tags"][0], "project");
}

fn garden() -> Graph {
    sample::sample(Date::new(2026, 10, 10).unwrap()).unwrap()
}

#[test]
fn query_parse_describe_round_trip() {
    let g = garden();
    for text in [
        "#task AND TODO",
        "#task OR #meeting",
        "Status:\"In progress\" AND NOT DONE",
        "Due:* AND \"Effort (hours)\">2",
        "Due<2026-10-20",
        "CREATED:7 AND seeds",
        "\"seed swap\"",
        "/^Build\\s/ AND NOT /beds?$/i",
    ] {
        let q = Query::parse(&g, text).unwrap();
        let back = Query::parse(&g, &q.describe(&g)).unwrap();
        assert_eq!(q, back, "{text} -> {}", q.describe(&g));
    }
    assert_eq!(Query::parse(&g, "#task and todo").unwrap().clauses.len(), 3, "lowercase words are text");
    assert!(Query::parse(&g, "#nosuchtag").is_err());
    assert!(Query::parse(&g, "\"unclosed").is_err());
    assert!(Query::parse(&g, "a AND b OR c").is_err());
    assert!(Query::parse(&g, "CREATED:soon").is_err());
}

#[test]
fn regular_expressions_in_searches() {
    // UC-06.41.
    let g = garden();
    let names = |q: &str| {
        let mut v: Vec<String> = Query::parse(&g, q).unwrap().run(&g, NodeId(0)).iter().map(|n| g.name(*n).to_string()).collect();
        v.sort();
        v
    };
    assert_eq!(names("#task AND /^build\\s+\\w+ beds$/i"), ["Build four beds"]);
    assert!(names("#task AND /^build/").is_empty(), "case matters without i");
    assert!(Query::parse(&g, "/(unclosed/").is_err());
    assert!(Query::parse(&g, "\"/not a regex/\"").is_ok(), "quoted is text");
}

#[test]
fn queries_find_the_right_nodes() {
    let g = garden();
    let names = |q: &str| {
        let q = Query::parse(&g, q).unwrap();
        let mut v: Vec<String> = q.run(&g, NodeId(0)).iter().map(|n| g.name(*n).to_string()).collect();
        v.sort();
        v
    };
    assert_eq!(names("#task AND TODO"), ["Ask the hardware store about bulk compost", "Build four beds", "Fill with compost"]);
    assert_eq!(names("#task AND DONE"), ["Order cedar boards"]);
    assert_eq!(names("#item AND \"Effort (hours)\">2"), ["Build four beds", "Fill with compost"], "inherited supertag matches");
    assert_eq!(names("Due<2026-10-12"), ["Order cedar boards"]);
    assert_eq!(names("Status:\"in progress\" AND #task"), ["Build four beds"]);
    assert_eq!(names("#meeting OR #person"), ["Mia Chen", "Robin Ash", "Volunteer meetup"]);
    assert_eq!(names("rota AND NOT water"), Vec::<String>::new());
    assert_eq!(names("water AND NOT weekdays"), ["Agree on the water rota", "Soil is alive: feed it, do not just water it", "Water rota"]);
    assert!(names("").is_empty(), "an empty search finds nothing");
    let garden_id = g.search("Community garden")[0];
    assert_eq!(names(&format!("UNDER:{garden_id} AND #task")).len(), 3);
}

#[test]
fn search_nodes_are_live() {
    let mut g = garden();
    let open = g.search("Open tasks")[0];
    assert_eq!(g.view_items(open).len(), 3);
    let t = g.create(g.sys.library, None, "New task").unwrap();
    g.add_tag(t, g.find_tag("task").unwrap()).unwrap();
    assert_eq!(g.view_items(open).len(), 4, "new tagged node shows up");
    g.set_done(t, Some(true)).unwrap();
    assert_eq!(g.view_items(open).len(), 3, "checking it off removes it");
    // The search node itself and trashed nodes never show.
    let q = Query::parse(&g, "tasks").unwrap();
    g.set_query(open, q).unwrap();
    assert!(!g.view_items(open).contains(&open));
    assert!(g.set_query(g.sys.library, Query::default()).is_err());
}

#[test]
fn views_sort_group_filter_and_columns() {
    let mut g = garden();
    let open = g.search("Open tasks")[0];
    let due = g.find_field("Due").unwrap();
    let status = g.find_field("Status").unwrap();
    let groups = g.view_groups(open);
    assert_eq!(groups.len(), 1);
    let order: Vec<&str> = groups[0].items.iter().map(|i| g.name(*i)).collect();
    assert_eq!(order, ["Ask the hardware store about bulk compost", "Build four beds", "Fill with compost"], "sorted by due date");
    g.view_mut(open).unwrap().group = Some(GroupBy::Field(status));
    let groups = g.view_groups(open);
    let labels: Vec<&str> = groups.iter().map(|x| x.label.as_str()).collect();
    assert_eq!(labels, ["Not started", "In progress", "Done", "Not defined"], "option order, empty options and not set too");
    g.view_mut(open).unwrap().filter = Query::parse(&g, "\"Effort (hours)\">4").unwrap();
    let total: usize = g.view_groups(open).iter().map(|x| x.items.len()).sum();
    assert_eq!(total, 1);
    assert_eq!(g.view_columns(open)[..2], [status, due]);
    assert_eq!(g.calendar_field(open), Some(due));
    // Views also apply to plain children and to supertag pages.
    let beds = g.search("Raised beds")[0];
    g.view_mut(beds).unwrap().sort = vec![Sort { by: SortBy::Name, desc: true }];
    let names: Vec<&str> = g.view_groups(beds)[0].items.iter().map(|i| g.name(*i)).collect();
    assert_eq!(names, ["Order cedar boards", "Fill with compost", "Build four beds"]);
    let task = g.find_tag("task").unwrap();
    assert_eq!(g.view_items(task).len(), 4);
    assert_eq!(ViewKind::parse("Lanes"), Some(ViewKind::Lanes));
    assert_eq!(ViewKind::parse("side menu"), Some(ViewKind::SideMenu));
    assert_eq!(ViewKind::parse("board"), Some(ViewKind::Cards));
    assert_eq!(g.view_def(task).kind, ViewKind::Table, "supertag pages default to a table");
    g.view_mut(open).unwrap().columns = vec![status, due];
    let due = export::shown_value(&g, &FieldValue::Date(Date::new(2026, 10, 22).unwrap()));
    assert!(due.starts_with("Thu, 22 Oct"), "{due}");
    assert_eq!(g.display_line(open, g.search("Fill with compost")[0]), format!("Not started · {due}"));
    assert_eq!(g.view_filtered_out(open), 2);
}

#[test]
fn views_and_searches_survive_save_and_undo() {
    let mut g = garden();
    let open = g.search("Open tasks")[0];
    let text = g.to_json().unwrap();
    let back = Graph::from_json(&text).unwrap();
    assert_eq!(back.view_def(open).kind, ViewKind::Table);
    assert_eq!(back.query(open).unwrap().describe(&back), "#task AND TODO");
    g.begin();
    g.view_mut(open).unwrap().kind = ViewKind::Cards;
    g.commit("view");
    g.undo();
    assert_eq!(g.view_def(open).kind, ViewKind::Table);
}

#[test]
fn nested_groups_and_operators() {
    let mut g = garden();
    let names = |g: &Graph, q: &str| {
        let q = Query::parse(g, q).unwrap();
        let mut v: Vec<String> = q.run(g, NodeId(0)).iter().map(|n| g.name(*n).to_string()).collect();
        v.sort();
        v
    };
    assert_eq!(names(&g, "(#meeting OR #person) AND NOT Outdoors:Yes"), ["Mia Chen", "Robin Ash"]);
    assert_eq!(names(&g, "#task AND (DONE OR \"Effort (hours)\">5)"), ["Build four beds", "Order cedar boards"]);
    assert_eq!(names(&g, "IS:TAG AND meet"), ["meeting"]);
    assert_eq!(names(&g, "IS:SEARCH"), ["Open tasks", "Open tasks", "Task board", "Task calendar"]);
    assert!(names(&g, "HAS:FIELD AND ON:DAY").contains(&"Volunteer meetup".to_string()));
    assert_eq!(names(&g, "DATE:2026-10-12"), ["Ask the hardware store about bulk compost"]);
    assert!(names(&g, "IN:LIBRARY AND seed").contains(&"Seed list".to_string()));
    assert_eq!(names(&g, "DONE:7"), ["Order cedar boards"], "checked just now (in the sample)");
    let robin = g.search("Robin Ash")[0];
    let links = names(&g, &format!("LINKS:{robin}"));
    assert_eq!(links.len(), 2, "the Owner field and the inline reference: {links:?}");
    assert!(links.contains(&"Community garden".to_string()));
    for text in ["(#meeting OR #person) AND NOT Outdoors:Yes", "IS:TAG", "DATE:today", "HAS:FIELD AND ON:DAY"] {
        let q = Query::parse(&g, text).unwrap();
        assert_eq!(Query::parse(&g, &q.describe(&g)).unwrap(), q, "{text}");
    }
    assert!(Query::parse(&g, "(a OR b").is_err());
    assert!(Query::parse(&g, "a OR b)").is_err());
    assert!(Query::parse(&g, "IS:WHATEVER").is_err());
    assert!(Query::parse(&g, &"(".repeat(40)).is_err(), "depth is bounded");
    // Timestamps: editing touches `edited`; checking sets done time.
    let t = g.search("Fill with compost")[0];
    g.set_done(t, Some(true)).unwrap();
    assert!(g.get(t).unwrap().done_at.is_some());
    g.set_done(t, Some(false)).unwrap();
    assert!(g.get(t).unwrap().done_at.is_none());
    assert_eq!(names(&g, "EDITED:1 AND compost").len(), 2);
}

#[test]
fn fuzzy_search_ranks_good_matches_first() {
    let g = garden();
    let names: Vec<&str> = g.fuzzy("gam", 5).iter().map(|n| g.name(*n)).collect();
    assert!(names.is_empty() || names.iter().all(|n| n.to_lowercase().contains('g')));
    let names: Vec<&str> = g.fuzzy("comgar", 3).iter().map(|n| g.name(*n)).collect();
    assert_eq!(names.first().copied(), Some("Community garden"));
    let names: Vec<&str> = g.fuzzy("fwc", 3).iter().map(|n| g.name(*n)).collect();
    assert_eq!(names.first().copied(), Some("Fill with compost"), "word starts");
    assert!(g.fuzzy("zzzz", 5).is_empty());
    assert!(g.fuzzy("  ", 5).is_empty());
}

#[test]
fn date_values_link_to_day_nodes() {
    let mut g = garden();
    let due = g.find_field("Due").unwrap();
    let t = g.search("Build four beds")[0];
    g.set_field_text(t, due, "2026-11-03").unwrap();
    let day = g.find_day(Date::new(2026, 11, 3).unwrap()).expect("setting a date creates its day");
    assert_eq!(g.references_to(day), vec![t]);
}

#[test]
fn supertag_configuration_behaviours() {
    let mut g = garden();
    let task = g.find_tag("task").unwrap();
    let status = g.find_field("Status").unwrap();
    // Done-state mapping both ways.
    let t = g.create(g.sys.library, None, "Mapped").unwrap();
    g.add_tag(t, task).unwrap();
    g.set_done(t, Some(true)).unwrap();
    assert_eq!(g.get(t).unwrap().field_value(status), Some(&FieldValue::Choice("Done".into())));
    g.set_field_text(t, status, "In progress").unwrap();
    assert_eq!(g.checkbox(t), Some(false), "a non-done value unchecks");
    g.set_field_text(t, status, "Done").unwrap();
    assert_eq!(g.checkbox(t), Some(true));
    // Template children are copied; default child supertag tags new children.
    let meeting = g.find_tag("meeting").unwrap();
    let m = g.create(g.sys.library, None, "Standup").unwrap();
    g.add_tag(m, meeting).unwrap();
    let kids: Vec<&str> = g.children(m).iter().map(|c| g.name(*c)).collect();
    assert_eq!(kids, ["Agenda", "Notes"]);
    g.tag_add_template_node(meeting, "Decisions").unwrap();
    assert_eq!(g.children(m).len(), 3, "template changes reach nodes tagged earlier");
    g.tag_set_child_tag(meeting, Some(task)).unwrap();
    let c = g.create(m, None, "Action item").unwrap();
    assert!(g.get(c).unwrap().tags.contains(&task));
    // Auto-initialize: the date of the ancestor day node.
    let due = g.find_field("Due").unwrap();
    g.field_def_mut(due).unwrap().auto_init = Some(AutoInit::AncestorDay);
    let day = g.day(Date::new(2026, 10, 10).unwrap()).unwrap();
    let d = g.create(day, None, "Due today").unwrap();
    g.add_tag(d, task).unwrap();
    assert_eq!(g.get(d).unwrap().field_value(due), Some(&FieldValue::Date(Date::new(2026, 10, 10).unwrap())));
    // Number ranges.
    let effort = g.find_field("Effort (hours)").unwrap();
    g.field_def_mut(effort).unwrap().min = Some(0.0);
    g.field_def_mut(effort).unwrap().max = Some(40.0);
    // Out of range is kept and flagged (UC-06.25).
    assert!(g.set_field_text(t, effort, "41").is_ok());
    assert!(g.out_of_range(effort, &FieldValue::Number(41.0)));
    assert!(g.set_field_text(t, effort, "8").is_ok());
    assert!(!g.out_of_range(effort, &FieldValue::Number(8.0)));
    // Views, pins, usage.
    assert_eq!(g.tag_def(task).unwrap().views.len(), 2);
    assert_eq!(g.view_items(g.tag_def(task).unwrap().views[0]).len(), g.tagged(task).len());
    assert!(g.pinned().contains(&task));
    assert_eq!(g.field_usage(status).1, 2, "task and project");
    assert!(g.tag_set_base_type(task, Some("spaceship")).is_err());
    let day = g.find_day(Date::new(2026, 10, 10).unwrap()).unwrap();
    let related = g.related_for(day);
    assert_eq!(related.len(), 1);
    assert_eq!(g.name(related[0]), "Open tasks");
    assert_eq!(HideMode::parse("when empty"), Some(HideMode::WhenEmpty));
}

#[test]
fn inline_references_are_backlinks_and_export_as_names() {
    let mut g = garden();
    let garden_id = g.search("Community garden")[0];
    let n = g.create(g.sys.library, None, &format!("Ask about {} tomorrow", format::ref_token(garden_id.0))).unwrap();
    assert!(g.references_to(garden_id).contains(&n));
    assert_eq!(g.text(n), "Ask about Community garden tomorrow");
    assert_eq!(g.search("about community")[0], n, "search sees the name");
    assert!(export::to_markdown(&g, n, false).contains("Ask about [[Community garden]] tomorrow"));
    let q = Query::parse(&g, &format!("LINKS:{garden_id}")).unwrap();
    assert!(q.run(&g, NodeId(0)).contains(&n));
}

#[test]
fn timed_date_values() {
    use crate::model::FieldType;
    let v = FieldValue::parse(FieldType::Date, "2026-10-17 11:15-12:30").unwrap();
    assert_eq!(v, FieldValue::Timed { date: Date::new(2026, 10, 17).unwrap(), start: 675, end: 750 });
    assert_eq!(v.date(), Date::new(2026, 10, 17));
    let one_hour = FieldValue::parse(FieldType::Date, "2026-10-17 23:30").unwrap();
    assert_eq!(one_hour, FieldValue::Timed { date: Date::new(2026, 10, 17).unwrap(), start: 1410, end: 1440 });
    assert!(FieldValue::parse(FieldType::Date, "2026-10-17 12:00-11:00").is_err());
    assert!(FieldValue::parse(FieldType::Date, "2026-10-17 25:00").is_err());
    let g = Graph::new();
    assert_eq!(export::display_value(&g, &v), "2026-10-17 11:15-12:30");
    assert!(export::shown_value(&g, &v).ends_with("11:15 - 12:30"));
}

#[test]
fn base_type_query_matches_tags_of_that_type() {
    let g = garden();
    let names = |q: &str| {
        let q = Query::parse(&g, q).unwrap();
        let mut v: Vec<String> = q.run(&g, NodeId(0)).iter().map(|n| g.name(*n).to_string()).collect();
        v.sort();
        v
    };
    assert_eq!(names("TYPE:task AND DONE"), ["Order cedar boards"]);
    assert!(Query::parse(&g, "TYPE:spaceship").is_err());
}

#[test]
fn search_snippets_come_from_content() {
    let mut g = Graph::new();
    let lib = g.sys.library;
    let a = g.create(lib, None, "Notes").unwrap();
    g.create(a, None, "Pumpkins were a game-changer this year").unwrap();
    assert_eq!(export::snippet(&g, a, "game"), Some("Pumpkins were a game-changer this year".to_string()));
    assert_eq!(export::snippet(&g, a, "tomato"), None);
    assert_eq!(export::snippet(&g, a, ""), None);
}

#[test]
fn queries_read_as_sentences() {
    let g = garden();
    let q = Query::parse(&g, "#task AND NOT DONE AND Status:\"In progress\"").unwrap();
    let words: String = q.sentence(&g).into_iter().map(|(w, _)| w).collect();
    assert_eq!(words, "Search for nodes tagged #task and not done and where Status set to In progress");
    let tokens: Vec<String> = q.sentence(&g).into_iter().filter(|(_, t)| *t).map(|(w, _)| w).collect();
    assert_eq!(tokens, ["#task", "Status", "In progress"]);
}

#[test]
fn parent_is_the_day_on_day_pages_and_the_page_in_related_content() {
    // UC-01.15: tasks due on this day and overdue ones, on any day page.
    let mut g = Graph::new();
    let due = g.create_field("Due", FieldType::Date).unwrap();
    let who = g.create_field("Attendees", FieldType::Plain).unwrap();
    let lib = g.sys.library;
    let d10 = Date::new(2026, 10, 10).unwrap();
    let d12 = Date::new(2026, 10, 12).unwrap();
    let on12 = g.create(lib, None, "on the 12th").unwrap();
    let on10 = g.create(lib, None, "on the 10th").unwrap();
    g.set_field(on12, due, Some(FieldValue::Date(d12))).unwrap();
    g.set_field(on10, due, Some(FieldValue::Date(d10))).unwrap();
    let day12 = g.day(d12).unwrap();
    let s = g.create_search(day12, None, "due", Query::parse(&g, "Due:PARENT").unwrap()).unwrap();
    assert_eq!(g.query(s).unwrap().run(&g, s), [on12]);
    let late = g.create_search(day12, None, "late", Query::parse(&g, "Due<PARENT").unwrap()).unwrap();
    assert_eq!(g.query(late).unwrap().run(&g, late), [on10]);
    // UC-03.05: a related search on #person runs for each person page.
    let person = g.create_tag("person", "blue").unwrap();
    let robin = g.create(lib, None, "Robin").unwrap();
    let mia = g.create(lib, None, "Mia").unwrap();
    g.add_tag(robin, person).unwrap();
    g.add_tag(mia, person).unwrap();
    let m = g.create(lib, None, "Tool chat").unwrap();
    g.set_field(m, who, Some(FieldValue::Node(robin))).unwrap();
    let r = g.tag_add_related(person, "Met with", Query::parse(&g, "Attendees:PARENT").unwrap()).unwrap();
    assert_eq!(g.related_items(r, robin), [m]);
    assert!(g.related_items(r, mia).is_empty());
}

#[test]
fn query_field_value_operators_and_workspace() {
    // DIFFS N13: Field values (PARENT, GRANDPARENT, relative date, Set / Not Set, Defined) and
    // Workspace.
    let mut g = Graph::new();
    let lib = g.sys.library;
    let project = g.create_field("Project", FieldType::Plain).unwrap();
    let due = g.create_field("Due", FieldType::Date).unwrap();
    let p = g.create(lib, None, "Garden").unwrap();
    let a = g.create(lib, None, "a").unwrap();
    let b = g.create(lib, None, "b").unwrap();
    let c = g.create(lib, None, "c").unwrap();
    g.set_field(a, project, Some(FieldValue::Node(p))).unwrap();
    g.set_field(b, project, None).unwrap();
    let today = Date::today();
    g.set_field(c, due, Some(FieldValue::Date(today))).unwrap();
    let search = g.create_search(p, None, "s", Query::parse(&g, "Project:PARENT").unwrap()).unwrap();
    assert_eq!(g.query(search).unwrap().run(&g, search), [a]);
    let run = |g: &Graph, q: &str| Query::parse(g, q).unwrap().run(g, NodeId(0));
    assert_eq!(run(&g, "Project:DEFINED"), [a, b]);
    assert_eq!(run(&g, "Project:SET"), [a]);
    assert!(run(&g, "Project:UNDEFINED").contains(&c));
    assert!(run(&g, "Project:NOTSET").contains(&b));
    assert_eq!(run(&g, "Due:today"), [c]);
    let q = Query::parse(&g, "Project:GRANDPARENT AND Due:NOTSET").unwrap();
    assert_eq!(Query::parse(&g, &q.describe(&g)).unwrap(), q, "text form round-trips");
    let ws = g.create_workspace("Lab").unwrap();
    let lab = g.workspace_library(ws);
    let x = g.create(lab, None, "in lab").unwrap();
    assert_eq!(run(&g, &format!("WORKSPACE:{}", ws.0)), [x]);
    assert!(!run(&g, &format!("WORKSPACE:{}", g.sys.root.0)).contains(&x));
}

#[test]
fn html_export_is_a_nested_escaped_list() {
    let (mut g, ids) = graph_with(&["B <b>", "C"]);
    g.create(ids[0], None, "B1 & co").unwrap();
    assert_eq!(export::to_html(&g, &ids), "<p>B &lt;b&gt;</p><ul><li>B1 &amp; co</li></ul><p>C</p>");
}

#[test]
fn clipboard_flavours_carry_real_formatting() {
    // DIFFS N21: HTML gets <b>, plain text drops the marks; the copied line has no dash.
    let (mut g, ids) = graph_with(&["Mulch **now** please", "Ideas"]);
    assert_eq!(export::to_html(&g, &ids[..1]), "<p>Mulch <b>now</b> please</p>");
    assert_eq!(export::to_plain(&g, &ids[..1]), "Mulch now please\n");
    g.create(ids[1], None, "Tool library").unwrap();
    g.create(ids[1], None, "Seed *swap*").unwrap();
    assert_eq!(export::to_html(&g, &ids[1..]), "<p>Ideas</p><ul><li>Tool library</li><li>Seed <i>swap</i></li></ul>");
    assert_eq!(export::to_plain(&g, &ids[1..]), "Ideas\n- Tool library\n- Seed swap\n");
}

#[test]
fn date_spans_cover_their_days() {
    // Spec 03 §8 "Add end": a date range across days.
    let v = FieldValue::parse(FieldType::Date, "2026-10-15/2026-10-17").unwrap();
    let d = |n| Date::new(2026, 10, n).unwrap();
    assert_eq!(v, FieldValue::Span { start: d(15), end: d(17) });
    assert!(v.covers(d(16)) && !v.covers(d(18)));
    assert_eq!(export::display_value(&Graph::new(), &v), "2026-10-15/2026-10-17");
    assert!(FieldValue::parse(FieldType::Date, "2026-10-17/2026-10-15").is_err());
    assert!(FieldValue::parse(FieldType::Date, "2026-10-17/3026-10-15").is_err());
    let mut g = Graph::new();
    let f = g.create_field("Trip", FieldType::Date).unwrap();
    let n = g.create(g.sys.library, None, "Holiday").unwrap();
    g.set_field_text(n, f, "2026-10-15/2026-10-17").unwrap();
    assert_eq!(g.item_dates(n, Some(f)), [d(15), d(16), d(17)]);
    assert_eq!(Query::parse(&g, "Trip:2026-10-16").unwrap().run(&g, NodeId(0)), [n]);
    assert!(g.get(n).unwrap().has_date(d(17)));
}

#[test]
fn opml_round_trips_and_reads_other_outliners() {
    let src = r#"<?xml version="1.0"?><opml version="2.0"><head><title>x</title></head><body>
      <outline text="Groceries &amp; more" _note="for Sunday">
        <outline text="Milk"/>
        <outline title="Bread"></outline>
      </outline>
      <outline text='Call &#77;um'/>
    </body></opml>"#;
    let items = opml::parse(src);
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].text, "Groceries & more");
    assert_eq!(items[0].note, "for Sunday");
    assert_eq!(items[0].children.iter().map(|c| c.text.as_str()).collect::<Vec<_>>(), ["Milk", "Bread"]);
    assert_eq!(items[1].text, "Call Mum");
    let mut g = Graph::new();
    let lib = g.sys.library;
    let a = g.create(lib, None, "Groceries & more").unwrap();
    g.set_description(a, "for Sunday").unwrap();
    g.create(a, None, "Milk").unwrap();
    g.create(a, None, "Bread").unwrap();
    g.create(lib, None, "Call Mum").unwrap();
    let out = opml::to_opml(&g, g.children(lib), "Library");
    let again = opml::parse(&out);
    assert_eq!(
        again,
        items
            .iter()
            .map(|i| opml::Item {
                children: i.children.iter().map(|c| opml::Item { text: c.text.clone(), ..Default::default() }).collect(),
                ..i.clone()
            })
            .collect::<Vec<_>>()
    );
    // Hostile input never panics.
    for bad in ["<outline", "<outline text=\"unterminated>", "</outline></outline>", "<outline text=&amp;>", "&#xFFFFFFFF;"] {
        let _ = opml::parse(bad);
    }
}

#[test]
fn a_template_never_copies_into_its_own_subtree() {
    // Found by the fuzz test: tagging a node inside a template doubled the outline every time.
    let mut g = Graph::new();
    let tag = g.create_tag("meeting", "grey").unwrap();
    let t = g.tag_add_template_node(tag, "Agenda").unwrap();
    let inner = g.create(t, None, "inside the template").unwrap();
    g.add_tag(inner, tag).unwrap();
    let before = g.len();
    for _ in 0..10 {
        let n = g.create(inner, None, "x").unwrap();
        g.add_tag(n, tag).unwrap();
    }
    assert!(g.len() < before + 40, "grew to {} from {before}", g.len());
}

#[test]
fn scope_child_sibling_and_command_operators() {
    // Spec 05 §2 search operators: PARENTS / GRANDPARENTS DESCENDANTS, CHILD OF, SIBLING NAMED,
    // IS COMMAND.
    let mut g = Graph::new();
    let lib = g.sys.library;
    let project = g.create(lib, None, "Project").unwrap();
    let notes = g.create(project, None, "Notes").unwrap();
    let a = g.create(notes, None, "alpha").unwrap();
    let b = g.create(notes, None, "beta").unwrap();
    let outside = g.create(lib, None, "alpha elsewhere").unwrap();
    let search = g.create_search(notes, None, "s", Query::parse(&g, "alpha AND SCOPE:PARENT").unwrap()).unwrap();
    assert_eq!(g.query(search).unwrap().run(&g, search), [a]);
    let up2 = g.create_search(notes, None, "s2", Query::parse(&g, "alpha AND SCOPE:GRANDPARENT").unwrap()).unwrap();
    assert_eq!(g.query(up2).unwrap().run(&g, up2), [a]);
    let run = |g: &Graph, q: &str| Query::parse(g, q).unwrap().run(g, NodeId(0));
    assert!(run(&g, "alpha").contains(&outside));
    assert_eq!(run(&g, &format!("CHILD:{}", notes.0)), [a, b]);
    assert_eq!(run(&g, "SIBLING:beta"), [a]);
    let cmd = g.create_command_node(g.sys.root, "Tidy", &[]).unwrap();
    assert_eq!(run(&g, "IS:COMMAND"), [cmd]);
    for q in ["SCOPE:PARENT", "SIBLING:\"two words\"", "IS:COMMAND"] {
        let parsed = Query::parse(&g, q).unwrap();
        assert_eq!(Query::parse(&g, &parsed.describe(&g)).unwrap(), parsed, "{q}");
    }
}

#[test]
fn loading_strips_empty_format_marks() {
    let mut g = Graph::new();
    let a = g.create(g.sys.library, None, "please****").unwrap();
    let g2 = Graph::from_json(&g.to_json().unwrap()).unwrap();
    assert_eq!(g2.name(a), "please");
}

#[test]
fn markdown_export_has_descriptions_and_fields_in_place() {
    let mut g = Graph::new();
    let root = g.sys.library;
    let page = g.create(root, None, "Page").unwrap();
    g.set_description(page, "about the page").unwrap();
    let a = g.create(page, None, "a").unwrap();
    g.create(page, None, "b").unwrap();
    let f = g.create_field("Prio", crate::FieldType::Plain).unwrap();
    g.set_field_text(page, f, "High").unwrap();
    g.set_field_after(page, f, Some(a)).unwrap();
    let md = crate::export::to_markdown(&g, page, false);
    assert_eq!(md, "- Page\n  about the page\n  - a\n  - Prio:: High\n  - b\n");
}
