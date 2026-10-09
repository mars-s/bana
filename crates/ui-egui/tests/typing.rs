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
    let clip = tanacraft_core::export::to_plain(&h.state().session.graph, &[tanacraft_core::NodeId(ideas)]);
    h.state_mut().exec("focus.set", json!({"node": reading})).unwrap();
    h.step();
    h.input_mut().events.push(Event::Paste(clip));
    h.step();
    let g = &h.state().session.graph;
    assert_eq!(g.references_to(tanacraft_core::NodeId(ideas)).len(), 1);
    // Pasting again next to it skips the duplicate.
    let again = tanacraft_core::export::to_plain(&h.state().session.graph, &[tanacraft_core::NodeId(ideas)]);
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
    assert!(copied.iter().any(|t| t.starts_with("B\n") && t.contains("- B1")), "{copied:?}");
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
    h.get_by_label("Done");
    assert!(h.query_by_label("Query builder").is_none(), "DIFFS N10: an opened search node shows no builder");
    h.state_mut().exec("ui.searchOptions", json!({})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    h.get_by_label("Query builder");
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
fn diffs_n6_n14_workspaces_side_by_side_and_no_inbox_row() {
    let mut h = harness();
    h.state_mut().exec("node.capture", json!({"name": "idea"})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    assert!(h.query_by_label("Inbox").is_none(), "the inbox is reached by command, not a sidebar row");
    h.state_mut().exec("ui.openInPanel", json!({})).unwrap();
    assert_eq!(h.state().panel_count(), 2);
    h.state_mut().exec("workspace.create", json!({"name": "Second"})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    assert_eq!(h.state().panel_count(), 2, "nothing switches");
    h.get_by_label("Workspaces");
    h.get_by_label("Second");
    h.get_by_label("Workspace");
    h.get_by_label("Manage workspaces");
    // DIFFS N24: workspace rows start folded and hold no Library or Trash rows of their own.
    assert_eq!(h.query_all_by_label("Trash").count(), 0);
}

#[test]
fn diffs_n23_settings_defaults_and_system_theme() {
    let mut h = harness();
    let st = &h.state().session.graph.settings;
    assert!(st.warn_referenced && st.url_embeds, "both on by default");
    h.state_mut().exec("ui.settings", json!({})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    h.get_by_label("Use system theme");
    h.get_by_label("User settings");
    assert!(h.query_all_by_label("Settings").count() >= 1, "heading");
    h.state_mut().exec("ui.toggleTheme", json!({})).unwrap();
    assert!(!h.state().system_theme, "picking a theme stops following the system");
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

#[test]
fn diffs_n3_escape_then_down_or_up_edits_the_next_or_previous_line() {
    let mut h = harness();
    let ids = page_with(&mut h, &["one", "two", "three"]);
    focus(&mut h, ids[1], 1);
    key(&mut h, Key::Escape, Modifiers::NONE);
    key(&mut h, Key::ArrowDown, Modifiers::NONE);
    text(&mut h, "X");
    assert_eq!(h.state().session.sel, None);
    assert_eq!(h.state().session.graph.name(ids[2]), "Xthree");
    key(&mut h, Key::Escape, Modifiers::NONE);
    key(&mut h, Key::ArrowUp, Modifiers::NONE);
    text(&mut h, "Y");
    assert_eq!(h.state().session.graph.name(ids[1]), "twoY");
}

#[test]
fn diffs_n4_cmd_period_puts_the_caret_in_the_page_title() {
    let mut h = harness();
    let p = page_with(&mut h, &["P"])[0];
    h.state_mut().exec("node.add", json!({"parent": p.0, "name": "c"})).unwrap();
    focus(&mut h, p, 1);
    key(&mut h, Key::Period, CMD);
    text(&mut h, "X");
    assert_eq!(h.state().session.zoom, p);
    assert_eq!(h.state().session.graph.name(p), "PX");
}

#[test]
fn diffs_n11_selection_toolbar_has_link_panel_and_menus() {
    let mut h = harness();
    let ids = page_with(&mut h, &["one", "two"]);
    focus(&mut h, ids[1], 1);
    key(&mut h, Key::Escape, Modifiers::NONE);
    for label in ["# Tag", "Move", "H", "Link", "Panel"] {
        h.get_by_label(label);
    }
    assert_eq!(h.query_all_by_label("...").count(), 2);
}

#[test]
fn diffs_n12_pasting_copied_nodes_into_an_empty_line_replaces_it() {
    let mut h = harness();
    let ids = page_with(&mut h, &["B", "C"]);
    h.state_mut().exec("node.add", json!({"parent": ids[0].0, "name": "B1"})).unwrap();
    let empty = tanacraft_core::NodeId(h.state_mut().exec("node.add", json!({"parent": ids[1].0, "name": ""})).unwrap()["id"].as_u64().unwrap());
    h.state_mut().exec("node.expand", json!({"node": ids[1].0})).unwrap();
    h.state_mut().exec("select.set", json!({"from": ids[0].0})).unwrap();
    h.input_mut().events.push(Event::Copy);
    h.step();
    let clip = tanacraft_core::export::to_plain(&h.state().session.graph, &[ids[0]]);
    focus(&mut h, empty, 0);
    h.input_mut().events.push(Event::Paste(clip));
    h.step();
    let g = &h.state().session.graph;
    let kids = g.children(ids[1]);
    assert_eq!(kids.len(), 1, "the empty line became the reference");
    assert_eq!(g.resolve(kids[0]), ids[0]);
}

#[test]
fn side_menu_and_tabs_add_an_item_and_type_its_heading() {
    // Spec 04 §8-9 (`160`, `161`).
    for (kind, label) in [("side_menu", "+ Add menu item"), ("tabs", "+ Add tab")] {
        let mut h = harness();
        let page = page_with(&mut h, &["Menu"])[0];
        h.state_mut().exec("node.add", json!({"parent": page.0, "name": "First"})).unwrap();
        h.state_mut().exec("nav.open", json!({"node": page.0})).unwrap();
        h.state_mut().exec("view.set", json!({"kind": kind})).unwrap();
        for _ in 0..3 {
            h.step();
        }
        h.get_by_label(label).click();
        for _ in 0..3 {
            h.step();
        }
        text(&mut h, "Second");
        let g = &h.state().session.graph;
        let kids = g.children(page);
        assert_eq!(kids.len(), 2, "{kind}");
        assert_eq!(g.name(kids[1]), "Second", "{kind}");
        assert_eq!(h.state().session.nav_selected.get(&page), Some(&kids[1]), "{kind}");
    }
}

#[test]
fn hidden_fields_fold_to_a_label_when_not_empty_or_always() {
    // Spec 03 §6 (`164`, `165`).
    for mode in ["when_not_empty", "always"] {
        let mut h = harness();
        let page = page_with(&mut h, &["Trip"])[0];
        h.state_mut().exec("field.add", json!({"node": page.0, "field": "Budget", "value": "ninety"})).unwrap();
        h.state_mut().exec("field.setHide", json!({"field": "Budget", "mode": mode})).unwrap();
        h.state_mut().exec("node.expand", json!({"node": page.0})).unwrap();
        for _ in 0..3 {
            h.step();
        }
        h.get_by_label("+ Budget");
        assert!(h.query_by_label("ninety").is_none(), "{mode}");
        h.get_by_label("+ Budget").click();
        for _ in 0..3 {
            h.step();
        }
        assert!(h.query_by_label("+ Budget").is_none(), "{mode}: clicking reveals the row");
    }
}

#[test]
fn every_menu_bar_entry_runs_a_known_command() {
    // Spec 00 §7: the native menu bar only runs commands.
    let mut h = harness();
    for (menu, items) in tanacraft_ui_egui::MENU_BAR {
        for (label, cmd) in *items {
            let known = h.state().session.registry.get(cmd).is_some() || tanacraft_ui_egui::UI_COMMANDS.iter().any(|(id, _, _)| id == cmd);
            assert!(known, "{menu} > {label}: unknown command {cmd}");
        }
    }
    // DIFFS N25: items show their keys, and Edit has the clipboard items.
    let app = h.state();
    assert_eq!(app.menu_shortcut("edit.undo").as_deref(), Some("Mod+Z"));
    assert_eq!(app.menu_shortcut("edit.redo").as_deref(), Some("Mod+Shift+Z"));
    assert_eq!(app.menu_shortcut("nav.back").as_deref(), Some("Mod+["));
    assert_eq!(app.menu_shortcut("ui.copy").as_deref(), Some("Mod+C"));
    let edit: Vec<&str> = tanacraft_ui_egui::MENU_BAR.iter().find(|m| m.0 == "Edit").unwrap().1.iter().map(|i| i.0).collect();
    for want in ["Cut", "Copy", "Paste", "Select all"] {
        assert!(edit.contains(&want), "Edit has {want}");
    }
    // A menu item with a key acts as the key: Undo from the menu undoes typing.
    let ids = page_with(&mut h, &["x"]);
    focus(&mut h, ids[0], 1);
    text(&mut h, "yz");
    h.state_mut().menu_item("edit.undo");
    for _ in 0..3 {
        h.step();
    }
    assert_eq!(h.state().session.graph.name(ids[0]), "x");
    // Copy from the menu copies the selected line.
    h.state_mut().exec("select.set", json!({"from": ids[0].0})).unwrap();
    h.state_mut().menu_item("ui.copy");
    h.step();
    let copied = h.output().platform_output.commands.iter().any(|c| matches!(c, egui::OutputCommand::CopyText(t) if t.starts_with('x')));
    assert!(copied);
    h.state_mut().exec("ui.zoomIn", json!({})).unwrap();
    h.step();
    h.step();
    assert!((h.ctx.zoom_factor() - 1.1).abs() < 1e-4, "{}", h.ctx.zoom_factor());
    h.state_mut().exec("ui.zoomReset", json!({})).unwrap();
    h.step();
    h.step();
    assert!((h.ctx.zoom_factor() - 1.0).abs() < 1e-4);
}

#[test]
fn copying_selected_nodes_puts_html_and_text_on_the_clipboard() {
    // Spec 02 §12: a nested HTML list plus plain text.
    let mut h = harness();
    let got = std::rc::Rc::new(std::cell::RefCell::new(None));
    let sink = got.clone();
    h.state_mut().rich_clipboard = Some(Box::new(move |html: &str, text: &str| *sink.borrow_mut() = Some((html.to_string(), text.to_string()))));
    let ids = page_with(&mut h, &["B"]);
    h.state_mut().exec("node.add", json!({"parent": ids[0].0, "name": "B1"})).unwrap();
    h.state_mut().exec("select.set", json!({"from": ids[0].0})).unwrap();
    h.input_mut().events.push(Event::Copy);
    h.step();
    let (html, text) = got.borrow().clone().expect("clipboard written");
    assert_eq!(html, "<p>B</p><ul><li>B1</li></ul>");
    assert_eq!(text, "B\n- B1\n");
    let r = h.state_mut().exec("export.clipboard", json!({"node": ids[0].0})).unwrap();
    assert_eq!(r["html"], html);
}

#[test]
fn settings_sub_pages_change_time_format_and_sidebar() {
    // Spec 07 §8: Settings has a left nav of sub-pages.
    let mut h = harness();
    h.state_mut().exec("ui.settings", json!({})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    for p in ["Preferences", "Notifications seen", "Progress stats", "Date and time", "Move targets", "Journaling", "API keys", "Keybindings"] {
        assert!(h.query_all_by_label(p).count() >= 1, "{p}");
    }
    h.get_by_label("Date and time").click();
    for _ in 0..3 {
        h.step();
    }
    h.get_by_label("12 hour (2:30 PM)").click();
    h.get_by_label("Sunday").click();
    for _ in 0..3 {
        h.step();
    }
    let st = &h.state().session.graph.settings;
    assert!(st.time_12h && st.week_starts_sunday);
    h.get_by_label("Recents");
    h.state_mut().exec("settings.set", json!({"key": "show_recents", "value": false})).unwrap();
    h.query_all_by_label("Preferences").next().unwrap().click();
    for _ in 0..3 {
        h.step();
    }
    assert!(h.query_by_label("Recents").is_none(), "hidden from the sidebar");
    h.get_by_label("Show Recents in sidebar");
}

#[test]
fn tag_and_field_pages_take_a_description_and_tags_have_building_block_options() {
    // Spec 03 §5 and §6.
    let mut h = harness();
    h.state_mut().exec("field.create", json!({"name": "Budget", "type": "number"})).unwrap();
    let fid = h.state().session.graph.find_field("Budget").unwrap();
    h.state_mut().exec("nav.open", json!({"node": fid.0})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    h.get_by_label("Add description").click();
    for _ in 0..3 {
        h.step();
    }
    text(&mut h, "How much it may cost");
    key(&mut h, Key::Enter, Modifiers::NONE);
    assert_eq!(h.state().session.graph.get(fid).unwrap().description, "How much it may cost");
    h.state_mut().exec("tag.create", json!({"name": "agenda"})).unwrap();
    let tag = h.state().session.graph.find_tag("agenda").unwrap();
    h.state_mut().exec("nav.open", json!({"node": tag.0})).unwrap();
    h.state_mut().exec("ui.configureSupertag", json!({})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    assert!(h.query_all_by_label("Add description").count() >= 1);
    h.get_by_label("Hide from searches and menus");
    h.get_by_label("Building block: promote for");
}

#[test]
fn editing_a_referenced_node_warns_once_when_the_setting_is_on() {
    // Spec 01 §3: a setting flags edits of nodes referenced elsewhere.
    let mut h = harness();
    let ids = page_with(&mut h, &["Shared", "Other"]);
    h.state_mut().exec("node.reference", json!({"target": ids[0].0, "parent": ids[1].0})).unwrap();
    h.state_mut().exec("settings.set", json!({"key": "warn_referenced", "value": true})).unwrap();
    focus(&mut h, ids[0], 6);
    text(&mut h, "!");
    let msg = "This node also appears in 1 other place(s); the edit shows everywhere";
    h.get_by_label(msg);
    assert_eq!(h.state().session.graph.name(ids[0]), "Shared!");
}

#[test]
fn alt_cmd_down_on_a_reference_writes_a_note_for_this_context() {
    // Spec 01 §3.
    let mut h = harness();
    let ids = page_with(&mut h, &["Recipe", "Plan"]);
    let r = h.state_mut().exec("node.reference", json!({"target": ids[0].0, "parent": ids[1].0})).unwrap();
    let r = tanacraft_core::NodeId(r["id"].as_u64().unwrap());
    h.state_mut().exec("node.expand", json!({"node": ids[1].0})).unwrap();
    focus(&mut h, r, 0);
    key(&mut h, Key::ArrowDown, Modifiers { alt: true, command: true, mac_cmd: true, ..Modifiers::NONE });
    // DIFFS N22: the notes are an outline under the reference, with a heading line.
    text(&mut h, "for Sunday");
    key(&mut h, Key::Enter, Modifiers::NONE);
    text(&mut h, "double it");
    let g = &h.state().session.graph;
    let notes = g.context_of(r).expect("notes node");
    let lines: Vec<&str> = g.children(notes).iter().map(|c| g.name(*c)).collect();
    assert_eq!(lines, ["for Sunday", "double it"]);
    assert!(g.children(ids[0]).is_empty(), "the referenced node is untouched");
    h.get_by_label("Notes and fields about Recipe, stored in Plan");
    // Shift+Tab does not carry a note out into the referenced node.
    key(&mut h, Key::Tab, Modifiers { shift: true, ..Modifiers::NONE });
    assert_eq!(h.state().session.graph.children(notes).len(), 2);
    // The notes survive a save and load.
    let back = tanacraft_core::Graph::from_json(&h.state().session.graph.to_json().unwrap()).unwrap();
    assert_eq!(back.context_of(r).map(|c| back.children(c).len()), Some(2));
}

#[test]
fn date_picker_add_end_sets_a_span_of_days() {
    // Spec 03 §8: "Add end" makes a date range.
    let mut h = harness();
    let page = page_with(&mut h, &["Holiday"])[0];
    h.state_mut().exec("field.add", json!({"node": page.0, "field": "Trip", "type": "date"})).unwrap();
    h.state_mut().exec("nav.open", json!({"node": page.0})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    h.get_by_label("Add date").click();
    for _ in 0..3 {
        h.step();
    }
    for label in ["2026-10-15", "Add end", "2026-10-17", "OK"] {
        h.get_by_label(label).click();
        for _ in 0..3 {
            h.step();
        }
    }
    let g = &h.state().session.graph;
    let f = g.find_field("Trip").unwrap();
    let v = g.get(page).unwrap().field_value(f).cloned();
    assert_eq!(v.map(|v| tanacraft_core::export::display_value(g, &v)), Some("2026-10-15/2026-10-17".to_string()));
    h.get_by_label("Thu, 15 Oct - Sat, 17 Oct");
}

#[test]
fn node_menu_has_filter_sort_group_display_and_copy() {
    // Spec 04 §1 options menu (`46-search-node-options-menu.png`).
    let mut h = harness();
    let p = page_with(&mut h, &["Reading"])[0];
    h.state_mut().exec("nav.open", json!({"node": p.0})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    h.get_by_label("...").click();
    for _ in 0..3 {
        h.step();
    }
    for label in ["Copy", "Duplicate", "Show edit log"] {
        h.get_by_label(label);
    }
    // Submenu buttons carry an arrow after their label.
    for label in ["View as", "Filter by", "Sort by", "Group by", "Display"] {
        h.get_by_label_contains(label);
    }
    h.get_by_label_contains("Sort by").click();
    for _ in 0..3 {
        h.step();
    }
    h.get_by_label("Last edited").click();
    for _ in 0..3 {
        h.step();
    }
    assert_eq!(h.state().session.graph.view_def(p).sort.first().map(|s| s.by), Some(tanacraft_core::SortBy::Edited));
}

#[test]
fn command_line_runs_a_command_node_on_the_focused_node() {
    let mut h = harness();
    h.state_mut().exec("commandNode.create", json!({"name": "Mark urgent", "steps": ["tag.add {\"tag\": \"urgent\"}"]})).unwrap();
    let ids = page_with(&mut h, &["Fix the gate"]);
    focus(&mut h, ids[0], 0);
    key(&mut h, Key::K, CMD);
    text(&mut h, "Mark urgent");
    key(&mut h, Key::Enter, Modifiers::NONE);
    let g = &h.state().session.graph;
    assert_eq!(g.get(ids[0]).unwrap().tags.iter().map(|t| g.name(*t)).collect::<Vec<_>>(), ["urgent"]);
}

#[test]
fn context_fields_show_under_the_reference() {
    let mut h = harness();
    let ids = page_with(&mut h, &["Ada", "Team"]);
    let r = h.state_mut().exec("node.reference", json!({"target": ids[0].0, "parent": ids[1].0})).unwrap();
    h.state_mut().exec("node.contextField", json!({"node": r["id"], "field": "Role", "value": "Lead"})).unwrap();
    h.state_mut().exec("node.expand", json!({"node": ids[1].0})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    h.get_by_label("Role: Lead");
}

#[test]
fn url_embeds_show_a_site_card_when_the_setting_is_on() {
    let mut h = harness();
    page_with(&mut h, &["https://www.example.org/garden/plan"]);
    // On by default (DIFFS N23); off hides the card.
    h.get_by_label("example.org");
    h.state_mut().exec("settings.set", json!({"key": "url_embeds", "value": false})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    assert!(h.query_by_label("example.org").is_none());
}

#[test]
fn query_builder_results_wait_for_run_once_and_done() {
    // Spec 05 §2. Clicks are real pointer presses on the buttons.
    fn press(h: &mut H<'_>, label: &str) {
        let at = h.get_by_label(label).rect().center();
        for e in [
            Event::PointerMoved(at),
            Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE },
            Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE },
        ] {
            h.input_mut().events.push(e);
            h.step();
        }
        for _ in 0..3 {
            h.step();
        }
    }
    let mut h = harness();
    page_with(&mut h, &["apple pie", "apple tart"]);
    let q = h.state_mut().exec("search.create", json!({"query": "apple"})).unwrap();
    let q = tanacraft_core::NodeId(q["id"].as_u64().unwrap());
    h.state_mut().exec("nav.open", json!({"node": q.0})).unwrap();
    h.state_mut().exec("ui.searchOptions", json!({})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    h.state_mut().exec("search.addClause", json!({"node": q.0, "clause": "pie"})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    assert_eq!(h.state().session.graph.view_items(q).len(), 2, "still the old results");
    press(&mut h, "Run once");
    assert_eq!(h.state().session.graph.view_items(q).len(), 1);
    press(&mut h, "Done");
    assert!(!h.state().query_open);
    let lib = h.state().session.graph.sys.library;
    h.state_mut().exec("node.add", json!({"parent": lib.0, "name": "apple pie two"})).unwrap();
    assert_eq!(h.state().session.graph.view_items(q).len(), 2, "live after Done");
}

/// A page with one line "x", caret at its end, then Enter and `>` + `name`: a field row named
/// `name` with its value editor open (Tab).
fn field_after_x(h: &mut H<'_>, name: &str) -> (tanacraft_core::NodeId, tanacraft_core::NodeId) {
    let page = page_with(h, &["Page"])[0];
    h.state_mut().exec("nav.open", json!({"node": page.0})).unwrap();
    let x = tanacraft_core::NodeId(h.state_mut().exec("node.add", json!({"parent": page.0, "name": "x"})).unwrap()["id"].as_u64().unwrap());
    focus(h, x, 1);
    key(h, Key::Enter, Modifiers::NONE);
    text(h, ">");
    text(h, name);
    key(h, Key::Tab, Modifiers::NONE);
    (page, x)
}

fn values_of(h: &H<'_>, node: tanacraft_core::NodeId, field: &str) -> Vec<String> {
    let g = &h.state().session.graph;
    let f = g.find_field(field).expect("field exists");
    g.get(node).unwrap().field_values(f).iter().map(|v| tanacraft_core::export::display_value(g, v)).collect()
}

#[test]
fn diffs_n15_field_values_are_saved_as_typed() {
    // Escape keeps the value, and the row is selected so typing goes on.
    let mut h = harness();
    let (page, x) = field_after_x(&mut h, "Prio");
    text(&mut h, "High");
    assert_eq!(values_of(&h, page, "Prio"), ["High"], "saved before any Enter");
    key(&mut h, Key::Escape, Modifiers::NONE);
    assert_eq!(values_of(&h, page, "Prio"), ["High"]);
    assert_eq!(h.state().session.sel, Some((x, x)));
    // One undo takes the whole typed value back, not one letter.
    key(&mut h, Key::Z, CMD);
    assert!(values_of(&h, page, "Prio").is_empty());

    // Up keeps it too, and the caret goes to the line above.
    let mut h = harness();
    let (page, x) = field_after_x(&mut h, "Lead");
    text(&mut h, "Ann");
    key(&mut h, Key::ArrowUp, Modifiers::NONE);
    text(&mut h, "Z");
    assert_eq!(values_of(&h, page, "Lead"), ["Ann"]);
    assert_eq!(h.state().session.graph.name(x), "xZ");

    // Several values: Escape after the second keeps both.
    let mut h = harness();
    let (page, _) = field_after_x(&mut h, "Ppl");
    text(&mut h, "Ann");
    key(&mut h, Key::Enter, Modifiers::NONE);
    text(&mut h, "Bob");
    key(&mut h, Key::Escape, Modifiers::NONE);
    assert_eq!(values_of(&h, page, "Ppl"), ["Ann", "Bob"]);

    // A field name typed after `>` stays with Escape; the line becomes the row.
    let mut h = harness();
    let page = page_with(&mut h, &["Page"])[0];
    h.state_mut().exec("nav.open", json!({"node": page.0})).unwrap();
    let x = tanacraft_core::NodeId(h.state_mut().exec("node.add", json!({"parent": page.0, "name": "x"})).unwrap()["id"].as_u64().unwrap());
    focus(&mut h, x, 1);
    key(&mut h, Key::Enter, Modifiers::NONE);
    text(&mut h, ">");
    text(&mut h, "Qq");
    key(&mut h, Key::Escape, Modifiers::NONE);
    let g = &h.state().session.graph;
    let f = g.find_field("Qq").expect("field kept");
    assert!(g.get(page).unwrap().fields.iter().any(|e| e.field == f && e.after == Some(x)));
    assert_eq!(g.children(page), &[x], "no blank line left");
    assert!(h.state().picker.is_none());
}

#[test]
fn diffs_n16_n17_shift_tab_leaves_the_value_to_a_line_below_the_field() {
    // Shift+Tab on an empty value line: a new line below the field row, typing goes on there.
    let mut h = harness();
    let (page, x) = field_after_x(&mut h, "Lead");
    text(&mut h, "Robin");
    key(&mut h, Key::Enter, Modifiers::NONE);
    key(&mut h, Key::Tab, Modifiers { shift: true, ..Modifiers::NONE });
    text(&mut h, "Timeline2");
    key(&mut h, Key::Enter, Modifiers::NONE);
    text(&mut h, "Week2");
    assert_eq!(values_of(&h, page, "Lead"), ["Robin"]);
    let g = &h.state().session.graph;
    let names: Vec<&str> = g.children(page).iter().map(|c| g.name(*c)).collect();
    assert_eq!(names, ["x", "Timeline2", "Week2"]);
    let _ = x;

    // Shift+Tab on a value with text: it becomes the line just below the field row, not the top.
    let mut h = harness();
    let (page, _) = field_after_x(&mut h, "Pq");
    text(&mut h, "Ann");
    key(&mut h, Key::Enter, Modifiers::NONE);
    text(&mut h, "Bob");
    key(&mut h, Key::Tab, Modifiers { shift: true, ..Modifiers::NONE });
    text(&mut h, "!");
    assert_eq!(values_of(&h, page, "Pq"), ["Ann"]);
    let g = &h.state().session.graph;
    let names: Vec<&str> = g.children(page).iter().map(|c| g.name(*c)).collect();
    assert_eq!(names, ["x", "Bob!"]);
}

/// The keyboard never goes nowhere (DIFFS N16 sweep): each case once lost it.
#[test]
fn keys_and_menus_hand_the_keyboard_back() {
    // Cmd+K then Escape: typing goes on in the line.
    let mut h = harness();
    let ids = page_with(&mut h, &["one", "two"]);
    focus(&mut h, ids[0], 3);
    key(&mut h, Key::K, CMD);
    assert!(h.state().palette.is_some());
    key(&mut h, Key::Escape, Modifiers::NONE);
    text(&mut h, "X");
    assert_eq!(h.state().session.graph.name(ids[0]), "oneX");

    // Deleting a selection selects the line that takes its place.
    focus(&mut h, ids[0], 0);
    key(&mut h, Key::Escape, Modifiers::NONE);
    key(&mut h, Key::Backspace, Modifiers::NONE);
    assert_eq!(h.state().session.sel, Some((ids[1], ids[1])));

    // A whole-line reference takes typing: it edits the node it points at.
    let mut h = harness();
    let ids = page_with(&mut h, &["Target", ""]);
    h.state_mut().exec("node.reference", json!({"target": ids[0].0, "replace": ids[1].0})).unwrap();
    let r = *h.state().session.graph.children(h.state().session.zoom).last().unwrap();
    focus(&mut h, r, 6);
    text(&mut h, "!");
    assert_eq!(h.state().session.graph.name(ids[0]), "Target!");

    // Escape in a page title selects the first line; Up from it does not jump to a button.
    let mut h = harness();
    let page = page_with(&mut h, &["Page"])[0];
    h.state_mut().exec("nav.open", json!({"node": page.0})).unwrap();
    let a = tanacraft_core::NodeId(h.state_mut().exec("node.add", json!({"parent": page.0, "name": "a"})).unwrap()["id"].as_u64().unwrap());
    focus(&mut h, a, 0);
    key(&mut h, Key::ArrowUp, Modifiers::NONE);
    text(&mut h, "Z");
    assert_eq!(h.state().session.graph.name(page), "PageZ");
    key(&mut h, Key::Escape, Modifiers::NONE);
    assert_eq!(h.state().session.sel, Some((a, a)));

    // Deleting a line that is already in the trash leaves the caret where it is.
    let mut h = harness();
    let ids = page_with(&mut h, &["gone"]);
    h.state_mut().exec("node.delete", json!({"node": ids[0].0})).unwrap();
    h.state_mut().exec("nav.trash", json!({})).unwrap();
    focus(&mut h, ids[0], 0);
    assert!(h.state_mut().exec("node.delete", json!({})).is_err());
    assert_eq!(h.state().session.focus, Some(ids[0]));
}

#[test]
fn diffs_n19_bold_toggled_off_leaves_no_empty_marks() {
    let mut h = harness();
    let ids = page_with(&mut h, &[""]);
    focus(&mut h, ids[0], 0);
    text(&mut h, "Mulch ");
    key(&mut h, Key::B, CMD);
    text(&mut h, "now");
    key(&mut h, Key::B, CMD);
    text(&mut h, " please");
    key(&mut h, Key::Enter, Modifiers::NONE);
    text(&mut h, "Next");
    let g = &h.state().session.graph;
    assert_eq!(g.name(ids[0]), "Mulch **now** please");
    assert_eq!(g.name(h.state().session.focus.unwrap()), "Next");
    // Bold on, then off with nothing typed: the line keeps no marks once the caret leaves.
    key(&mut h, Key::B, CMD);
    key(&mut h, Key::B, CMD);
    key(&mut h, Key::B, CMD);
    let next = h.state().session.focus.unwrap();
    key(&mut h, Key::ArrowUp, Modifiers::NONE);
    assert_eq!(h.state().session.graph.name(next), "Next");
}

#[test]
fn diffs_n20_deleting_a_node_used_elsewhere_asks_first() {
    let mut h = harness();
    let ids = page_with(&mut h, &["Kickoff", "", "after"]);
    h.state_mut().exec("node.reference", json!({"target": ids[0].0, "replace": ids[1].0})).unwrap();
    let del = Modifiers { command: true, mac_cmd: true, shift: true, ..Modifiers::NONE };
    focus(&mut h, ids[0], 0);
    key(&mut h, Key::Backspace, del);
    assert_eq!(h.state().confirm_delete.as_ref().map(|c| c.2), Some(1));
    assert!(!h.state().session.graph.in_trash(ids[0]));
    // The question's wording (DIFFS N30).
    h.get_by_label("Kickoff is used in 1 place. What would you like to do?");
    h.get_by_label("Show where it's used");
    key(&mut h, Key::Escape, Modifiers::NONE);
    assert!(h.state().confirm_delete.is_none());
    assert!(!h.state().session.graph.in_trash(ids[0]));
    // Enter deletes anyway.
    focus(&mut h, ids[0], 0);
    key(&mut h, Key::Backspace, del);
    key(&mut h, Key::Enter, Modifiers::NONE);
    assert!(h.state().session.graph.in_trash(ids[0]));
    // Deleting the reference line itself does not ask.
    let mut h = harness();
    let ids = page_with(&mut h, &["Kickoff", ""]);
    h.state_mut().exec("node.reference", json!({"target": ids[0].0, "replace": ids[1].0})).unwrap();
    let r = *h.state().session.graph.children(h.state().session.zoom).last().unwrap();
    h.state_mut().exec("node.delete", json!({"node": r.0})).unwrap();
    assert!(h.state().confirm_delete.is_none());
}

#[test]
fn diffs_n13_query_editor_opens_from_the_keyboard() {
    let mut h = harness();
    let q = h.state_mut().exec("search.create", json!({"query": "garden", "name": "Garden search"})).unwrap();
    h.state_mut().exec("nav.open", json!({"node": q["id"]})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    assert!(!h.state().query_open);
    key(&mut h, Key::O, Modifiers { shift: true, ..CMD });
    assert!(h.state().query_open, "Cmd+Shift+O opens it");
    key(&mut h, Key::O, Modifiers { shift: true, ..CMD });
    assert!(!h.state().query_open);
    // The command line finds it by name.
    key(&mut h, Key::K, CMD);
    text(&mut h, "query editor");
    key(&mut h, Key::Enter, Modifiers::NONE);
    assert!(h.state().query_open);
}

fn type_chars(h: &mut H<'_>, s: &str) {
    for c in s.chars() {
        h.input_mut().events.push(Event::Text(c.to_string()));
        h.step();
    }
}

/// Quits the app the way Cmd+Q does (commands, then the app's exit hook), and reads the saved
/// file back.
fn quit_and_reload(h: &mut H<'_>, name: &str) -> Graph {
    let dir = std::env::temp_dir().join(format!("tanacraft-quit-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("graph.json");
    h.state_mut().session.path = Some(path.clone());
    h.state_mut().exec("ui.quit", json!({})).unwrap();
    h.step();
    h.state_mut().save_on_quit().unwrap();
    let g = Graph::load(&path).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    g
}

fn has_name(g: &Graph, name: &str) -> bool {
    g.iter().any(|n| n.name == name && !g.in_trash(n.id))
}

fn field_texts(g: &Graph, node: tanacraft_core::NodeId, field: &str) -> Vec<String> {
    let f = g.find_field(field).expect("field exists");
    g.get(node).unwrap().field_values(f).iter().map(|v| tanacraft_core::export::display_value(g, v)).collect()
}

#[test]
fn diffs_n26_quitting_mid_edit_keeps_every_editor() {
    // Node text.
    let mut h = harness();
    let ids = page_with(&mut h, &[""]);
    focus(&mut h, ids[0], 0);
    type_chars(&mut h, "half a line");
    assert!(has_name(&quit_and_reload(&mut h, "node"), "half a line"));

    // A field value, quit at once.
    let mut h = harness();
    let (page, _) = field_after_x(&mut h, "Note8");
    type_chars(&mut h, "value");
    assert_eq!(field_texts(&quit_and_reload(&mut h, "value"), page, "Note8"), ["value"]);

    // A field name typed after `>`: the field row is kept and the blank line goes.
    let mut h = harness();
    let page = page_with(&mut h, &["Page"])[0];
    h.state_mut().exec("nav.open", json!({"node": page.0})).unwrap();
    let x = tanacraft_core::NodeId(h.state_mut().exec("node.add", json!({"parent": page.0, "name": "x"})).unwrap()["id"].as_u64().unwrap());
    focus(&mut h, x, 1);
    key(&mut h, Key::Enter, Modifiers::NONE);
    type_chars(&mut h, ">Qq7");
    let g = quit_and_reload(&mut h, "name");
    let f = g.find_field("Qq7").expect("field kept");
    assert!(g.get(page).unwrap().fields.iter().any(|e| e.field == f));
    assert_eq!(g.children(page), &[x]);

    // A value that is not valid for its type (a number field) is kept as typed.
    let mut h = harness();
    h.state_mut().exec("field.create", json!({"name": "Count", "type": "number"})).unwrap();
    let (page, _) = field_after_x(&mut h, "Count");
    type_chars(&mut h, "about 4");
    assert_eq!(field_texts(&quit_and_reload(&mut h, "number"), page, "Count"), ["about 4"]);

    // An options field, whose value is only collected when the editor closes.
    let mut h = harness();
    h.state_mut().exec("field.create", json!({"name": "Mood", "type": "options"})).unwrap();
    let (page, _) = field_after_x(&mut h, "Mood");
    type_chars(&mut h, "calm");
    assert_eq!(field_texts(&quit_and_reload(&mut h, "options"), page, "Mood"), ["calm"]);

    // The page title.
    let mut h = harness();
    let page = page_with(&mut h, &["Page"])[0];
    h.state_mut().exec("nav.open", json!({"node": page.0})).unwrap();
    let a = tanacraft_core::NodeId(h.state_mut().exec("node.add", json!({"parent": page.0, "name": "a"})).unwrap()["id"].as_u64().unwrap());
    focus(&mut h, a, 0);
    key(&mut h, Key::ArrowUp, Modifiers::NONE);
    type_chars(&mut h, " two");
    assert!(has_name(&quit_and_reload(&mut h, "title"), "Page two"));

    // A description. (A reference's notes for this place are lines, like node text.)
    let mut h = harness();
    let ids = page_with(&mut h, &["Thing", ""]);
    focus(&mut h, ids[0], 0);
    h.state_mut().exec("ui.editDescription", json!({})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    type_chars(&mut h, "about it");
    let g = quit_and_reload(&mut h, "desc");
    assert_eq!(g.get(ids[0]).unwrap().description, "about it");
    // Quick add lines.
    let mut h = harness();
    key(&mut h, Key::E, CMD);
    type_chars(&mut h, "Later");
    assert!(has_name(&quit_and_reload(&mut h, "quick"), "Later"));

    // A table column being renamed.
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
    type_chars(&mut h, "Length");
    assert_eq!(quit_and_reload(&mut h, "header").name(pages), "Length");
}

fn by_hint<'a>(h: &'a H<'_>, hint: &'a str) -> egui_kittest::Node<'a> {
    h.query_all(egui_kittest::kittest::by().predicate(move |n| n.placeholder() == Some(hint))).next().expect("a text box with this hint")
}

#[test]
fn diffs_n26_quitting_keeps_supertag_and_field_page_boxes() {
    // The "add a template node" box of a supertag and the option box of a field page.
    let mut h = harness();
    h.state_mut().exec("tag.create", json!({"name": "agenda"})).unwrap();
    let tag = h.state().session.graph.find_tag("agenda").unwrap();
    h.state_mut().exec("nav.open", json!({"node": tag.0})).unwrap();
    h.state_mut().exec("ui.configureSupertag", json!({})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    by_hint(&h, "Add a template node, Enter").click();
    for _ in 0..3 {
        h.step();
    }
    type_chars(&mut h, "Minutes");
    let g = quit_and_reload(&mut h, "template");
    assert!(g.tag_def(tag).unwrap().template.iter().any(|n| g.name(*n) == "Minutes"));

    let mut h = harness();
    h.state_mut().exec("field.create", json!({"name": "Mood", "type": "options"})).unwrap();
    let f = h.state().session.graph.find_field("Mood").unwrap();
    h.state_mut().exec("nav.open", json!({"node": f.0})).unwrap();
    for _ in 0..3 {
        h.step();
    }
    by_hint(&h, "Empty").click();
    for _ in 0..3 {
        h.step();
    }
    type_chars(&mut h, "calm");
    let g = quit_and_reload(&mut h, "option");
    assert_eq!(g.field_def(f).unwrap().options, ["calm"]);
}

#[test]
fn diffs_n27_typing_on_a_selected_line_goes_into_it() {
    // Escape, then a letter.
    let mut h = harness();
    let ids = page_with(&mut h, &["one", "two", "three"]);
    focus(&mut h, ids[1], 3);
    key(&mut h, Key::Escape, Modifiers::NONE);
    assert_eq!(h.state().session.sel, Some((ids[1], ids[1])));
    type_chars(&mut h, "Z");
    assert_eq!(h.state().session.graph.name(ids[1]), "twoZ");
    type_chars(&mut h, "y");
    assert_eq!(h.state().session.graph.name(ids[1]), "twoZy", "the caret stays in the line");

    // Shift+Down twice, then Escape: the selection is gone but typing lands in its last line.
    let mut h = harness();
    let ids = page_with(&mut h, &["one", "two", "three"]);
    focus(&mut h, ids[0], 0);
    let shift = Modifiers { shift: true, ..Modifiers::NONE };
    key(&mut h, Key::ArrowDown, shift);
    key(&mut h, Key::ArrowDown, shift);
    key(&mut h, Key::Escape, Modifiers::NONE);
    assert_eq!(h.state().session.sel, None);
    type_chars(&mut h, "W");
    let g = &h.state().session.graph;
    let names: Vec<&str> = ids.iter().map(|i| g.name(*i)).collect();
    assert!(names.iter().filter(|n| n.ends_with('W')).count() == 1, "{names:?}");

    // Cmd+E, then Escape: the caret goes back into the line.
    let mut h = harness();
    let ids = page_with(&mut h, &["one", "two"]);
    focus(&mut h, ids[1], 3);
    key(&mut h, Key::E, CMD);
    assert!(h.state().quick_add.is_some());
    key(&mut h, Key::Escape, Modifiers::NONE);
    type_chars(&mut h, "Q");
    assert_eq!(h.state().session.graph.name(ids[1]), "twoQ");

    // Cmd+Shift+O on an ordinary line does nothing; Escape then a letter types into it.
    let mut h = harness();
    let ids = page_with(&mut h, &["one", "two"]);
    focus(&mut h, ids[1], 3);
    key(&mut h, Key::O, Modifiers { shift: true, ..CMD });
    assert!(!h.state().query_open);
    type_chars(&mut h, "x");
    key(&mut h, Key::Escape, Modifiers::NONE);
    type_chars(&mut h, "Q");
    assert_eq!(h.state().session.graph.name(ids[1]), "twoxQ");
}

#[test]
fn diffs_n28_escape_in_a_new_field_name_selects_the_row_and_tab_edits_the_value() {
    let mut h = harness();
    let page = page_with(&mut h, &["Page"])[0];
    h.state_mut().exec("nav.open", json!({"node": page.0})).unwrap();
    let x = tanacraft_core::NodeId(h.state_mut().exec("node.add", json!({"parent": page.0, "name": "x"})).unwrap()["id"].as_u64().unwrap());
    focus(&mut h, x, 1);
    key(&mut h, Key::Enter, Modifiers::NONE);
    type_chars(&mut h, ">Ppl");
    key(&mut h, Key::Escape, Modifiers::NONE);
    let f = h.state().session.graph.find_field("Ppl").unwrap();
    assert_eq!(h.state().field_sel, Some((page, f)));
    assert_eq!(h.state().session.sel, None, "the line above is not selected");
    key(&mut h, Key::Tab, Modifiers::NONE);
    type_chars(&mut h, "Ann");
    assert_eq!(values_of(&h, page, "Ppl"), ["Ann"]);
    // Escape from the value, then Up and Down around a selected field row.
    key(&mut h, Key::Escape, Modifiers::NONE);
    h.state_mut().field_sel = Some((page, f));
    key(&mut h, Key::ArrowUp, Modifiers::NONE);
    assert_eq!(h.state().session.focus, Some(x));
    // Typing on a selected field row goes on at the end of its value.
    h.state_mut().exec("focus.set", json!({})).ok();
    h.state_mut().session.focus = None;
    h.state_mut().field_sel = Some((page, f));
    h.step();
    type_chars(&mut h, "e");
    assert_eq!(values_of(&h, page, "Ppl"), ["Anne"]);
}

#[test]
fn diffs_n31_space_after_a_checkbox_field_name_does_nothing() {
    let mut h = harness();
    h.state_mut().exec("field.create", json!({"name": "Outdoors", "type": "checkbox"})).unwrap();
    let page = page_with(&mut h, &["Page"])[0];
    h.state_mut().exec("nav.open", json!({"node": page.0})).unwrap();
    let x = tanacraft_core::NodeId(h.state_mut().exec("node.add", json!({"parent": page.0, "name": "x"})).unwrap()["id"].as_u64().unwrap());
    focus(&mut h, x, 1);
    key(&mut h, Key::Enter, Modifiers::NONE);
    type_chars(&mut h, ">Outdoors");
    key(&mut h, Key::Tab, Modifiers::NONE);
    let f = h.state().session.graph.find_field("Outdoors").unwrap();
    assert_eq!(h.state().field_sel, Some((page, f)));
    let before = h.state().session.graph.to_json().unwrap();
    key(&mut h, Key::Space, Modifiers::NONE);
    type_chars(&mut h, " ");
    key(&mut h, Key::Tab, Modifiers { shift: true, ..Modifiers::NONE });
    assert_eq!(h.state().session.graph.children(page), &[x], "no stray node");
    assert!(values_of(&h, page, "Outdoors").is_empty() || before == h.state().session.graph.to_json().unwrap());
}
