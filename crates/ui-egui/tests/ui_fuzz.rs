//! Fuzz-style test of the real UI: random keys, text and commands typed into the sample graph. The
//! UI must never panic (egui code runs outside the command guard) and the graph must stay sound.
//! While typing, the keyboard must never go nowhere: after each key an editor or a menu still has
//! it, or a line is selected, and typed text shows up somewhere (DIFFS N16).
//! Longer runs: `UI_FUZZ_SEEDS=200 cargo test -p tanacraft-ui-egui --test ui_fuzz --release`.

use egui::{Event, Key, Modifiers};
use egui_kittest::kittest::Queryable;
use serde_json::json;
use tanacraft_commands::Session;
use tanacraft_core::{Date, Graph, NodeId, sample};
use tanacraft_ui_egui::TanaApp;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        if n == 0 { 0 } else { (self.next() % n as u64) as usize }
    }
}

const KEYS: &[Key] = &[
    Key::Enter,
    Key::Tab,
    Key::Backspace,
    Key::Delete,
    Key::ArrowUp,
    Key::ArrowDown,
    Key::ArrowLeft,
    Key::ArrowRight,
    Key::Escape,
    Key::Home,
    Key::End,
    Key::Z,
    Key::B,
    Key::I,
    Key::Period,
    Key::Comma,
    Key::K,
    Key::E,
    Key::A,
    Key::D,
    Key::Space,
];
const TEXT: &[&str] = &["a", "Hello", " ", "#", "#ta", ">", "@", "/", "!", "x", "`", "**", "Due", "2026-10-11"];
const UI_CMDS: &[&str] = &[
    "ui.openInPanel",
    "ui.newTab",
    "ui.closeTab",
    "ui.singlePanel",
    "ui.toggleSidebar",
    "ui.searchOptions",
    "ui.configureSupertag",
    "ui.recents",
    "ui.settings",
    "ui.help",
];

fn sound(g: &Graph) -> Result<(), String> {
    for n in g.iter() {
        for c in &n.children {
            if g.parent(*c) != Some(n.id) {
                return Err(format!("{c} is listed under {} but owned by {:?}", n.id, g.parent(*c)));
            }
        }
        if n.id != g.sys.root {
            let p = n.parent.ok_or(format!("orphan {}", n.id))?;
            if !g.get(p).is_some_and(|o| o.children.contains(&n.id)) {
                return Err(format!("{} not listed by its owner {p}", n.id));
            }
        }
    }
    Ok(())
}

/// A text box has the keyboard (egui's Tab traversal can also focus buttons; typing there is not
/// typing into anything).
fn editing(ctx: &egui::Context) -> bool {
    ctx.memory(|m| m.focused()).is_some_and(|id| egui::text_edit::TextEditState::load(ctx, id).is_some())
}

fn run_seed(seed: u64, events: usize) -> egui_kittest::Harness<'static, TanaApp> {
    let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let today = Date::new(2026, 10, 10).unwrap_or_else(|| panic!("date"));
    let s = Session::with_today(sample::sample(today).unwrap_or_else(|e| panic!("{e}")), today);
    let mut app = TanaApp::new(s);
    app.mac = true;
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1200.0, 800.0)).build_ui_state(
        |ui, app: &mut TanaApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        app,
    );
    h.state_mut().exec("nav.today", json!({})).ok();
    h.step();
    let mut log = Vec::new();
    let typing_seeds = std::env::var("UI_FUZZ_TYPING").map(|v| v != "0").unwrap_or(true);
    for _ in 0..events {
        // Was the person typing? (an editor or an overlay box had the keyboard; one more frame
        // lets egui drop the focus of a box that just closed)
        h.step();
        let before_focus = editing(&h.ctx);
        let before_target = h.state().typing_target().is_some();
        let before_home = before_focus || h.state().overlay_open() || h.state().session.sel.is_some() || before_target;
        let before_graph = h.state().session.graph.to_json().unwrap_or_default();
        let before_text = h.state().pending_text();
        let action = r.below(12);
        let mut typed_text = "";
        log.push(format!(
            "  (focus {:?}, egui focus {before_focus}, sel {:?}, overlay {:?}, zoom {})",
            h.state().session.focus,
            h.state().session.sel,
            h.state().overlay_name(),
            h.state().session.zoom
        ));
        match action {
            10 | 11 => {
                // A click somewhere: buttons, chips, menus, rows.
                let pos = egui::pos2(r.below(1200) as f32, r.below(800) as f32);
                log.push(format!("click {pos:?}"));
                let ev = |pressed| Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
                h.input_mut().events.push(Event::PointerMoved(pos));
                h.input_mut().events.push(ev(true));
                h.input_mut().events.push(ev(false));
            }
            0..=4 => {
                let k = KEYS[r.below(KEYS.len())];
                let m = match r.below(5) {
                    0 => Modifiers { command: true, mac_cmd: true, ..Modifiers::NONE },
                    1 => Modifiers { shift: true, ..Modifiers::NONE },
                    2 => Modifiers { command: true, mac_cmd: true, shift: true, ..Modifiers::NONE },
                    _ => Modifiers::NONE,
                };
                log.push(format!("key {k:?} {m:?}"));
                h.input_mut().events.push(Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: m });
            }
            5..=7 => {
                let t = TEXT[r.below(TEXT.len())];
                typed_text = t;
                log.push(format!("text {t:?}"));
                h.input_mut().events.push(Event::Text(t.into()));
            }
            8 => {
                // Focus a random visible row (what a click does).
                let rows = h.state().session.rows();
                if !rows.is_empty() {
                    let id: NodeId = rows[r.below(rows.len())].id;
                    log.push(format!("focus {id}"));
                    h.state_mut().exec("select.clear", json!({})).ok();
                    h.state_mut().exec("focus.set", json!({"node": id.0, "caret": r.below(6)})).ok();
                }
            }
            _ => {
                let c = UI_CMDS[r.below(UI_CMDS.len())];
                log.push(format!("cmd {c}"));
                h.state_mut().exec(c, json!({})).ok();
            }
        }
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| h.step()));
        if caught.is_err() {
            panic!("seed {seed}: the UI panicked after:\n{}", log.iter().rev().take(15).rev().cloned().collect::<Vec<_>>().join("\n"));
        }
        if let Err(e) = sound(&h.state().session.graph) {
            panic!("seed {seed}: {e} after:\n{}", log.iter().rev().take(15).rev().cloned().collect::<Vec<_>>().join("\n"));
        }
        // The keyboard never goes nowhere while typing: after a key, an editor or overlay still
        // has it, or a node is selected (DIFFS N16). Typed text always lands somewhere.
        if typing_seeds && before_home && action <= 7 {
            for _ in 0..2 {
                h.step();
            }
            let app = h.state();
            let page = app.recents_page || app.settings_page || app.help_page;
            let home = editing(&h.ctx) || app.overlay_open() || app.session.sel.is_some() || app.typing_target().is_some() || page;
            let tail = || {
                log.iter()
                    .rev()
                    .take(std::env::var("UI_FUZZ_TAIL").ok().and_then(|v| v.parse().ok()).unwrap_or(12))
                    .rev()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            if !home && app.outline_shown() && !app.session.rows().is_empty() {
                let focused = h.query_all(egui_kittest::kittest::by()).find(|n| n.is_focused()).map(|n| format!("{n:?}")).unwrap_or_default();
                let app = h.state();
                panic!(
                    "seed {seed}: the keyboard was lost (focused widget {focused:?}, focus {:?}, zoom {}, status {:?}) after:\n{}",
                    app.session.focus,
                    app.session.zoom,
                    app.session.status,
                    tail()
                );
            }
            // (Spaces alone may be trimmed when saved.) Text typed while a line is selected, or
            // while nothing has the keyboard, goes into a line too (DIFFS N27).
            // A lone `**` is an empty format mark, which is never kept (DIFFS N19).
            // (The Recents, Settings and Help pages have no lines to type into.)
            let typed = !typed_text.trim().trim_matches('*').is_empty() && !page && (before_focus || before_target || app.outline_shown());
            // It shows in the text box that has the keyboard, or it changed the graph or an editor.
            let shown_in_box =
                h.query_all(egui_kittest::kittest::by()).any(|n| n.is_focused() && n.value().is_some_and(|v| v.contains(typed_text.trim())));
            let app = h.state();
            let landed = shown_in_box
                || app.session.graph.to_json().unwrap_or_default() != before_graph
                || app.pending_text() != before_text
                || app.overlay_open();
            if typed && !landed {
                panic!("seed {seed}: typed text vanished after:\n{}", tail());
            }
        }
    }
    h
}

#[test]
fn random_keys_never_crash_the_ui() {
    let seeds: u64 = std::env::var("UI_FUZZ_SEEDS").ok().and_then(|v| v.parse().ok()).unwrap_or(6);
    let events: usize = std::env::var("UI_FUZZ_EVENTS").ok().and_then(|v| v.parse().ok()).unwrap_or(150);
    let first: u64 = std::env::var("UI_FUZZ_FIRST").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    for seed in first..=seeds.max(first) {
        run_seed(seed, events);
    }
}

/// Quit at a random point (DIFFS N26): after any run of random keys, text and clicks, saving on
/// quit and loading the file again keeps every line's text and the text of every open editor.
#[test]
fn quitting_at_a_random_point_keeps_typed_text() {
    let seeds: u64 = std::env::var("UI_FUZZ_SEEDS").ok().and_then(|v| v.parse().ok()).unwrap_or(6);
    let dir = std::env::temp_dir().join(format!("tanacraft-quit-fuzz-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{e}"));
    for seed in 1..=seeds {
        let events = 20 + (Rng(seed | 1).next() % 120) as usize;
        let mut h = run_seed(seed + 1000, events);
        let editors = h.state().unsaved_text();
        let path = dir.join(format!("{seed}.json"));
        h.state_mut().session.path = Some(path.clone());
        h.state_mut().exec("ui.quit", json!({})).ok();
        let before = h.state().session.graph.clone();
        h.state_mut().save_on_quit().unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        let g = Graph::load(&path).unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        // (Empty format marks are never saved, DIFFS N19.)
        let clean = |s: &str| tanacraft_core::format::strip_empty(s, 0).0;
        for n in before.iter() {
            assert_eq!(g.get(n.id).map(|m| clean(&m.name)), Some(clean(&n.name)), "seed {seed}: node {} lost its text", n.id);
        }
        let file = g.to_json().unwrap_or_default();
        for t in editors {
            let quoted = serde_json::to_string(&t).unwrap_or_default();
            assert!(file.contains(quoted.trim_matches('"')), "seed {seed}: {t:?} from an open editor was not saved");
        }
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// A file with broken links (references to nothing, views naming missing fields) opens and every
/// view draws without a panic.
#[test]
fn damaged_files_draw_in_every_view() {
    let today = Date::new(2026, 10, 10).unwrap_or_else(|| panic!("date"));
    let g = sample::sample(today).unwrap_or_else(|e| panic!("{e}"));
    let mut file: serde_json::Value = serde_json::from_str(&g.to_json().unwrap_or_default()).unwrap_or_default();
    let lib = g.sys.library.0;
    if let Some(nodes) = file["nodes"].as_array_mut() {
        for n in nodes.iter_mut() {
            if n["id"] == lib {
                n["view"] = json!({"kind": "table", "columns": [999_994, 3], "date_field": 3, "sort": [{"by": {"by": "field", "field": 999_993}}], "group": {"by": "field", "field": 999_992}});
            }
        }
        let broken = json!({"id": 990_001, "name": "", "kind": {"type": "reference", "target": 999_997}, "parent": lib});
        nodes.push(broken);
        if let Some(c) = nodes.iter_mut().find(|n| n["id"] == lib).and_then(|l| l["children"].as_array_mut()) {
            c.push(json!(990_001));
        }
    }
    let graph = Graph::from_json(&file.to_string()).unwrap_or_else(|e| panic!("{e}"));
    let s = Session::with_today(graph, today);
    let mut app = TanaApp::new(s);
    app.mac = true;
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1200.0, 800.0)).build_ui_state(
        |ui, app: &mut TanaApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        app,
    );
    h.state_mut().exec("nav.open", json!({"node": lib})).ok();
    for kind in ["table", "cards", "board", "lanes", "list", "side_menu", "tabs", "calendar", "outline"] {
        h.state_mut().exec("view.set", json!({"node": lib, "kind": kind})).ok();
        for _ in 0..3 {
            h.step();
        }
    }
    h.state_mut().exec("view.expandAll", json!({})).ok();
    h.step();
}
