//! Bana engine for Swift (and any other UniFFI language).
//!
//! An [`Engine`] is the same [`Host`] the desktop app and `bana serve` run: the session
//! (the only copy of the data) plus per-panel navigation. The phone app talks to it with the same
//! surface as the web UI: [`Engine::rpc`] takes a method (`run`, `page`, `sidebar`, `find`, ...) and
//! JSON params and answers JSON, so snapshots have exactly the shapes `crates/server` documents.
//!
//! Every edit goes to the engine at once; the engine writes the graph file (a Loro document,
//! byte-for-byte the desktop format) within half a second of a change, and at once on
//! [`Engine::save`] (the app calls it when it goes to the background).
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use bana_server::Host;
use bana_server::http::Shared;
use serde_json::{Value, json};

uniffi::setup_scaffolding!();

/// How long a change waits before the autosave writes it.
const AUTOSAVE_AFTER: Duration = Duration::from_millis(500);

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum EngineError {
    #[error("{msg}")]
    Open { msg: String },
    #[error("{msg}")]
    Save { msg: String },
}

/// Told after every request that may have changed what is shown (the web UI's `changed` event).
/// Called on the thread that made the request, after the engine lock is released, so it may call
/// back into the engine.
#[uniffi::export(with_foreign)]
pub trait ChangeListener: Send + Sync {
    fn changed(&self, version: u64);
}

type Listeners = Arc<Mutex<Vec<Arc<dyn ChangeListener>>>>;

#[derive(uniffi::Object)]
pub struct Engine {
    shared: Arc<Shared>,
    listeners: Listeners,
}

#[uniffi::export]
impl Engine {
    /// Opens `path` (created on the first save when missing). `sample`: a missing file starts as
    /// the built-in sample graph instead of an empty one. `today` (`YYYY-MM-DD`) fixes the date
    /// for tests and screenshots.
    #[uniffi::constructor]
    pub fn open(path: String, sample: bool, today: Option<String>) -> Result<Arc<Engine>, EngineError> {
        let today = match today.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
            Some(t) => Some(bana_core::Date::parse(t).ok_or_else(|| EngineError::Open { msg: format!("bad date `{t}`") })?),
            None => None,
        };
        let session = bana_server::open_session(Some(PathBuf::from(path)), sample, today).map_err(|msg| EngineError::Open { msg })?;
        let listeners: Listeners = Arc::new(Mutex::new(Vec::new()));
        let engine = Arc::new(Engine { shared: Shared::new(Host::new(session)), listeners: Arc::clone(&listeners) });
        // Edits merged from another device redraw the app like local ones.
        if let Ok(mut f) = engine.shared.on_change.lock() {
            *f = Some(Box::new(move |version| {
                let all: Vec<Arc<dyn ChangeListener>> = listeners.lock().map(|l| l.clone()).unwrap_or_default();
                for l in all {
                    l.changed(version);
                }
            }));
        }
        spawn_autosave(Arc::downgrade(&engine.shared));
        Ok(engine)
    }

    /// One request, as `POST /rpc` of the web UI: `params_json` is an object (empty means `{}`).
    /// Never panics: bad input answers `{"error": ...}`.
    pub fn rpc(&self, method: String, params_json: String) -> String {
        let params = match parse(&params_json) {
            Ok(p) => p,
            Err(e) => return json!({"error": e}).to_string(),
        };
        self.request(&json!({"method": method, "params": params})).to_string()
    }

    /// Runs the command `command` (an id such as `node.setName`) with JSON params; answers
    /// `{"result": ...}` or `{"error": ...}`.
    pub fn run(&self, command: String, params_json: String) -> String {
        let params = match parse(&params_json) {
            Ok(p) => p,
            Err(e) => return json!({"error": e}).to_string(),
        };
        self.request(&json!({"method": "run", "params": {"command": command, "params": params}})).to_string()
    }

    /// Writes the file now if anything changed.
    pub fn save(&self) -> Result<(), EngineError> {
        let mut host = self.shared.host.lock().map_err(|_| EngineError::Save { msg: "engine lock poisoned".into() })?;
        host.save().map_err(|msg| EngineError::Save { msg })
    }

    /// The change counter (bumped by every request that may change what is shown).
    pub fn version(&self) -> u64 {
        self.shared.host.lock().map(|h| h.version).unwrap_or(0)
    }

    /// The graph file's path.
    pub fn path(&self) -> String {
        self.shared.host.lock().ok().and_then(|h| h.session.path.clone()).map(|p| p.display().to_string()).unwrap_or_default()
    }

    /// Starts syncing with the other devices; the settings (and the workspace key) live at
    /// `config_path`, a private file outside the shared Documents folder. Then `rpc("sync.*")`
    /// sets it up, joins, lists devices (docs/sync.md). Once per engine.
    pub fn start_sync(&self, config_path: String) {
        self.shared.start_sync(PathBuf::from(config_path));
    }

    /// Connects now and waits up to `wait_ms` until this copy and the relay have each other's
    /// edits (launch, coming to the foreground, background refresh). Answers the sync status JSON.
    /// Blocks: call it off the main thread.
    pub fn sync_now(&self, wait_ms: u64) -> String {
        match self.shared.sync_service() {
            Some(s) => s.sync_now(Duration::from_millis(wait_ms.min(60_000))).to_string(),
            None => json!({"state": "off"}).to_string(),
        }
    }

    /// Calls `listener` after every request that changed the version.
    pub fn subscribe(&self, listener: Arc<dyn ChangeListener>) {
        if let Ok(mut l) = self.listeners.lock() {
            l.push(listener);
        }
    }
}

impl Engine {
    fn request(&self, req: &Value) -> Value {
        let before = self.version();
        let out = self.shared.handle(req);
        let after = self.version();
        if after != before {
            let listeners: Vec<Arc<dyn ChangeListener>> = self.listeners.lock().map(|l| l.clone()).unwrap_or_default();
            for l in listeners {
                l.changed(after);
            }
        }
        out
    }
}

fn parse(text: &str) -> Result<Value, String> {
    if text.trim().is_empty() {
        return Ok(json!({}));
    }
    match serde_json::from_str::<Value>(text) {
        Ok(v @ Value::Object(_)) => Ok(v),
        Ok(_) => Err("params must be a JSON object".into()),
        Err(e) => Err(format!("bad params: {e}")),
    }
}

/// Saves every 200 ms once a change is half a second old; ends when the engine is dropped.
fn spawn_autosave(shared: Weak<Shared>) {
    std::thread::spawn(move || {
        while let Some(s) = shared.upgrade() {
            if let Ok(mut h) = s.host.lock() {
                h.autosave(AUTOSAVE_AFTER);
            }
            drop(s);
            std::thread::sleep(Duration::from_millis(200));
        }
    });
}

#[cfg(test)]
mod tests;
