//! Offscreen screenshots of the whole Bana window (wgpu), for README images and for checking UI
//! work without a visible window.
//!
//! `cargo run -p bana-ui-egui --example ui_shot -- script.jsonl` (or `cargo xtask shots`).
//!
//! The script is JSON lines, run in order:
//! - `{"size": [1440, 900]}` window size in points (any line; applies from the start)
//! - `{"empty": true}` start from an empty graph instead of the sample (any line)
//! - `{"theme": "light"}` or `{"theme": "dark"}`
//! - `{"cmd": "node.indent", "params": {...}}` run a command (also `ui.*` commands)
//! - `{"focus": "node name"}` focus the first node whose name contains the text
//! - `{"open": "node name"}` open (zoom into) that node
//! - `{"type": "text"}` type text; `{"key": "Enter", "shift": true, "mod": true}` press a key
//! - `{"steps": 3}` run extra frames; `{"shot": "docs/images/x.png"}` save a PNG (2x)
//!
//! "Today" is fixed to 2026-10-10 so screenshots are stable.

use egui::{Event, Key, Modifiers};
use serde_json::{Value, json};
use bana_commands::Session;
use bana_core::{Date, Graph, sample};
use bana_ui_egui::TanaApp;

type H<'a> = egui_kittest::Harness<'a, TanaApp>;

fn main() {
    let path = std::env::args().nth(1).expect("usage: ui_shot script.jsonl");
    let script = std::fs::read_to_string(&path).expect("read script");
    let lines: Vec<Value> = script
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with("//"))
        .map(|l| serde_json::from_str(l).expect("json line"))
        .collect();
    let today = Date::new(2026, 10, 10).expect("date");
    let graph = if lines.iter().any(|l| l.get("empty").is_some()) { Graph::new() } else { sample::sample(today).expect("sample") };
    let mut session = Session::with_today(graph, today);
    session.run("nav.today", &json!({})).expect("today");
    let mut app = TanaApp::new(session);
    app.mac = true;
    let size = lines
        .iter()
        .find_map(|l| l.get("size"))
        .and_then(|s| Some((s.get(0)?.as_f64()? as f32, s.get(1)?.as_f64()? as f32)))
        .unwrap_or((1440.0, 900.0));
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(size.0, size.1)).with_pixels_per_point(2.0).wgpu().build_ui_state(
        |ui, app: &mut TanaApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        app,
    );
    steps(&mut h, 4);
    for l in lines {
        if let Some(t) = l.get("theme").and_then(Value::as_str) {
            h.state_mut().dark = t != "light";
            h.state_mut().system_theme = false;
        } else if let Some(id) = l.get("cmd").and_then(Value::as_str) {
            let params = l.get("params").cloned().unwrap_or(json!({}));
            let r = h.state_mut().exec(id, params);
            println!("{id}: {}", r.map(|v| v.to_string()).unwrap_or_else(|e| format!("error: {e}")));
        } else if let Some(name) = l.get("focus").and_then(Value::as_str) {
            let id = find(&h, name);
            h.state_mut().exec("focus.set", json!({"node": id})).ok();
        } else if let Some(name) = l.get("open").and_then(Value::as_str) {
            let id = find(&h, name);
            h.state_mut().exec("nav.open", json!({"node": id})).ok();
        } else if let Some(text) = l.get("type").and_then(Value::as_str) {
            h.input_mut().events.push(Event::Text(text.to_string()));
        } else if let Some(k) = l.get("key").and_then(Value::as_str) {
            let key = Key::from_name(k).expect("key name");
            let flag = |n: &str| l.get(n).and_then(Value::as_bool).unwrap_or(false);
            let m = Modifiers { shift: flag("shift"), command: flag("mod"), mac_cmd: flag("mod"), ctrl: flag("ctrl"), alt: flag("alt") };
            h.input_mut().events.push(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: m });
        } else if let Some(n) = l.get("steps").and_then(Value::as_u64) {
            steps(&mut h, n as usize);
            continue;
        } else if let Some(p) = l.get("shot").and_then(Value::as_str) {
            steps(&mut h, 4);
            match h.render() {
                Ok(img) => {
                    img.save(p).expect("save png");
                    println!("saved {p}");
                }
                Err(e) => eprintln!("render failed: {e}"),
            }
            continue;
        }
        steps(&mut h, 3);
    }
}

fn find(h: &H<'_>, name: &str) -> u64 {
    let g = &h.state().session.graph;
    g.search(name).first().map(|n| n.0).unwrap_or_else(|| panic!("no node matching {name:?}"))
}

fn steps(h: &mut H<'_>, n: usize) {
    for _ in 0..n {
        h.step();
    }
}
