//! Drives the real UI with key and text events (headless, no renderer).

use egui::{Event, Key, Modifiers};
use egui_kittest::kittest::Queryable;
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

    // Backspace at the start merges into the previous sibling (spec 02 §4), not the row above.
    key(&mut h, Key::Home, Modifiers::NONE);
    key(&mut h, Key::Backspace, Modifiers::NONE);
    assert_eq!(lines(&mut h), ["> - GroceriesCall mum", "    - Milk", "    - Bread"]);

    // Cmd+Z undoes the merge; Cmd+Shift+Up moves a node.
    key(&mut h, Key::Z, Modifiers { command: true, mac_cmd: true, ..Modifiers::NONE });
    assert_eq!(lines(&mut h)[2..], ["    - Bread".to_string(), "  - Call mum".to_string()]);
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

#[test]
fn every_view_renders_on_search_tag_and_plain_pages() {
    let today = Date::new(2026, 10, 10).unwrap();
    let s = Session::with_today(tanacraft_core::sample::sample(today).unwrap(), today);
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
    let g = &h.state().session.graph;
    let pages = [g.search("Open tasks")[0], g.find_tag("task").unwrap(), g.search("Raised beds")[0]];
    for page in pages {
        h.state_mut().exec("nav.open", json!({"node": page.0})).unwrap();
        for kind in ["outline", "table", "cards", "board", "lanes", "list", "side_menu", "tabs", "calendar"] {
            h.state_mut().exec("view.set", json!({"kind": kind})).unwrap();
            for _ in 0..3 {
                h.step();
            }
        }
    }
    // Quick add puts a node on today's page and keeps the current page open.
    h.state_mut().exec("ui.quickAdd", json!({})).unwrap();
    h.step();
    text(&mut h, "Water the seedlings #task");
    key(&mut h, Key::Enter, Modifiers::NONE);
    text(&mut h, "Label the trays");
    assert_eq!(h.state().quick_add.as_ref().map(Vec::len), Some(2), "Enter starts another line");
    key(&mut h, Key::E, Modifiers { command: true, mac_cmd: true, ..Modifiers::NONE });
    assert!(h.state().quick_add.is_none(), "Cmd+E saves and closes");
    assert_eq!(h.state().session.graph.search("Label the trays").len(), 1);
    assert_eq!(h.state().session.graph.search("Water the seedlings")[0], h.state().session.graph.search("Water the seedlings")[0]);
    assert_eq!(h.state().session.graph.name(h.state().session.graph.search("Label the trays")[0]), "Label the trays", "second line is its own node");
    let g = &h.state().session.graph;
    let added = g.search("Water the seedlings")[0];
    assert_eq!(g.name(g.parent(added).unwrap()), "Sat, 10 Oct");
    assert_eq!(h.state().session.zoom, pages[2]);
}

#[test]
fn shift_enter_and_bold_shortcut() {
    let mut h = harness();
    let r = h.state_mut().exec("node.add", json!({"name": ""})).unwrap();
    h.state_mut().exec("focus.set", json!({"node": r["id"]})).unwrap();
    h.step();
    text(&mut h, "make bold");
    key(&mut h, Key::B, Modifiers { command: true, mac_cmd: true, ..Modifiers::NONE });
    assert_eq!(h.state().session.graph.name(tanacraft_core::NodeId(r["id"].as_u64().unwrap())), "**make bold**");
    key(&mut h, Key::Enter, Modifiers { shift: true, ..Modifiers::NONE });
    assert_eq!(lines(&mut h), ["  - **make bold**", "> - "]);
}

#[test]
fn drag_a_bullet_onto_another_row() {
    use egui::{PointerButton, Pos2};
    let mut h = harness();
    for n in ["alpha", "beta", "gamma"] {
        h.state_mut().exec("node.add", json!({"name": n})).unwrap();
    }
    for _ in 0..3 {
        h.step();
    }
    let alpha = h.get_by_label("alpha").rect();
    let gamma = h.get_by_label("gamma").rect();
    let from = Pos2::new(alpha.left() - 12.0, alpha.center().y); // the bullet
    let to = Pos2::new(gamma.left() + 4.0, gamma.center().y);
    let ev = |h: &mut H<'_>, e: Event| {
        h.input_mut().events.push(e);
        h.step();
    };
    ev(&mut h, Event::PointerMoved(from));
    ev(&mut h, Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    for k in 1..=10 {
        let p = from + (to - from) * (k as f32 / 10.0);
        ev(&mut h, Event::PointerMoved(p));
    }
    ev(&mut h, Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    for _ in 0..3 {
        h.step();
    }
    assert_eq!(lines(&mut h), ["  - beta", "  - gamma", "  - alpha"]);
}

#[test]
fn shift_down_selects_and_tab_indents_the_selection() {
    let mut h = harness();
    for n in ["one", "two", "three"] {
        h.state_mut().exec("node.add", json!({"name": n})).unwrap();
    }
    let two = h.state().session.graph.search("two")[0].0;
    h.state_mut().exec("focus.set", json!({"node": two})).unwrap();
    h.step();
    key(&mut h, Key::ArrowDown, Modifiers { shift: true, ..Modifiers::NONE });
    assert_eq!(h.state().session.selected().len(), 1, "first press selects the current node");
    key(&mut h, Key::ArrowDown, Modifiers { shift: true, ..Modifiers::NONE });
    assert_eq!(h.state().session.selected().len(), 2);
    key(&mut h, Key::Tab, Modifiers::NONE);
    assert_eq!(lines(&mut h), ["  - one", "    - two", "    - three"]);
    key(&mut h, Key::Escape, Modifiers::NONE);
    assert!(h.state().session.sel.is_none());
}

#[test]
fn search_and_open_and_slash_search_node() {
    let today = Date::new(2026, 10, 10).unwrap();
    let s = Session::with_today(tanacraft_core::sample::sample(today).unwrap(), today);
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
    h.step();
    key(&mut h, Key::S, Modifiers { command: true, mac_cmd: true, ..Modifiers::NONE });
    assert!(h.state().search_open.is_some());
    text(&mut h, "comgar");
    key(&mut h, Key::Enter, Modifiers::NONE);
    assert_eq!(h.state().session.graph.name(h.state().session.zoom), "Community garden");
    // `/` on an empty line, "Search node", then a supertag: a "List of" search node in its place.
    let r = h.state_mut().exec("node.add", json!({"name": ""})).unwrap();
    h.state_mut().exec("focus.set", json!({"node": r["id"]})).unwrap();
    h.step();
    text(&mut h, "/");
    assert!(h.state().picker.is_some());
    key(&mut h, Key::Enter, Modifiers::NONE); // Search node
    text(&mut h, "task");
    key(&mut h, Key::Enter, Modifiers::NONE);
    let g = &h.state().session.graph;
    let list = g.search("List of task");
    assert_eq!(list.len(), 1);
    assert_eq!(g.view_items(list[0]).len(), 4);
}

#[test]
fn tabs_and_panels_keep_their_own_pages() {
    let today = Date::new(2026, 10, 10).unwrap();
    let s = Session::with_today(tanacraft_core::sample::sample(today).unwrap(), today);
    let mut app = TanaApp::new(s);
    app.mac = true;
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1400.0, 800.0)).build_ui_state(
        |ui, app: &mut TanaApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        app,
    );
    h.step();
    let lib = h.state().session.graph.sys.library;
    let garden = h.state().session.graph.search("Community garden")[0];
    h.state_mut().exec("nav.open", json!({"node": lib.0})).unwrap();
    // A second panel on the garden; the first panel keeps Library.
    h.state_mut().exec("focus.set", json!({"node": garden.0})).unwrap();
    key(&mut h, Key::M, Modifiers { command: true, mac_cmd: true, ..Modifiers::NONE });
    assert_eq!(h.state().panel_count(), 2);
    assert_eq!(h.state().session.zoom, garden);
    for _ in 0..3 {
        h.step();
    }
    assert_eq!(h.state().session.zoom, garden, "drawing other panels leaves the active one alone");
    assert_eq!(h.state().tabs[0].panels[0].zoom, lib);
    h.state_mut().single_panel();
    assert_eq!(h.state().panel_count(), 1);
    // A new tab on today; Mod+1 goes back.
    let day = h.state().session.graph.find_day(today).unwrap();
    h.state_mut().new_tab(day);
    assert_eq!(h.state().tabs.len(), 2);
    assert_eq!(h.state().session.zoom, day);
    key(&mut h, Key::Num1, Modifiers { command: true, mac_cmd: true, ..Modifiers::NONE });
    assert_eq!(h.state().tab, 0);
    assert_eq!(h.state().session.zoom, garden);
    key(&mut h, Key::W, Modifiers { command: true, mac_cmd: true, ..Modifiers::NONE });
    assert_eq!(h.state().tabs.len(), 1);
    assert_eq!(h.state().session.zoom, day);
    // Sidebar cycles full, mini, hidden.
    for want in [1, 2, 0] {
        key(&mut h, Key::Backslash, Modifiers { command: true, mac_cmd: true, ..Modifiers::NONE });
        assert_eq!(h.state().sidebar, want);
    }
}

#[test]
fn inline_reference_left_right_and_select_all() {
    let mut h = harness();
    let a = h.state_mut().exec("node.add", json!({"name": "Target note"})).unwrap();
    let b = h.state_mut().exec("node.add", json!({"name": ""})).unwrap();
    h.state_mut().exec("focus.set", json!({"node": b["id"]})).unwrap();
    h.step();
    text(&mut h, "see ");
    text(&mut h, "@");
    assert!(h.state().picker.is_some(), "@ mid-sentence opens the inline picker");
    text(&mut h, "target");
    key(&mut h, Key::Enter, Modifiers::NONE);
    let g = &h.state().session.graph;
    let bid = tanacraft_core::NodeId(b["id"].as_u64().unwrap());
    let aid = tanacraft_core::NodeId(a["id"].as_u64().unwrap());
    assert_eq!(g.text(bid), "see Target note");
    assert!(g.references_to(aid).contains(&bid));
    // Left at the start goes to the end of the line above.
    h.state_mut().exec("focus.set", json!({"node": bid.0, "caret": 0})).unwrap();
    h.step();
    key(&mut h, Key::ArrowLeft, Modifiers::NONE);
    assert_eq!(h.state().session.focus, Some(aid));
    assert_eq!(h.state().session.caret, "Target note".len());
    key(&mut h, Key::ArrowRight, Modifiers::NONE);
    assert_eq!(h.state().session.focus, Some(bid));
    // Cmd+A twice: text, then every bullet.
    let cmd = Modifiers { command: true, mac_cmd: true, ..Modifiers::NONE };
    key(&mut h, Key::A, cmd);
    key(&mut h, Key::A, cmd);
    assert_eq!(h.state().session.selected().len(), 2);
}
