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

/// The native menu bar (spec 00 §7, DIFFS N75): (title, [(label, command id, accelerator)]). An
/// empty label is a separator. Clipboard, select-all, minimize, zoom and full screen are the
/// system's own items so they work as everywhere else; `window.*` ids are handled here.
type MenuItems = &'static [(&'static str, &'static str, &'static str)];
const MENUS: &[(&str, MenuItems)] = &[
    (
        "Edit",
        &[
            ("Undo", "edit.undo", "CmdOrCtrl+Z"),
            ("Redo", "edit.redo", "CmdOrCtrl+Shift+Z"),
            ("", "", ""),
            ("Cut", "", ""),
            ("Copy", "", ""),
            ("Paste", "", ""),
            ("Select All", "", ""),
            ("", "", ""),
            ("Find…", "ui.searchOpen", "CmdOrCtrl+S"),
            ("Command line…", "ui.commandLine", "CmdOrCtrl+K"),
            ("Quick add", "ui.quickAdd", "CmdOrCtrl+E"),
        ],
    ),
    (
        "View",
        &[
            ("Reload", "window.reload", "CmdOrCtrl+R"),
            ("", "", ""),
            ("Actual Size", "window.zoomReset", "CmdOrCtrl+0"),
            ("Zoom In", "window.zoomIn", "CmdOrCtrl+="),
            ("Zoom Out", "window.zoomOut", "CmdOrCtrl+-"),
            ("", "", ""),
            ("Toggle Full Screen", "", ""),
            ("", "", ""),
            ("Toggle sidebar", "ui.toggleSidebar", "CmdOrCtrl+\\"),
            ("Switch light or dark theme", "ui.toggleTheme", ""),
        ],
    ),
    (
        "History",
        &[
            ("Back", "nav.back", "CmdOrCtrl+["),
            ("Forward", "nav.forward", "CmdOrCtrl+]"),
            ("", "", ""),
            ("Today", "nav.today", ""),
            ("Recents", "ui.recents", ""),
        ],
    ),
    ("Options", &[("Customize Keybindings…", "ui.settings", ""), ("Settings…", "ui.settings", "CmdOrCtrl+,")]),
    (
        "Window",
        &[
            ("Minimize", "", ""),
            ("Zoom", "", ""),
            ("", "", ""),
            ("New tab", "ui.newTab", "CmdOrCtrl+T"),
            ("Close tab", "ui.closeTab", "CmdOrCtrl+W"),
            ("Open in new panel", "ui.openInPanel", "CmdOrCtrl+M"),
        ],
    ),
    ("Help", &[("Help", "ui.help", ""), ("Keyboard shortcuts", "ui.shortcuts", "CmdOrCtrl+/")]),
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
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::show_all(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::quit(app, None)?,
        ],
    )?;
    menu.append(&app_menu)?;
    for (title, items) in MENUS {
        let sub = Submenu::new(app, *title, true)?;
        for (label, id, accel) in *items {
            match (*label, *id) {
                ("", _) => sub.append(&PredefinedMenuItem::separator(app)?)?,
                ("Cut", "") => sub.append(&PredefinedMenuItem::cut(app, None)?)?,
                ("Copy", "") => sub.append(&PredefinedMenuItem::copy(app, None)?)?,
                ("Paste", "") => sub.append(&PredefinedMenuItem::paste(app, None)?)?,
                ("Select All", "") => sub.append(&PredefinedMenuItem::select_all(app, None)?)?,
                ("Minimize", "") => sub.append(&PredefinedMenuItem::minimize(app, None)?)?,
                ("Zoom", "") => sub.append(&PredefinedMenuItem::maximize(app, None)?)?,
                ("Toggle Full Screen", "") => sub.append(&PredefinedMenuItem::fullscreen(app, None)?)?,
                _ => {
                    let accel = (!accel.is_empty()).then_some(*accel);
                    // Two items may run one command (Settings): menu ids must be unique.
                    let item_id = format!("{id}#{label}");
                    sub.append(&MenuItem::with_id(app, item_id, *label, true, accel)?)?
                }
            }
        }
        menu.append(&sub)?;
    }
    Ok(menu)
}

/// Page zoom of the window (View > Actual Size, Zoom In, Zoom Out).
static ZOOM: std::sync::Mutex<f64> = std::sync::Mutex::new(1.0);

/// A `window.*` menu item: reload or zoom the web view. Other ids go to the UI.
fn window_action(app: &tauri::AppHandle, id: &str) -> bool {
    let Some(w) = app.get_webview_window("main") else { return false };
    let zoom = |f: &dyn Fn(f64) -> f64| {
        if let Ok(mut z) = ZOOM.lock() {
            *z = f(*z).clamp(0.5, 3.0);
            let _ = w.set_zoom(*z);
        }
    };
    match id {
        // The write-ahead log keeps whatever was still on its way (see engine.ts).
        "window.reload" => {
            let _ = w.eval("location.reload()");
        }
        "window.zoomReset" => zoom(&|_| 1.0),
        "window.zoomIn" => zoom(&|z| z + 0.1),
        "window.zoomOut" => zoom(&|z| z - 0.1),
        _ => return false,
    }
    true
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
    // The same engine over HTTP on 127.0.0.1 (any free port unless --serve PORT), announced in
    // the live file beside the graph so MCP and the CLI work on the running app (UC-10.11).
    let listener = tanacraft_server::http::bind(serve.unwrap_or(0))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let live = tanacraft_server::http::announce(&shared, port, None);
    {
        let s = Arc::clone(&shared);
        let web = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../web/dist");
        std::thread::spawn(move || {
            if let Err(e) = tanacraft_server::http::serve_on(s, listener, web) {
                eprintln!("serve: {e}");
            }
        });
    }
    let on_exit = Arc::clone(&shared);
    let watched = Arc::clone(&shared);
    let app = tauri::Builder::default()
        .manage(shared)
        .invoke_handler(tauri::generate_handler![rpc])
        .setup(move |app| {
            app.set_menu(menu_bar(app.handle())?)?;
            // Edits that come over HTTP (MCP, the CLI, a browser window) redraw the app too.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let mut seen = watched.host.lock().map(|h| h.version).unwrap_or(0);
                loop {
                    let Ok(guard) = watched.host.lock() else { return };
                    let Ok((h, _)) = watched.changed.wait_timeout_while(guard, std::time::Duration::from_secs(30), |h| h.version <= seen) else {
                        return;
                    };
                    if h.version > seen {
                        seen = h.version;
                        drop(h);
                        let _ = handle.emit("changed", seen);
                    }
                }
            });
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_focus();
            }
            Ok(())
        })
        // Every menu item runs a command: the UI gets its id (spec 00 §7).
        .on_menu_event(|app, event| {
            let id = event.id().0.split('#').next().unwrap_or_default().to_string();
            if !window_action(app, &id) {
                let _ = app.emit("menu", id);
            }
        })
        .build(tauri::generate_context!())
        .map_err(|e| e.to_string())?;
    app.run(move |_, event| {
        // Quitting writes everything (the UI has sent every keystroke already).
        if let tauri::RunEvent::Exit | tauri::RunEvent::ExitRequested { .. } = event {
            if let Ok(mut h) = on_exit.host.lock()
                && let Err(e) = h.save()
            {
                eprintln!("{}: could not save: {e}", tanacraft_core::PRODUCT_NAME);
            }
            if let (tauri::RunEvent::Exit, Some(l)) = (&event, &live) {
                let _ = std::fs::remove_file(l);
            }
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::MENUS;

    #[test]
    fn menu_bar_follows_the_spec_and_every_item_runs_a_known_command() {
        // Spec 00 §7: (app menu), Edit, View, History, Options, Window, Help.
        let titles: Vec<&str> = MENUS.iter().map(|(t, _)| *t).collect();
        assert_eq!(titles, ["Edit", "View", "History", "Options", "Window", "Help"]);
        let session = tanacraft_commands::Session::new(tanacraft_core::Graph::new());
        let known = session.registry.describe().to_string();
        for (_, items) in MENUS {
            for (label, id, _) in *items {
                if id.is_empty() || id.starts_with("ui.") || id.starts_with("window.") {
                    continue;
                }
                assert!(known.contains(&format!("\"{id}\"")), "{label}: unknown command {id}");
            }
        }
    }
}
