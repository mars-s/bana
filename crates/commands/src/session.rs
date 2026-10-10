//! The editing session: the graph plus what the person is looking at and editing.

use std::path::PathBuf;
use std::sync::Arc;

use bana_core::{Date, Graph, NodeId};
use serde_json::{Value, json};

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

/// Where one panel is: its page, caret, selection and history. Tabs and panels each keep one; the
/// session holds the active panel's.
#[derive(Debug, Clone, PartialEq)]
pub struct NavState {
    pub zoom: NodeId,
    pub focus: Option<NodeId>,
    pub caret: usize,
    pub anchor: usize,
    pub sel: Option<(NodeId, NodeId)>,
    pub back: Vec<NodeId>,
    pub forward: Vec<NodeId>,
    /// The panel shows the configuration of the supertag it is on (spec 03 §5).
    pub config: bool,
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
    /// The other end of the text selection in the focused bullet (== caret when nothing is selected).
    pub anchor: usize,
    /// Multi-selection of bullets: (anchor, head) rows; everything between is selected.
    pub sel: Option<(NodeId, NodeId)>,
    /// The month a calendar view shows (year, month); `None` = the month of `today`.
    pub calendar_month: Option<(i32, u8)>,
    /// Calendar views show a month (0), a week (1) or a day (2).
    pub calendar_mode: u8,
    /// The item a navigation view (list, side menu, tabs) shows, per view node.
    pub nav_selected: std::collections::HashMap<NodeId, NodeId>,
    back: Vec<NodeId>,
    forward: Vec<NodeId>,
    pub recents: Vec<NodeId>,
    /// Where "Move to move target" sends nodes.
    pub move_target: Option<NodeId>,
    /// This panel shows the open supertag's configuration instead of its page.
    pub config_view: bool,
    /// Last status line (result or error of the last command).
    pub status: String,
    /// Where the engine's web UI answers (`http://127.0.0.1:PORT/`), once it serves one: node
    /// links point there (UC-07.10).
    pub web_url: Option<String>,
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
            anchor: 0,
            calendar_month: None,
            calendar_mode: 0,
            nav_selected: Default::default(),
            sel: None,
            back: Vec::new(),
            forward: Vec::new(),
            recents: Vec::new(),
            move_target: None,
            config_view: false,
            status: String::new(),
            web_url: None,
            command_count: 0,
        }
    }

    /// A locked node (UC-06.35) refuses edits: commands that change a node (`node` in their params)
    /// or add under one (`parent`). Locking itself, folding and reading still work.
    pub fn check_lock(&self, spec: &crate::CommandSpec, params: &Value) -> Result<(), CmdError> {
        if !spec.mutates || matches!(spec.id, "node.toggleLock" | "node.expand" | "node.collapse" | "node.toggleExpand") {
            return Ok(());
        }
        let mut hit = Vec::new();
        if spec.params.contains("\"node\"") {
            hit.extend(self.targets(params).unwrap_or_default());
        }
        if let Ok(Some(p)) = crate::p::node(params, "parent") {
            hit.push(p);
        }
        match hit.into_iter().find(|n| self.graph.locked(*n)) {
            Some(n) => Err(CmdError::Disabled(format!("“{}” is locked; unlock it to change it", self.graph.text(n)))),
            None => Ok(()),
        }
    }

    /// Runs a command by id. A mutating command is one undo step; a failing command leaves the graph
    /// untouched, and so does a panic (last-resort guard, not a licence).
    pub fn run(&mut self, id: &str, params: &Value) -> CmdResult {
        let spec = self.registry.get(id).cloned().ok_or_else(|| CmdError::Unknown(id.to_string()))?;
        self.check_lock(&spec, params)?;
        // Every command is a transaction: "pure" ones can still make built-in structure (today's
        // day node, the inbox). That is not an undo step, but history must learn about it, or a
        // later undo would cut the new day out of its week.
        self.graph.begin();
        let left = self.focus;
        self.graph.take_events();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (spec.run)(self, params)))
            .unwrap_or_else(|_| Err(CmdError::Failed(format!("`{id}` crashed; nothing was changed"))));
        // Commands that run on events (UC-02.20), in the same undo step; they start no others.
        if result.is_ok() && spec.mutates {
            for ev in self.graph.take_events() {
                for (cmd, node) in self.graph.triggers(ev) {
                    if let Err(e) = crate::cmds::run_command_node(self, &serde_json::json!({"command": cmd.0, "node": node.0})) {
                        self.status = format!("{}: {e}", self.graph.name(cmd));
                    }
                }
            }
            self.graph.take_events();
        }
        // The line the caret left keeps no empty format marks (DIFFS N19).
        if let Some(l) = left.filter(|l| result.is_ok() && self.focus != Some(*l)) {
            let name = self.graph.name(l);
            let (clean, _) = bana_core::format::strip_empty(name, 0);
            if clean != name {
                let _ = self.graph.set_name(l, &clean);
            }
        }
        match &result {
            Ok(_) if !spec.mutates => self.graph.commit_silent(),
            Ok(_) => {
                // Typing into one bullet is one undo step, until any other command runs.
                // Typing into a field value (saved as typed, DIFFS N15) is one step the same way.
                let node = || crate::p::node(params, "node").ok().flatten().or(self.focus);
                let key = match id {
                    "node.setName" => Some(format!("rename:{:?}:{}", node(), self.command_count)),
                    "field.set" => Some(format!("field:{:?}:{}:{}:{}", node(), params["field"], params["index"], self.command_count)),
                    _ => None,
                };
                self.graph.commit_keyed(spec.label, key);
            }
            Err(_) => self.graph.abort(),
        }
        if id != "node.setName" && id != "field.set" {
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
        // A selection left on another page would let Delete remove lines that are not on screen.
        if let Some((a, b)) = self.sel {
            let rows = self.rows();
            if !rows.iter().any(|r| r.id == a) || !rows.iter().any(|r| r.id == b) {
                self.sel = None;
            }
        }
        if let Some(f) = self.focus {
            let n = self.graph.name(f).chars().count();
            self.caret = self.caret.min(n);
            self.anchor = self.anchor.min(n);
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
        for c in self.row_children(parent) {
            out.push(Row { id: c, depth });
            // A reference's notes for this place show under it, as lines of their own (DIFFS N22).
            if let Some(notes) = self.graph.context_of(c) {
                self.rows_into(notes, depth + 1, path, out);
            }
            let target = self.graph.resolve(c);
            if self.graph.is_expanded(c) && !path.contains(&target) {
                path.push(target);
                self.rows_into(c, depth + 1, path, out);
                path.pop();
            }
        }
    }

    /// The active panel's navigation state.
    pub fn nav_state(&self) -> NavState {
        NavState {
            zoom: self.zoom,
            focus: self.focus,
            caret: self.caret,
            anchor: self.anchor,
            sel: self.sel,
            back: self.back.clone(),
            forward: self.forward.clone(),
            config: self.config_view,
        }
    }

    /// Switches to another panel's navigation state.
    pub fn set_nav_state(&mut self, n: NavState) {
        self.zoom = n.zoom;
        self.focus = n.focus;
        self.caret = n.caret;
        self.anchor = n.anchor;
        self.sel = n.sel;
        self.back = n.back;
        self.forward = n.forward;
        self.config_view = n.config;
        self.fix_view();
    }

    /// A fresh panel on `id` (no history).
    pub fn new_nav(&self, id: NodeId) -> NavState {
        NavState { zoom: id, focus: None, caret: 0, anchor: 0, sel: None, back: Vec::new(), forward: Vec::new(), config: false }
    }

    /// What a bullet shows when expanded: its children, or a search node's results, after the
    /// node's view filter and sort.
    pub fn row_children(&self, id: NodeId) -> Vec<NodeId> {
        let target = self.graph.resolve(id);
        let plain = self.graph.query(target).is_none() && self.graph.get(target).is_some_and(|n| n.view.is_none());
        if plain {
            return self.graph.children(target).to_vec();
        }
        self.graph.view_groups(target).into_iter().flat_map(|g| g.items).collect()
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
        self.config_view = false;
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
        self.anchor = self.caret;
    }

    /// The selected bullets in row order, without any whose ancestor is also selected (moving a
    /// parent moves its children).
    pub fn selected(&self) -> Vec<NodeId> {
        let Some((a, h)) = self.sel else { return Vec::new() };
        let rows = self.rows();
        let (Some(i), Some(j)) = (rows.iter().position(|r| r.id == a), rows.iter().position(|r| r.id == h)) else { return Vec::new() };
        let picked: Vec<NodeId> = rows.get(i.min(j)..=i.max(j)).unwrap_or(&[]).iter().map(|r| r.id).collect();
        picked.iter().copied().filter(|n| !picked.iter().any(|p| p != n && self.graph.is_ancestor_or_self(*p, *n))).collect()
    }

    /// The nodes a command acts on: `nodes` (array), else `node`, else the selection, else focus.
    pub fn targets(&self, params: &Value) -> Result<Vec<NodeId>, CmdError> {
        if let Some(arr) = params.get("nodes").and_then(Value::as_array) {
            let mut out = Vec::new();
            for v in arr.iter().take(10_000) {
                let id = crate::p::node(&json!({"n": v}), "n")?.ok_or_else(|| CmdError::Params("`nodes` must hold node ids".into()))?;
                self.graph.node(id)?;
                out.push(id);
            }
            return Ok(out);
        }
        if params.get("node").is_none() {
            let sel = self.selected();
            if !sel.is_empty() {
                return Ok(sel);
            }
        }
        Ok(vec![self.target(params)?])
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
