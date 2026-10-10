//! Sync for a running engine: the [`bana_sync::Service`] works on the host's graph through
//! [`Synced`], and `sync.*` requests (Settings > Sync, the phone's Manage tab) go to it.
//!
//! Merged edits from another device are journaled like typed ones, bump the version (views read
//! again; a line being edited keeps its caret, see the web editor's `shiftedOffset`) and wake the
//! desktop's and the phone's change listeners.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

use bana_core::{Graph, VersionVector};
use bana_sync::{Replica, Result as SyncResult};
use serde_json::json;

use crate::Host;
use crate::http::Shared;

/// The host's graph as a sync replica (holds the host lock for the call).
struct HostReplica<'a> {
    host: &'a mut Host,
    merged: &'a AtomicBool,
}

fn err(e: impl std::fmt::Display) -> bana_sync::Error {
    bana_sync::Error::Graph(e.to_string())
}

impl Replica for HostReplica<'_> {
    fn version(&mut self) -> SyncResult<VersionVector> {
        self.host.session.graph.version().map_err(err)
    }
    fn updates_since(&mut self, since: &VersionVector) -> SyncResult<Vec<u8>> {
        self.host.session.graph.updates_since(since).map_err(err)
    }
    fn snapshot(&mut self) -> SyncResult<Vec<u8>> {
        self.host.session.graph.to_snapshot().map_err(err)
    }
    fn import(&mut self, parts: &[Vec<u8>]) -> SyncResult<()> {
        self.host.session.graph.merge_all(parts).map_err(err)?;
        // On disk before the relay hears it arrived (the cursor is saved after this).
        self.host.journal();
        self.host.version += 1;
        self.merged.store(true, Ordering::SeqCst);
        Ok(())
    }
    fn replace(&mut self, parts: &[Vec<u8>]) -> SyncResult<()> {
        let g = Graph::from_parts(parts).map_err(err)?;
        if let Some(path) = self.host.session.path.clone() {
            // The notes this copy had before joining stay in a file beside it.
            if path.exists() {
                let mut keep = path.file_name().unwrap_or_default().to_os_string();
                keep.push(".before-sync");
                std::fs::copy(&path, path.with_file_name(keep)).map_err(err)?;
            }
        }
        self.host.replace_graph(g);
        if let Some(path) = self.host.session.path.clone() {
            self.host.session.graph.save(&path).map_err(err)?;
        }
        self.merged.store(true, Ordering::SeqCst);
        Ok(())
    }
    fn compact(&mut self, acked: &[VersionVector]) -> SyncResult<bool> {
        self.host.session.graph.compact_history(acked).map_err(err)
    }
}

/// What the sync service holds: the engine, weakly (the engine owns the service).
pub(crate) struct Synced {
    shared: Weak<Shared>,
    merged: AtomicBool,
}

impl bana_sync::SharedReplica for Synced {
    fn with(&self, f: &mut dyn FnMut(&mut dyn Replica)) {
        let Some(shared) = self.shared.upgrade() else { return };
        let Ok(mut host) = shared.host.lock() else { return };
        f(&mut HostReplica { host: &mut host, merged: &self.merged });
    }

    fn settled(&self) {
        if !self.merged.swap(false, Ordering::SeqCst) {
            return;
        }
        let Some(shared) = self.shared.upgrade() else { return };
        let version = shared.host.lock().map(|h| h.version).unwrap_or(0);
        shared.changed.notify_all();
        if let Ok(l) = shared.on_change.lock()
            && let Some(f) = l.as_ref()
        {
            f(version);
        }
    }
}

impl Shared {
    /// Starts the sync service with its settings at `config` (syncing at once when the device
    /// was set up before). Once per engine.
    pub fn start_sync(self: &Arc<Self>, config: std::path::PathBuf) {
        let replica = Arc::new(Synced { shared: Arc::downgrade(self), merged: AtomicBool::new(false) });
        let _ = self.sync.set(bana_sync::Service::new(config, replica));
    }

    /// [`Shared::start_sync`] with the settings beside the graph file (`graph.bana.sync`); no
    /// file, no sync.
    pub fn start_sync_beside_graph(self: &Arc<Self>) {
        let path = self.host.lock().ok().and_then(|h| h.session.path.clone());
        if let Some(p) = path {
            self.start_sync(bana_core::sync_file(&p));
        }
    }

    /// A `sync.*` request.
    pub(crate) fn sync_request(&self, method: &str, params: &serde_json::Value) -> serde_json::Value {
        match self.sync.get() {
            Some(s) => s.handle(method, params),
            None if method == "sync.status" => json!({"state": "off", "unavailable": true}),
            None => json!({"error": "sync is not available here (this engine has no file)"}),
        }
    }

    /// The sync service, once started.
    pub fn sync_service(&self) -> Option<&bana_sync::Service> {
        self.sync.get()
    }
}
