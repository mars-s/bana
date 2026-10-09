//! TanaCraft desktop app.
//!
//! `tanacraft [FILE] [--sample] [--control PORT]`
//! - FILE: the graph file (default `~/TanaCraft/graph.json`, created on first save). Autosaves.
//! - `--sample`: start from the built-in sample graph (not saved unless FILE is given).
//! - `--control PORT`: accept JSON lines on 127.0.0.1:PORT, e.g.
//!   `{"method":"command","params":{"command":"nav.today"}}`; replies are JSON lines.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::mpsc;

use serde_json::{Value, json};
use tanacraft_commands::Session;
use tanacraft_core::{Date, Graph, PRODUCT_NAME, sample};
use tanacraft_ui_egui::{ControlRequest, TanaApp};

struct App(TanaApp, #[cfg(target_os = "macos")] Option<menu::Bar>);

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        #[cfg(target_os = "macos")]
        if let Some(bar) = &self.1 {
            for cmd in bar.clicked() {
                self.0.menu_item(&cmd);
                ctx.request_repaint();
            }
        }
        self.0.logic(ctx);
        if self.0.control.is_some() {
            // ponytail: poll the control channel 10 times a second; wake from the thread if idle cost matters.
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.0.ui(ui);
    }
    fn on_exit(&mut self) {
        // Text still in an open editor (a field value or name, a description...) is saved too
        // (DIFFS N26).
        if let Err(e) = self.0.save_on_quit() {
            eprintln!("{PRODUCT_NAME}: could not save: {e}");
        }
    }
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("usage: tanacraft [FILE] [--sample] [--control PORT]");
        return Ok(());
    }
    let sample_mode = args.iter().any(|a| a == "--sample");
    let control = args.iter().position(|a| a == "--control").and_then(|i| args.get(i + 1)).and_then(|p| p.parse::<u16>().ok());
    let file = args
        .iter()
        .enumerate()
        .find(|(i, a)| !a.starts_with("--") && (*i == 0 || args.get(i - 1).map(String::as_str) != Some("--control")))
        .map(|(_, a)| PathBuf::from(a));

    let today = Date::today();
    let (graph, path) = match (&file, sample_mode) {
        (Some(f), _) if f.exists() => (Graph::load(f).map_err(|e| e.to_string())?, Some(f.clone())),
        (f, true) => (sample::sample(today).map_err(|e| e.to_string())?, f.clone()),
        (Some(f), false) => (Graph::new(), Some(f.clone())),
        (None, false) => {
            let p = tanacraft_core::default_graph_path();
            if p.exists() { (Graph::load(&p).map_err(|e| e.to_string())?, Some(p)) } else { (Graph::new(), Some(p)) }
        }
    };
    if let Some(dir) = path.as_ref().and_then(|p| p.parent()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let mut session = Session::with_today(graph, today);
    session.path = path;
    let _ = session.run("nav.today", &json!({}));
    let mut app = TanaApp::new(session);
    app.autosave = true;
    // Custom keybindings: `keybindings.json` beside the default graph file.
    let keys = tanacraft_core::default_graph_path().with_file_name("keybindings.json");
    if let Ok(text) = std::fs::read_to_string(&keys)
        && let Err(e) = app.set_keybindings(&text)
    {
        eprintln!("{}: {e}", keys.display());
    }
    // Copying nodes puts formatted HTML and plain text on the clipboard (spec 02 §12); Edit > Paste
    // reads it back.
    if let Ok(mut clip) = arboard::Clipboard::new() {
        app.rich_clipboard = Some(Box::new(move |html: &str, text: &str| {
            if let Err(e) = clip.set_html(html, Some(text)) {
                eprintln!("clipboard: {e}");
            }
        }));
    }
    if let Ok(mut clip) = arboard::Clipboard::new() {
        app.clipboard_text = Some(Box::new(move || clip.get_text().ok()));
    }
    if let Some(port) = control {
        app.control = Some(control_server(port)?);
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 820.0]).with_min_inner_size([640.0, 400.0]).with_title(PRODUCT_NAME),
        ..Default::default()
    };
    eframe::run_native(
        PRODUCT_NAME,
        options,
        Box::new(|_cc| {
            // The native menu bar is made once the app object exists (spec 00 §7).
            #[cfg(target_os = "macos")]
            let bar = menu::Bar::install(&app);
            let app = App(app, bar);
            #[cfg(not(target_os = "macos"))]
            let app = App(app);
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| e.to_string())
}

/// The macOS menu bar, built from `MENU_BAR`: each item runs a command.
#[cfg(target_os = "macos")]
mod menu {
    use muda::accelerator::Accelerator;
    use muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
    use tanacraft_ui_egui::{MENU_BAR, TanaApp};

    pub struct Bar {
        _menu: Menu,
        ids: Vec<(muda::MenuId, &'static str)>,
    }

    impl Bar {
        pub fn install(app: &TanaApp) -> Option<Bar> {
            let menu = Menu::new();
            let mut ids = Vec::new();
            for (title, items) in MENU_BAR {
                let sub = Submenu::new(if *title == "App" { tanacraft_core::PRODUCT_NAME } else { title }, true);
                for (label, cmd) in *items {
                    // Each item shows its key (DIFFS N25). A key the menu takes comes back as a
                    // menu event and is replayed as that key (`TanaApp::menu_item`).
                    let mut accel =
                        app.menu_shortcut(cmd).and_then(|sc| sc.replace("Mod+", "CmdOrCtrl+").replace("Alt+", "Option+").parse::<Accelerator>().ok());
                    if *label == "Paste and match style" {
                        accel = "CmdOrCtrl+Option+Shift+V".parse::<Accelerator>().ok();
                    }
                    if *title == "App" && *cmd == "ui.quit" {
                        sub.append(&PredefinedMenuItem::separator()).ok()?;
                        sub.append(&PredefinedMenuItem::hide(None)).ok()?;
                        sub.append(&PredefinedMenuItem::separator()).ok()?;
                    }
                    let item = MenuItem::new(*label, true, accel);
                    ids.push((item.id().clone(), *cmd));
                    sub.append(&item).ok()?;
                }
                if *title == "Window" {
                    sub.append(&PredefinedMenuItem::separator()).ok()?;
                    sub.append(&PredefinedMenuItem::maximize(Some("Zoom"))).ok()?;
                    sub.append(&PredefinedMenuItem::bring_all_to_front(None)).ok()?;
                }
                menu.append(&sub).ok()?;
            }
            menu.init_for_nsapp();
            Some(Bar { _menu: menu, ids })
        }

        /// Commands of the items clicked since the last frame.
        pub fn clicked(&self) -> Vec<String> {
            let mut out = Vec::new();
            while let Ok(ev) = MenuEvent::receiver().try_recv() {
                if let Some((_, cmd)) = self.ids.iter().find(|(id, _)| *id == ev.id) {
                    out.push(cmd.to_string());
                }
            }
            out
        }
    }
}

/// Listens on 127.0.0.1 only. Each line is a JSON request; each reply a JSON line.
fn control_server(port: u16) -> Result<mpsc::Receiver<ControlRequest>, String> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", port)).map_err(|e| format!("control port {port}: {e}"))?;
    let (tx, rx) = mpsc::channel::<ControlRequest>();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let tx = tx.clone();
            std::thread::spawn(move || {
                let Ok(read) = stream.try_clone() else { return };
                let mut out = stream;
                for line in BufReader::new(read).lines().map_while(Result::ok) {
                    let reply = match serde_json::from_str::<Value>(&line) {
                        Ok(req) => {
                            let (rtx, rrx) = mpsc::channel();
                            let method = req.get("method").and_then(Value::as_str).unwrap_or_default().to_string();
                            let params = req.get("params").cloned().unwrap_or(Value::Null);
                            if tx.send(ControlRequest { method, params, reply: rtx }).is_err() {
                                return;
                            }
                            let mut r = rrx.recv_timeout(std::time::Duration::from_secs(10)).unwrap_or(json!({"error": "app did not answer"}));
                            if let Some(id) = req.get("id") {
                                r["id"] = id.clone();
                            }
                            r
                        }
                        Err(e) => json!({"error": format!("bad json: {e}")}),
                    };
                    if writeln!(out, "{reply}").is_err() {
                        return;
                    }
                }
            });
        }
    });
    Ok(rx)
}
