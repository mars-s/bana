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
    /// The highest write-ahead-log number applied per front end page (`wal: [client, seq]` on a
    /// request): the same edit sent twice (in flight at a reload, then replayed by the next page,
    /// or also in the unload beacon) runs once.
    applied: HashMap<String, u64>,
    /// The newest `rev` of each text sent (see "run").
    revs: HashMap<String, u64>,
    /// The first window (`window` on a request) to talk to the host uses the plain panel names
    /// ("main"); every other window gets its own (`<window>:main`), so each has its own page,
    /// caret and selection, and two windows never type into each other's line (DIFFS N60).
    primary: Option<String>,
    /// A field name being typed after `>` (owner, line, name): saves write it in as a field, so a
    /// force-kill keeps it (DIFFS N26); the live graph is left as it is.
    draft: Option<(u64, u64, String)>,
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
            applied: HashMap::new(),
            revs: HashMap::new(),
            draft: None,
            primary: None,
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
        let own;
        let params = match self.window_params(method, params) {
            Some(p) => {
                own = p;
                &own
            }
            None => params,
        };
        let panel = params.get("panel").and_then(Value::as_str);
        // Bounded: a hostile client cannot grow the panel map without limit.
        if self.panels.len() > 64 {
            self.panels.clear();
        }
        if let Some((client, seq)) = params.get("wal").and_then(Value::as_array).and_then(|w| Some((w.first()?.as_str()?, w.get(1)?.as_u64()?))) {
            let client: String = client.chars().take(64).collect();
            if self.applied.get(&client).is_some_and(|done| *done >= seq) {
                return json!({"result": null, "dup": true});
            }
            // ponytail: bounded by clearing; a duplicate older than 1024 page loads cannot arrive.
            if self.applied.len() > 1024 {
                self.applied.clear();
            }
            self.applied.insert(client, seq);
        }
        self.activate(panel);
        match method {
            "run" => {
                let id = params.get("command").and_then(Value::as_str).unwrap_or_default();
                let p = params.get("params").cloned().unwrap_or_else(|| json!({}));
                // Text that arrives late (a request overtaken by the unload batch that repeats it)
                // never types over newer text: a rename or value with an older `rev` is dropped.
                // A line typed in two windows at once: this window's change is rebased over the
                // text the other one wrote since (`base` is what this window started from).
                let mut p = p;
                if id == "node.setName" {
                    rebase_name(&self.session, &mut p);
                }
                // (Per window: two windows' clocks and queues are independent.)
                if let (Some(rev), "node.setName" | "field.set") = (params.get("rev").and_then(Value::as_u64), id) {
                    let client = params.get("wal").and_then(|w| w.get(0)).and_then(Value::as_str).unwrap_or_default();
                    let key = format!("{id}:{client}:{}:{}:{}", p["node"], p["field"], p["index"]);
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
            "fieldDraft" => {
                let owner = params.get("owner").and_then(Value::as_u64);
                let line = params.get("line").and_then(Value::as_u64);
                let name = params.get("name").and_then(Value::as_str).unwrap_or_default().trim().to_string();
                self.draft = match (owner, line) {
                    (Some(o), Some(l)) if !name.is_empty() => Some((o, l, name)),
                    _ => None,
                };
                if self.draft.is_some() {
                    self.note_change();
                }
                json!({"ok": true})
            }
            "save" => match self.save() {
                Ok(()) => json!({"ok": true}),
                Err(e) => json!({"error": e}),
            },
            "version" => json!({"version": self.version, "reads": self.reads}),
            other => json!({"error": format!("unknown method `{other}`")}),
        }
    }

    /// Requests from a window that is not the primary one name their panels in its own space.
    fn window_params(&mut self, method: &str, params: &Value) -> Option<Value> {
        let window: String = params.get("window").and_then(Value::as_str)?.chars().take(64).collect();
        let primary = self.primary.get_or_insert_with(|| window.clone());
        if *primary == window {
            // A request with no panel is for this window's main panel, not the last one used.
            if params.get("panel").is_some() {
                return None;
            }
            let mut p = params.clone();
            p["panel"] = json!(MAIN_PANEL);
            return Some(p);
        }
        let mut p = params.clone();
        let mine = |name: &str| format!("{window}:{name}");
        let panel = p.get("panel").and_then(Value::as_str).unwrap_or(MAIN_PANEL).to_string();
        p["panel"] = json!(mine(&panel));
        if let (true, Some(n)) = (matches!(method, "panel.open" | "panel.close"), p.get("name").and_then(Value::as_str).map(str::to_string)) {
            p["name"] = json!(mine(&n));
        }
        Some(p)
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
        if !self.session.graph.is_dirty() && self.draft.is_none() {
            return Ok(());
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        if let Some((owner, line, name)) = self.draft.clone() {
            // The file gets the field the name is turning into; the live graph stays as it is.
            let live = self.session.graph.clone();
            let mut state = (self.session.focus, self.session.caret, self.session.anchor, self.session.sel);
            let r = self
                .session
                .run("field.add", &json!({"node": owner, "field": name}))
                .and_then(|_| self.session.run("node.delete", &json!({"node": line, "force": true})));
            let saved = if r.is_ok() { self.session.graph.save(&path).map_err(|e| e.to_string()) } else { Ok(()) };
            self.session.graph = live;
            (self.session.focus, self.session.caret, self.session.anchor, self.session.sel) = std::mem::take(&mut state);
            if r.is_err() {
                return self.session.graph.save(&path).map_err(|e| e.to_string());
            }
            return saved;
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

/// `p` renames a line from `base` (what the window had) to `name`; when the line changed since
/// (another window typed in it), the window's own change is moved over that one: one edit each
/// side, the other window's text stays, and the caret moves with this window's edit.
fn rebase_name(s: &Session, p: &mut Value) {
    let (Some(base), Some(mine)) = (p.get("base").and_then(Value::as_str), p.get("name").and_then(Value::as_str)) else { return };
    let Some(node) = tanacraft_commands::p::node(p, "node").ok().flatten() else { return };
    let current = s.graph.name(s.graph.resolve(node));
    if current == base || current == mine {
        return;
    }
    let (text, shift) = rebase(base, mine, current);
    if let Some(c) = p.get("caret").and_then(Value::as_u64) {
        p["caret"] = json!((c as i64 + shift).max(0));
    }
    p["name"] = json!(text);
}

/// One splice turning `a` into `b`: (start, chars removed, text inserted), in chars.
fn splice(a: &[char], b: &[char]) -> (usize, usize, Vec<char>) {
    let pre = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let suf = a[pre..].iter().rev().zip(b[pre..].iter().rev()).take_while(|(x, y)| x == y).count();
    (pre, a.len() - pre - suf, b[pre..b.len() - suf].to_vec())
}

/// `mine` and `theirs` both edited `base`: `mine` applied on `theirs`, and how far it moved.
/// ponytail: one splice per side; overlapping edits keep `mine` whole (last writer wins).
pub fn rebase(base: &str, mine: &str, theirs: &str) -> (String, i64) {
    let (base, mine_c, theirs_c): (Vec<char>, Vec<char>, Vec<char>) = (base.chars().collect(), mine.chars().collect(), theirs.chars().collect());
    let (p1, d1, ins1) = splice(&base, &mine_c);
    let (p2, d2, ins2) = splice(&base, &theirs_c);
    let at = if p2 + d2 <= p1 {
        p1 + ins2.len() - d2
    } else if p2 >= p1 + d1 {
        p1
    } else {
        return (mine.to_string(), 0);
    };
    let mut out = theirs_c;
    out.splice(at..at + d1, ins1);
    (out.into_iter().collect(), at as i64 - p1 as i64)
}
