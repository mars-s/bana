use serde_json::json;
use tanacraft_commands::Session;
use tanacraft_core::{Date, sample};

use super::*;

fn host() -> Host {
    let today = Date::new(2026, 10, 10).unwrap();
    let mut s = Session::with_today(sample::sample(today).unwrap(), today);
    s.run("nav.today", &json!({})).unwrap();
    Host::new(s)
}

#[test]
fn page_lists_rows_and_typing_reaches_the_graph() {
    let mut h = host();
    let p = h.handle("page", &json!({}));
    assert!(p["title"].as_str().unwrap().starts_with("Today"));
    let rows = p["rows"]["items"].as_array().unwrap();
    assert!(!rows.is_empty());
    let first = rows.iter().find(|r| r["k"] == "node").unwrap()["id"].as_u64().unwrap();
    let r = h.handle("run", &json!({"command": "node.setName", "params": {"node": first, "name": "typed", "caret": 5}}));
    assert!(r.get("error").is_none(), "{r}");
    assert_eq!(h.session.graph.name(tanacraft_core::NodeId(first)), "typed");
}

#[test]
fn panels_keep_their_own_page() {
    let mut h = host();
    let today = h.session.zoom;
    h.handle("panel.open", &json!({"name": "p2", "node": h.session.graph.sys.library.0}));
    let p2 = h.handle("page", &json!({"panel": "p2"}));
    assert_eq!(p2["zoom"], json!(h.session.graph.sys.library.0));
    let main = h.handle("page", &json!({"panel": MAIN_PANEL}));
    assert_eq!(main["zoom"], json!(today.0));
}

#[test]
fn windows_are_clamped_and_bad_input_is_an_error() {
    let mut h = host();
    let p = h.handle("page", &json!({"start": 1_000_000, "count": 99_999_999}));
    assert_eq!(p["rows"]["items"].as_array().unwrap().len(), 0);
    assert!(h.handle("run", &json!({"command": "nope"}))["error"].is_string());
    assert!(h.handle("bogus", &json!({}))["error"].is_string());
    assert!(h.handle("sidebar", &json!({}))["pinned"].is_array());
}

#[test]
fn origins_from_other_sites_are_refused() {
    assert!(http_origin_ok(None));
    assert!(http_origin_ok(Some("http://127.0.0.1:5173")));
    assert!(http_origin_ok(Some("http://localhost:8787")));
    assert!(http_origin_ok(Some("tauri://localhost")));
    assert!(!http_origin_ok(Some("https://evil.example")));
    assert!(!http_origin_ok(Some("null")));
    assert!(!http_origin_ok(Some("http://localhost.evil.example")));
}

fn http_origin_ok(o: Option<&str>) -> bool {
    crate::http::origin_ok(o)
}
