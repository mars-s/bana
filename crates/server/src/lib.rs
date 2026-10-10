//! TanaCraft host for web front ends.
//!
//! A [`Host`] owns the [`Session`] (the only copy of the data) plus one navigation state per open
//! panel. Front ends talk to it with [`Host::handle`]: a method name and JSON params in, JSON out.
//! The Tauri app calls it through `invoke`; [`http::serve`] exposes the same surface over HTTP so
//! the same web UI runs in a normal browser (and in Playwright).
//!
//! The web UI never owns data: every edit is a command run here, and [`Host::autosave`] writes
//! the file within a second of a change.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod http;
pub mod snapshot;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tanacraft_commands::{NavState, Session};

/// Panels are named by the front end (`"t1p0"`); this one exists from the start.
pub const MAIN_PANEL: &str = "main";

pub struct Host {
    pub session: Session,
    /// Navigation state of every panel except the active one (which lives in the session).
    panels: HashMap<String, NavState>,
    active: String,
    /// Bumped after every request that may have changed what is shown.
    pub version: u64,
    /// Page reads served (a health check: a front end is alive and reading).
    pub reads: u64,
    dirty_since: Option<Instant>,
    /// Tokens of the last unload batches applied: a reloaded page waits for its own batch before
    /// typing kept keys again (else it would type over text still on its way).
    marks: std::collections::VecDeque<String>,
    /// The newest `rev` of each text sent (see "run").
    revs: HashMap<String, u64>,
}

impl Host {
    pub fn new(session: Session) -> Host {
        Host {
            session,
            panels: HashMap::new(),
            active: MAIN_PANEL.to_string(),
            version: 1,
            reads: 0,
            dirty_since: None,
            marks: Default::default(),
            revs: HashMap::new(),
        }
    }

    /// Makes `panel` the session's active panel (creating it on the active page if new).
    fn activate(&mut self, panel: Option<&str>) {
        let Some(p) = panel.filter(|p| *p != self.active) else { return };
        let mine = self.session.nav_state();
        let next = self.panels.remove(p).unwrap_or_else(|| self.session.new_nav(self.session.zoom));
        self.panels.insert(std::mem::replace(&mut self.active, p.to_string()), mine);
        self.session.set_nav_state(next);
    }

    /// One request from a front end. Never panics; errors come back as `{"error": ...}`.
    pub fn handle(&mut self, method: &str, params: &Value) -> Value {
        let panel = params.get("panel").and_then(Value::as_str);
        // Bounded: a hostile client cannot grow the panel map without limit.
        if self.panels.len() > 64 {
            self.panels.clear();
        }
        self.activate(panel);
        match method {
            "run" => {
                let id = params.get("command").and_then(Value::as_str).unwrap_or_default();
                let p = params.get("params").cloned().unwrap_or_else(|| json!({}));
                // Text that arrives late (a request overtaken by the unload batch that repeats it)
                // never types over newer text: a rename or value with an older `rev` is dropped.
                if let (Some(rev), "node.setName" | "field.set") = (params.get("rev").and_then(Value::as_u64), id) {
                    let key = format!("{id}:{}:{}:{}", p["node"], p["field"], p["index"]);
                    if self.revs.get(&key).is_some_and(|r| *r > rev) {
                        return json!({"result": {"stale": true}});
                    }
                    if self.revs.len() > 10_000 {
                        self.revs.clear();
                    }
                    self.revs.insert(key, rev);
                }
                let r = self.session.run(id, &p);
                self.version += 1;
                self.note_change();
                match r {
                    Ok(v) => json!({"result": v}),
                    Err(e) => json!({"error": e.to_string()}),
                }
            }
            "page" => {
                self.reads += 1;
                let mut p = snapshot::page(&self.session, params);
                p["version"] = json!(self.version);
                p
            }
            "sidebar" => snapshot::sidebar(&self.session),
            "find" => snapshot::find(&self.session, params),
            "choices" => snapshot::choices(&self.session, params),
            "view" => snapshot::view(&self.session, params),
            "tags" => snapshot::tags(&self.session),
            "schema" => snapshot::schema(&self.session, params),
            "usedElsewhere" => snapshot::used_elsewhere(&self.session, params),
            "commands" => self.session.registry.describe(),
            "panel.open" => {
                // A new panel showing `node` (or a copy of the active panel's page).
                let node = tanacraft_commands::p::node(params, "node").ok().flatten().filter(|n| self.session.graph.get(*n).is_some());
                let name = params.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
                if name.is_empty() || name == self.active {
                    return json!({"error": "panel.open needs a new `name`"});
                }
                let mut nav = self.session.new_nav(node.unwrap_or(self.session.zoom));
                if node.is_none() {
                    nav.back = self.session.nav_state().back;
                }
                self.panels.insert(name, nav);
                self.version += 1;
                json!({"ok": true})
            }
            "panel.close" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
                if name != self.active && name != MAIN_PANEL {
                    self.panels.remove(name);
                }
                json!({"ok": true})
            }
            "flushMark" => {
                let token = params.get("token").and_then(Value::as_str).unwrap_or_default().chars().take(64).collect::<String>();
                self.marks.push_back(token);
                while self.marks.len() > 32 {
                    self.marks.pop_front();
                }
                json!({"ok": true})
            }
            "marked" => {
                let token = params.get("token").and_then(Value::as_str).unwrap_or_default();
                json!({"marked": self.marks.iter().any(|m| m == token)})
            }
            "save" => match self.save() {
                Ok(()) => json!({"ok": true}),
                Err(e) => json!({"error": e}),
            },
            "version" => json!({"version": self.version, "reads": self.reads}),
            other => json!({"error": format!("unknown method `{other}`")}),
        }
    }

    fn note_change(&mut self) {
        if self.session.graph.is_dirty() && self.dirty_since.is_none() {
            self.dirty_since = Some(Instant::now());
        }
    }

    /// Writes the file now when there are unsaved changes.
    pub fn save(&mut self) -> Result<(), String> {
        self.dirty_since = None;
        let Some(path) = self.session.path.clone() else { return Ok(()) };
        if !self.session.graph.is_dirty() {
            return Ok(());
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        self.session.graph.save(&path).map_err(|e| e.to_string())
    }

    /// Saves when changes have waited `after` (call often, e.g. every 200 ms).
    pub fn autosave(&mut self, after: Duration) {
        if self.dirty_since.is_some_and(|t| t.elapsed() >= after)
            && let Err(e) = self.save()
        {
            self.session.status = e;
        }
    }
}

/// Opens a graph the way the desktop app does: `file` when it exists, else the sample (`sample`)
/// or an empty graph, saved to `file` (or the default path when not a sample). `today` fixes the
/// date (tests and screenshots).
pub fn open_session(file: Option<std::path::PathBuf>, sample: bool, today: Option<tanacraft_core::Date>) -> Result<Session, String> {
    use tanacraft_core::{Date, Graph};
    let today = today.unwrap_or_else(Date::today);
    let (graph, path) = match (&file, sample) {
        (Some(f), _) if f.exists() => (Graph::load(f).map_err(|e| e.to_string())?, Some(f.clone())),
        (f, true) => (tanacraft_core::sample::sample(today).map_err(|e| e.to_string())?, f.clone()),
        (Some(f), false) => (Graph::new(), Some(f.clone())),
        (None, false) => {
            let p = tanacraft_core::default_graph_path();
            (tanacraft_core::load_or_migrate(&p).map_err(|e| e.to_string())?.unwrap_or_default(), Some(p))
        }
    };
    let mut s = Session::with_today(graph, today);
    s.path = path;
    let _ = s.run("nav.today", &json!({}));
    Ok(s)
}

#[cfg(test)]
mod tests;
