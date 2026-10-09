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

struct App(TanaApp);

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
        if self.0.session.path.is_some() && self.0.session.graph.is_dirty() {
            let _ = self.0.session.run("file.save", &json!({}));
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
    if let Some(port) = control {
        app.control = Some(control_server(port)?);
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 820.0]).with_min_inner_size([640.0, 400.0]).with_title(PRODUCT_NAME),
        ..Default::default()
    };
    eframe::run_native(PRODUCT_NAME, options, Box::new(|_cc| Ok(Box::new(App(app))))).map_err(|e| e.to_string())
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
