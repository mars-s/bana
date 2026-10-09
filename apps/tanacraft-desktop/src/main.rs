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

/// The native menu bar: (title, [(label, command id, accelerator)]). Clipboard and select-all are
/// the system's own items so text editing works as everywhere else.
const MENUS: &[(&str, &[(&str, &str, &str)])] = &[
    ("Edit", &[("Undo", "edit.undo", "CmdOrCtrl+Z"), ("Redo", "edit.redo", "CmdOrCtrl+Shift+Z")]),
    (
        "Go",
        &[
            ("Command line…", "ui.commandLine", "CmdOrCtrl+K"),
            ("Search and open…", "ui.searchOpen", "CmdOrCtrl+S"),
            ("Quick add", "ui.quickAdd", "CmdOrCtrl+E"),
            ("Today", "nav.today", ""),
            ("Back", "nav.back", "CmdOrCtrl+["),
            ("Forward", "nav.forward", "CmdOrCtrl+]"),
            ("Recents", "ui.recents", ""),
        ],
    ),
    (
        "View",
        &[
            ("Toggle sidebar", "ui.toggleSidebar", "CmdOrCtrl+\\"),
            ("Switch light or dark theme", "ui.toggleTheme", ""),
            ("Keyboard shortcuts", "ui.shortcuts", "CmdOrCtrl+/"),
        ],
    ),
    (
        "Window",
        &[
            ("New tab", "ui.newTab", "CmdOrCtrl+T"),
            ("Close tab", "ui.closeTab", "CmdOrCtrl+W"),
            ("Open in new panel", "ui.openInPanel", "CmdOrCtrl+M"),
        ],
    ),
    ("Help", &[("Help", "ui.help", "")]),
];

fn menu_bar(app: &tauri::AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
    let menu = Menu::new(app)?;
    let name = tanacraft_core::PRODUCT_NAME;
    let app_menu = Submenu::with_items(
        app,
        name,
        true,
        &[
            &PredefinedMenuItem::about(app, None, None)?,
            &MenuItem::with_id(app, "ui.settings", "Settings…", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::quit(app, None)?,
        ],
    )?;
    menu.append(&app_menu)?;
    for (title, items) in MENUS {
        let sub = Submenu::new(app, *title, true)?;
        for (label, id, accel) in *items {
            let accel = (!accel.is_empty()).then_some(*accel);
            sub.append(&MenuItem::with_id(app, *id, *label, true, accel)?)?;
        }
        if *title == "Edit" {
            sub.append(&PredefinedMenuItem::separator(app)?)?;
            sub.append(&PredefinedMenuItem::cut(app, None)?)?;
            sub.append(&PredefinedMenuItem::copy(app, None)?)?;
            sub.append(&PredefinedMenuItem::paste(app, None)?)?;
            sub.append(&PredefinedMenuItem::select_all(app, None)?)?;
        }
        if *title == "Window" {
            sub.append(&PredefinedMenuItem::separator(app)?)?;
            sub.append(&PredefinedMenuItem::minimize(app, None)?)?;
            sub.append(&PredefinedMenuItem::fullscreen(app, None)?)?;
        }
        menu.append(&sub)?;
    }
    Ok(menu)
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
            app.set_menu(menu_bar(app.handle())?)?;
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_focus();
            }
            Ok(())
        })
        // Every menu item runs a command: the UI gets its id (spec 00 §7).
        .on_menu_event(|app, event| {
            let _ = app.emit("menu", event.id().0.clone());
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
