//! Drives the real UI with key and text events (headless, no renderer).

use egui::{Event, Key, Modifiers};
use serde_json::json;
use tanacraft_commands::Session;
use tanacraft_core::{Date, Graph};
use tanacraft_ui_egui::TanaApp;

type H<'a> = egui_kittest::Harness<'a, TanaApp>;

fn harness() -> H<'static> {
    let s = Session::with_today(Graph::new(), Date::new(2026, 10, 10).unwrap());
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
    for _ in 0..3 {
        h.step();
    }
    h
}

fn key(h: &mut H<'_>, k: Key, m: Modifiers) {
    h.input_mut().events.push(Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: m });
    for _ in 0..3 {
        h.step();
    }
}

fn text(h: &mut H<'_>, t: &str) {
    h.input_mut().events.push(Event::Text(t.into()));
    for _ in 0..3 {
        h.step();
    }
}

fn lines(h: &mut H<'_>) -> Vec<String> {
    let v = h.state_mut().session.run("graph.view", &json!({})).unwrap();
    v["lines"].as_array().unwrap().iter().map(|l| l.as_str().unwrap().to_string()).collect()
}

#[test]
fn typing_an_outline_with_the_keyboard() {
    let mut h = harness();
    let r = h.state_mut().exec("node.add", json!({"name": ""})).unwrap();
    h.state_mut().exec("focus.set", json!({"node": r["id"]})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    text(&mut h, "Groceries");
    key(&mut h, Key::Enter, Modifiers::NONE);
    text(&mut h, "Milk");
    key(&mut h, Key::Tab, Modifiers::NONE);
    key(&mut h, Key::Enter, Modifiers::NONE);
    text(&mut h, "Bread");
    key(&mut h, Key::Enter, Modifiers::NONE);
    text(&mut h, "Call mum");
    key(&mut h, Key::Tab, Modifiers { shift: true, ..Modifiers::NONE });
    assert_eq!(lines(&mut h), ["  - Groceries", "    - Milk", "    - Bread", "> - Call mum"]);

    // Backspace at the start merges into the node above.
    key(&mut h, Key::Home, Modifiers::NONE);
    key(&mut h, Key::Backspace, Modifiers::NONE);
    assert_eq!(lines(&mut h), ["  - Groceries", "    - Milk", ">   - BreadCall mum"]);

    // Cmd+Z undoes the merge; Cmd+Shift+Up moves a node.
    key(&mut h, Key::Z, Modifiers { command: true, mac_cmd: true, ..Modifiers::NONE });
    assert_eq!(lines(&mut h)[2], ">   - Bread");
    let bread = h.state().session.graph.search("Bread")[0].0;
    h.state_mut().exec("focus.set", json!({"node": bread})).unwrap();
    key(&mut h, Key::ArrowUp, Modifiers { command: true, mac_cmd: true, shift: true, ..Modifiers::NONE });
    assert_eq!(lines(&mut h)[1], ">   - Bread");

    // Typing `#task` then Enter applies (and creates) the supertag.
    text(&mut h, "#");
    assert!(h.state().picker.is_some());
    text(&mut h, "task");
    key(&mut h, Key::Enter, Modifiers::NONE);
    assert!(h.state().picker.is_none());
    let g = &h.state().session.graph;
    let bread = g.search("Bread")[0];
    assert_eq!(g.name(bread), "Bread");
    assert_eq!(g.get(bread).unwrap().tags, vec![g.find_tag("task").unwrap()]);
}

#[test]
fn command_line_runs_commands_and_opens_nodes() {
    let mut h = harness();
    key(&mut h, Key::K, Modifiers { command: true, mac_cmd: true, ..Modifiers::NONE });
    assert!(h.state().palette.is_some());
    text(&mut h, "go to today");
    key(&mut h, Key::Enter, Modifiers::NONE);
    assert!(h.state().palette.is_none());
    assert_eq!(h.state().session.graph.name(h.state().session.zoom), "Sat, 10 Oct");
}
