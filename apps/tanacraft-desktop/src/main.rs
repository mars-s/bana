//! TanaCraft desktop app: a Tauri window showing the web UI, with the engine in this process.
//!
//! `tanacraft [FILE] [--sample] [--serve PORT]`
//! - FILE: the graph file (default `~/TanaCraft/graph.tanacraft`). Autosaves within a second; saves on quit.
//! - `--sample`: start from the built-in sample graph (not saved unless FILE is given).
//! - `--serve PORT`: also serve the same engine over HTTP on 127.0.0.1 (agents, a browser window).
//!
//! The UI talks to the engine through one command, `rpc(method, params)`, the same surface as
//! `POST /rpc` of `tanacraft-cli serve`; a `changed` event tells it to read again.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
// No console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;
use tanacraft_server::Host;
use tanacraft_server::http::Shared;
use tauri::{Emitter, Manager};

#[tauri::command]
fn rpc(method: String, params: Value, shared: tauri::State<'_, Arc<Shared>>, app: tauri::AppHandle) -> Value {
    let before = shared.host.lock().map(|h| h.version).unwrap_or(0);
    let out = shared.handle(&serde_json::json!({"method": method, "params": params}));
    let after = shared.host.lock().map(|h| h.version).unwrap_or(0);
    if after != before {
        let _ = app.emit("changed", after);
    }
    out
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{}: {e}", tanacraft_core::PRODUCT_NAME);
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("usage: tanacraft [FILE] [--sample] [--serve PORT]");
        return Ok(());
    }
    let sample = args.iter().any(|a| a == "--sample");
    let serve = args.iter().position(|a| a == "--serve").and_then(|i| args.get(i + 1)).and_then(|p| p.parse::<u16>().ok());
    let file = args
        .iter()
        .enumerate()
        .find(|(i, a)| !a.starts_with("--") && (*i == 0 || args.get(i - 1).map(String::as_str) != Some("--serve")))
        .map(|(_, a)| PathBuf::from(a));
    let session = tanacraft_server::open_session(file, sample, None)?;
    let shared = Shared::new(Host::new(session));
    shared.spawn_autosave();
    if let Some(port) = serve {
        let s = Arc::clone(&shared);
        let web = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../web/dist");
        std::thread::spawn(move || {
            if let Err(e) = tanacraft_server::http::serve(s, port, web) {
                eprintln!("serve: {e}");
            }
        });
    }
    let on_exit = Arc::clone(&shared);
    let app = tauri::Builder::default()
        .manage(shared)
        .invoke_handler(tauri::generate_handler![rpc])
        .setup(|app| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_focus();
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .map_err(|e| e.to_string())?;
    app.run(move |_, event| {
        // Quitting writes everything (the UI has sent every keystroke already).
        if let tauri::RunEvent::Exit | tauri::RunEvent::ExitRequested { .. } = event
            && let Ok(mut h) = on_exit.host.lock()
            && let Err(e) = h.save()
        {
            eprintln!("{}: could not save: {e}", tanacraft_core::PRODUCT_NAME);
        }
    });
    Ok(())
}
