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
    // Bold applies to the selection (Cmd+A selects the line's text first).
    key(&mut h, Key::A, Modifiers { command: true, mac_cmd: true, ..Modifiers::NONE });
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

#[test]
fn dragging_a_selected_bullet_moves_the_selection() {
    use egui::{PointerButton, Pos2};
    let mut h = harness();
    for n in ["one", "two", "three", "four"] {
        h.state_mut().exec("node.add", json!({"name": n})).unwrap();
    }
    let g = &h.state().session.graph;
    let (one, two) = (g.search("one")[0].0, g.search("two")[0].0);
    h.state_mut().exec("select.set", json!({"from": one, "to": two})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    let src = h.get_by_label("one").rect();
    let dst = h.get_by_label("four").rect();
    let from = Pos2::new(src.left() - 12.0, src.center().y);
    let to = Pos2::new(dst.left() + 4.0, dst.center().y);
    let ev = |h: &mut H<'_>, e: Event| {
        h.input_mut().events.push(e);
        h.step();
    };
    ev(&mut h, Event::PointerMoved(from));
    ev(&mut h, Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    for k in 1..=10 {
        ev(&mut h, Event::PointerMoved(from + (to - from) * (k as f32 / 10.0)));
    }
    ev(&mut h, Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    for _ in 0..3 {
        h.step();
    }
    assert_eq!(lines(&mut h), ["  - three", "  - four", "  - one", "  - two"]);
}

#[test]
fn create_new_recents_settings_and_copy_paste_references() {
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
    // Create new: an untitled page on today, caret in the title.
    h.state_mut().exec("node.createNew", json!({})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    text(&mut h, "Shed inventory");
    let zoom = h.state().session.zoom;
    assert_eq!(h.state().session.graph.name(zoom), "Shed inventory");
    // Recents and Settings are pages; navigating leaves them.
    h.state_mut().exec("ui.recents", json!({})).unwrap();
    h.step();
    assert!(h.state().recents_page);
    h.state_mut().exec("nav.today", json!({})).unwrap();
    assert!(!h.state().recents_page);
    h.state_mut().exec("ui.settings", json!({})).unwrap();
    h.step();
    assert!(h.state().settings_page);
    h.state_mut().exec("nav.home", json!({})).unwrap();
    // Copy a selected node, paste it after another: a reference to the same node.
    let g = &h.state().session.graph;
    let (ideas, reading) = (g.search("Ideas")[0].0, g.search("Seed list")[0].0);
    h.state_mut().exec("select.set", json!({"from": ideas})).unwrap();
    h.input_mut().events.push(Event::Copy);
    h.step();
    let clip = tanacraft_core::export::to_markdown(&h.state().session.graph, tanacraft_core::NodeId(ideas), false);
    h.state_mut().exec("focus.set", json!({"node": reading})).unwrap();
    h.step();
    h.input_mut().events.push(Event::Paste(clip));
    h.step();
    let g = &h.state().session.graph;
    assert_eq!(g.references_to(tanacraft_core::NodeId(ideas)).len(), 1);
    // Pasting again next to it skips the duplicate.
    let again = tanacraft_core::export::to_markdown(&h.state().session.graph, tanacraft_core::NodeId(ideas), false);
    h.input_mut().events.push(Event::Paste(again));
    h.step();
    assert_eq!(h.state().session.graph.references_to(tanacraft_core::NodeId(ideas)).len(), 1, "duplicates are skipped");
}

#[test]
fn long_lines_wrap_and_typing_never_adds_line_breaks() {
    let mut h = harness();
    let long = "word ".repeat(80);
    let r = h.state_mut().exec("node.add", json!({"name": long.trim()})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    let label = h.get_by_label(long.trim());
    let rect = label.rect();
    assert!(rect.height() > 40.0 && rect.width() < 1000.0, "wrapped label {rect:?}");
    // Editing: Enter is a command (new sibling), never a line break inside the node.
    h.state_mut().exec("focus.set", json!({"node": r["id"], "caret": 4})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    key(&mut h, Key::End, Modifiers::NONE);
    text(&mut h, "end");
    key(&mut h, Key::Enter, Modifiers { shift: true, ..Modifiers::NONE });
    let names: Vec<String> = h.state().session.graph.iter().map(|n| n.name.clone()).collect();
    assert!(names.iter().all(|n| !n.contains('\n')), "{names:?}");
    assert!(names.iter().any(|n| n.ends_with("wordend")), "{names:?}");
}

#[test]
fn descriptions_are_edited_in_place() {
    let mut h = harness();
    let r = h.state_mut().exec("node.add", json!({"name": "Plan"})).unwrap();
    let id = tanacraft_core::NodeId(r["id"].as_u64().unwrap());
    h.state_mut().exec("focus.set", json!({"node": id.0})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    // Ctrl+I opens the description editor under the name; Enter keeps it and returns to the name.
    key(&mut h, Key::I, Modifiers { ctrl: true, ..Modifiers::NONE });
    text(&mut h, "Why we do this");
    key(&mut h, Key::Enter, Modifiers::NONE);
    assert_eq!(h.state().session.graph.get(id).unwrap().description, "Why we do this");
    assert_eq!(h.state().session.focus, Some(id));
    // Down from the name enters the description; Up goes back.
    key(&mut h, Key::ArrowDown, Modifiers::NONE);
    text(&mut h, "!");
    key(&mut h, Key::ArrowUp, Modifiers::NONE);
    assert_eq!(h.state().session.graph.get(id).unwrap().description, "Why we do this!");
    assert_eq!(h.state().session.focus, Some(id));
}

#[test]
fn enter_in_a_field_value_adds_another_and_shift_tab_moves_it_out() {
    let mut h = harness();
    let r = h.state_mut().exec("node.add", json!({"name": "Trip"})).unwrap();
    let id = tanacraft_core::NodeId(r["id"].as_u64().unwrap());
    h.state_mut().exec("field.add", json!({"node": id.0, "field": "Who", "type": "plain"})).unwrap();
    h.state_mut().exec("nav.open", json!({"node": id.0})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    h.get_by_label("Empty").click();
    for _ in 0..3 {
        h.step();
    }
    text(&mut h, "Ada");
    key(&mut h, Key::Enter, Modifiers::NONE);
    text(&mut h, "Sam");
    key(&mut h, Key::Enter, Modifiers::NONE);
    text(&mut h, "a note, not a value");
    key(&mut h, Key::Tab, Modifiers { shift: true, ..Modifiers::NONE });
    let g = &h.state().session.graph;
    let who = g.find_field("Who").unwrap();
    let vals: Vec<String> = g.get(id).unwrap().field_values(who).iter().map(|v| tanacraft_core::export::display_value(g, v)).collect();
    assert_eq!(vals, ["Ada", "Sam"]);
    assert_eq!(g.name(g.children(id)[0]), "a note, not a value");
}

#[test]
fn field_name_tab_goes_to_the_value_and_enter_starts_a_node() {
    let mut h = harness();
    let r = h.state_mut().exec("node.add", json!({"name": "Trip"})).unwrap();
    let id = tanacraft_core::NodeId(r["id"].as_u64().unwrap());
    h.state_mut().exec("nav.open", json!({"node": id.0})).unwrap();
    let c = h.state_mut().exec("node.add", json!({"parent": id.0, "name": ""})).unwrap();
    h.state_mut().exec("focus.set", json!({"node": c["id"]})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    text(&mut h, ">");
    text(&mut h, "Budget");
    key(&mut h, Key::Tab, Modifiers::NONE);
    text(&mut h, "300");
    key(&mut h, Key::Enter, Modifiers::NONE);
    key(&mut h, Key::Escape, Modifiers::NONE);
    let g = &h.state().session.graph;
    let budget = g.find_field("Budget").unwrap();
    assert_eq!(g.get(id).unwrap().field_values(budget).iter().map(|v| tanacraft_core::export::display_value(g, v)).collect::<Vec<_>>(), ["300"]);
    // `>` then Enter: the field is added and a fresh node below takes the caret.
    let c = h.state_mut().exec("node.add", json!({"parent": id.0, "name": ""})).unwrap();
    h.state_mut().exec("focus.set", json!({"node": c["id"]})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    text(&mut h, ">");
    text(&mut h, "Place");
    key(&mut h, Key::Enter, Modifiers::NONE);
    text(&mut h, "Pack bags");
    let g = &h.state().session.graph;
    assert!(g.find_field("Place").is_some());
    assert!(g.children(id).iter().any(|n| g.name(*n) == "Pack bags"), "{:?}", g.children(id).iter().map(|n| g.name(*n)).collect::<Vec<_>>());
}

#[test]
fn cmd_enter_in_a_menu_creates_from_the_typed_text() {
    let mut h = harness();
    let r = h.state_mut().exec("node.add", json!({"name": "Gamma"})).unwrap();
    let id = tanacraft_core::NodeId(r["id"].as_u64().unwrap());
    h.state_mut().exec("tag.create", json!({"name": "task"})).ok();
    h.state_mut().exec("focus.set", json!({"node": id.0})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    key(&mut h, Key::End, Modifiers::NONE);
    text(&mut h, " #");
    text(&mut h, "ta");
    key(&mut h, Key::Enter, Modifiers { command: true, mac_cmd: true, ..Modifiers::NONE });
    let g = &h.state().session.graph;
    let tags: Vec<String> = g.get(id).unwrap().tags.iter().map(|t| g.name(*t).to_string()).collect();
    assert_eq!(tags, ["ta"], "a new tag, not the existing #task");
    assert_eq!(g.name(id), "Gamma");
}

#[test]
fn bold_with_a_collapsed_caret_carries_on_for_typing() {
    let mut h = harness();
    let r = h.state_mut().exec("node.add", json!({"name": "Note"})).unwrap();
    h.state_mut().exec("focus.set", json!({"node": r["id"]})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    key(&mut h, Key::End, Modifiers::NONE);
    text(&mut h, " ");
    key(&mut h, Key::B, Modifiers { command: true, mac_cmd: true, ..Modifiers::NONE });
    text(&mut h, "loud");
    let id = tanacraft_core::NodeId(r["id"].as_u64().unwrap());
    assert_eq!(h.state().session.graph.name(id), "Note **loud**");
}

// ---- DIFFS.md round 3, P1 regressions -------------------------------------------------------

fn page_with(h: &mut H<'_>, names: &[&str]) -> Vec<tanacraft_core::NodeId> {
    let ids: Vec<_> =
        names.iter().map(|n| tanacraft_core::NodeId(h.state_mut().exec("node.add", json!({"name": n})).unwrap()["id"].as_u64().unwrap())).collect();
    for _ in 0..3 {
        h.step();
    }
    ids
}

fn focus(h: &mut H<'_>, id: tanacraft_core::NodeId, caret: usize) {
    h.state_mut().exec("focus.set", json!({"node": id.0, "caret": caret})).unwrap();
    for _ in 0..3 {
        h.step();
    }
}

const CMD: Modifiers = Modifiers { command: true, mac_cmd: true, alt: false, ctrl: false, shift: false };

#[test]
fn diffs_p1_cmd_b_at_a_collapsed_caret_keeps_the_text() {
    let mut h = harness();
    let ids = page_with(&mut h, &["hello"]);
    focus(&mut h, ids[0], 5);
    key(&mut h, Key::B, CMD);
    text(&mut h, "Z");
    key(&mut h, Key::I, CMD);
    text(&mut h, "Y");
    assert_eq!(h.state().session.graph.name(ids[0]), "hello**Z*Y***");
}

#[test]
fn diffs_p1_gt_on_an_empty_line_makes_a_field_row() {
    let mut h = harness();
    let ids = page_with(&mut h, &["x"]);
    focus(&mut h, ids[0], 1);
    key(&mut h, Key::Enter, Modifiers::NONE);
    text(&mut h, ">");
    text(&mut h, "Prio");
    key(&mut h, Key::Tab, Modifiers::NONE);
    text(&mut h, "High");
    key(&mut h, Key::Enter, Modifiers::NONE);
    key(&mut h, Key::Escape, Modifiers::NONE);
    let g = &h.state().session.graph;
    assert_eq!(g.name(ids[0]), "x", "nothing typed into the line above");
    let prio = g.find_field("Prio").expect("field created");
    let owner = g.iter().find(|n| n.fields.iter().any(|e| e.field == prio)).expect("a node has the field");
    assert_eq!(owner.field_values(prio).iter().map(|v| tanacraft_core::export::display_value(g, v)).collect::<Vec<_>>(), ["High"]);
}

#[test]
fn diffs_p1_escape_selects_then_backspace_deletes() {
    let mut h = harness();
    let ids = page_with(&mut h, &["one", "two", "three"]);
    focus(&mut h, ids[1], 1);
    key(&mut h, Key::Escape, Modifiers::NONE);
    assert_eq!(h.state().session.sel, Some((ids[1], ids[1])));
    key(&mut h, Key::Backspace, Modifiers::NONE);
    let g = &h.state().session.graph;
    assert!(g.in_trash(ids[1]));
    // Escape then Shift+Up extends the selection.
    focus(&mut h, ids[2], 1);
    key(&mut h, Key::Escape, Modifiers::NONE);
    key(&mut h, Key::ArrowUp, Modifiers { shift: true, ..Modifiers::NONE });
    assert_eq!(h.state().session.selected(), [ids[0], ids[2]]);
}

#[test]
fn diffs_p1_copy_selected_nodes_fills_the_clipboard() {
    let mut h = harness();
    let ids = page_with(&mut h, &["B"]);
    h.state_mut().exec("node.add", json!({"name": "B1", "parent": ids[0].0})).unwrap();
    focus(&mut h, ids[0], 1);
    key(&mut h, Key::ArrowUp, Modifiers { shift: true, ..Modifiers::NONE });
    assert!(h.state().session.sel.is_some());
    h.input_mut().events.push(Event::Copy);
    h.step();
    let out = h.output().platform_output.commands.clone();
    let copied: Vec<String> = out
        .iter()
        .filter_map(|c| match c {
            egui::OutputCommand::CopyText(t) => Some(t.clone()),
            _ => None,
        })
        .collect();
    assert!(copied.iter().any(|t| t.contains("- B\n") && t.contains("  - B1")), "{copied:?}");
}

#[test]
fn diffs_p1_cmd_e_opens_quick_add_while_editing() {
    let mut h = harness();
    let ids = page_with(&mut h, &["x"]);
    focus(&mut h, ids[0], 1);
    key(&mut h, Key::E, CMD);
    assert!(h.state().quick_add.is_some());
    text(&mut h, "Quick one");
    key(&mut h, Key::E, CMD);
    let g = &h.state().session.graph;
    assert_eq!(g.name(ids[0]), "x");
    assert!(
        g.search("Quick one")
            .iter()
            .any(|n| g.ancestors(*n).iter().any(|a| matches!(g.get(*a).map(|x| &x.kind), Some(tanacraft_core::Kind::Day { .. }))))
    );
}

#[test]
fn diffs_p2_delete_moves_the_caret_to_the_start_of_the_next_line() {
    let mut h = harness();
    let ids = page_with(&mut h, &["one", "two", "three"]);
    focus(&mut h, ids[1], 1);
    key(&mut h, Key::Backspace, Modifiers { command: true, mac_cmd: true, shift: true, ..Modifiers::NONE });
    text(&mut h, "X");
    let g = &h.state().session.graph;
    assert_eq!((g.name(ids[0]), g.name(ids[2])), ("one", "Xthree"));
}

#[test]
fn diffs_p2_hash_then_cmd_enter_creates_and_applies() {
    let mut h = harness();
    let ids = page_with(&mut h, &["Gamma"]);
    focus(&mut h, ids[0], 5);
    text(&mut h, " #");
    text(&mut h, "Zebra");
    key(&mut h, Key::Enter, CMD);
    let g = &h.state().session.graph;
    assert_eq!(g.name(ids[0]), "Gamma");
    assert_eq!(g.get(ids[0]).unwrap().tags.iter().map(|t| g.name(*t)).collect::<Vec<_>>(), ["Zebra"]);
}

#[test]
fn diffs_p2_board_ends_with_a_not_set_column_without_counts() {
    let s = Session::with_today(tanacraft_core::sample::sample(Date::new(2026, 10, 10).unwrap()).unwrap(), Date::new(2026, 10, 10).unwrap());
    let mut app = TanaApp::new(s);
    app.mac = true;
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1800.0, 900.0)).build_ui_state(
        |ui, app: &mut TanaApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        app,
    );
    let tasks = h.state().session.graph.search("Open tasks")[0];
    h.state_mut().exec("nav.open", json!({"node": tasks.0})).unwrap();
    h.state_mut().exec("view.set", json!({"kind": "board"})).unwrap();
    h.state_mut().exec("view.group", json!({"by": "Status"})).unwrap();
    for _ in 0..4 {
        h.step();
    }
    h.get_by_label("Status not set");
    assert!(h.query_all_by_label("Done").count() >= 2, "the Done column (and the builder's Done button)");
}

#[test]
fn diffs_p2_command_line_lists_steps_for_the_focused_node_not_nodes() {
    let mut h = harness();
    let ids = page_with(&mut h, &["garden plan"]);
    focus(&mut h, ids[0], 0);
    key(&mut h, Key::K, CMD);
    text(&mut h, "add tag");
    key(&mut h, Key::Enter, Modifiers::NONE);
    assert_eq!(h.state().picker.as_ref().map(|p| p.mode), Some(tanacraft_ui_egui::PickMode::Tag));
}

#[test]
fn diffs_p2_supertags_overview_has_workspace_section_and_no_field_list() {
    let mut h = harness();
    h.state_mut().exec("tag.create", json!({"name": "book"})).unwrap();
    h.state_mut().exec("field.create", json!({"name": "Pages", "type": "number"})).unwrap();
    h.state_mut().exec("nav.supertags", json!({})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    assert!(h.query_by_label("Fields").is_none());
    assert!(h.query_by_label("Pages").is_none());
    assert!(h.query_all_by_label("Workspace").count() >= 2, "sidebar and the overview section");
}

#[test]
fn diffs_p2_configure_supertag_opens_in_its_own_panel() {
    let mut h = harness();
    let tag = h.state_mut().exec("tag.create", json!({"name": "book"})).unwrap()["tag"].as_u64().unwrap();
    h.state_mut().exec("nav.open", json!({"node": tag})).unwrap();
    h.state_mut().exec("ui.configureSupertag", json!({})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    assert_eq!(h.state().panel_count(), 2);
    assert!(h.state().session.config_view);
    h.get_by_label("Building blocks");
    h.get_by_label("Advanced options");
    h.state_mut().exec("ui.configureSupertag", json!({})).unwrap();
    h.step();
    assert_eq!(h.state().panel_count(), 1);
    assert!(!h.state().session.config_view);
}

#[test]
fn diffs_p2_query_builder_adds_nested_groups() {
    let mut h = harness();
    let lib = h.state().session.graph.sys.library.0;
    h.state_mut().exec("tag.create", json!({"name": "task"})).unwrap();
    let q = h.state_mut().exec("search.create", json!({"query": "#task", "parent": lib})).unwrap()["id"].as_u64().unwrap();
    h.state_mut().exec("nav.open", json!({"node": q})).unwrap();
    h.state_mut().query_open = true;
    for _ in 0..3 {
        h.step();
    }
    h.get_by_label("+ OR").click();
    for _ in 0..3 {
        h.step();
    }
    let g = &h.state().session.graph;
    let desc = g.query(tanacraft_core::NodeId(q)).unwrap().describe(g);
    assert_eq!(desc, "#task AND ()");
    h.get_by_label("Done");
    h.get_by_label("Run once");
}

#[test]
fn diffs_p3_tag_page_breadcrumb_reads_schema() {
    let mut h = harness();
    let tag = h.state_mut().exec("tag.create", json!({"name": "book"})).unwrap()["tag"].as_u64().unwrap();
    h.state_mut().exec("nav.open", json!({"node": tag})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    h.get_by_label("Schema");
}

#[test]
fn diffs_p3_square_search_button_and_day_nav_icons() {
    let mut h = harness();
    h.state_mut().exec("nav.today", json!({})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    h.get_by_label("Search").click();
    for _ in 0..3 {
        h.step();
    }
    assert!(h.state().search_open.is_some());
    h.state_mut().search_open = None;
    h.step();
    h.get_by_label("Next").click();
    for _ in 0..3 {
        h.step();
    }
    let g = &h.state().session.graph;
    assert_eq!(g.name(h.state().session.zoom), "Sun, 11 Oct");
}

#[test]
fn diffs_p3_hash_menu_filters_from_the_line() {
    let mut h = harness();
    h.state_mut().exec("tag.create", json!({"name": "task"})).unwrap();
    h.state_mut().exec("tag.create", json!({"name": "topic"})).unwrap();
    let ids = page_with(&mut h, &["Read"]);
    focus(&mut h, ids[0], 4);
    text(&mut h, " #");
    text(&mut h, "ta");
    assert_eq!(h.state().session.graph.name(ids[0]), "Read #ta", "the typed text stays in the line");
    assert_eq!(h.state().picker.as_ref().map(|p| p.query.clone()), Some("ta".to_string()));
    // Escape closes the menu and leaves the text.
    key(&mut h, Key::Escape, Modifiers::NONE);
    assert!(h.state().picker.is_none());
    assert_eq!(h.state().session.graph.name(ids[0]), "Read #ta");
}

#[test]
fn calendar_time_grid_drag_to_create_and_resize() {
    use egui::{PointerButton, Pos2};
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1400.0, 1000.0)).build_ui_state(
        |ui, app: &mut TanaApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        {
            let mut a = TanaApp::new(Session::with_today(Graph::new(), Date::new(2026, 10, 10).unwrap()));
            a.mac = true;
            a
        },
    );
    let s = h.state_mut();
    s.exec("tag.create", json!({"name": "event"})).unwrap();
    s.exec("tag.addField", json!({"tag": "event", "field": "When", "type": "date"})).unwrap();
    let lib = s.session.graph.sys.library.0;
    let q = s.exec("search.create", json!({"query": "#event", "parent": lib})).unwrap()["id"].as_u64().unwrap();
    s.exec("nav.open", json!({"node": q})).unwrap();
    s.exec("view.set", json!({"kind": "calendar"})).unwrap();
    s.exec("view.dateField", json!({"field": "When"})).unwrap();
    s.exec("view.calendarMode", json!({"mode": "week"})).unwrap();
    s.query_open = false;
    for _ in 0..4 {
        h.step();
    }
    let grid = h.get_by_label("Time grid").rect();
    // Week of Mon 5 Oct; Wednesday is column 2. Columns start 50 points in.
    let col_w = ((grid.width() - 50.0) / 7.0).floor();
    let x = grid.left() + 50.0 + col_w * 2.5;
    let y = |min: f32| grid.top() + min / 60.0 * 40.0;
    let drag = |h: &mut H<'_>, from: Pos2, to: Pos2| {
        let ev = |h: &mut H<'_>, e: Event| {
            h.input_mut().events.push(e);
            h.step();
        };
        ev(h, Event::PointerMoved(from));
        ev(h, Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
        for k in 1..=8 {
            ev(h, Event::PointerMoved(from + (to - from) * (k as f32 / 8.0)));
        }
        ev(h, Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
        for _ in 0..3 {
            h.step();
        }
    };
    drag(&mut h, Pos2::new(x, y(11.0 * 60.0 + 2.0)), Pos2::new(x, y(12.5 * 60.0)));
    assert!(h.state().cal_new.as_ref().is_some_and(|e| (e.start, e.end) == (660, 750)), "{:?}", h.state().cal_new);
    text(&mut h, "Planting day");
    key(&mut h, Key::Enter, Modifiers::NONE);
    let g = &h.state().session.graph;
    let ev = g.search("Planting day")[0];
    let when = g.find_field("When").unwrap();
    assert_eq!(tanacraft_core::export::display_value(g, g.get(ev).unwrap().field_value(when).unwrap()), "2026-10-07 11:00-12:30");
    // Drag the bottom edge from 12:30 to 13:30.
    drag(&mut h, Pos2::new(x, y(12.5 * 60.0) - 4.0), Pos2::new(x, y(13.5 * 60.0)));
    let g = &h.state().session.graph;
    assert_eq!(tanacraft_core::export::display_value(g, g.get(ev).unwrap().field_value(when).unwrap()), "2026-10-07 11:00-13:30");
}

#[test]
fn switching_workspace_resets_tabs_and_shows_inbox_in_the_sidebar() {
    let dir = std::env::temp_dir().join(format!("tanacraft-ui-ws-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut h = harness();
    h.state_mut().session.path = Some(dir.join("graph.json"));
    h.state_mut().exec("node.capture", json!({"name": "idea"})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    h.get_by_label("Inbox");
    h.state_mut().exec("ui.openInPanel", json!({})).unwrap();
    assert_eq!(h.state().panel_count(), 2);
    h.state_mut().exec("workspace.create", json!({"name": "Second"})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    assert_eq!(h.state().panel_count(), 1);
    h.get_by_label("Second");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn ctrl_shift_page_keys_move_the_tab() {
    let mut h = harness();
    let lib = h.state().session.graph.sys.library;
    h.state_mut().new_tab(lib);
    h.step();
    assert_eq!(h.state().tab, 1);
    key(&mut h, Key::PageUp, Modifiers { ctrl: true, shift: true, ..Modifiers::NONE });
    assert_eq!(h.state().tab, 0, "the active tab moved left");
    assert_eq!(h.state().tabs[0].panels[0].zoom, lib);
    key(&mut h, Key::PageDown, Modifiers { ctrl: true, shift: true, ..Modifiers::NONE });
    assert_eq!(h.state().tab, 1);
}

#[test]
fn reference_counter_setting_shows_counts() {
    let mut h = harness();
    let ids = page_with(&mut h, &["Target", "Other"]);
    h.state_mut().exec("node.reference", json!({"target": ids[0].0, "parent": ids[1].0})).unwrap();
    h.state_mut().ref_counter = true;
    for _ in 0..3 {
        h.step();
    }
    h.get_by_label("1");
}

#[test]
fn options_from_supertag_picker_and_invalid_value_warning() {
    let mut h = harness();
    let s = h.state_mut();
    s.exec("tag.create", json!({"name": "person"})).unwrap();
    let ada = s.exec("node.add", json!({"name": "Ada"})).unwrap()["id"].as_u64().unwrap();
    s.exec("tag.add", json!({"node": ada, "tag": "person"})).unwrap();
    s.exec("field.create", json!({"name": "Owner", "type": "options_from_tag"})).unwrap();
    s.exec("field.setSourceTag", json!({"field": "Owner", "tag": "person"})).unwrap();
    let n = s.exec("node.add", json!({"name": "Plan"})).unwrap()["id"].as_u64().unwrap();
    s.exec("field.add", json!({"node": n, "field": "Owner"})).unwrap();
    s.exec("field.add", json!({"node": n, "field": "Site", "type": "url", "value": "example.org"})).unwrap();
    s.exec("nav.open", json!({"node": n})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    h.get_by_label("Not a valid value for this field type");
    h.get_by_label("Select node").click();
    for _ in 0..3 {
        h.step();
    }
    key(&mut h, Key::Enter, Modifiers::NONE);
    let g = &h.state().session.graph;
    let owner = g.find_field("Owner").unwrap();
    assert_eq!(g.get(tanacraft_core::NodeId(n)).unwrap().field_value(owner), Some(&tanacraft_core::FieldValue::Node(tanacraft_core::NodeId(ada))));
}

#[test]
fn date_picker_takes_a_day_and_a_time_then_ok() {
    let mut h = harness();
    let s = h.state_mut();
    let n = s.exec("node.add", json!({"name": "Dentist"})).unwrap()["id"].as_u64().unwrap();
    s.exec("field.add", json!({"node": n, "field": "When", "type": "date"})).unwrap();
    s.exec("nav.open", json!({"node": n})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    h.get_by_label("Add date").click();
    h.step();
    h.step();
    h.step();
    assert!(h.state().date_pick.is_some(), "the picker opened");
    h.get_by_label("2026-10-15").click();
    h.step();
    assert!(h.state().date_pick.is_some(), "a day click alone does not close the picker");
    if let Some(dp) = h.state_mut().date_pick.as_mut() {
        dp.time = ("09:30".into(), "10:15".into());
    }
    h.step();
    h.get_by_label("OK").click();
    for _ in 0..3 {
        h.step();
    }
    let g = &h.state().session.graph;
    let when = g.find_field("When").unwrap();
    let v = g.get(tanacraft_core::NodeId(n)).unwrap().field_value(when).unwrap();
    assert_eq!(tanacraft_core::export::display_value(g, v), "2026-10-15 09:30-10:15");
}

#[test]
fn top_bar_help_lists_every_command() {
    let mut h = harness();
    h.get_by_label("Help").click();
    for _ in 0..3 {
        h.step();
    }
    assert!(h.state().help_page);
    h.get_by_label("edit.insertDate");
    h.get_by_label("Account");
}

#[test]
fn table_header_renames_the_field_in_place() {
    let mut h = harness();
    let s = h.state_mut();
    s.exec("tag.create", json!({"name": "book"})).unwrap();
    s.exec("tag.addField", json!({"tag": "book", "field": "Pages", "type": "number"})).unwrap();
    let tag = s.session.graph.find_tag("book").unwrap().0;
    let b = s.exec("node.add", json!({"name": "Dune"})).unwrap()["id"].as_u64().unwrap();
    s.exec("tag.add", json!({"node": b, "tag": "book"})).unwrap();
    s.exec("nav.open", json!({"node": tag})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    let pages = h.state().session.graph.find_field("Pages").unwrap();
    h.state_mut().header_edit = Some((pages, String::new()));
    for _ in 0..3 {
        h.step();
    }
    text(&mut h, "Page count");
    key(&mut h, Key::Enter, Modifiers::NONE);
    for _ in 0..2 {
        h.step();
    }
    assert_eq!(h.state().session.graph.name(pages), "Page count");
    assert!(h.state().header_edit.is_none());
}

#[test]
fn list_view_opens_the_selected_item_in_the_next_panel() {
    let mut h = harness();
    let ids = page_with(&mut h, &["Alpha", "Beta"]);
    h.state_mut().exec("view.set", json!({"kind": "list"})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    h.get_by_label("Beta").click();
    for _ in 0..3 {
        h.step();
    }
    assert_eq!(h.state().panel_count(), 2);
    assert_eq!(h.state().tabs[0].active, 0, "the list keeps the focus");
    assert_eq!(h.state().tabs[0].panels[1].zoom, ids[1]);
    h.get_by_label("Alpha").click();
    for _ in 0..3 {
        h.step();
    }
    assert_eq!(h.state().panel_count(), 2, "the same panel is reused");
    assert_eq!(h.state().tabs[0].panels[1].zoom, ids[0]);
}

#[test]
fn content_width_panel_split_and_mini_sidebar_flyouts() {
    use egui::{PointerButton, Pos2};
    let mut h = harness();
    h.state_mut().exec("ui.cycleContentWidth", json!({})).unwrap();
    assert_eq!(h.state().content_width, 1);
    // Two panels: dragging the gap between them moves the split.
    h.state_mut().exec("ui.openInPanel", json!({})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    let before = h.state().panel_split;
    let y = h.get_by_label("Account").rect().bottom() + 200.0;
    // Sweep for the column-resize handle: press where dragging changes the split.
    let mut moved = false;
    for x in (200..1100).step_by(4) {
        let from = Pos2::new(x as f32, y);
        let to = Pos2::new(x as f32 + 120.0, y);
        let ev = |h: &mut H<'_>, e: Event| {
            h.input_mut().events.push(e);
            h.step();
        };
        ev(&mut h, Event::PointerMoved(from));
        ev(&mut h, Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
        for k in 1..=4 {
            ev(&mut h, Event::PointerMoved(from + (to - from) * (k as f32 / 4.0)));
        }
        ev(&mut h, Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
        if (h.state().panel_split - before).abs() > 0.02 {
            moved = true;
            break;
        }
    }
    assert!(moved, "dragging the gap between panels resizes them");
    // Mini sidebar: the Pinned flyout lists pinned nodes.
    let ids = page_with(&mut h, &["Pin me"]);
    h.state_mut().exec("node.togglePin", json!({"node": ids[0].0})).unwrap();
    h.state_mut().exec("ui.singlePanel", json!({})).unwrap();
    h.state_mut().sidebar = 1;
    for _ in 0..3 {
        h.step();
    }
    let before = h.query_all_by_label("Pin me").count();
    h.get_by_label("Pinned").click();
    for _ in 0..3 {
        h.step();
    }
    assert_eq!(h.query_all_by_label("Pin me").count(), before + 1, "the flyout lists the pinned node");
}

#[test]
fn lanes_drag_a_node_into_another_lane() {
    use egui::{PointerButton, Pos2};
    let mut h = harness();
    let ids = page_with(&mut h, &["A", "B"]);
    let b1 = h.state_mut().exec("node.add", json!({"name": "B1", "parent": ids[1].0})).unwrap()["id"].as_u64().unwrap();
    h.state_mut().exec("view.set", json!({"kind": "lanes"})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    let grip = h.get_by_label(&format!("grip {b1}")).rect().center();
    let a = h.get_by_label("A").rect();
    let to = Pos2::new(a.center().x, a.bottom() + 10.0);
    let ev = |h: &mut H<'_>, e: Event| {
        h.input_mut().events.push(e);
        h.step();
    };
    ev(&mut h, Event::PointerMoved(grip));
    ev(&mut h, Event::PointerButton { pos: grip, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    for k in 1..=10 {
        ev(&mut h, Event::PointerMoved(grip + (to - grip) * (k as f32 / 10.0)));
    }
    ev(&mut h, Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    for _ in 0..3 {
        h.step();
    }
    assert_eq!(h.state().session.graph.parent(tanacraft_core::NodeId(b1)), Some(ids[0]));
}

#[test]
fn clicking_text_puts_the_caret_there() {
    use egui::{PointerButton, Pos2};
    let mut h = harness();
    let ids = page_with(&mut h, &["abcdefghij"]);
    let r = h.get_by_label("abcdefghij").rect();
    // Click a little left of the middle: the caret lands inside the word, not at its end.
    let p = Pos2::new(r.left() + r.width() * 0.45, r.center().y);
    h.input_mut().events.push(Event::PointerMoved(p));
    h.step();
    h.input_mut().events.push(Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.step();
    h.input_mut().events.push(Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    for _ in 0..3 {
        h.step();
    }
    text(&mut h, "X");
    let name = h.state().session.graph.name(ids[0]).to_string();
    let at = name.find('X').unwrap();
    assert!((3..=6).contains(&at), "{name}");
}

#[test]
fn week_page_has_a_schedule() {
    let mut h = harness();
    let s = h.state_mut();
    let n = s.exec("node.add", json!({"name": "Dentist"})).unwrap()["id"].as_u64().unwrap();
    s.exec("field.add", json!({"node": n, "field": "When", "type": "date", "value": "2026-10-08 09:00-10:00"})).unwrap();
    let week = s.session.graph.iter().find(|x| x.kind == tanacraft_core::Kind::Week { year: 2026, week: 41 }).unwrap().id;
    s.exec("nav.open", json!({"node": week.0})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    h.get_by_label("Schedule");
    h.get_by_label("Time grid");
}

#[test]
fn new_search_results_flash() {
    let mut h = harness();
    let lib = h.state().session.graph.sys.library.0;
    h.state_mut().exec("tag.create", json!({"name": "task"})).unwrap();
    let q = h.state_mut().exec("search.create", json!({"query": "#task", "parent": lib})).unwrap()["id"].as_u64().unwrap();
    h.state_mut().exec("nav.open", json!({"node": q})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    let n = h.state_mut().exec("node.add", json!({"name": "New one", "parent": lib})).unwrap()["id"].as_u64().unwrap();
    h.state_mut().exec("tag.add", json!({"node": n, "tag": "task"})).unwrap();
    h.step();
    assert!(h.state().flash_until.contains_key(&tanacraft_core::NodeId(n)), "the new result is marked to flash");
}

#[test]
fn custom_keybindings_run_their_command() {
    let mut h = harness();
    assert!(h.state_mut().set_keybindings(r#"{"Mod+Shift+H": "nope.nothing"}"#).is_err());
    assert_eq!(h.state_mut().set_keybindings(r#"{"Mod+Shift+H": "node.toggleHeading"}"#), Ok(1));
    let ids = page_with(&mut h, &["Title"]);
    focus(&mut h, ids[0], 0);
    key(&mut h, Key::H, Modifiers { command: true, mac_cmd: true, shift: true, ..Modifiers::NONE });
    assert!(h.state().session.graph.get(ids[0]).unwrap().heading);
}

#[test]
fn drop_on_an_empty_expanded_nodes_ghost_line_nests_it() {
    use egui::{PointerButton, Pos2};
    let mut h = harness();
    let ids = page_with(&mut h, &["Parent", "Loose"]);
    h.state_mut().exec("node.toggleExpand", json!({"node": ids[0].0})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    let ghost = h.get_by_label(&format!("ghost {}", ids[0].0)).rect().center();
    let loose = h.get_by_label("Loose").rect();
    let from = Pos2::new(loose.left() - 12.0, loose.center().y);
    let ev = |h: &mut H<'_>, e: Event| {
        h.input_mut().events.push(e);
        h.step();
    };
    ev(&mut h, Event::PointerMoved(from));
    ev(&mut h, Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    for k in 1..=10 {
        ev(&mut h, Event::PointerMoved(from + (ghost - from) * (k as f32 / 10.0)));
    }
    ev(&mut h, Event::PointerButton { pos: ghost, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    for _ in 0..3 {
        h.step();
    }
    assert_eq!(h.state().session.graph.parent(ids[1]), Some(ids[0]));
}

// ---- DIFFS.md round 4 ----------------------------------------------------------------------

#[test]
fn diffs_n1_enter_in_a_new_field_value_adds_another_value() {
    let mut h = harness();
    let page = page_with(&mut h, &["Page"])[0];
    h.state_mut().exec("nav.open", json!({"node": page.0})).unwrap();
    let x = tanacraft_core::NodeId(h.state_mut().exec("node.add", json!({"parent": page.0, "name": "x"})).unwrap()["id"].as_u64().unwrap());
    focus(&mut h, x, 1);
    key(&mut h, Key::Enter, Modifiers::NONE);
    text(&mut h, ">");
    text(&mut h, "Ppl");
    key(&mut h, Key::Tab, Modifiers::NONE);
    text(&mut h, "Ann");
    key(&mut h, Key::Enter, Modifiers::NONE);
    text(&mut h, "Bob");
    key(&mut h, Key::Enter, Modifiers::NONE);
    key(&mut h, Key::Escape, Modifiers::NONE);
    let g = &h.state().session.graph;
    assert_eq!(g.name(x), "x", "nothing typed into the line above");
    let f = g.find_field("Ppl").expect("field created");
    let vals: Vec<String> = g.get(page).unwrap().field_values(f).iter().map(|v| tanacraft_core::export::display_value(g, v)).collect();
    assert_eq!(vals, ["Ann", "Bob"]);
}

#[test]
fn diffs_n2_gt_field_row_stays_where_it_was_typed() {
    let mut h = harness();
    let page = page_with(&mut h, &["Page"])[0];
    h.state_mut().exec("nav.open", json!({"node": page.0})).unwrap();
    let kids: Vec<_> = ["a", "b", "c"]
        .iter()
        .map(|n| tanacraft_core::NodeId(h.state_mut().exec("node.add", json!({"parent": page.0, "name": n})).unwrap()["id"].as_u64().unwrap()))
        .collect();
    focus(&mut h, kids[2], 1);
    key(&mut h, Key::Enter, Modifiers::NONE);
    text(&mut h, ">");
    text(&mut h, "Multi4");
    key(&mut h, Key::Tab, Modifiers::NONE);
    text(&mut h, "v");
    key(&mut h, Key::Escape, Modifiers::NONE);
    let g = &h.state().session.graph;
    let f = g.find_field("Multi4").unwrap();
    let e = g.get(page).unwrap().fields.iter().find(|e| e.field == f).unwrap();
    assert_eq!(e.after, Some(kids[2]), "the row shows below c, not above a");
    assert_eq!(g.children(page).len(), 3, "the > line became the field row");
}
