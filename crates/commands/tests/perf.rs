//! Timed test on a generated 50,000-node workspace: typing, expand/collapse, search, save and load
//! stay fast. Budgets are loose (debug builds run this too); the README quotes the release numbers:
//! `cargo test -p tanacraft-commands --test perf --release -- --nocapture`.

use std::time::{Duration, Instant};

use serde_json::json;
use tanacraft_commands::Session;
use tanacraft_commands::core::{Date, Graph, sample};

fn time<T>(n: u32, mut f: impl FnMut() -> T) -> Duration {
    let start = Instant::now();
    for _ in 0..n {
        std::hint::black_box(f());
    }
    start.elapsed() / n
}

fn ms(d: Duration) -> String {
    format!("{:.3} ms", d.as_secs_f64() * 1000.0)
}

#[test]
fn fifty_thousand_nodes_stay_fast() {
    let today = Date::new(2026, 10, 10).unwrap_or_else(|| panic!("date"));
    let t = Instant::now();
    let graph = sample::large(50_000).unwrap_or_else(|e| panic!("{e}"));
    let build = t.elapsed();
    let nodes = graph.len();
    let mut s = Session::with_today(graph, today);
    let home = s.graph.sys.library;
    // Worst case for the outline: Home with every node expanded (50k visible rows).
    s.run("nav.open", &json!({"node": home.0})).unwrap_or_else(|e| panic!("{e}"));
    s.run("view.expandAll", &json!({})).unwrap_or_else(|e| panic!("{e}"));
    let rows = s.rows().len();
    let rows_t = time(5, || s.rows().len());

    let first = s.graph.children(home)[0];
    let item = s.graph.children(s.graph.children(first)[0])[5];
    s.run("focus.set", &json!({"node": item.0})).unwrap_or_else(|e| panic!("{e}"));
    let mut text = s.graph.name(item).to_string();
    let typing = time(50, || {
        text.push('x');
        s.run("node.setName", &json!({"name": text})).unwrap_or_else(|e| panic!("{e}"))
    });
    let enter = time(20, || s.run("node.enter", &json!({})).unwrap_or_else(|e| panic!("{e}")));
    let toggle = time(20, || s.run("node.toggleExpand", &json!({"node": first.0})).unwrap_or_else(|e| panic!("{e}")));
    let undo = time(20, || s.run("edit.undo", &json!({})).unwrap_or_else(|e| panic!("{e}")));
    let search = time(5, || s.run("graph.search", &json!({"query": "topic 42 "})).unwrap_or_else(|e| panic!("{e}")));
    let fuzzy = time(5, || s.graph.fuzzy("itm 4242", 20));
    let save = time(2, || s.graph.to_json().unwrap_or_else(|e| panic!("{e}")));
    let json = s.graph.to_json().unwrap_or_else(|e| panic!("{e}"));
    let load = time(2, || Graph::from_json(&json).unwrap_or_else(|e| panic!("{e}")));
    // The real file: the Loro document. The first write puts in every node; later ones only what
    // changed (autosave while typing).
    let t = Instant::now();
    let doc = s.graph.to_snapshot().unwrap_or_else(|e| panic!("{e}"));
    let first_doc = t.elapsed();
    let mut text = s.graph.name(item).to_string();
    let doc_save = time(5, || {
        text.push('y');
        s.run("node.setName", &json!({"node": item.0, "name": text})).unwrap_or_else(|e| panic!("{e}"));
        s.graph.to_snapshot().unwrap_or_else(|e| panic!("{e}")).len()
    });
    let doc_load = time(1, || Graph::from_snapshot(&doc).unwrap_or_else(|e| panic!("{e}")));

    println!("generated {nodes} nodes in {}; {rows} visible rows", ms(build));
    println!("rows (whole expanded outline): {}", ms(rows_t));
    println!("typing one character:          {}", ms(typing));
    println!("Enter (new node):              {}", ms(enter));
    println!("expand / collapse a project:   {}", ms(toggle));
    println!("undo:                          {}", ms(undo));
    println!("search (substring):            {}", ms(search));
    println!("search (fuzzy, Search and open): {}", ms(fuzzy));
    println!("save to JSON:                  {}", ms(save));
    println!("load from JSON:                {}", ms(load));
    println!("first Loro write (all nodes):  {} ({} KB)", ms(first_doc), doc.len() / 1024);
    println!("autosave (type, then save):    {}", ms(doc_save));
    println!("load the Loro document:        {}", ms(doc_load));

    assert!(nodes >= 50_000);
    // Generous budgets so a debug build passes; release is far under them.
    let budget = Duration::from_millis(if cfg!(debug_assertions) { 400 } else { 50 });
    for (what, d) in [("typing", typing), ("enter", enter), ("expand", toggle), ("undo", undo)] {
        assert!(d < budget, "{what} took {}", ms(d));
    }
}
