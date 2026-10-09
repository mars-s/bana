//! Fuzz-style robustness test: random command sequences against a [`Session`], checking graph
//! invariants after every step (owners and children agree, no cycles, no orphans), that every
//! mutating command undoes and redoes exactly, and that save/load round-trips.
//!
//! Params come from each command's own params description, so new commands are fuzzed for free.
//! Longer runs: `FUZZ_SEEDS=500 FUZZ_STEPS=400 cargo test -p tanacraft-commands --test fuzz --release`.

use std::collections::BTreeSet;

use serde_json::{Value, json};
use tanacraft_commands::Session;
use tanacraft_commands::core::{Date, Graph, Kind, NodeId, sample};

/// Commands that touch the file system or other workspaces.
const SKIP: &[&str] = &["file.save", "file.open", "import.file"];

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        if n == 0 { 0 } else { (self.next() % n as u64) as usize }
    }
    fn chance(&mut self, pct: u64) -> bool {
        self.next() % 100 < pct
    }
    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.below(xs.len())]
    }
}

const WORDS: &[&str] = &[
    "",
    "a",
    "Hello world",
    "task",
    "#task",
    "#new tag",
    "@Alpha",
    "Status",
    "Prio",
    "Done",
    "today",
    "2026-10-10",
    "2026-10-11 09:30",
    "2026-10-10/2026-10-13",
    "x\ny",
    "**bold** and _it_",
    "🙂 emoji",
    "${name} - ${Status}",
    "TODO",
    "DONE",
    "#task AND TODO",
    "text:hello",
    "- one\n  - two\n- three",
    "https://example.com",
    "#ffcc00",
    "red",
    "12",
    "-3",
];

fn ids(s: &Session) -> Vec<NodeId> {
    s.graph.iter().map(|n| n.id).collect()
}

fn pick_node(s: &Session, r: &mut Rng) -> Value {
    let all = ids(s);
    if r.chance(5) {
        return json!(999_999);
    }
    // Bias toward what the person sees.
    let rows = s.rows();
    if !rows.is_empty() && r.chance(50) {
        return json!(r.pick(&rows).id.0);
    }
    json!(r.pick(&all).0)
}

fn pick_named(s: &Session, r: &mut Rng, field: bool) -> Value {
    let defs: Vec<NodeId> =
        s.graph.iter().filter(|n| if field { matches!(n.kind, Kind::Field(_)) } else { matches!(n.kind, Kind::Tag(_)) }).map(|n| n.id).collect();
    match r.below(4) {
        0 if !defs.is_empty() => json!(r.pick(&defs).0),
        1 if !defs.is_empty() => json!(s.graph.name(*r.pick(&defs))),
        2 => pick_node(s, r),
        _ => json!(*r.pick(WORDS)),
    }
}

/// Builds params for a command from its params description (`{"node"?: id, "name": string}`).
fn params(desc: &str, s: &Session, r: &mut Rng) -> Value {
    let mut out = serde_json::Map::new();
    let body = desc.split('}').next().unwrap_or("");
    let mut rest = body;
    while let Some(start) = rest.find('"') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('"') else { break };
        let key = &after[..end];
        let tail = &after[end + 1..];
        let optional = tail.starts_with('?');
        let Some(colon) = tail.find(':') else { break };
        let ty_and_more = &tail[colon + 1..];
        // The type runs to the next `, "` (enum strings contain no commas).
        let ty_end = ty_and_more.find(", \"").unwrap_or(ty_and_more.len());
        let ty = ty_and_more[..ty_end].trim();
        rest = &ty_and_more[ty_end..];
        if optional && r.chance(40) {
            continue;
        }
        let v = if ty.starts_with("[id]") {
            json!((0..1 + r.below(3)).map(|_| pick_node(s, r)).collect::<Vec<_>>())
        } else if ty.starts_with("[number]") {
            json!((0..r.below(3)).map(|_| r.below(3)).collect::<Vec<_>>())
        } else if ty.starts_with("[string]") {
            json!((0..r.below(4)).map(|_| *r.pick(WORDS)).collect::<Vec<_>>())
        } else if key == "tag" || key == "base" || key == "child" {
            pick_named(s, r, false)
        } else if key == "field" {
            pick_named(s, r, true)
        } else if ty.starts_with("id") {
            pick_node(s, r)
        } else if ty.starts_with("bool") {
            if r.chance(10) { Value::Null } else { json!(r.chance(50)) }
        } else if ty.starts_with("number") || ty.starts_with("chars") {
            json!(r.below(12))
        } else if let Some(q) = ty.strip_prefix('"') {
            let opts: Vec<&str> = q.split('"').next().unwrap_or("").split('|').map(str::trim).collect();
            if r.chance(85) { json!(*r.pick(&opts)) } else { json!(*r.pick(WORDS)) }
        } else if key == "date" {
            json!(*r.pick(&["today", "2026-10-11", "yesterday", "nope"]))
        } else if key == "step" {
            json!(*r.pick(&[-1, 1, 0]))
        } else {
            json!(*r.pick(WORDS))
        };
        out.insert(key.to_string(), v);
    }
    Value::Object(out)
}

/// Owners and children agree, every node reaches the root, nothing is listed twice.
fn check_invariants(g: &Graph, ctx: &str) {
    let root = g.sys.root;
    let all: BTreeSet<NodeId> = g.iter().map(|n| n.id).collect();
    for id in [g.sys.root, g.sys.library, g.sys.journal, g.sys.schema, g.sys.trash, g.sys.day_tag] {
        assert!(all.contains(&id), "{ctx}: built-in node {id} is gone");
    }
    let mut listed = BTreeSet::new();
    for n in g.iter() {
        for c in &n.children {
            assert!(all.contains(c), "{ctx}: {} lists missing child {c}", n.id);
            assert!(listed.insert(*c), "{ctx}: {c} is listed twice");
            assert_eq!(g.parent(*c), Some(n.id), "{ctx}: {c} is under {} but its owner is {:?}", n.id, g.parent(*c));
        }
    }
    for n in g.iter() {
        if n.id == root {
            assert!(n.parent.is_none(), "{ctx}: root has an owner");
            continue;
        }
        let p = n.parent.unwrap_or_else(|| panic!("{ctx}: orphan {} ({:?})", n.id, n.name));
        assert!(
            n.reference_target().is_none() || n.children.is_empty() && n.tags.is_empty(),
            "{ctx}: reference {} has its own children or tags",
            n.id
        );
        assert!(
            g.get(p).is_some_and(|o| o.children.contains(&n.id)) && g.children(p).contains(&n.id),
            "{ctx}: {} names owner {p}, which does not list it\n{:?}\n{:?}",
            n.id,
            n,
            g.get(p)
        );
        // No cycles: the owner chain reaches the root.
        let mut at = n.id;
        let mut steps = 0;
        while at != root {
            at = g.parent(at).unwrap_or_else(|| panic!("{ctx}: {} does not reach the root", n.id));
            steps += 1;
            assert!(steps <= all.len(), "{ctx}: cycle through {}", n.id);
        }
        if let Some(t) = n.reference_target() {
            let _ = t; // dangling references are allowed (shown as broken); they must not crash
        }
    }
}

fn first_diff(a: &str, b: &str) -> String {
    let (a, b): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
    let i = a.iter().zip(&b).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
    let from = i.saturating_sub(8);
    format!("before:\n{}\nafter:\n{}", a[from..(i + 4).min(a.len())].join("\n"), b[from..(i + 4).min(b.len())].join("\n"))
}

fn snapshot(g: &Graph) -> Value {
    json!(g.iter().collect::<Vec<_>>())
}

fn run_seed(seed: u64, steps: usize) {
    let today = Date::new(2026, 10, 10).unwrap_or_else(|| panic!("date"));
    let graph = if seed.is_multiple_of(3) { Graph::new() } else { sample::sample(today).unwrap_or_else(|e| panic!("sample: {e}")) };
    run_on(graph, seed, steps);
}

fn run_on(graph: Graph, seed: u64, steps: usize) {
    let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let today = Date::new(2026, 10, 10).unwrap_or_else(|| panic!("date"));
    let mut s = Session::with_today(graph, today);
    let specs: Vec<(&'static str, &'static str, bool)> =
        s.registry.all().iter().filter(|c| !SKIP.contains(&c.id)).map(|c| (c.id, c.params, c.mutates)).collect();
    let mut log: Vec<String> = Vec::new();
    let trace = std::env::var("FUZZ_TRACE").is_ok();
    for step in 0..steps {
        let (id, desc, mutates) = *r.pick(&specs);
        let v = params(desc, &s, &mut r);
        log.push(format!("{id} {v}"));
        if trace {
            eprintln!("seed {seed} step {step} ({} nodes): {id} {v}", s.graph.len());
        }
        if let Ok(dir) = std::env::var("FUZZ_DUMP")
            && env("FUZZ_DUMP_STEP", u64::MAX) == step as u64
            && env("FUZZ_DUMP_SEED", seed) == seed
        {
            let _ = std::fs::write(format!("{dir}/graph.json"), s.graph.to_json().unwrap_or_default());
            let _ = std::fs::write(format!("{dir}/cmd.txt"), format!("{id}={v}"));
            return;
        }
        let ctx = || format!("seed {seed} step {step}\n{}", log.iter().rev().take(12).rev().cloned().collect::<Vec<_>>().join("\n"));
        let before = snapshot(&s.graph);
        let result = s.run(id, &v);
        if let Err(e) = &result {
            assert!(!e.to_string().contains("crashed"), "{}: {e}", ctx());
            assert_eq!(snapshot(&s.graph), before, "{}: a failed command changed the graph", ctx());
        }
        check_invariants(&s.graph, &ctx());
        // Undo and redo of one mutating step land exactly on the states around it. Renames merge
        // with the previous rename of the same node, so they are skipped here.
        if mutates && result.is_ok() && id != "node.setName" {
            let after = snapshot(&s.graph);
            if after != before {
                s.graph.undo();
                assert_eq!(snapshot(&s.graph), before, "{}: undo did not restore the graph", ctx());
                check_invariants(&s.graph, &format!("{} (after undo)", ctx()));
                s.graph.redo();
                assert_eq!(snapshot(&s.graph), after, "{}: redo did not restore the graph", ctx());
            }
        }
        if step % 20 == 0 || step + 1 == steps {
            let text = s.graph.to_json().unwrap_or_else(|e| panic!("{}: save: {e}", ctx()));
            let back = Graph::from_json(&text).unwrap_or_else(|e| panic!("{}: load: {e}", ctx()));
            let (x, y) = (
                serde_json::to_string_pretty(&snapshot(&s.graph)).unwrap_or_default(),
                serde_json::to_string_pretty(&snapshot(&back)).unwrap_or_default(),
            );
            assert!(x == y, "{}: save/load changed the graph\n{}", ctx(), first_diff(&x, &y));
            let again = back.to_json().unwrap_or_default();
            if again != text {
                panic!("{}: save/load is not stable\n{}", ctx(), first_diff(&text, &again));
            }
        }
    }
    // Undoing everything never breaks the graph either.
    while s.graph.undo().is_some() {
        check_invariants(&s.graph, &format!("seed {seed} undo all"));
    }
}

fn env(k: &str, d: u64) -> u64 {
    std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d)
}

#[test]
fn random_command_sequences_keep_the_graph_sound() {
    let seeds = env("FUZZ_SEEDS", 40);
    let steps = env("FUZZ_STEPS", 250) as usize;
    let first = env("FUZZ_FIRST", 1);
    for seed in first..first + seeds {
        run_seed(seed, steps);
    }
}

/// Hostile files: shuffled owners and children (cycles, self-loops, dangling ids) load into a sound
/// graph.
#[test]
fn damaged_files_load_into_a_sound_graph() {
    let today = Date::new(2026, 10, 10).unwrap_or_else(|| panic!("date"));
    for seed in 1..env("FUZZ_DAMAGED", 200) {
        let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let g = sample::sample(today).unwrap_or_else(|e| panic!("sample: {e}"));
        let mut file: Value = serde_json::from_str(&g.to_json().unwrap_or_default()).unwrap_or_default();
        let nodes = file["nodes"].as_array_mut().unwrap_or_else(|| panic!("nodes"));
        for _ in 0..1 + r.below(6) {
            let i = r.below(nodes.len());
            let j = r.below(nodes.len());
            let other = nodes[j]["id"].clone();
            match r.below(7) {
                0 => nodes[i]["children"] = json!([other, 999_999]),
                1 => nodes[i]["parent"] = other,
                // Links to nodes that are not there, or are the wrong kind.
                2 => nodes[i]["tags"] = json!([other, 999_998]),
                3 => nodes[i]["kind"] = json!({"type": "reference", "target": 999_997}),
                4 => nodes[i]["fields"] = json!([{"field": other, "value": {"type": "node", "value": 999_996}}, {"field": 999_995, "value": null}]),
                5 => {
                    nodes[i]["view"] =
                        json!({"kind": "calendar", "columns": [999_994], "date_field": other, "sort": [{"by": {"by": "field", "field": 999_993}}]})
                }
                _ => {
                    let me = nodes[i]["id"].clone();
                    nodes[j]["children"] = json!([me.clone()]);
                    nodes[i]["children"] = json!([other]);
                }
            }
        }
        let text = file.to_string();
        if let Ok(g) = Graph::from_json(&text) {
            check_invariants(&g, &format!("damaged file seed {seed}"));
            // And the app keeps working on it: random commands, no crash, sound after each.
            run_on(g, seed, 120);
        }
    }
}
