//! The editing session: the graph plus what the person is looking at and editing.

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{Value, json};
use tanacraft_core::{Date, Graph, NodeId};

use crate::{CmdError, CmdResult, Registry};

/// Rows deeper than this are not shown (bounds reference cycles and hostile files).
const MAX_ROW_DEPTH: usize = 64;
const MAX_RECENTS: usize = 12;

/// One visible bullet in the outline panel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Row {
    pub id: NodeId,
    /// 0 = a direct child of the zoomed node.
    pub depth: usize,
}

pub struct Session {
    pub graph: Graph,
    pub registry: Arc<Registry>,
    /// Where `file.save` writes.
    pub path: Option<PathBuf>,
    /// "Today" for daily notes (fixed in tests and screenshots).
    pub today: Date,
    /// The node shown in the panel (zoomed into).
    pub zoom: NodeId,
    /// The bullet being edited.
    pub focus: Option<NodeId>,
    /// Caret position in the focused bullet, in chars.
    pub caret: usize,
    back: Vec<NodeId>,
    forward: Vec<NodeId>,
    pub recents: Vec<NodeId>,
    /// Last status line (result or error of the last command).
    pub status: String,
    command_count: u64,
}

impl Session {
    pub fn new(graph: Graph) -> Session {
        Session::with_today(graph, Date::today())
    }

    pub fn with_today(graph: Graph, today: Date) -> Session {
        let zoom = graph.sys.library;
        Session {
            graph,
            registry: Arc::new(crate::cmds::registry()),
            path: None,
            today,
            zoom,
            focus: None,
            caret: 0,
            back: Vec::new(),
            forward: Vec::new(),
            recents: Vec::new(),
            status: String::new(),
            command_count: 0,
        }
    }

    /// Runs a command by id. Mutating commands are one undo step; a failing command leaves the
    /// graph untouched, and so does a panic (last-resort guard, not a licence).
    pub fn run(&mut self, id: &str, params: &Value) -> CmdResult {
        let spec = self.registry.get(id).cloned().ok_or_else(|| CmdError::Unknown(id.to_string()))?;
        if spec.mutates {
            self.graph.begin();
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (spec.run)(self, params)))
            .unwrap_or_else(|_| Err(CmdError::Failed(format!("`{id}` crashed; nothing was changed"))));
        if spec.mutates {
            match &result {
                Ok(_) => {
                    // Typing into one bullet is one undo step, until any other command runs.
                    let key = (id == "node.setName").then(|| {
                        let node = crate::p::node(params, "node").ok().flatten().or(self.focus);
                        format!("rename:{node:?}:{}", self.command_count)
                    });
                    self.graph.commit_keyed(spec.label, key);
                }
                Err(_) => self.graph.abort(),
            }
        }
        if id != "node.setName" {
            self.command_count += 1;
        }
        self.fix_view();
        self.status = match &result {
            Ok(_) => String::new(),
            Err(e) => e.to_string(),
        };
        result
    }

    /// Keeps zoom and focus pointing at live nodes (after undo, delete, load...).
    pub fn fix_view(&mut self) {
        if self.graph.get(self.zoom).is_none() {
            self.zoom = self.graph.sys.library;
        }
        if let Some(f) = self.focus
            && (self.graph.get(f).is_none() || !self.graph.is_ancestor_or_self(self.zoom, f) && !self.visible(f))
        {
            self.focus = None;
        }
        if let Some(f) = self.focus {
            self.caret = self.caret.min(self.graph.name(f).chars().count());
        }
    }

    fn visible(&self, id: NodeId) -> bool {
        self.rows().iter().any(|r| r.id == id)
    }

    /// The bullets visible under the zoomed node, depth-first through expanded nodes. A reference
    /// whose target is already on the path is not expanded again (no infinite mirrors).
    pub fn rows(&self) -> Vec<Row> {
        let mut out = Vec::new();
        let mut path = vec![self.graph.resolve(self.zoom)];
        self.rows_into(self.zoom, 0, &mut path, &mut out);
        out
    }

    fn rows_into(&self, parent: NodeId, depth: usize, path: &mut Vec<NodeId>, out: &mut Vec<Row>) {
        if depth >= MAX_ROW_DEPTH {
            return;
        }
        for c in self.graph.children(parent) {
            out.push(Row { id: *c, depth });
            let target = self.graph.resolve(*c);
            if self.graph.is_expanded(*c) && !path.contains(&target) {
                path.push(target);
                self.rows_into(*c, depth + 1, path, out);
                path.pop();
            }
        }
    }

    /// Opens `id` in the panel, remembering where we were.
    pub fn open(&mut self, id: NodeId) -> Result<(), CmdError> {
        self.graph.node(id)?;
        if id != self.zoom {
            self.back.push(self.zoom);
            self.forward.clear();
        }
        self.zoom = id;
        self.focus = None;
        let target = self.graph.resolve(id);
        self.recents.retain(|r| *r != target);
        self.recents.insert(0, target);
        self.recents.truncate(MAX_RECENTS);
        Ok(())
    }

    pub fn go_back(&mut self) -> bool {
        let Some(prev) = self.back.pop() else { return false };
        self.forward.push(self.zoom);
        self.zoom = prev;
        self.focus = None;
        true
    }

    pub fn go_forward(&mut self) -> bool {
        let Some(next) = self.forward.pop() else { return false };
        self.back.push(self.zoom);
        self.zoom = next;
        self.focus = None;
        true
    }

    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }

    /// Focuses `id` with the caret at `caret` (clamped).
    pub fn set_focus(&mut self, id: NodeId, caret: usize) {
        self.focus = Some(id);
        self.caret = caret.min(self.graph.name(id).chars().count());
    }

    /// The target node of a command: the `node` param, else the focused bullet.
    pub fn target(&self, params: &Value) -> Result<NodeId, CmdError> {
        let id = crate::p::node(params, "node")?.or(self.focus).ok_or_else(|| CmdError::Params("no node: pass `node` or focus a bullet".into()))?;
        self.graph.node(id)?;
        Ok(id)
    }

    /// Breadcrumbs of the zoomed node (root excluded).
    pub fn breadcrumbs(&self) -> Vec<NodeId> {
        self.graph.ancestors(self.zoom).into_iter().filter(|a| *a != self.graph.sys.root).collect()
    }

    /// The standard result: where focus ended up.
    pub fn focus_json(&self) -> Value {
        json!({"zoom": self.zoom.0, "focus": self.focus.map(|f| f.0), "caret": self.caret})
    }
}
