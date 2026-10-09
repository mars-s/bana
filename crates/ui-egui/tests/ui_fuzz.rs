//! Fuzz-style test of the real UI: random keys, text and commands typed into the sample graph. The
//! UI must never panic (egui code runs outside the command guard) and the graph must stay sound.
//! Longer runs: `UI_FUZZ_SEEDS=200 cargo test -p tanacraft-ui-egui --test ui_fuzz --release`.

use egui::{Event, Key, Modifiers};
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

fn run_seed(seed: u64, events: usize) {
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
    for _ in 0..events {
        match r.below(12) {
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
                log.push(format!("text {t:?}"));
                h.input_mut().events.push(Event::Text(t.into()));
            }
            8 => {
                // Focus a random visible row (what a click does).
                let rows = h.state().session.rows();
                if !rows.is_empty() {
                    let id: NodeId = rows[r.below(rows.len())].id;
                    log.push(format!("focus {id}"));
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
    }
}

#[test]
fn random_keys_never_crash_the_ui() {
    let seeds: u64 = std::env::var("UI_FUZZ_SEEDS").ok().and_then(|v| v.parse().ok()).unwrap_or(6);
    let events: usize = std::env::var("UI_FUZZ_EVENTS").ok().and_then(|v| v.parse().ok()).unwrap_or(150);
    for seed in 1..=seeds {
        run_seed(seed, events);
    }
}
