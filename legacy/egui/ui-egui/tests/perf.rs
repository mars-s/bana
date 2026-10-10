//! Frame times of the real UI on a generated 50,000-node workspace (headless, no renderer): an
//! outline with every node expanded, typing into it, expanding and collapsing, Search and open.
//! README numbers: `cargo test -p bana-ui-egui --test perf --release -- --nocapture`.

use std::time::{Duration, Instant};

use egui::{Event, Key, Modifiers};
use serde_json::json;
use bana_commands::Session;
use bana_core::{Date, sample};
use bana_ui_egui::TanaApp;

type H<'a> = egui_kittest::Harness<'a, TanaApp>;

fn ms(d: Duration) -> String {
    format!("{:.2} ms", d.as_secs_f64() * 1000.0)
}

/// Average time of one frame that handles `ev` (pushed before every frame when given).
fn frames(h: &mut H<'_>, n: u32, mut ev: impl FnMut(u32) -> Option<Event>) -> Duration {
    let start = Instant::now();
    for i in 0..n {
        if let Some(e) = ev(i) {
            h.input_mut().events.push(e);
        }
        h.step();
    }
    start.elapsed() / n
}

#[test]
fn fifty_thousand_nodes_draw_fast() {
    let today = Date::new(2026, 10, 10).unwrap_or_else(|| panic!("date"));
    let s = Session::with_today(sample::large(50_000).unwrap_or_else(|e| panic!("{e}")), today);
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
    let home = h.state().session.graph.sys.library;
    h.state_mut().exec("nav.open", json!({"node": home.0})).unwrap_or_else(|e| panic!("{e}"));
    h.state_mut().exec("view.expandAll", json!({})).unwrap_or_else(|e| panic!("{e}"));
    let t = Instant::now();
    h.step();
    let open = t.elapsed();
    let idle = frames(&mut h, 10, |_| None);

    let g = &h.state().session.graph;
    let first = g.children(home)[0];
    let item = g.children(g.children(first)[0])[5];
    h.state_mut().exec("focus.set", json!({"node": item.0, "caret": 3})).unwrap_or_else(|e| panic!("{e}"));
    h.step();
    let typing = frames(&mut h, 20, |_| Some(Event::Text("x".into())));
    assert!(h.state().session.graph.name(item).contains("xxxxx"), "typing reached the node");

    let toggle = frames(&mut h, 10, |_| {
        Some(Event::Key {
            key: Key::ArrowDown,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers { command: true, mac_cmd: true, ..Modifiers::NONE },
        })
    });

    h.state_mut().exec("focus.set", json!({"node": item.0})).unwrap_or_else(|e| panic!("{e}"));
    let rows = h.state().session.rows().len();
    println!("{} nodes, {rows} visible rows", h.state().session.graph.len());
    println!("first frame (open, expand all): {}", ms(open));
    println!("idle frame (expanded outline): {}", ms(idle));
    println!("typing one character (frame):  {}", ms(typing));
    println!("expand key (frame):            {}", ms(toggle));
    let budget = Duration::from_millis(if cfg!(debug_assertions) { 1500 } else { 50 });
    for (what, d) in [("open", open), ("idle", idle), ("typing", typing), ("expand", toggle)] {
        assert!(d < budget, "{what} frame took {}", ms(d));
    }
}
