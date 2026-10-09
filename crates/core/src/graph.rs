//! The graph: node storage, structural edits, supertags, fields, references, daily notes, trash and
//! undo/redo.
//!
//! Undo works by pre-images: inside a transaction ([`Graph::begin`] .. [`Graph::commit`]) every node
//! touched for the first time is copied before it changes. Undo puts those copies back (and keeps the
//! current versions for redo). Every mutation goes through `node_mut`, `put` or `remove`, so nothing
//! can bypass the journal.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::date::Date;
use crate::model::*;
use crate::{Error, Result};

/// Deepest ancestor chain we walk. Real outlines are far shallower; this bounds hostile files.
const MAX_DEPTH: usize = 10_000;
/// Undo steps kept.
const MAX_UNDO: usize = 500;

/// Built-in nodes every graph has.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SystemNodes {
    pub root: NodeId,
    /// Home: where hierarchy-based notes live.
    pub library: NodeId,
    /// Daily notes: year, week and day nodes.
    pub journal: NodeId,
    /// Supertag and field definitions.
    pub schema: NodeId,
    pub trash: NodeId,
    /// The built-in `day` supertag on day nodes.
    pub day_tag: NodeId,
}

type PreImages = BTreeMap<NodeId, Option<Node>>;

#[derive(Debug, Clone)]
struct Step {
    label: String,
    nodes: PreImages,
    /// Consecutive steps with the same key merge into one (typing a word is one undo step).
    key: Option<String>,
}

/// The node graph of one workspace.
#[derive(Debug, Clone)]
pub struct Graph {
    nodes: BTreeMap<NodeId, Node>,
    next_id: u64,
    pub sys: SystemNodes,
    /// Expanded nodes (view state; saved with the file, not undoable).
    expanded: BTreeSet<NodeId>,
    txn: Option<PreImages>,
    undo: Vec<Step>,
    redo: Vec<Step>,
    dirty: bool,
}

impl Default for Graph {
    fn default() -> Self {
        Graph::new()
    }
}

impl Graph {
    /// An empty workspace with its system nodes.
    pub fn new() -> Graph {
        let ids = SystemNodes { root: NodeId(1), library: NodeId(2), journal: NodeId(3), schema: NodeId(4), trash: NodeId(5), day_tag: NodeId(6) };
        let mut g = Graph {
            nodes: BTreeMap::new(),
            next_id: 7,
            sys: ids,
            expanded: BTreeSet::new(),
            txn: None,
            undo: Vec::new(),
            redo: Vec::new(),
            dirty: false,
        };
        let mut root = Node::new(ids.root, "Workspace", None);
        root.children = vec![ids.library, ids.journal, ids.schema, ids.trash];
        g.nodes.insert(ids.root, root);
        for (id, name) in [(ids.library, "Home"), (ids.journal, "Daily notes"), (ids.schema, "Supertags"), (ids.trash, "Trash")] {
            g.nodes.insert(id, Node::new(id, name, Some(ids.root)));
        }
        let mut day = Node::new(ids.day_tag, "day", Some(ids.schema));
        day.kind = Kind::Tag(TagDef { color: "grey".into(), ..TagDef::default() });
        g.nodes.insert(ids.day_tag, day);
        if let Some(s) = g.nodes.get_mut(&ids.schema) {
            s.children.push(ids.day_tag);
        }
        g
    }

    // ---- reading ------------------------------------------------------------------------------

    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(&id)
    }

    pub fn node(&self, id: NodeId) -> Result<&Node> {
        self.nodes.get(&id).ok_or(Error::NoNode(id))
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Node> {
        self.nodes.values()
    }

    pub fn name(&self, id: NodeId) -> &str {
        self.get(self.resolve(id)).map(|n| n.name.as_str()).unwrap_or("")
    }

    /// A reference resolves to its target; anything else to itself.
    pub fn resolve(&self, id: NodeId) -> NodeId {
        self.get(id).and_then(Node::reference_target).unwrap_or(id)
    }

    /// The children shown under `id` (a reference shows its target's children).
    pub fn children(&self, id: NodeId) -> &[NodeId] {
        self.get(self.resolve(id)).map(|n| n.children.as_slice()).unwrap_or(&[])
    }

    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.get(id).and_then(|n| n.parent)
    }

    pub fn index_in_parent(&self, id: NodeId) -> Option<usize> {
        let p = self.parent(id)?;
        self.get(p)?.children.iter().position(|c| *c == id)
    }

    /// Ancestors from the root down to the parent of `id`.
    pub fn ancestors(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut cur = self.parent(id);
        while let Some(p) = cur {
            if out.len() >= MAX_DEPTH || out.contains(&p) {
                break;
            }
            out.push(p);
            cur = self.parent(p);
        }
        out.reverse();
        out
    }

    /// Whether `a` is `b` or an ancestor of `b`.
    pub fn is_ancestor_or_self(&self, a: NodeId, b: NodeId) -> bool {
        a == b || self.ancestors(b).contains(&a)
    }

    pub fn in_trash(&self, id: NodeId) -> bool {
        id != self.sys.trash && self.is_ancestor_or_self(self.sys.trash, id)
    }

    pub fn is_system(&self, id: NodeId) -> bool {
        let s = self.sys;
        [s.root, s.library, s.journal, s.schema, s.trash, s.day_tag].contains(&id)
    }

    pub fn is_expanded(&self, id: NodeId) -> bool {
        self.expanded.contains(&id)
    }

    pub fn set_expanded(&mut self, id: NodeId, on: bool) {
        if on {
            self.expanded.insert(id);
        } else {
            self.expanded.remove(&id);
        }
        self.dirty = true;
    }

    /// Case-insensitive substring search over names, outside the trash.
    pub fn search(&self, query: &str) -> Vec<NodeId> {
        let q = query.to_lowercase();
        self.nodes
            .values()
            .filter(|n| {
                n.reference_target().is_none()
                    && !self.is_system(n.id)
                    && crate::format::plain(&n.name).to_lowercase().contains(&q)
                    && !self.in_trash(n.id)
            })
            .map(|n| n.id)
            .collect()
    }

    /// Fuzzy search: every query character must appear in order. Contiguous runs, word starts and
    /// prefixes score higher. Best first, at most `limit`.
    pub fn fuzzy(&self, query: &str, limit: usize) -> Vec<NodeId> {
        let q: Vec<char> = query.to_lowercase().chars().filter(|c| !c.is_whitespace()).collect();
        if q.is_empty() {
            return Vec::new();
        }
        let mut hits: Vec<(i64, NodeId)> = self
            .nodes
            .values()
            .filter(|n| n.reference_target().is_none() && !self.is_system(n.id) && !self.in_trash(n.id))
            .filter_map(|n| fuzzy_score(&q, &crate::format::plain(&n.name).to_lowercase()).map(|s| (s, n.id)))
            .collect();
        hits.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        hits.into_iter().take(limit).map(|(_, id)| id).collect()
    }

    /// Nodes that point at `target`: reference nodes and node-valued fields.
    pub fn references_to(&self, target: NodeId) -> Vec<NodeId> {
        self.nodes
            .values()
            .filter(|n| n.reference_target() == Some(target) || n.fields.iter().any(|f| f.value == Some(FieldValue::Node(target))))
            .filter(|n| !self.in_trash(n.id))
            .map(|n| n.id)
            .collect()
    }

    /// Nodes carrying `tag` (directly), outside the trash.
    pub fn tagged(&self, tag: NodeId) -> Vec<NodeId> {
        self.nodes.values().filter(|n| n.tags.contains(&tag) && !self.in_trash(n.id)).map(|n| n.id).collect()
    }

    pub fn tag_def(&self, tag: NodeId) -> Option<&TagDef> {
        match &self.get(tag)?.kind {
            Kind::Tag(t) => Some(t),
            _ => None,
        }
    }

    pub fn field_def(&self, field: NodeId) -> Option<&FieldDef> {
        match &self.get(field)?.kind {
            Kind::Field(f) => Some(f),
            _ => None,
        }
    }

    /// All supertags (outside the trash).
    pub fn tags(&self) -> Vec<NodeId> {
        self.nodes.values().filter(|n| matches!(n.kind, Kind::Tag(_)) && !self.in_trash(n.id)).map(|n| n.id).collect()
    }

    pub fn find_tag(&self, name: &str) -> Option<NodeId> {
        let name = name.trim().trim_start_matches('#');
        self.tags().into_iter().find(|t| self.name(*t).eq_ignore_ascii_case(name))
    }

    pub fn find_field(&self, name: &str) -> Option<NodeId> {
        self.nodes
            .values()
            .find(|n| matches!(n.kind, Kind::Field(_)) && n.name.eq_ignore_ascii_case(name.trim()) && !self.in_trash(n.id))
            .map(|n| n.id)
    }

    /// The supertags `tag` extends, transitively, followed by `tag` itself (cycle-safe).
    pub fn tag_lineage(&self, tag: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        self.lineage_into(tag, &mut out, 0);
        out
    }

    fn lineage_into(&self, tag: NodeId, out: &mut Vec<NodeId>, depth: usize) {
        if out.contains(&tag) || depth > 64 {
            return;
        }
        if let Some(def) = self.tag_def(tag) {
            for p in &def.extends {
                self.lineage_into(*p, out, depth + 1);
            }
            if !out.contains(&tag) {
                out.push(tag);
            }
        }
    }

    /// Template fields of a supertag: inherited ones first, no duplicates.
    pub fn template_fields(&self, tag: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        for t in self.tag_lineage(tag) {
            for f in self.tag_def(t).map(|d| d.fields.as_slice()).unwrap_or(&[]) {
                if !out.contains(f) {
                    out.push(*f);
                }
            }
        }
        out
    }

    /// Whether a node shows a checkbox because one of its supertags asks for it.
    pub fn tag_wants_checkbox(&self, node: NodeId) -> bool {
        let tags = self.get(node).map(|n| n.tags.clone()).unwrap_or_default();
        tags.iter().any(|t| self.tag_lineage(*t).iter().any(|x| self.tag_def(*x).is_some_and(|d| d.checkbox)))
    }

    /// The checkbox a node shows: its own state, or unchecked when a supertag asks for a checkbox.
    pub fn checkbox(&self, id: NodeId) -> Option<bool> {
        let id = self.resolve(id);
        self.get(id)?.done.or_else(|| self.tag_wants_checkbox(id).then_some(false))
    }

    /// Day node for `date`, if it exists.
    pub fn find_day(&self, date: Date) -> Option<NodeId> {
        self.nodes.values().find(|n| n.kind == Kind::Day { date } && !self.in_trash(n.id)).map(|n| n.id)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|s| s.label.as_str())
    }

    /// Unsaved changes since the last save or load.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_saved(&mut self) {
        self.dirty = false;
    }

    // ---- transactions and undo ----------------------------------------------------------------

    /// Starts recording an undoable step. Nested calls join the open step.
    pub fn begin(&mut self) {
        if self.txn.is_none() {
            self.txn = Some(PreImages::new());
        }
    }

    /// Closes the open step. Empty steps are dropped.
    pub fn commit(&mut self, label: &str) {
        self.commit_keyed(label, None);
    }

    /// Like [`Graph::commit`], but merges into the previous step when both carry the same `key`.
    pub fn commit_keyed(&mut self, label: &str, key: Option<String>) {
        let Some(nodes) = self.txn.take() else { return };
        if nodes.is_empty() {
            return;
        }
        if let (Some(k), Some(last)) = (&key, self.undo.last_mut())
            && last.key.as_ref() == Some(k)
            && self.redo.is_empty()
        {
            for (id, img) in nodes {
                last.nodes.entry(id).or_insert(img);
            }
            self.dirty = true;
            return;
        }
        self.undo.push(Step { label: label.to_string(), nodes, key });
        if self.undo.len() > MAX_UNDO {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
    }

    /// Rolls the open step back (used when a command fails halfway).
    pub fn abort(&mut self) {
        if let Some(nodes) = self.txn.take() {
            self.apply(nodes);
        }
    }

    pub fn undo(&mut self) -> Option<String> {
        self.commit("edit");
        let step = self.undo.pop()?;
        let back = self.apply(step.nodes);
        self.redo.push(Step { label: step.label.clone(), nodes: back, key: None });
        self.dirty = true;
        Some(step.label)
    }

    pub fn redo(&mut self) -> Option<String> {
        let step = self.redo.pop()?;
        let back = self.apply(step.nodes);
        self.undo.push(Step { label: step.label.clone(), nodes: back, key: None });
        self.dirty = true;
        Some(step.label)
    }

    /// Writes `images` into the graph and returns what was there before.
    fn apply(&mut self, images: PreImages) -> PreImages {
        let mut before = PreImages::new();
        for (id, img) in images {
            let old = match img {
                Some(n) => self.nodes.insert(id, n),
                None => self.nodes.remove(&id),
            };
            before.insert(id, old);
        }
        before
    }

    fn touch(&mut self, id: NodeId) {
        if let Some(t) = self.txn.as_mut() {
            t.entry(id).or_insert_with(|| self.nodes.get(&id).cloned());
        }
        self.dirty = true;
    }

    fn node_mut(&mut self, id: NodeId) -> Result<&mut Node> {
        if !self.nodes.contains_key(&id) {
            return Err(Error::NoNode(id));
        }
        self.touch(id);
        let n = self.nodes.get_mut(&id).ok_or(Error::NoNode(id))?;
        n.edited = now_secs();
        Ok(n)
    }

    fn put(&mut self, node: Node) {
        self.touch(node.id);
        self.nodes.insert(node.id, node);
    }

    fn remove(&mut self, id: NodeId) {
        self.touch(id);
        self.nodes.remove(&id);
        self.expanded.remove(&id);
    }

    fn alloc(&mut self) -> NodeId {
        let id = NodeId(self.next_id);
        self.next_id += 1;
        id
    }

    // ---- structure ----------------------------------------------------------------------------

    /// Creates a node under `parent` (a reference parent means its target) at `index` (clamped;
    /// `None` appends).
    pub fn create(&mut self, parent: NodeId, index: Option<usize>, name: &str) -> Result<NodeId> {
        let parent = self.resolve(parent);
        self.node(parent)?;
        let id = self.alloc();
        self.put(Node::new(id, clean_name(name), Some(parent)));
        self.attach(id, parent, index)?;
        Ok(id)
    }

    fn attach(&mut self, id: NodeId, parent: NodeId, index: Option<usize>) -> Result<()> {
        let p = self.node_mut(parent)?;
        let at = index.unwrap_or(p.children.len()).min(p.children.len());
        p.children.insert(at, id);
        self.node_mut(id)?.parent = Some(parent);
        Ok(())
    }

    fn detach(&mut self, id: NodeId) -> Result<usize> {
        let parent = self.parent(id).ok_or_else(|| Error::Invalid("the root cannot move".into()))?;
        let p = self.node_mut(parent)?;
        let at = p.children.iter().position(|c| *c == id).ok_or_else(|| Error::Invalid(format!("node {id} is not a child of {parent}")))?;
        p.children.remove(at);
        Ok(at)
    }

    /// Moves `id` under `parent` at `index` (clamped; `None` appends).
    pub fn move_to(&mut self, id: NodeId, parent: NodeId, index: Option<usize>) -> Result<()> {
        let parent = self.resolve(parent);
        self.node(id)?;
        self.node(parent)?;
        if self.is_system(id) {
            return Err(Error::Invalid("built-in nodes cannot move".into()));
        }
        if self.is_ancestor_or_self(id, parent) {
            return Err(Error::Invalid("a node cannot move inside itself".into()));
        }
        let old_parent = self.parent(id);
        let old = self.detach(id)?;
        // Moving down within the same parent: the removal shifted the target slot.
        let index = match (index, old_parent == Some(parent)) {
            (Some(i), true) if i > old => Some(i - 1),
            (i, _) => i,
        };
        self.attach(id, parent, index)
    }

    pub fn set_name(&mut self, id: NodeId, name: &str) -> Result<()> {
        let target = self.resolve(id);
        self.node_mut(target)?.name = clean_name(name);
        Ok(())
    }

    pub fn set_heading(&mut self, id: NodeId, on: bool) -> Result<()> {
        let target = self.resolve(id);
        self.node_mut(target)?.heading = on;
        Ok(())
    }

    pub fn set_description(&mut self, id: NodeId, text: &str) -> Result<()> {
        let target = self.resolve(id);
        self.node_mut(target)?.description = text.to_string();
        Ok(())
    }

    /// `Some(done)` shows a checkbox, `None` hides it.
    pub fn set_done(&mut self, id: NodeId, done: Option<bool>) -> Result<()> {
        let target = self.resolve(id);
        let n = self.node_mut(target)?;
        if done == Some(true) && n.done != Some(true) {
            n.done_at = Some(now_secs());
        } else if done != Some(true) {
            n.done_at = None;
        }
        n.done = done;
        Ok(())
    }

    /// Indents `id` under its previous sibling (as the last child). Returns the new parent.
    pub fn indent(&mut self, id: NodeId) -> Result<NodeId> {
        let parent = self.parent(id).ok_or(Error::NoNode(id))?;
        let at = self.index_in_parent(id).ok_or(Error::NoNode(id))?;
        let prev = at
            .checked_sub(1)
            .and_then(|i| self.node(parent).ok()?.children.get(i).copied())
            .ok_or_else(|| Error::Invalid("no previous sibling to indent under".into()))?;
        self.move_to(id, prev, None)?;
        self.set_expanded(prev, true);
        Ok(self.resolve(prev))
    }

    /// Outdents `id` to sit right after its parent. Built-in containers stop it.
    pub fn outdent(&mut self, id: NodeId) -> Result<NodeId> {
        let parent = self.parent(id).ok_or(Error::NoNode(id))?;
        let grand = self.parent(parent).ok_or_else(|| Error::Invalid("already at the top".into()))?;
        if self.is_system(parent) {
            return Err(Error::Invalid("already at the top".into()));
        }
        let at = self.index_in_parent(parent).unwrap_or(0);
        self.move_to(id, grand, Some(at + 1))?;
        Ok(grand)
    }

    /// Swaps `id` with its previous (`up`) or next sibling.
    /// Moves `id` one place up or down among its siblings. At the edge it crosses into the
    /// neighbouring branch: the first child moving up becomes the last child of its parent's previous
    /// sibling; the last child moving down becomes the first child of its parent's next sibling.
    pub fn move_sibling(&mut self, id: NodeId, up: bool) -> Result<()> {
        let parent = self.parent(id).ok_or(Error::NoNode(id))?;
        let at = self.index_in_parent(id).ok_or(Error::NoNode(id))?;
        let len = self.node(parent)?.children.len();
        let to = if up { at.checked_sub(1) } else { (at + 1 < len).then_some(at + 1) };
        if let Some(to) = to {
            self.node_mut(parent)?.children.swap(at, to);
            return Ok(());
        }
        let edge = || Error::Invalid(if up { "already first" } else { "already last" }.into());
        if self.is_system(parent) {
            return Err(edge());
        }
        let grand = self.parent(parent).ok_or_else(edge)?;
        let pat = self.index_in_parent(parent).ok_or_else(edge)?;
        let uncles = self.node(grand)?.children.clone();
        let neighbour = if up { pat.checked_sub(1).and_then(|i| uncles.get(i)) } else { uncles.get(pat + 1) };
        let n = *neighbour.ok_or_else(edge)?;
        if up {
            self.move_to(id, n, None)?;
        } else {
            self.move_to(id, n, Some(0))?;
        }
        self.set_expanded(n, true);
        Ok(())
    }

    /// Moves `id` to the trash (restorable). Built-in nodes cannot be trashed.
    pub fn trash(&mut self, id: NodeId) -> Result<()> {
        if self.is_system(id) {
            return Err(Error::Invalid("built-in nodes cannot be deleted".into()));
        }
        if self.in_trash(id) {
            return Ok(());
        }
        let parent = self.parent(id).ok_or(Error::NoNode(id))?;
        let index = self.index_in_parent(id).unwrap_or(0);
        let trash = self.sys.trash;
        self.move_to(id, trash, None)?;
        self.node_mut(id)?.trashed_from = Some(TrashedFrom { parent, index });
        Ok(())
    }

    /// Puts a trashed node back where it was (or in Home if that place is gone).
    pub fn restore(&mut self, id: NodeId) -> Result<NodeId> {
        let from = self.node(id)?.trashed_from.ok_or_else(|| Error::Invalid(format!("node {id} is not in the trash")))?;
        let parent = if self.get(from.parent).is_some() && !self.in_trash(from.parent) { from.parent } else { self.sys.library };
        self.move_to(id, parent, Some(from.index))?;
        self.node_mut(id)?.trashed_from = None;
        Ok(parent)
    }

    /// Deletes `id` and its owned subtree for good (still undoable within the session).
    pub fn delete_permanently(&mut self, id: NodeId) -> Result<()> {
        if self.is_system(id) {
            return Err(Error::Invalid("built-in nodes cannot be deleted".into()));
        }
        self.detach(id)?;
        let mut stack = vec![id];
        let mut seen = BTreeSet::new();
        while let Some(n) = stack.pop() {
            if !seen.insert(n) {
                continue;
            }
            if let Some(node) = self.get(n) {
                stack.extend(node.children.iter().copied());
            }
            self.remove(n);
        }
        Ok(())
    }

    pub fn empty_trash(&mut self) -> Result<usize> {
        let items = self.node(self.sys.trash)?.children.clone();
        for id in &items {
            self.delete_permanently(*id)?;
        }
        Ok(items.len())
    }

    /// Backspace at the start of `src`: its text joins the end of `dst`, its children move under
    /// `dst`, and `src` goes away. Returns the caret position (chars) in `dst`.
    pub fn merge_into(&mut self, src: NodeId, dst: NodeId) -> Result<usize> {
        if src == dst || self.is_ancestor_or_self(src, dst) {
            return Err(Error::Invalid("cannot merge a node into itself".into()));
        }
        let dst_t = self.resolve(dst);
        let caret = self.node(dst_t)?.name.chars().count();
        if self.node(src)?.reference_target().is_none() {
            let text = self.node(src)?.name.clone();
            let children = self.node(src)?.children.clone();
            self.node_mut(dst_t)?.name.push_str(&text);
            for c in children {
                self.move_to(c, dst_t, None)?;
            }
        }
        self.delete_permanently(src)?;
        Ok(caret)
    }

    /// Copies `id` and its owned subtree right after it. References stay references.
    pub fn duplicate(&mut self, id: NodeId) -> Result<NodeId> {
        if self.is_system(id) {
            return Err(Error::Invalid("built-in nodes cannot be duplicated".into()));
        }
        let parent = self.parent(id).ok_or(Error::NoNode(id))?;
        let at = self.index_in_parent(id).unwrap_or(0);
        let copy = self.copy_subtree(id, parent, 0)?;
        self.attach(copy, parent, Some(at + 1))?;
        Ok(copy)
    }

    fn copy_subtree(&mut self, id: NodeId, parent: NodeId, depth: usize) -> Result<NodeId> {
        if depth > 1000 {
            return Err(Error::Invalid("outline too deep to copy".into()));
        }
        let src = self.node(id)?.clone();
        let new = self.alloc();
        let mut n = src.clone();
        n.id = new;
        n.parent = Some(parent);
        n.children = Vec::new();
        n.trashed_from = None;
        self.put(n);
        for c in src.children {
            let cc = self.copy_subtree(c, new, depth + 1)?;
            self.node_mut(new)?.children.push(cc);
        }
        Ok(new)
    }

    /// Creates a reference to `target` under `parent`.
    pub fn create_reference(&mut self, parent: NodeId, index: Option<usize>, target: NodeId) -> Result<NodeId> {
        let target = self.resolve(target);
        self.node(target)?;
        let id = self.create(parent, index, "")?;
        self.node_mut(id)?.kind = Kind::Reference { target };
        Ok(id)
    }

    // ---- search nodes and views ---------------------------------------------------------------

    /// Creates a search node under `parent` that lists the query's results live.
    pub fn create_search(&mut self, parent: NodeId, index: Option<usize>, name: &str, query: crate::query::Query) -> Result<NodeId> {
        let id = self.create(parent, index, name)?;
        self.node_mut(id)?.kind = Kind::Search(query);
        Ok(id)
    }

    pub fn set_query(&mut self, id: NodeId, query: crate::query::Query) -> Result<()> {
        let id = self.resolve(id);
        match &mut self.node_mut(id)?.kind {
            Kind::Search(q) => {
                *q = query;
                Ok(())
            }
            _ => Err(Error::Invalid(format!("node {id} is not a search node"))),
        }
    }

    pub fn query(&self, id: NodeId) -> Option<&crate::query::Query> {
        match &self.get(self.resolve(id))?.kind {
            Kind::Search(q) => Some(q),
            _ => None,
        }
    }

    /// The view settings of `id`, created on first change (undoable like any edit).
    pub fn view_mut(&mut self, id: NodeId) -> Result<&mut crate::view::ViewDef> {
        let id = self.resolve(id);
        Ok(self.node_mut(id)?.view.get_or_insert_with(Default::default))
    }

    // ---- supertags and fields -----------------------------------------------------------------

    pub fn create_tag(&mut self, name: &str, color: &str) -> Result<NodeId> {
        let name = name.trim().trim_start_matches('#');
        if name.is_empty() {
            return Err(Error::Invalid("a supertag needs a name".into()));
        }
        if let Some(t) = self.find_tag(name) {
            return Err(Error::Invalid(format!("supertag #{} already exists ({t})", self.name(t))));
        }
        let color = if TAG_COLORS.contains(&color) { color } else { "grey" };
        let id = self.create(self.sys.schema, None, name)?;
        self.node_mut(id)?.kind = Kind::Tag(TagDef { color: color.to_string(), ..TagDef::default() });
        Ok(id)
    }

    pub fn create_field(&mut self, name: &str, field_type: FieldType) -> Result<NodeId> {
        if name.trim().is_empty() {
            return Err(Error::Invalid("a field needs a name".into()));
        }
        let id = self.create(self.sys.schema, None, name)?;
        self.node_mut(id)?.kind = Kind::Field(FieldDef { field_type, ..FieldDef::default() });
        Ok(id)
    }

    fn tag_def_mut(&mut self, tag: NodeId) -> Result<&mut TagDef> {
        match &mut self.node_mut(tag)?.kind {
            Kind::Tag(t) => Ok(t),
            _ => Err(Error::Invalid(format!("node {tag} is not a supertag"))),
        }
    }

    pub fn field_def_mut(&mut self, field: NodeId) -> Result<&mut FieldDef> {
        match &mut self.node_mut(field)?.kind {
            Kind::Field(f) => Ok(f),
            _ => Err(Error::Invalid(format!("node {field} is not a field"))),
        }
    }

    /// Adds `field` to the template of `tag`; nodes already tagged gain it.
    pub fn tag_add_field(&mut self, tag: NodeId, field: NodeId) -> Result<()> {
        self.field_def(field).ok_or_else(|| Error::Invalid(format!("node {field} is not a field")))?;
        let def = self.tag_def_mut(tag)?;
        if !def.fields.contains(&field) {
            def.fields.push(field);
        }
        for n in self.nodes_with_lineage(tag) {
            self.ensure_fields(n, &[field])?;
        }
        Ok(())
    }

    /// Makes `tag` extend `base`. Refuses cycles.
    pub fn tag_extend(&mut self, tag: NodeId, base: NodeId) -> Result<()> {
        self.tag_def(base).ok_or_else(|| Error::Invalid(format!("node {base} is not a supertag")))?;
        if self.tag_lineage(base).contains(&tag) {
            return Err(Error::Invalid("that would make the supertags extend each other".into()));
        }
        let def = self.tag_def_mut(tag)?;
        if !def.extends.contains(&base) {
            def.extends.push(base);
        }
        let fields = self.template_fields(tag);
        for n in self.nodes_with_lineage(tag) {
            self.ensure_fields(n, &fields)?;
        }
        Ok(())
    }

    pub fn set_tag_color(&mut self, tag: NodeId, color: &str) -> Result<()> {
        if !TAG_COLORS.contains(&color) {
            return Err(Error::Invalid(format!("unknown colour `{color}`; use one of {}", TAG_COLORS.join(", "))));
        }
        self.tag_def_mut(tag)?.color = color.to_string();
        Ok(())
    }

    pub fn set_tag_checkbox(&mut self, tag: NodeId, on: bool) -> Result<()> {
        self.tag_def_mut(tag)?.checkbox = on;
        Ok(())
    }

    /// Nodes tagged with `tag` or with a supertag that extends it.
    fn nodes_with_lineage(&self, tag: NodeId) -> Vec<NodeId> {
        self.nodes.values().filter(|n| n.tags.iter().any(|t| self.tag_lineage(*t).contains(&tag))).map(|n| n.id).collect()
    }

    fn ensure_fields(&mut self, node: NodeId, fields: &[NodeId]) -> Result<()> {
        let missing: Vec<NodeId> =
            fields.iter().copied().filter(|f| self.get(node).is_some_and(|n| !n.fields.iter().any(|e| e.field == *f))).collect();
        for f in missing {
            let value = self.field_def(f).and_then(|d| d.default.clone());
            self.node_mut(node)?.fields.push(FieldEntry { field: f, value });
        }
        Ok(())
    }

    /// Applies supertag `tag` to `node`: the node gains the template fields (with defaults) and a
    /// checkbox when the supertag asks for one.
    pub fn add_tag(&mut self, node: NodeId, tag: NodeId) -> Result<()> {
        let node = self.resolve(node);
        self.tag_def(tag).ok_or_else(|| Error::Invalid(format!("node {tag} is not a supertag")))?;
        if self.node(node)?.tags.contains(&tag) {
            return Ok(());
        }
        self.node_mut(node)?.tags.push(tag);
        let fields = self.template_fields(tag);
        self.ensure_fields(node, &fields)?;
        if self.tag_wants_checkbox(node) && self.node(node)?.done.is_none() {
            self.node_mut(node)?.done = Some(false);
        }
        Ok(())
    }

    /// Removes supertag `tag` from `node`. Empty fields that only that supertag brought go too.
    pub fn remove_tag(&mut self, node: NodeId, tag: NodeId) -> Result<()> {
        let node = self.resolve(node);
        if !self.node(node)?.tags.contains(&tag) {
            return Ok(());
        }
        self.node_mut(node)?.tags.retain(|t| *t != tag);
        let still: Vec<NodeId> = self.node(node)?.tags.clone().iter().flat_map(|t| self.template_fields(*t)).collect();
        let gone = self.template_fields(tag);
        let n = self.node_mut(node)?;
        n.fields.retain(|e| e.value.is_some() || still.contains(&e.field) || !gone.contains(&e.field));
        Ok(())
    }

    /// Sets (or clears, with `None`) a field value on `node`.
    pub fn set_field(&mut self, node: NodeId, field: NodeId, value: Option<FieldValue>) -> Result<()> {
        let node = self.resolve(node);
        self.field_def(field).ok_or_else(|| Error::Invalid(format!("node {field} is not a field")))?;
        let n = self.node_mut(node)?;
        match n.fields.iter_mut().find(|e| e.field == field) {
            Some(e) => e.value = value,
            None => n.fields.push(FieldEntry { field, value }),
        }
        Ok(())
    }

    /// Sets a field from typed text, parsed by the field's type. Options fields collect new choices;
    /// options-from-supertag fields link to (or create) a node with that supertag.
    pub fn set_field_text(&mut self, node: NodeId, field: NodeId, text: &str) -> Result<()> {
        let def = self.field_def(field).cloned().ok_or_else(|| Error::Invalid(format!("node {field} is not a field")))?;
        if text.trim().is_empty() {
            return self.set_field(node, field, None);
        }
        let value = match (def.field_type, def.source_tag) {
            (FieldType::OptionsFromTag, Some(tag)) => {
                let found = self.tagged(tag).into_iter().find(|n| self.name(*n).eq_ignore_ascii_case(text.trim()));
                let target = match found {
                    Some(t) => t,
                    None => {
                        let t = self.create(self.sys.library, None, text)?;
                        self.add_tag(t, tag)?;
                        t
                    }
                };
                FieldValue::Node(target)
            }
            (ty, _) => FieldValue::parse(ty, text).map_err(Error::Invalid)?,
        };
        if let FieldValue::Choice(c) = &value
            && !def.options.iter().any(|o| o == c)
        {
            self.field_def_mut(field)?.options.push(c.clone());
        }
        self.set_field(node, field, Some(value))
    }

    /// Removes the field entry entirely.
    pub fn remove_field(&mut self, node: NodeId, field: NodeId) -> Result<()> {
        let node = self.resolve(node);
        self.node_mut(node)?.fields.retain(|e| e.field != field);
        Ok(())
    }

    // ---- daily notes --------------------------------------------------------------------------

    /// The day node for `date`, created (with its year and week) when missing.
    pub fn day(&mut self, date: Date) -> Result<NodeId> {
        if let Some(d) = self.find_day(date) {
            return Ok(d);
        }
        let (wy, wk) = date.iso_week();
        let journal = self.sys.journal;
        let year = self.calendar_child(journal, Kind::Year { year: wy }, &wy.to_string(), wy as i64)?;
        let week = self.calendar_child(year, Kind::Week { year: wy, week: wk }, &format!("Week {wk}"), wk as i64)?;
        let day = self.calendar_child(week, Kind::Day { date }, &date.day_title(), date.to_days())?;
        let tag = self.sys.day_tag;
        self.add_tag(day, tag)?;
        Ok(day)
    }

    /// Finds or creates a calendar child, keeping siblings sorted by `key`.
    fn calendar_child(&mut self, parent: NodeId, kind: Kind, name: &str, key: i64) -> Result<NodeId> {
        let kids = self.node(parent)?.children.clone();
        if let Some(c) = kids.iter().find(|c| self.get(**c).is_some_and(|n| n.kind == kind)) {
            return Ok(*c);
        }
        let at = kids.iter().position(|c| self.get(*c).and_then(|n| calendar_key(&n.kind)).is_some_and(|k| k > key)).unwrap_or(kids.len());
        let id = self.create(parent, Some(at), name)?;
        self.node_mut(id)?.kind = kind;
        Ok(id)
    }

    // ---- persistence --------------------------------------------------------------------------

    pub fn to_json(&self) -> Result<String> {
        let file = GraphFile {
            format: FORMAT.to_string(),
            version: 1,
            next_id: self.next_id,
            system: self.sys,
            expanded: self.expanded.iter().copied().collect(),
            nodes: self.nodes.values().cloned().collect(),
        };
        serde_json::to_string_pretty(&file).map_err(|e| Error::Format(e.to_string()))
    }

    /// Loads a graph, repairing dangling links instead of failing.
    pub fn from_json(text: &str) -> Result<Graph> {
        let file: GraphFile = serde_json::from_str(text).map_err(|e| Error::Format(e.to_string()))?;
        if file.format != FORMAT {
            return Err(Error::Format(format!("not a {FORMAT} file")));
        }
        if file.version > 1 {
            return Err(Error::Format(format!("file version {} is newer than this app understands", file.version)));
        }
        let mut nodes = BTreeMap::new();
        let mut max = 0;
        for n in file.nodes {
            max = max.max(n.id.0);
            nodes.insert(n.id, n);
        }
        let s = file.system;
        for id in [s.root, s.library, s.journal, s.schema, s.trash, s.day_tag] {
            if !nodes.contains_key(&id) {
                return Err(Error::Format(format!("built-in node {id} is missing")));
            }
        }
        // Repair: children must exist and point back; each node listed by one parent at most.
        let ids: BTreeSet<NodeId> = nodes.keys().copied().collect();
        let mut owner: BTreeMap<NodeId, NodeId> = BTreeMap::new();
        for n in nodes.values_mut() {
            let me = n.id;
            n.children.retain(|c| ids.contains(c) && *c != me && owner.insert(*c, me).is_none());
        }
        for n in nodes.values_mut() {
            n.parent = owner.get(&n.id).copied();
        }
        let mut g = Graph {
            nodes,
            next_id: file.next_id.max(max + 1),
            sys: s,
            expanded: file.expanded.into_iter().collect(),
            txn: None,
            undo: Vec::new(),
            redo: Vec::new(),
            dirty: false,
        };
        // Orphans (no owner, not the root) are kept in Home rather than lost.
        let orphans: Vec<NodeId> = g.nodes.values().filter(|n| n.parent.is_none() && n.id != s.root).map(|n| n.id).collect();
        for o in orphans {
            if let Some(lib) = g.nodes.get_mut(&s.library) {
                lib.children.push(o);
            }
            if let Some(n) = g.nodes.get_mut(&o) {
                n.parent = Some(s.library);
            }
        }
        Ok(g)
    }

    /// Saves atomically (write a temp file, then rename).
    pub fn save(&mut self, path: &std::path::Path) -> Result<()> {
        let text = self.to_json()?;
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, text).map_err(|e| Error::Io(e.to_string()))?;
        std::fs::rename(&tmp, path).map_err(|e| Error::Io(e.to_string()))?;
        self.dirty = false;
        Ok(())
    }

    pub fn load(path: &std::path::Path) -> Result<Graph> {
        let text = std::fs::read_to_string(path).map_err(|e| Error::Io(format!("{}: {e}", path.display())))?;
        Graph::from_json(&text)
    }
}

const FORMAT: &str = "tanacraft-graph";

#[derive(Serialize, Deserialize)]
struct GraphFile {
    format: String,
    version: u32,
    next_id: u64,
    system: SystemNodes,
    #[serde(default)]
    expanded: Vec<NodeId>,
    nodes: Vec<Node>,
}

fn calendar_key(k: &Kind) -> Option<i64> {
    match k {
        Kind::Year { year } => Some(*year as i64),
        Kind::Week { week, .. } => Some(*week as i64),
        Kind::Day { date } => Some(date.to_days()),
        _ => None,
    }
}

/// Node names are one line.
fn clean_name(s: &str) -> String {
    if s.contains(['\n', '\r']) { s.replace("\r\n", " ").replace(['\n', '\r'], " ") } else { s.to_string() }
}

/// Subsequence score of `q` in `text` (both lowercase), or `None` when it does not match.
pub fn fuzzy_score(q: &[char], text: &str) -> Option<i64> {
    let t: Vec<char> = text.chars().collect();
    let mut score = 0i64;
    let mut ti = 0;
    let mut prev: Option<usize> = None;
    for qc in q {
        let at = (ti..t.len()).find(|i| t.get(*i) == Some(qc))?;
        score += 10;
        if prev.is_some_and(|p| p + 1 == at) {
            score += 15;
        }
        if at == 0 || t.get(at - 1).is_some_and(|c| !c.is_alphanumeric()) {
            score += 20;
        }
        prev = Some(at);
        ti = at + 1;
    }
    // Shorter names and earlier matches rank higher.
    Some(score - t.len() as i64 / 4 - prev.unwrap_or(0) as i64 / 8)
}
