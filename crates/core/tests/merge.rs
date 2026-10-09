//! Two copies of a graph edited apart (offline), then merged both ways, end up the same, keep both
//! sides' work where both can stay, and stay sound. The file is a Loro document (crates/core/src/store.rs).

use tanacraft_core::{Date, FieldType, Graph, NodeId, sample};

fn sound(g: &Graph) {
    for n in g.iter() {
        for c in &n.children {
            assert_eq!(g.parent(*c), Some(n.id), "{c} listed under {} but owned elsewhere", n.id);
        }
        if n.id != g.sys.root {
            let p = n.parent.unwrap_or_else(|| panic!("orphan {}", n.id));
            assert!(g.get(p).is_some_and(|o| o.children.contains(&n.id)), "{} not listed by {p}", n.id);
        }
    }
    // Everything reachable from the root (no loops).
    let mut seen = std::collections::BTreeSet::new();
    let mut stack = vec![g.sys.root];
    while let Some(x) = stack.pop() {
        if seen.insert(x) {
            stack.extend(g.children(x).iter().copied());
        }
    }
    assert_eq!(seen.len(), g.len(), "some nodes are not reachable from the root");
}

/// Two copies of `g` (as if opened on two devices).
fn fork(g: &mut Graph) -> (Graph, Graph) {
    let bytes = g.to_snapshot().unwrap();
    (Graph::from_snapshot(&bytes).unwrap(), Graph::from_snapshot(&bytes).unwrap())
}

/// Exchanges changes both ways and checks both copies agree.
fn sync(a: &mut Graph, b: &mut Graph) {
    let (va, vb) = (a.version().unwrap(), b.version().unwrap());
    let to_b = a.updates_since(&vb).unwrap();
    let to_a = b.updates_since(&va).unwrap();
    a.merge(&to_a).unwrap();
    b.merge(&to_b).unwrap();
    let (ja, jb) = (nodes_json(a), nodes_json(b));
    assert_eq!(ja, jb, "the copies differ after merging");
    sound(a);
    sound(b);
}

fn nodes_json(g: &Graph) -> String {
    serde_json::to_string_pretty(&g.iter().collect::<Vec<_>>()).unwrap()
}

fn today() -> Date {
    Date::new(2026, 10, 10).unwrap()
}

fn find(g: &Graph, name: &str) -> NodeId {
    g.iter().find(|n| n.name == name).map(|n| n.id).unwrap_or_else(|| panic!("no node {name}"))
}

#[test]
fn snapshot_round_trip_keeps_everything() {
    let mut g = sample::sample(today()).unwrap();
    let before = nodes_json(&g);
    let bytes = g.to_snapshot().unwrap();
    let back = Graph::from_snapshot(&bytes).unwrap();
    assert_eq!(nodes_json(&back), before);
    assert_eq!(back.sys, g.sys);
    assert_eq!(back.settings, g.settings);
}

#[test]
fn concurrent_typing_in_one_line_keeps_both() {
    let mut g = Graph::new();
    let lib = g.sys.library;
    let n = g.create(lib, None, "garden").unwrap();
    let (mut a, mut b) = fork(&mut g);
    a.set_name(n, "big garden").unwrap();
    b.set_name(n, "garden shed").unwrap();
    sync(&mut a, &mut b);
    assert_eq!(a.name(n), "big garden shed");
}

#[test]
fn new_nodes_on_both_sides_get_different_ids_and_both_stay() {
    let mut g = Graph::new();
    let lib = g.sys.library;
    let (mut a, mut b) = fork(&mut g);
    let x = a.create(lib, None, "from a").unwrap();
    let y = b.create(lib, None, "from b").unwrap();
    assert_ne!(x, y);
    assert!(x.0 < (1 << 53) && y.0 < (1 << 53), "ids stay safe for JavaScript");
    sync(&mut a, &mut b);
    let names: Vec<&str> = a.children(lib).iter().map(|c| a.name(*c)).collect();
    assert!(names.contains(&"from a") && names.contains(&"from b"), "{names:?}");
}

#[test]
fn concurrent_moves_never_make_a_loop() {
    let mut g = Graph::new();
    let lib = g.sys.library;
    let x = g.create(lib, None, "x").unwrap();
    let y = g.create(lib, None, "y").unwrap();
    let (mut a, mut b) = fork(&mut g);
    a.move_to(x, y, None).unwrap(); // x under y
    b.move_to(y, x, None).unwrap(); // y under x
    sync(&mut a, &mut b);
    // One of the moves wins; both nodes are still there.
    assert!(a.get(x).is_some() && a.get(y).is_some());
    assert!(!(a.parent(x) == Some(y) && a.parent(y) == Some(x)));
}

#[test]
fn delete_on_one_side_and_edit_on_the_other_agree() {
    let mut g = Graph::new();
    let lib = g.sys.library;
    let n = g.create(lib, None, "draft").unwrap();
    let kid = g.create(n, None, "kid").unwrap();
    let (mut a, mut b) = fork(&mut g);
    a.delete_permanently(n).unwrap();
    b.set_name(n, "draft two").unwrap();
    b.create(kid, None, "grandkid").unwrap();
    sync(&mut a, &mut b);
    // The delete wins: the node, its children and the new grandchild are gone everywhere.
    assert!(a.get(n).is_none() && a.get(kid).is_none());
    assert!(!a.iter().any(|x| x.name == "grandkid"));
}

#[test]
fn trash_on_one_side_and_edit_on_the_other_keeps_the_edit_in_the_trash() {
    let mut g = Graph::new();
    let lib = g.sys.library;
    let n = g.create(lib, None, "note").unwrap();
    let (mut a, mut b) = fork(&mut g);
    a.trash(n).unwrap();
    b.set_name(n, "note, edited").unwrap();
    sync(&mut a, &mut b);
    assert_eq!(a.name(n), "note, edited");
    assert!(a.in_trash(n));
}

#[test]
fn concurrent_tags_and_fields_both_stay() {
    let mut g = Graph::new();
    let lib = g.sys.library;
    let n = g.create(lib, None, "task one").unwrap();
    let t1 = g.create_tag("task", "pink").unwrap();
    let t2 = g.create_tag("urgent", "red").unwrap();
    let due = g.create_field("Due", FieldType::Plain).unwrap();
    let who = g.create_field("Who", FieldType::Plain).unwrap();
    let (mut a, mut b) = fork(&mut g);
    a.add_tag(n, t1).unwrap();
    a.set_field_text(n, due, "Friday").unwrap();
    b.add_tag(n, t2).unwrap();
    b.set_field_text(n, who, "Robin").unwrap();
    sync(&mut a, &mut b);
    let node = a.get(n).unwrap();
    assert!(node.tags.contains(&t1) && node.tags.contains(&t2), "{:?}", node.tags);
    assert!(node.field_value(due).is_some() && node.field_value(who).is_some());
}

#[test]
fn concurrent_inserts_in_one_list_keep_both_in_place() {
    let mut g = Graph::new();
    let lib = g.sys.library;
    let first = g.create(lib, None, "first").unwrap();
    let last = g.create(lib, None, "last").unwrap();
    let (mut a, mut b) = fork(&mut g);
    a.create(lib, Some(1), "from a").unwrap();
    b.create(lib, Some(1), "from b").unwrap();
    sync(&mut a, &mut b);
    let kids = a.children(lib).to_vec();
    assert_eq!(kids.first(), Some(&first));
    assert_eq!(kids.last(), Some(&last));
    assert_eq!(kids.len(), 4);
}

#[test]
fn many_random_edits_on_both_sides_converge() {
    let mut g = sample::sample(today()).unwrap();
    let (mut a, mut b) = fork(&mut g);
    let mut seed = 7u64;
    let mut rnd = |n: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % n.max(1) as u64) as usize
    };
    for round in 0..6 {
        for side in [&mut a, &mut b] {
            for _ in 0..15 {
                let ids: Vec<NodeId> = side.iter().map(|n| n.id).filter(|id| !side.is_system(*id)).collect();
                let pick = ids[rnd(ids.len())];
                let other = ids[rnd(ids.len())];
                let _ = match rnd(5) {
                    0 => side.set_name(pick, &format!("{} r{round}", side.name(pick))),
                    1 => side.create(pick, None, "new").map(drop),
                    2 => side.move_to(pick, other, None),
                    3 => side.trash(pick),
                    _ => side.set_description(pick, "note"),
                };
            }
        }
        sync(&mut a, &mut b);
    }
    let _ = find(&a, "Library");
}

#[test]
fn an_old_json_file_loads_once_and_saves_as_a_document() {
    let dir = std::env::temp_dir().join(format!("tanacraft-merge-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("graph.json");
    let g = sample::sample(today()).unwrap();
    std::fs::write(&path, g.to_json().unwrap()).unwrap();
    let mut loaded = Graph::load(&path).unwrap();
    assert_eq!(nodes_json(&loaded), nodes_json(&g));
    assert!(path.with_extension("json-backup").exists(), "the JSON file is kept as a backup");
    loaded.save(&path).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    assert!(bytes.first() != Some(&b'{'), "saved as a Loro document");
    let again = Graph::load(&path).unwrap();
    assert_eq!(nodes_json(&again), nodes_json(&g));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn undo_after_a_save_and_reload_of_edits_is_kept() {
    let dir = std::env::temp_dir().join(format!("tanacraft-undo-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("g.tanacraft");
    let mut g = Graph::new();
    let lib = g.sys.library;
    g.begin();
    let n = g.create(lib, None, "keep").unwrap();
    g.commit("add");
    g.begin();
    g.set_name(n, "keep me").unwrap();
    g.commit("rename");
    g.save(&path).unwrap();
    g.undo();
    g.save(&path).unwrap();
    let back = Graph::load(&path).unwrap();
    assert_eq!(back.name(n), "keep");
    let _ = std::fs::remove_dir_all(&dir);
}
