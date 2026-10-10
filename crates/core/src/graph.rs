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
    /// The capture inbox, made the first time it is needed (older files have none).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inbox: Option<NodeId>,
}

type PreImages = BTreeMap<NodeId, Option<Node>>;

#[derive(Debug, Clone)]
struct Step {
    label: String,
    nodes: PreImages,
    /// Consecutive steps with the same key merge into one (typing a word is one undo step).
    key: Option<String>,
}

/// Settings saved with the graph (Settings > Date/Time and Preferences, spec 07 §8). Not undoable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// Times read "2:30 PM" instead of "14:30".
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub time_12h: bool,
    /// Calendar weeks start on Sunday instead of Monday.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub week_starts_sunday: bool,
    /// Sidebar entries and top bar buttons switched off (`daily_notes`, `recents`, `supertags`,
    /// `help`, `shortcuts`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden: Vec<String>,
    /// Warn on the first edit of a node that is referenced elsewhere (spec 01 §3). On by default
    /// (DIFFS N23).
    #[serde(default = "yes")]
    pub warn_referenced: bool,
    /// A node that is just a link shows a card under it (spec 08: URL embeds). On by default.
    #[serde(default = "yes")]
    pub url_embeds: bool,
    /// A small counter beside nodes that are referenced elsewhere (spec 01 §3). Off by default.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ref_counter: bool,
    /// Custom keybindings (spec 09, Customize keybindings): shortcut (`Mod+Shift+H`) to command id,
    /// checked before the defaults.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub keybindings: BTreeMap<String, String>,
}

fn yes() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            time_12h: false,
            week_starts_sunday: false,
            hidden: Vec::new(),
            warn_referenced: true,
            url_embeds: true,
            ref_counter: false,
            keybindings: BTreeMap::new(),
        }
    }
}

impl Settings {
    pub const HIDEABLE: [&'static str; 5] = ["daily_notes", "recents", "supertags", "help", "shortcuts"];

    pub fn shows(&self, what: &str) -> bool {
        !self.hidden.iter().any(|h| h == what)
    }
}

/// The node graph of one workspace.
#[derive(Debug, Clone)]
pub struct Graph {
    nodes: BTreeMap<NodeId, Node>,
    next_id: u64,
    pub sys: SystemNodes,
    /// Expanded nodes (view state; saved with the file, not undoable).
    expanded: BTreeSet<NodeId>,
    /// Search results held while the query builder is open (spec 05 §2: results wait for Run
    /// once or Done). View state: not saved.
    held: BTreeMap<NodeId, Vec<NodeId>>,
    pub settings: Settings,
    txn: Option<PreImages>,
    undo: Vec<Step>,
    redo: Vec<Step>,
    dirty: bool,
    /// The durable, mergeable copy (a Loro document). Written from `changed` on save and export.
    store: crate::store::Store,
    /// Nodes changed (or gone) since the store was last written.
    changed: BTreeSet<NodeId>,
}

impl Default for Graph {
    fn default() -> Self {
        Graph::new()
    }
}

impl Graph {
    /// An empty workspace with its system nodes.
    pub fn new() -> Graph {
        let ids = SystemNodes {
            root: NodeId(1),
            library: NodeId(2),
            journal: NodeId(3),
            schema: NodeId(4),
            trash: NodeId(5),
            day_tag: NodeId(6),
            inbox: None,
        };
        let mut g = Graph {
            nodes: BTreeMap::new(),
            next_id: 7,
            sys: ids,
            expanded: BTreeSet::new(),
            held: BTreeMap::new(),
            settings: Settings::default(),
            txn: None,
            undo: Vec::new(),
            redo: Vec::new(),
            dirty: false,
            store: crate::store::Store::new(),
            changed: BTreeSet::new(),
        };
        let mut root = Node::new(ids.root, "Workspace", None);
        root.children = vec![ids.library, ids.journal, ids.schema, ids.trash];
        g.nodes.insert(ids.root, root);
        for (id, name) in [(ids.library, "Library"), (ids.journal, "Daily notes"), (ids.schema, "Schema"), (ids.trash, "Trash")] {
            g.nodes.insert(id, Node::new(id, name, Some(ids.root)));
        }
        let mut day = Node::new(ids.day_tag, "day", Some(ids.schema));
        day.kind = Kind::Tag(TagDef { color: "grey".into(), ..TagDef::default() });
        g.nodes.insert(ids.day_tag, day);
        if let Some(s) = g.nodes.get_mut(&ids.schema) {
            s.children.push(ids.day_tag);
        }
        g.changed = g.nodes.keys().copied().collect();
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

    /// Inside a trash (the main one or a workspace's). Walks up without allocating: searches ask
    /// this of every node.
    pub fn in_trash(&self, id: NodeId) -> bool {
        let mut below = id;
        let mut cur = self.parent(id);
        for _ in 0..MAX_DEPTH {
            let Some(p) = cur else { return false };
            if p == self.sys.trash {
                return true;
            }
            if let Some(n) = self.get(p)
                && n.kind == Kind::Workspace
            {
                return n.children.get(2) == Some(&below);
            }
            below = p;
            cur = self.parent(p);
        }
        false
    }

    /// Part `i` of a further workspace (0 Library, 1 Schema, 2 Trash): its children in that order.
    fn is_workspace_part(&self, id: NodeId, i: usize) -> bool {
        self.parent(id).and_then(|p| self.get(p)).is_some_and(|p| p.kind == Kind::Workspace && p.children.get(i) == Some(&id))
    }

    pub fn is_system(&self, id: NodeId) -> bool {
        let s = self.sys;
        [s.root, s.library, s.journal, s.schema, s.trash, s.day_tag].contains(&id)
            || s.inbox == Some(id)
            || self.get(id).is_some_and(|n| n.kind == Kind::Workspace)
            // A further workspace's Library, Schema and Trash.
            || self.parent(id).and_then(|p| self.get(p)).is_some_and(|p| p.kind == Kind::Workspace && p.children.iter().take(3).any(|c| *c == id))
    }

    /// Every workspace, the main one (the root) first. All are open together (spec 07 §2).
    pub fn workspaces(&self) -> Vec<NodeId> {
        let root = self.sys.root;
        std::iter::once(root).chain(self.children(root).iter().copied().filter(|c| self.get(*c).is_some_and(|n| n.kind == Kind::Workspace))).collect()
    }

    /// The workspace a node belongs to.
    pub fn workspace_of(&self, id: NodeId) -> NodeId {
        self.ancestors(id)
            .into_iter()
            .chain(std::iter::once(id))
            .rev()
            .find(|a| self.get(*a).is_some_and(|n| n.kind == Kind::Workspace))
            .unwrap_or(self.sys.root)
    }

    fn workspace_part(&self, ws: NodeId, i: usize, main: NodeId) -> NodeId {
        if ws == self.sys.root { main } else { self.children(ws).get(i).copied().unwrap_or(main) }
    }

    /// Where content put "into" a node goes: a further workspace keeps only its Library, Schema
    /// and Trash, so content goes to its Library.
    fn content_parent(&self, parent: NodeId) -> NodeId {
        if self.get(parent).is_some_and(|n| n.kind == Kind::Workspace) { self.workspace_library(parent) } else { parent }
    }

    /// A workspace's Library.
    pub fn workspace_library(&self, ws: NodeId) -> NodeId {
        if ws == self.sys.root { self.sys.library } else { self.children(ws).first().copied().unwrap_or(ws) }
    }

    /// A workspace's Schema (its supertags and fields).
    pub fn workspace_schema(&self, ws: NodeId) -> NodeId {
        self.workspace_part(ws, 1, self.sys.schema)
    }

    /// A workspace's Trash.
    pub fn workspace_trash(&self, ws: NodeId) -> NodeId {
        self.workspace_part(ws, 2, self.sys.trash)
    }

    /// Adds a workspace beside the others, with its own Library, Schema and Trash (spec 01 §7).
    pub fn create_workspace(&mut self, name: &str) -> Result<NodeId> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Invalid("a workspace needs a name".into()));
        }
        let root = self.sys.root;
        let ws = self.create_raw(root, None, name, Kind::Workspace)?;
        // ponytail: daily notes stay the main workspace's (Today follows the main one, spec 07 §2).
        for part in ["Library", "Schema", "Trash"] {
            self.create_raw(ws, None, part, Kind::Plain)?;
        }
        Ok(ws)
    }

    /// Command nodes of every workspace, oldest first.
    pub fn command_nodes(&self) -> Vec<NodeId> {
        self.nodes.values().filter(|n| n.kind == Kind::Command && !self.in_trash(n.id)).map(|n| n.id).collect()
    }

    /// Creates a command node in workspace `ws`'s Schema with one child per step.
    pub fn create_command_node(&mut self, ws: NodeId, name: &str, steps: &[String]) -> Result<NodeId> {
        if name.trim().is_empty() {
            return Err(Error::Invalid("a command node needs a name".into()));
        }
        let schema = self.workspace_schema(ws);
        let id = self.create_raw(schema, None, name.trim(), Kind::Command)?;
        for st in steps {
            self.create_raw(id, None, st, Kind::Plain)?;
        }
        Ok(id)
    }

    /// The capture inbox (spec 01 §7), created under the workspace on first use.
    pub fn inbox(&mut self) -> Result<NodeId> {
        if let Some(i) = self.sys.inbox.filter(|i| self.get(*i).is_some()) {
            return Ok(i);
        }
        let root = self.sys.root;
        let at = self.node(root)?.children.iter().position(|c| *c == self.sys.trash);
        // After an undo removed it, the inbox comes back under the same id, so redo stays exact.
        if let Some(i) = self.sys.inbox {
            self.put(Node::new(i, "Inbox", None));
            self.attach(i, root, at)?;
            return Ok(i);
        }
        let id = self.create(root, at, "Inbox")?;
        self.sys.inbox = Some(id);
        Ok(id)
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

    /// A node's text as people read it: marks removed, inline references replaced by names.
    pub fn text(&self, id: NodeId) -> String {
        let raw = self.name(id);
        if !raw.contains("[[id:") {
            return crate::format::plain(raw);
        }
        crate::format::spans(raw)
            .into_iter()
            .map(|sp| match sp.node {
                Some(n) => crate::format::plain(self.name(NodeId(n))),
                None => sp.text,
            })
            .collect()
    }

    /// Case-insensitive substring search over names, outside the trash.
    pub fn search(&self, query: &str) -> Vec<NodeId> {
        let q = query.to_lowercase();
        self.nodes
            .values()
            .filter(|n| {
                n.reference_target().is_none() && !self.is_system(n.id) && self.text(n.id).to_lowercase().contains(&q) && !self.in_trash(n.id)
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
            .filter(|n| {
                n.reference_target().is_none() && !self.is_system(n.id) && !self.in_trash(n.id) && !self.tag_def(n.id).is_some_and(|d| d.hidden)
            })
            .filter_map(|n| fuzzy_score(&q, &self.text(n.id).to_lowercase()).map(|s| (s, n.id)))
            .collect();
        hits.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        hits.into_iter().take(limit).map(|(_, id)| id).collect()
    }

    /// Nodes that point at `target`: reference nodes and node-valued fields.
    pub fn references_to(&self, target: NodeId) -> Vec<NodeId> {
        // A date value points at its day node (spec 01 §8).
        let day_date = match self.get(target).map(|n| &n.kind) {
            Some(Kind::Day { date }) => Some(*date),
            _ => None,
        };
        self.nodes
            .values()
            .filter(|n| {
                n.reference_target() == Some(target)
                    || (n.name.contains("[[id:") && crate::format::refs(&n.name).contains(&target.0))
                    || n.has_value(&FieldValue::Node(target))
                    || day_date.is_some_and(|d| n.has_date(d))
            })
            .filter(|n| !self.in_trash(n.id))
            .map(|n| n.id)
            .collect()
    }

    /// How many places point at each node (references, inline links, node field values), outside
    /// the trash: one pass over the graph (for the reference counter).
    pub fn reference_counts(&self) -> std::collections::HashMap<NodeId, usize> {
        let mut out: std::collections::HashMap<NodeId, usize> = std::collections::HashMap::new();
        for n in self.nodes.values().filter(|n| !self.in_trash(n.id)) {
            let mut to: BTreeSet<NodeId> = BTreeSet::new();
            to.extend(n.reference_target());
            if n.name.contains("[[id:") {
                to.extend(crate::format::refs(&n.name).into_iter().map(NodeId));
            }
            for e in &n.fields {
                to.extend(e.values().filter_map(|v| if let FieldValue::Node(t) = v { Some(*t) } else { None }));
            }
            for t in to {
                *out.entry(t).or_default() += 1;
            }
        }
        out
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
    /// A time of day as the settings want it: `14:30` or `2:30 PM`.
    pub fn fmt_time(&self, min: u16) -> String {
        if !self.settings.time_12h {
            return fmt_hm(min);
        }
        let (h, m) = (min / 60 % 24, min % 60);
        let h12 = if h % 12 == 0 { 12 } else { h % 12 };
        format!("{h12}:{m:02} {}", if h < 12 { "AM" } else { "PM" })
    }

    /// The first day of the week holding `d` (Monday, or Sunday when set).
    pub fn week_start(&self, d: crate::Date) -> crate::Date {
        let off = if self.settings.week_starts_sunday { (d.weekday() + 1) % 7 } else { d.weekday() };
        d.add_days(-(off as i64))
    }

    /// Weekday names in calendar order.
    pub fn weekday_names(&self) -> [&'static str; 7] {
        if self.settings.week_starts_sunday {
            ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
        } else {
            ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
        }
    }

    /// Supertags offered in the `#` menu for `node`: hidden building blocks left out, the ones
    /// promoted for the base types of the node's supertags first (spec 03 §5).
    pub fn menu_tags(&self, node: NodeId) -> Vec<NodeId> {
        let bases: Vec<String> = self
            .get(self.resolve(node))
            .map(|n| n.tags.iter().flat_map(|t| self.tag_lineage(*t)).filter_map(|t| self.tag_def(t).and_then(|d| d.base_type.clone())).collect())
            .unwrap_or_default();
        let mut tags: Vec<NodeId> =
            self.tags().into_iter().filter(|t| *t != self.sys.day_tag && self.tag_def(*t).is_some_and(|d| !d.hidden)).collect();
        // Stable: promoted ones move to the front, the rest keep their order.
        tags.sort_by_key(|t| !self.tag_def(*t).is_some_and(|d| d.promote_for.iter().any(|b| bases.contains(b))));
        tags
    }

    pub fn tag_set_building_block(&mut self, tag: NodeId, promote_for: Option<Vec<String>>, hidden: Option<bool>) -> Result<()> {
        if let Some(p) = &promote_for
            && let Some(bad) = p.iter().find(|b| !BASE_TYPES.contains(&b.as_str()))
        {
            return Err(Error::Invalid(format!("`{bad}` is not a base type ({})", BASE_TYPES.join(", "))));
        }
        let d = self.tag_def_mut(tag)?;
        if let Some(p) = promote_for {
            d.promote_for = p;
        }
        if let Some(h) = hidden {
            d.hidden = h;
        }
        Ok(())
    }

    /// Holds a search node's results as they are now (`true`, again to refresh) or lets them
    /// follow every edit (`false`).
    pub fn hold_results(&mut self, search: NodeId, on: bool) {
        let search = self.resolve(search);
        let fresh = match self.get(search).map(|n| &n.kind) {
            Some(Kind::Search(q)) if on => Some(q.run(self, search)),
            _ => None,
        };
        match fresh {
            Some(items) => {
                self.held.insert(search, items);
            }
            None => {
                self.held.remove(&search);
            }
        }
    }

    /// Lets every held search follow edits again.
    pub fn release_held(&mut self) {
        self.held.clear();
    }

    /// The held results of a search node, if held (gone or trashed nodes left out).
    pub fn held_results(&self, search: NodeId) -> Option<Vec<NodeId>> {
        self.held.get(&search).map(|v| v.iter().copied().filter(|n| self.get(*n).is_some() && !self.in_trash(*n)).collect())
    }

    /// Changing settings is a change to save (not an undo step).
    pub fn settings_mut(&mut self) -> &mut Settings {
        self.dirty = true;
        &mut self.settings
    }

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

    /// Closes the open step without making it an undo step: structure made while looking around
    /// (today's day node, the inbox) counts as having always been there. Older undo and redo steps
    /// learn about it, so undoing them later does not cut it out again.
    pub fn commit_silent(&mut self) {
        let Some(pre) = self.txn.take() else { return };
        // New nodes hung under a node that some step makes or removes go and come back with it.
        let made: BTreeSet<NodeId> = pre.iter().filter(|(id, b)| b.is_none() && self.nodes.contains_key(id)).map(|(id, _)| *id).collect();
        for n in &made {
            let mut anchor = *n;
            while made.contains(&anchor) {
                match self.parent(anchor) {
                    Some(p) => anchor = p,
                    None => break,
                }
            }
            for step in self.undo.iter_mut().chain(self.redo.iter_mut()) {
                if matches!(step.nodes.get(&anchor), Some(None)) {
                    step.nodes.entry(*n).or_insert(None);
                }
            }
        }
        for (id, before) in &pre {
            let (Some(before), Some(after)) = (before, self.nodes.get(id)) else { continue };
            for step in self.undo.iter_mut().chain(self.redo.iter_mut()) {
                if let Some(Some(old)) = step.nodes.get_mut(id) {
                    rebase(old, before, after);
                }
            }
        }
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

    /// Writes `images` into the graph and returns what was there before. Images rebased over a
    /// merge can disagree about who owns whom; the links are settled afterwards.
    fn apply(&mut self, images: PreImages) -> PreImages {
        let ids: Vec<NodeId> = images.keys().copied().collect();
        let prior: BTreeMap<NodeId, Vec<NodeId>> = ids.iter().filter_map(|id| Some((*id, self.nodes.get(id)?.children.clone()))).collect();
        let before = self.apply_raw(images);
        let mut ids = ids;
        // Children of a node that went, which were not taken with it (another copy put them there),
        // move up to its parent.
        for (id, old) in &before {
            let Some(gone) = old.as_ref().filter(|_| !self.nodes.contains_key(id)) else { continue };
            for c in &gone.children {
                if let Some(n) = self.nodes.get_mut(c).filter(|n| n.parent == Some(*id)) {
                    n.parent = gone.parent;
                    ids.push(*c);
                }
            }
        }
        // Old owners that still list a node that moved away (or went) let it go.
        for (id, old) in &before {
            let Some(op) = old.as_ref().and_then(|n| n.parent) else { continue };
            if self.nodes.get(id).and_then(|n| n.parent) != Some(op)
                && let Some(o) = self.nodes.get_mut(&op)
                && o.children.contains(id)
            {
                o.children.retain(|c| c != id);
                self.changed.insert(op);
            }
        }
        self.settle_links(&ids, &prior);
        before
    }

    /// Makes parents and children agree for the nodes `ids` (a node's `parent` decides): children
    /// owned elsewhere leave the list, owned ones missing from it are added, and nodes whose parent
    /// is gone or that sit in a loop move to the Library.
    fn settle_links(&mut self, ids: &[NodeId], prior: &BTreeMap<NodeId, Vec<NodeId>>) {
        let library = self.sys.library;
        for id in ids {
            let Some(n) = self.nodes.get(id) else { continue };
            let mut kids: Vec<NodeId> = Vec::new();
            for c in n.children.iter().chain(prior.get(id).into_iter().flatten()) {
                if !kids.contains(c) && self.nodes.get(c).is_some_and(|x| x.parent == Some(*id)) {
                    kids.push(*c);
                }
            }
            if kids != n.children {
                self.changed.insert(*id);
                if let Some(n) = self.nodes.get_mut(id) {
                    n.children = kids;
                }
            }
        }
        for id in ids {
            let Some(p) = self.nodes.get(id).and_then(|n| n.parent) else { continue };
            // Up from the parent: reaching this node again (or never the root) is a loop.
            let mut at = Some(p);
            let mut steps = 0usize;
            while let Some(x) = at.filter(|x| *x != *id && steps < 100_000) {
                at = self.nodes.get(&x).and_then(|n| n.parent);
                steps += 1;
            }
            let looped = at.is_some();
            let home = if looped || !self.nodes.contains_key(&p) { library } else { p };
            if home != p {
                if let Some(old) = self.nodes.get_mut(&p) {
                    old.children.retain(|c| c != id);
                }
                if let Some(n) = self.nodes.get_mut(id) {
                    n.parent = Some(home);
                }
            }
            if let Some(h) = self.nodes.get_mut(&home)
                && !h.children.contains(id)
            {
                h.children.push(*id);
                self.changed.insert(home);
            }
            self.changed.insert(*id);
        }
    }

    fn apply_raw(&mut self, images: PreImages) -> PreImages {
        let mut before = PreImages::new();
        for (id, img) in images {
            self.changed.insert(id);
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
        self.changed.insert(id);
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
        let parent = self.content_parent(self.resolve(parent));
        self.node(parent)?;
        let id = self.alloc();
        self.put(Node::new(id, clean_name(name), Some(parent)));
        self.attach(id, parent, index)?;
        // A supertag's "default child supertag" tags new children (spec 03 §5).
        let child_tags: Vec<NodeId> = self
            .node(parent)?
            .tags
            .clone()
            .iter()
            .filter_map(|t| self.tag_def(*t).and_then(|d| d.child_tag))
            .filter(|t| self.tag_def(*t).is_some())
            .collect();
        for t in child_tags {
            self.add_tag(id, t)?;
        }
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
        let parent = self.content_parent(self.resolve(parent));
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

    /// A note for this place only, on a reference node (spec 01 §3, "Notes and fields for this
    /// context"). Stored on the reference itself, not on what it points at.
    pub fn set_context_note(&mut self, reference: NodeId, text: &str) -> Result<()> {
        if self.node(reference)?.reference_target().is_none() {
            return Err(Error::Invalid("context notes belong to references".into()));
        }
        self.node_mut(reference)?.description = clean_name(text);
        Ok(())
    }

    /// The node holding a reference's notes for this place (spec 01 §3, DIFFS N22): the one plain
    /// node a reference may own. Its children are the notes, an ordinary outline.
    pub fn context_of(&self, reference: NodeId) -> Option<NodeId> {
        let n = self.get(reference)?;
        n.reference_target()?;
        n.children.first().copied()
    }

    /// Whether `id` holds a reference's notes (its owner is a reference).
    pub fn is_context(&self, id: NodeId) -> bool {
        self.parent(id).and_then(|p| self.get(p)).is_some_and(|p| p.reference_target().is_some())
    }

    /// A line in a reference's notes for this place: the first one, or a new empty one.
    pub fn context_line(&mut self, reference: NodeId) -> Result<NodeId> {
        if self.node(reference)?.reference_target().is_none() {
            return Err(Error::Invalid("notes for a context belong to references".into()));
        }
        let holder = match self.context_of(reference) {
            Some(c) => c,
            None => {
                let c = self.alloc();
                self.put(Node::new(c, String::new(), Some(reference)));
                self.attach(c, reference, None)?;
                c
            }
        };
        match self.children(holder).first() {
            Some(first) => Ok(*first),
            None => self.create(holder, None, ""),
        }
    }

    /// A field value for this place only, on a reference node (spec 01 §3, "Notes and fields for
    /// this context"). Empty text removes it.
    pub fn set_context_field(&mut self, reference: NodeId, field: NodeId, text: &str) -> Result<()> {
        if self.node(reference)?.reference_target().is_none() {
            return Err(Error::Invalid("context fields belong to references".into()));
        }
        self.field_def(field).ok_or_else(|| Error::Invalid(format!("node {field} is not a field")))?;
        let value = if text.trim().is_empty() { None } else { Some(self.parse_field_text(field, text)?) };
        let n = self.node_mut(reference)?;
        n.fields.retain(|e| e.field != field);
        if let Some(v) = value {
            n.fields.push(FieldEntry::new(field, Some(v)));
        }
        Ok(())
    }

    /// `Some(done)` shows a checkbox, `None` hides it.
    pub fn set_done(&mut self, id: NodeId, done: Option<bool>) -> Result<()> {
        let target = self.resolve(id);
        self.write_done(target, done)?;
        // Done-state mapping: checking sets the mapped field (spec 03 §5).
        for m in self.done_maps(target) {
            let value = match done {
                Some(true) => m.checked.clone(),
                _ => m.unchecked.clone(),
            };
            if !value.is_empty() {
                let v = self.parse_field_text(m.field, &value)?;
                self.set_field(target, m.field, Some(v))?;
            }
        }
        Ok(())
    }

    fn write_done(&mut self, target: NodeId, done: Option<bool>) -> Result<()> {
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
        if self.is_system(parent) || self.is_context(parent) {
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
        // Each workspace has its own trash.
        let trash = self.workspace_trash(self.workspace_of(id));
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
        // A supertag or field that is gone leaves no tags, field entries or template slots behind.
        let gone: Vec<NodeId> = seen.into_iter().collect();
        let users: Vec<NodeId> = self
            .nodes
            .values()
            .filter(|n| {
                n.tags.iter().any(|t| gone.contains(t))
                    || n.fields.iter().any(|e| gone.contains(&e.field))
                    || matches!(&n.kind, Kind::Tag(d) if d.extends.iter().chain(&d.fields).chain(&d.optional).chain(&d.pinned_fields).any(|x| gone.contains(x))
                        || d.child_tag.is_some_and(|c| gone.contains(&c))
                        || d.done_map.as_ref().is_some_and(|m| gone.contains(&m.field)))
            })
            .map(|n| n.id)
            .collect();
        for u in users {
            let n = self.node_mut(u)?;
            n.tags.retain(|t| !gone.contains(t));
            n.fields.retain(|e| !gone.contains(&e.field));
            if let Kind::Tag(d) = &mut n.kind {
                for list in [&mut d.extends, &mut d.fields, &mut d.optional, &mut d.pinned_fields] {
                    list.retain(|x| !gone.contains(x));
                }
                if d.child_tag.is_some_and(|c| gone.contains(&c)) {
                    d.child_tag = None;
                }
                if d.done_map.as_ref().is_some_and(|m| gone.contains(&m.field)) {
                    d.done_map = None;
                }
            }
        }
        Ok(())
    }

    pub fn empty_trash(&mut self) -> Result<usize> {
        self.empty_trash_of(self.sys.trash)
    }

    /// Empties one workspace's trash.
    pub fn empty_trash_of(&mut self, trash: NodeId) -> Result<usize> {
        if trash != self.sys.trash && !self.is_workspace_part(trash, 2) {
            return Err(Error::Invalid(format!("node {trash} is not a trash")));
        }
        let items = self.node(trash)?.children.clone();
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
        // Only content merges: a supertag, field, search or calendar node is not text to join.
        if self.is_system(src) || !matches!(self.node(src)?.kind, Kind::Plain | Kind::Reference { .. }) {
            return Err(Error::Invalid("only a plain node can be merged into another".into()));
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

    /// A deep copy of `id` placed under `parent` at `index`.
    pub fn copy_into(&mut self, id: NodeId, parent: NodeId, index: usize) -> Result<NodeId> {
        let copy = self.copy_subtree(id, parent, 0)?;
        let kids = &mut self.node_mut(parent)?.children;
        kids.insert(index.min(kids.len()), copy);
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
        // Not `create`: a reference carries no tags, fields or children of its own (a parent's
        // default child supertag would otherwise land on it).
        self.create_raw(parent, index, "", Kind::Reference { target })
    }

    /// Creates a node of `kind` without applying the parent's default child supertag.
    fn create_raw(&mut self, parent: NodeId, index: Option<usize>, name: &str, kind: Kind) -> Result<NodeId> {
        let parent = self.resolve(parent);
        self.node(parent)?;
        let id = self.alloc();
        let mut n = Node::new(id, clean_name(name), Some(parent));
        n.kind = kind;
        self.put(n);
        self.attach(id, parent, index)?;
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
        // Starts from what the node shows now (a supertag page shows a table), so a first sort or
        // filter keeps the view as it is (DIFFS N47).
        let shown = self.view_def(id);
        Ok(self.node_mut(id)?.view.get_or_insert(shown))
    }

    // ---- supertags and fields -----------------------------------------------------------------

    pub fn create_tag(&mut self, name: &str, color: &str) -> Result<NodeId> {
        self.create_tag_in(self.sys.root, name, color)
    }

    /// Creates a supertag in workspace `ws`'s Schema.
    pub fn create_tag_in(&mut self, ws: NodeId, name: &str, color: &str) -> Result<NodeId> {
        let name = name.trim().trim_start_matches('#');
        if name.is_empty() {
            return Err(Error::Invalid("a supertag needs a name".into()));
        }
        if let Some(t) = self.find_tag(name) {
            return Err(Error::Invalid(format!("supertag #{} already exists ({t})", self.name(t))));
        }
        let color = if TAG_COLORS.contains(&color) || is_custom_color(color) { color } else { "grey" };
        let schema = self.workspace_schema(ws);
        let id = self.create(schema, None, name)?;
        self.node_mut(id)?.kind = Kind::Tag(TagDef { color: color.to_string(), ..TagDef::default() });
        Ok(id)
    }

    /// Sets a page's icon (`Some(icon)`, where `None` inside clears) and/or banner.
    pub fn set_decoration(&mut self, id: NodeId, icon: Option<Option<String>>, banner: Option<Option<String>>) -> Result<()> {
        let n = self.node_mut(id)?;
        if let Some(i) = icon {
            n.icon = i;
        }
        if let Some(b) = banner {
            n.banner = b;
        }
        Ok(())
    }

    /// "Convert to supertag": a plain node becomes a supertag (its children become template
    /// content). It moves to the Schema and a reference stays where it was.
    pub fn convert_to_tag(&mut self, id: NodeId, color: &str) -> Result<NodeId> {
        let n = self.node(id)?.clone();
        if self.is_system(id) || n.kind != Kind::Plain {
            return Err(Error::Invalid("only a plain node can become a supertag".into()));
        }
        let name = crate::format::plain(&n.name);
        if name.trim().is_empty() {
            return Err(Error::Invalid("a supertag needs a name".into()));
        }
        if let Some(t) = self.find_tag(&name) {
            return Err(Error::Invalid(format!("supertag #{} already exists ({t})", self.name(t))));
        }
        let (parent, index) = (self.parent(id), self.index_in_parent(id));
        let color = if TAG_COLORS.contains(&color) || crate::model::is_custom_color(color) { color } else { "grey" };
        let template = n.children.clone();
        self.node_mut(id)?.kind = Kind::Tag(TagDef { color: color.to_string(), template, ..TagDef::default() });
        self.node_mut(id)?.name = name;
        let schema = self.sys.schema;
        self.move_to(id, schema, None)?;
        if let Some(p) = parent {
            self.create_reference(p, index, id)?;
        }
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
        if !TAG_COLORS.contains(&color) && !crate::model::is_custom_color(color) {
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
        self.nodes
            .values()
            .filter(|n| n.reference_target().is_none() && n.tags.iter().any(|t| self.tag_lineage(*t).contains(&tag)))
            .map(|n| n.id)
            .collect()
    }

    fn ensure_fields(&mut self, node: NodeId, fields: &[NodeId]) -> Result<()> {
        let missing: Vec<NodeId> = fields
            .iter()
            .copied()
            .filter(|f| self.field_def(*f).is_some() && self.get(node).is_some_and(|n| !n.fields.iter().any(|e| e.field == *f)))
            .collect();
        for f in missing {
            let value = self.field_def(f).and_then(|d| d.default.clone());
            self.node_mut(node)?.fields.push(FieldEntry::new(f, value));
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
        let had: Vec<NodeId> = self.node(node)?.fields.iter().map(|f| f.field).collect();
        self.ensure_fields(node, &fields)?;
        for f in fields.iter().filter(|f| !had.contains(f)) {
            self.auto_init(node, *f)?;
        }
        // Template content: plain nodes copied under the tagged node.
        let mut templates = Vec::new();
        for t in self.tag_lineage(tag) {
            templates.extend(self.tag_def(t).map(|d| d.template.clone()).unwrap_or_default());
        }
        // A template never copies into its own subtree (that copy would hold the template again,
        // and every later tagging would double the outline).
        templates.retain(|t| self.get(*t).is_some() && !self.is_ancestor_or_self(*t, node));
        for t in templates {
            let copy = self.copy_subtree(t, node, 0)?;
            self.node_mut(node)?.children.push(copy);
        }
        if self.tag_wants_checkbox(node) && self.node(node)?.done.is_none() {
            self.node_mut(node)?.done = Some(false);
        }
        Ok(())
    }

    /// Fills a newly added field from its auto-initialize rule.
    fn auto_init(&mut self, node: NodeId, field: NodeId) -> Result<()> {
        let Some(rule) = self.field_def(field).and_then(|d| d.auto_init) else { return Ok(()) };
        let value = match rule {
            AutoInit::CurrentDate => Some(FieldValue::Date(Date::today())),
            AutoInit::AncestorDay => self.ancestors(node).iter().rev().find_map(|a| match self.get(*a).map(|n| &n.kind) {
                Some(Kind::Day { date }) => Some(FieldValue::Date(*date)),
                _ => None,
            }),
            AutoInit::AncestorValue => self.ancestors(node).iter().rev().find_map(|a| self.get(*a).and_then(|n| n.field_value(field)).cloned()),
        };
        if value.is_some() {
            self.set_field(node, field, value)?;
        }
        Ok(())
    }

    /// The done mappings of a node's supertags.
    fn done_maps(&self, node: NodeId) -> Vec<DoneMap> {
        let tags = self.get(node).map(|n| n.tags.clone()).unwrap_or_default();
        tags.iter().flat_map(|t| self.tag_lineage(*t)).filter_map(|t| self.tag_def(t).and_then(|d| d.done_map.clone())).collect()
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

    /// Sets (or clears, with `None`) the first value of a field on `node`. Clearing promotes the
    /// next value, if any.
    pub fn set_field(&mut self, node: NodeId, field: NodeId, value: Option<FieldValue>) -> Result<()> {
        self.set_field_at(node, field, 0, value)
    }

    /// Sets value number `index` of a field (`index` == count appends; `None` removes that value).
    pub fn set_field_at(&mut self, node: NodeId, field: NodeId, index: usize, value: Option<FieldValue>) -> Result<()> {
        let node = self.resolve(node);
        // A date value is a link to its day: make sure the day exists (spec 01 §8).
        if let Some(d) = value.as_ref().and_then(FieldValue::date) {
            self.day(d)?;
        }
        self.field_def(field).ok_or_else(|| Error::Invalid(format!("node {field} is not a field")))?;
        let n = self.node_mut(node)?;
        let ix = match n.fields.iter().position(|e| e.field == field) {
            Some(i) => i,
            None => {
                n.fields.push(FieldEntry::new(field, None));
                n.fields.len() - 1
            }
        };
        let e = &mut n.fields[ix];
        let mut vals: Vec<FieldValue> = e.values().cloned().collect();
        let index = index.min(vals.len());
        match value {
            Some(v) if index < vals.len() => vals[index] = v,
            Some(v) if !vals.contains(&v) => vals.push(v),
            Some(_) => {}
            None if index < vals.len() => {
                vals.remove(index);
            }
            None => {}
        }
        e.set_values(vals);
        Ok(())
    }

    /// Sets a field from typed text, parsed by the field's type. Options fields collect new choices;
    /// options-from-supertag fields link to (or create) a node with that supertag.
    pub fn set_field_text(&mut self, node: NodeId, field: NodeId, text: &str) -> Result<()> {
        self.set_field_text_at(node, field, 0, text)
    }

    /// Like [`Graph::set_field_text`] for value number `index` (`index` == count adds a value; empty
    /// text removes that value).
    pub fn set_field_text_at(&mut self, node: NodeId, field: NodeId, index: usize, text: &str) -> Result<()> {
        let node = self.resolve(node);
        let value = if text.trim().is_empty() { None } else { Some(self.parse_field_text(field, text)?) };
        self.set_field_at(node, field, index, value)?;
        // Done-state mapping the other way: the mapped field moves the checkbox.
        if index == 0 {
            for m in self.done_maps(node).into_iter().filter(|m| m.field == field) {
                let checked = text.trim().eq_ignore_ascii_case(m.checked.trim());
                if self.checkbox(node) != Some(checked) {
                    self.write_done(node, Some(checked))?;
                }
            }
        }
        Ok(())
    }

    /// Parses text into a value of `field`'s type. Options fields collect new choices;
    /// options-from-supertag fields link to (or create) a node with that supertag.
    fn parse_field_text(&mut self, field: NodeId, text: &str) -> Result<FieldValue> {
        let def = self.field_def(field).cloned().ok_or_else(|| Error::Invalid(format!("node {field} is not a field")))?;
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
        if let FieldValue::Number(x) = value
            && (def.min.is_some_and(|m| x < m) || def.max.is_some_and(|m| x > m))
        {
            let range =
                format!("{} to {}", def.min.map(|m| m.to_string()).unwrap_or("any".into()), def.max.map(|m| m.to_string()).unwrap_or("any".into()));
            return Err(Error::Invalid(format!("{x} is outside the allowed range ({range})")));
        }
        if let FieldValue::Choice(c) = &value
            && !def.no_auto_collect
            && !self.field_options(field).iter().any(|o| o == c)
        {
            self.field_def_mut(field)?.options.push(c.clone());
        }
        Ok(value)
    }

    /// The people a user field can pick: the current user, then every name used in a user field.
    /// The person using this computer: the only editor of a local graph.
    pub fn local_user() -> String {
        std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_else(|_| "Me".into())
    }

    pub fn user_names(&self) -> Vec<String> {
        let mut out = vec![Graph::local_user()];
        for n in self.nodes.values() {
            for e in &n.fields {
                if self.field_def(e.field).is_some_and(|d| d.field_type == FieldType::User) {
                    for v in e.values() {
                        let name = crate::export::display_value(self, v);
                        if !name.is_empty() && !out.contains(&name) {
                            out.push(name);
                        }
                    }
                }
            }
        }
        out
    }

    /// Every option of an options field: its own, then the children of its sources.
    pub fn field_options(&self, field: NodeId) -> Vec<String> {
        let Some(d) = self.field_def(field) else { return Vec::new() };
        let mut out = d.options.clone();
        for s in &d.sources {
            for c in self.children(self.resolve(*s)) {
                let name = self.text(*c);
                if !name.is_empty() && !out.contains(&name) {
                    out.push(name);
                }
            }
        }
        out
    }

    /// Removes the field entry entirely.
    /// Shows a field row of `node` after its child `after` (`None`: above the children).
    pub fn set_field_after(&mut self, node: NodeId, field: NodeId, after: Option<NodeId>) -> Result<()> {
        let node = self.resolve(node);
        if after.is_some_and(|a| !self.node(node).is_ok_and(|n| n.children.contains(&a))) {
            return Err(Error::Invalid("`after` must be a child of the node".into()));
        }
        let n = self.node_mut(node)?;
        let e = n.fields.iter_mut().find(|e| e.field == field).ok_or_else(|| Error::Invalid(format!("node {node} has no field {field}")))?;
        e.after = after;
        Ok(())
    }

    pub fn remove_field(&mut self, node: NodeId, field: NodeId) -> Result<()> {
        let node = self.resolve(node);
        self.node_mut(node)?.fields.retain(|e| e.field != field);
        Ok(())
    }

    // ---- supertag and field configuration -----------------------------------------------------

    pub fn tag_set_done_map(&mut self, tag: NodeId, map: Option<DoneMap>) -> Result<()> {
        if let Some(m) = &map {
            self.field_def(m.field).ok_or_else(|| Error::Invalid(format!("node {} is not a field", m.field)))?;
        }
        self.tag_def_mut(tag)?.done_map = map;
        Ok(())
    }

    pub fn tag_set_child_tag(&mut self, tag: NodeId, child: Option<NodeId>) -> Result<()> {
        if let Some(c) = child {
            self.tag_def(c).ok_or_else(|| Error::Invalid(format!("node {c} is not a supertag")))?;
        }
        self.tag_def_mut(tag)?.child_tag = child;
        Ok(())
    }

    pub fn tag_set_base_type(&mut self, tag: NodeId, base: Option<&str>) -> Result<()> {
        if let Some(b) = base
            && !BASE_TYPES.contains(&b)
        {
            return Err(Error::Invalid(format!("unknown base type `{b}`; use one of {}", BASE_TYPES.join(", "))));
        }
        self.tag_def_mut(tag)?.base_type = base.map(str::to_string);
        Ok(())
    }

    /// Adds a plain template node to the supertag (copied onto nodes tagged from now on).
    pub fn tag_add_template_node(&mut self, tag: NodeId, name: &str) -> Result<NodeId> {
        self.tag_def(tag).ok_or_else(|| Error::Invalid(format!("node {tag} is not a supertag")))?;
        let id = self.create(tag, None, name)?;
        self.tag_def_mut(tag)?.template.push(id);
        // Template edits reach nodes tagged earlier (spec 03 §10).
        let targets: Vec<NodeId> = self.nodes_with_lineage(tag).into_iter().filter(|n| !self.is_ancestor_or_self(id, *n)).collect();
        for n in targets {
            let copy = self.copy_subtree(id, n, 0)?;
            self.node_mut(n)?.children.push(copy);
        }
        Ok(id)
    }

    pub fn tag_remove_field(&mut self, tag: NodeId, field: NodeId) -> Result<()> {
        self.tag_def_mut(tag)?.fields.retain(|f| *f != field);
        Ok(())
    }

    /// Adds a view tab to a supertag's page: a search for the tag shown as `kind`.
    pub fn tag_add_view(&mut self, tag: NodeId, kind: crate::view::ViewKind) -> Result<NodeId> {
        self.tag_def(tag).ok_or_else(|| Error::Invalid(format!("node {tag} is not a supertag")))?;
        let q =
            crate::query::Query { any: false, clauses: vec![crate::query::Clause { not: false, kind: crate::query::ClauseKind::Tagged { tag } }] };
        let name = format!("{} of #{}", kind.label(), self.name(tag));
        let id = self.create_search(tag, None, &name, q)?;
        self.view_mut(id)?.kind = kind;
        self.tag_def_mut(tag)?.views.push(id);
        Ok(id)
    }

    /// Turns a search node into a plain node whose children are references to today's results.
    pub fn search_to_plain(&mut self, id: NodeId) -> Result<usize> {
        let id = self.resolve(id);
        let results = self.view_items(id);
        if self.query(id).is_none() {
            return Err(Error::Invalid(format!("node {id} is not a search node")));
        }
        self.node_mut(id)?.kind = Kind::Plain;
        for r in &results {
            self.create_reference(id, None, *r)?;
        }
        Ok(results.len())
    }

    /// Reorders children for good: by name, created time, or not done first.
    pub fn sort_children(&mut self, id: NodeId, by: crate::view::SortBy, desc: bool) -> Result<()> {
        let id = self.resolve(id);
        let mut kids = self.node(id)?.children.clone();
        kids.sort_by(|a, b| {
            let (a, b) = (self.resolve(*a), self.resolve(*b));
            let o = match by {
                crate::view::SortBy::Name => self.text(a).to_lowercase().cmp(&self.text(b).to_lowercase()),
                crate::view::SortBy::Done => self.checkbox(a).unwrap_or(false).cmp(&self.checkbox(b).unwrap_or(false)),
                _ => self.get(a).map(|n| n.created).cmp(&self.get(b).map(|n| n.created)),
            };
            if desc { o.reverse() } else { o }
        });
        self.node_mut(id)?.children = kids;
        Ok(())
    }

    /// Adds related content to a supertag: a search shown beside every tagged node's page.
    pub fn tag_add_related(&mut self, tag: NodeId, name: &str, query: crate::query::Query) -> Result<NodeId> {
        self.tag_def(tag).ok_or_else(|| Error::Invalid(format!("node {tag} is not a supertag")))?;
        let id = self.create_search(tag, None, name, query)?;
        self.tag_def_mut(tag)?.related.push(id);
        Ok(id)
    }

    /// Removes a related-content search from a supertag (the search node goes to the trash).
    pub fn tag_remove_related(&mut self, tag: NodeId, search: NodeId) -> Result<()> {
        self.tag_def_mut(tag)?.related.retain(|r| *r != search);
        if self.get(search).is_some() {
            self.trash(search)?;
        }
        Ok(())
    }

    pub fn tag_set_optional(&mut self, tag: NodeId, field: NodeId, on: bool) -> Result<()> {
        self.field_def(field).ok_or_else(|| Error::Invalid(format!("node {field} is not a field")))?;
        let d = self.tag_def_mut(tag)?;
        d.optional.retain(|f| *f != field);
        if on {
            d.optional.push(field);
        }
        Ok(())
    }

    pub fn tag_set_pinned_field(&mut self, tag: NodeId, field: NodeId, on: bool) -> Result<()> {
        let d = self.tag_def_mut(tag)?;
        d.pinned_fields.retain(|f| *f != field);
        if on {
            d.pinned_fields.push(field);
        }
        Ok(())
    }

    /// Sets (or clears) a supertag's title expression. It must start with `${name}` so the title
    /// stays editable.
    pub fn tag_set_title_expr(&mut self, tag: NodeId, expr: Option<&str>) -> Result<()> {
        let expr = expr.map(str::trim).filter(|e| !e.is_empty());
        if let Some(e) = expr
            && !e.starts_with("${name}")
        {
            return Err(Error::Invalid("a title expression starts with ${name}".into()));
        }
        self.tag_def_mut(tag)?.title_expr = expr.map(str::to_string);
        Ok(())
    }

    /// The computed part of a node's title (after its name), from the title expression of one of
    /// its own supertags (not ones they extend, spec 03 §10). Each piece is (text, is a missing
    /// field shown by its name).
    pub fn title_suffix(&self, node: NodeId) -> Vec<(String, bool)> {
        let id = self.resolve(node);
        let Some(n) = self.get(id) else { return Vec::new() };
        let Some(expr) = n.tags.iter().find_map(|t| self.tag_def(*t).and_then(|d| d.title_expr.clone())) else { return Vec::new() };
        let mut rest = expr.strip_prefix("${name}").unwrap_or(&expr);
        let mut out = Vec::new();
        while let Some(open) = rest.find("${") {
            if open > 0 {
                out.push((rest[..open].to_string(), false));
            }
            let after = &rest[open + 2..];
            let Some(close) = after.find('}') else {
                out.push((rest[open..].to_string(), false));
                rest = "";
                break;
            };
            let label = &after[..close];
            let value = self
                .find_field(label)
                .map(|f| n.field_values(f).into_iter().map(|v| crate::export::shown_value(self, v)).collect::<Vec<_>>().join(", "))
                .unwrap_or_default();
            out.push(if value.is_empty() { (label.to_string(), true) } else { (value, false) });
            rest = &after[close + 1..];
        }
        if !rest.is_empty() {
            out.push((rest.to_string(), false));
        }
        out
    }

    /// Related content shown on `node`'s page (from its supertags).
    pub fn related_for(&self, node: NodeId) -> Vec<NodeId> {
        let tags = self.get(self.resolve(node)).map(|n| n.tags.clone()).unwrap_or_default();
        tags.iter()
            .flat_map(|t| self.tag_lineage(*t))
            .flat_map(|t| self.tag_def(t).map(|d| d.related.clone()).unwrap_or_default())
            .filter(|r| self.get(*r).is_some())
            .collect()
    }

    pub fn set_progress(&mut self, id: NodeId, on: bool) -> Result<()> {
        let id = self.resolve(id);
        self.node_mut(id)?.progress = on;
        Ok(())
    }

    /// Checked and total checkbox children (for the progress bar).
    pub fn progress(&self, id: NodeId) -> (usize, usize) {
        let kids = self.children(id);
        let boxes: Vec<bool> = kids.iter().filter_map(|c| self.checkbox(*c)).collect();
        (boxes.iter().filter(|b| **b).count(), boxes.len())
    }

    pub fn set_pinned(&mut self, id: NodeId, on: bool) -> Result<()> {
        self.node_mut(id)?.pinned = on;
        Ok(())
    }

    /// Pinned nodes, oldest first.
    pub fn pinned(&self) -> Vec<NodeId> {
        self.nodes.values().filter(|n| n.pinned && !self.in_trash(n.id)).map(|n| n.id).collect()
    }

    /// How many nodes carry the field, and how many supertags have it in their template.
    pub fn field_usage(&self, field: NodeId) -> (usize, usize) {
        let nodes = self.nodes.values().filter(|n| n.fields.iter().any(|f| f.field == field) && !self.in_trash(n.id)).count();
        let tags = self.tags().into_iter().filter(|t| self.tag_def(*t).is_some_and(|d| d.fields.contains(&field))).count();
        (nodes, tags)
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
        // Weeks are listed newest first on the year page (spec 01 §8).
        let week = self.calendar_child(year, Kind::Week { year: wy, week: wk }, &format!("Week {wk}"), -(wk as i64))?;
        let day = self.calendar_child(week, Kind::Day { date }, &date.day_title(), date.to_days())?;
        let tag = self.sys.day_tag;
        self.add_tag(day, tag)?;
        let week_tag = match self.find_tag("week") {
            Some(t) => t,
            None => self.create_tag("week", "grey")?,
        };
        if !self.node(week)?.tags.contains(&week_tag) {
            self.add_tag(week, week_tag)?;
        }
        Ok(day)
    }

    /// Finds or creates a calendar child, keeping siblings sorted by `key`.
    fn calendar_child(&mut self, parent: NodeId, kind: Kind, name: &str, key: i64) -> Result<NodeId> {
        let kids = self.node(parent)?.children.clone();
        if let Some(c) = kids.iter().find(|c| self.get(**c).is_some_and(|n| n.kind == kind)) {
            return Ok(*c);
        }
        let at = kids
            .iter()
            .position(|c| self.get(*c).and_then(|n| calendar_key(&n.kind)).map(|k| if n_kind_week(&kind) { -k } else { k }).is_some_and(|k| k > key))
            .unwrap_or(kids.len());
        // Raw: tagging it first (a week's default child supertag) could ask for this very day
        // again before it knows it is one.
        self.create_raw(parent, Some(at), name, kind)
    }

    // ---- persistence --------------------------------------------------------------------------

    pub fn to_json(&self) -> Result<String> {
        let file = GraphFile {
            format: FORMAT.to_string(),
            version: 1,
            next_id: self.next_id,
            system: self.sys,
            expanded: self.expanded.iter().copied().collect(),
            settings: self.settings.clone(),
            // Empty format marks of a line still being typed are not saved (DIFFS N19).
            nodes: self
                .nodes
                .values()
                .cloned()
                .map(|mut n| {
                    n.name = crate::format::strip_empty(&n.name, 0).0;
                    n
                })
                .collect(),
        };
        serde_json::to_string_pretty(&file).map_err(|e| Error::Format(e.to_string()))
    }

    /// Loads a graph, repairing dangling links instead of failing.
    pub fn from_json(text: &str) -> Result<Graph> {
        let file: GraphFile = serde_json::from_str(text).map_err(|e| Error::Format(e.to_string()))?;
        Graph::from_file(file)
    }

    /// A graph from a parsed file, repaired (dangling links, loops, wrong kinds).
    fn from_file(file: GraphFile) -> Result<Graph> {
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
        // Repair kinds and links: built-in nodes keep their kind, a reference points at a node and
        // owns at most its notes node (else it becomes a plain node holding what it had), tags are supertags and
        // fields are fields.
        let kinds: BTreeMap<NodeId, (bool, bool)> =
            nodes.values().map(|n| (n.id, (matches!(n.kind, Kind::Tag(_)), matches!(n.kind, Kind::Field(_))))).collect();
        for n in nodes.values_mut() {
            // Empty format marks left by older versions (DIFFS N19).
            n.name = crate::format::strip_empty(&n.name, 0).0;
            let builtin = [s.root, s.library, s.journal, s.schema, s.trash].contains(&n.id);
            if builtin && n.kind != Kind::Plain {
                n.kind = Kind::Plain;
            }
            if n.id == s.day_tag && !matches!(n.kind, Kind::Tag(_)) {
                n.kind = Kind::Tag(TagDef { color: "grey".into(), ..TagDef::default() });
            }
            // (A reference to a node that is gone stays, shown as broken.)
            // A reference may own one node: its notes for this place (DIFFS N22).
            if let Kind::Reference { target } = n.kind
                && (target == n.id || n.children.len() > 1 || !n.tags.is_empty())
            {
                n.kind = Kind::Plain;
            }
            let is_tag = |t: &NodeId| kinds.get(t).is_some_and(|k| k.0);
            let is_field = |f: &NodeId| kinds.get(f).is_some_and(|k| k.1);
            n.tags.retain(is_tag);
            n.tags.dedup();
            n.fields.retain(|e| is_field(&e.field));
            if let Kind::Tag(d) = &mut n.kind {
                d.extends.retain(is_tag);
                for list in [&mut d.fields, &mut d.optional, &mut d.pinned_fields] {
                    list.retain(is_field);
                }
                d.child_tag = d.child_tag.filter(is_tag);
                d.done_map = d.done_map.take().filter(|m| is_field(&m.field));
            }
        }
        // Repair: children must exist and point back; each node listed by one parent at most.
        let ids: BTreeSet<NodeId> = nodes.keys().copied().collect();
        let mut owner: BTreeMap<NodeId, NodeId> = BTreeMap::new();
        let root = s.root;
        for n in nodes.values_mut() {
            let me = n.id;
            n.children.retain(|c| {
                let ok = ids.contains(c) && *c != me && *c != root && !owner.contains_key(c);
                if ok {
                    owner.insert(*c, me);
                }
                ok
            });
        }
        for n in nodes.values_mut() {
            n.parent = owner.get(&n.id).copied();
        }
        // A missing inbox is remade under its id on first use (after an undo removed it).
        let max = max.max(s.inbox.map_or(0, |i| i.0));
        let mut g = Graph {
            nodes,
            next_id: file.next_id.max(max + 1),
            sys: s,
            expanded: file.expanded.into_iter().collect(),
            held: BTreeMap::new(),
            settings: file.settings,
            txn: None,
            undo: Vec::new(),
            redo: Vec::new(),
            dirty: false,
            store: crate::store::Store::new(),
            changed: BTreeSet::new(),
        };
        g.rehome_unreachable();
        g.changed = g.nodes.keys().copied().collect();
        Ok(g)
    }

    /// Nodes the root cannot reach (no owner, or owners in a loop) are kept in the Library rather
    /// than lost: the top of each stray branch moves there.
    fn rehome_unreachable(&mut self) {
        let (root, library) = (self.sys.root, self.sys.library);
        let mut seen = BTreeSet::new();
        let mark = |g: &Graph, from: NodeId, seen: &mut BTreeSet<NodeId>| {
            let mut stack = vec![from];
            while let Some(x) = stack.pop() {
                if seen.insert(x) {
                    stack.extend(g.nodes.get(&x).map(|n| n.children.clone()).unwrap_or_default());
                }
            }
        };
        mark(self, root, &mut seen);
        if !seen.contains(&library) {
            // The Library itself was lost: put it back under the root first.
            self.reparent_raw(library, root);
            mark(self, library, &mut seen);
        }
        while let Some(stray) = self.nodes.keys().find(|k| !seen.contains(k)).copied() {
            // Climb to the top of the stray branch (a node with no owner, or where the loop closes).
            let mut top = stray;
            let mut path = BTreeSet::from([stray]);
            while let Some(p) = self.parent(top).filter(|p| !seen.contains(p) && path.insert(*p)) {
                top = p;
            }
            self.reparent_raw(top, library);
            mark(self, top, &mut seen);
        }
    }

    /// Moves `id` under `parent` outside the journal (file repair only).
    fn reparent_raw(&mut self, id: NodeId, parent: NodeId) {
        if let Some(old) = self.parent(id).and_then(|p| self.nodes.get_mut(&p)) {
            old.children.retain(|c| *c != id);
        }
        if let Some(p) = self.nodes.get_mut(&parent) {
            p.children.push(id);
        }
        if let Some(n) = self.nodes.get_mut(&id) {
            n.parent = Some(parent);
        }
    }

    /// Saves atomically (write a temp file, then rename). The file is the Loro document (see
    /// [`crate::store`]); [`Graph::to_json`] stays as an export.
    pub fn save(&mut self, path: &std::path::Path) -> Result<()> {
        // Saving never cuts history: no paired copy has acknowledged a version yet (see
        // `compact_history`), and a copy made by hand may come back after any time.
        let bytes = self.to_snapshot()?;
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, bytes).map_err(|e| Error::Io(e.to_string()))?;
        std::fs::rename(&tmp, path).map_err(|e| Error::Io(e.to_string()))?;
        self.dirty = false;
        Ok(())
    }

    /// Loads a graph file: a Loro document, or (once, from older versions) a JSON graph file. A JSON
    /// file is copied to `<name>.json-backup` first; the next save writes the new format.
    pub fn load(path: &std::path::Path) -> Result<Graph> {
        let bytes = std::fs::read(path).map_err(|e| Error::Io(format!("{}: {e}", path.display())))?;
        if crate::store::Store::is_ours(&bytes) {
            return Graph::from_snapshot(&bytes);
        }
        let text = String::from_utf8(bytes).map_err(|e| Error::Format(e.to_string()))?;
        let g = Graph::from_json(&text)?;
        let backup = path.with_extension("json-backup");
        if !backup.exists() {
            std::fs::write(&backup, &text).map_err(|e| Error::Io(format!("{}: {e}", backup.display())))?;
        }
        Ok(g)
    }

    // ---- the Loro document: snapshots and merging ----------------------------------------------

    /// Writes every change into the document.
    fn flush(&mut self) -> Result<()> {
        // (Inside a transaction is fine: an abort puts nodes back through `apply`, which marks them
        // changed again.)
        let meta = crate::store::Meta {
            sys: serde_json::to_value(self.sys).map_err(|e| Error::Format(e.to_string()))?,
            settings: serde_json::to_value(&self.settings).map_err(|e| Error::Format(e.to_string()))?,
            expanded: self.expanded.iter().copied().collect(),
        };
        // Empty format marks of a line still being typed are not saved (DIFFS N19).
        let changed = std::mem::take(&mut self.changed);
        let mut view: BTreeMap<NodeId, Node> = BTreeMap::new();
        let nodes: &BTreeMap<NodeId, Node> = if changed.iter().any(|c| self.nodes.get(c).is_some_and(|n| n.name.contains(['*', '_', '=', '`']))) {
            view = self.nodes.clone();
            for c in &changed {
                if let Some(n) = view.get_mut(c) {
                    n.name = crate::format::strip_empty(&n.name, 0).0;
                }
            }
            &view
        } else {
            &self.nodes
        };
        let r = self.store.flush(nodes, &changed, Some(&meta));
        drop(view);
        if r.is_err() {
            self.changed.extend(changed);
        }
        r
    }

    /// Drops history every paired copy already has: `acked` holds the version each one last
    /// confirmed. With none (no sync yet) nothing is cut.
    /// ponytail: callers pass the versions; when sync lands, keep each paired copy's acknowledged
    /// version in the document and compact on save with them.
    pub fn compact_history(&mut self, acked: &[loro::VersionVector]) -> Result<bool> {
        self.flush()?;
        self.store.compact(acked)
    }

    /// Operations in the document's history.
    pub fn history_len(&self) -> usize {
        self.store.ops()
    }

    /// The whole graph as a Loro snapshot (what [`Graph::save`] writes).
    pub fn to_snapshot(&mut self) -> Result<Vec<u8>> {
        self.flush()?;
        self.store.snapshot()
    }

    /// A graph from a Loro snapshot (or updates), repaired like a loaded file.
    pub fn from_snapshot(bytes: &[u8]) -> Result<Graph> {
        let store = crate::store::Store::from_bytes(bytes)?;
        let mut g = Graph::new();
        g.store = store;
        g.reload_from_store()?;
        Ok(g)
    }

    /// The document's version: pass it to [`Graph::updates_since`] on another copy.
    pub fn version(&mut self) -> Result<loro::VersionVector> {
        self.flush()?;
        Ok(self.store.version())
    }

    /// This copy's changes since `since` (another copy's [`Graph::version`]).
    pub fn updates_since(&mut self, since: &loro::VersionVector) -> Result<Vec<u8>> {
        self.flush()?;
        self.store.updates_since(since)
    }

    /// Merges another copy's changes (a snapshot or updates) into this graph. Undo and redo stay:
    /// each step is rebased so it only takes back this copy's own edits, never the other copy's.
    pub fn merge(&mut self, bytes: &[u8]) -> Result<()> {
        self.commit("edit");
        self.flush()?;
        let ids: BTreeSet<NodeId> = self.undo.iter().chain(self.redo.iter()).flat_map(|s| s.nodes.keys().copied()).collect();
        let before: PreImages = ids.iter().map(|id| (*id, self.nodes.get(id).cloned())).collect();
        let (undo, redo) = (std::mem::take(&mut self.undo), std::mem::take(&mut self.redo));
        self.store.import(bytes)?;
        self.reload_from_store()?;
        let mut steps = [undo, redo];
        for step in steps.iter_mut().flatten() {
            self.rebase_step(&mut step.nodes, &before);
        }
        let [undo, redo] = steps;
        self.undo = undo.into_iter().filter(|s| !s.nodes.is_empty()).collect();
        self.redo = redo.into_iter().filter(|s| !s.nodes.is_empty()).collect();
        self.dirty = true;
        Ok(())
    }

    /// Rebases one undo step's images over the other copy's changes (`before` the merge vs now).
    fn rebase_step(&self, images: &mut PreImages, before: &PreImages) {
        let ids: Vec<NodeId> = images.keys().copied().collect();
        for id in ids {
            let b = before.get(&id).cloned().flatten();
            let a = self.nodes.get(&id);
            if b.as_ref() == a {
                continue;
            }
            match (images.get_mut(&id), b, a) {
                // Both sides changed the node: the other copy's changes win where they touched.
                (Some(Some(old)), Some(b), Some(a)) => *old = rebase3(old, &b, a),
                // The other copy deleted it: the delete wins (undo does not bring it back).
                (Some(Some(_)), Some(_), None) => {
                    images.remove(&id);
                }
                // (Undo removing a node made here, which the other copy hung children under: the
                // children move up when the step is applied, see `apply`.)
                _ => {}
            }
        }
    }

    /// Rebuilds the nodes from the document, then repairs them like a loaded file.
    fn reload_from_store(&mut self) -> Result<()> {
        let block = crate::store::id_block(self.store.peer());
        let mut next = self.next_id.max(block + 1);
        let max_existing = std::cell::Cell::new(0u64);
        let (nodes, meta, _renamed) = {
            let mut fresh = || {
                let id = NodeId(next.max(max_existing.get() + 1));
                next = id.0 + 1;
                id
            };
            self.store.read(&mut fresh)?
        };
        let Some(meta) = meta else { return Err(Error::Format("not a TanaCraft document".into())) };
        let file = GraphFile {
            format: FORMAT.to_string(),
            version: 1,
            next_id: 0,
            system: serde_json::from_value(meta.sys).map_err(|e| Error::Format(e.to_string()))?,
            expanded: meta.expanded,
            settings: serde_json::from_value(meta.settings).unwrap_or_default(),
            nodes: nodes.values().cloned().collect(),
        };
        let mut g = Graph::from_file(file)?;
        // New ids go in this copy's own block (two copies edited apart never collide).
        let in_block = g.nodes.keys().map(|k| k.0).filter(|k| (block..block + (1 << 32)).contains(k)).max();
        g.next_id = in_block.map_or(block + 1, |m| m + 1).max(next);
        // Only what the repair changed needs writing back.
        let store = std::mem::take(&mut self.store);
        let fresh_nodes = std::mem::take(&mut g.nodes);
        g.changed.clear();
        *self = Graph { store, nodes: fresh_nodes, ..g };
        // What the repair changed (if anything) is written back.
        self.changed = self.nodes.iter().filter(|(id, n)| nodes.get(id) != Some(n)).map(|(id, _)| *id).collect();
        self.changed.extend(nodes.keys().filter(|id| !self.nodes.contains_key(id)));
        self.flush()?;
        self.dirty = false;
        Ok(())
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
    #[serde(default, skip_serializing_if = "is_default_settings")]
    settings: Settings,
    nodes: Vec<Node>,
}

fn is_default_settings(s: &Settings) -> bool {
    *s == Settings::default()
}

fn n_kind_week(k: &Kind) -> bool {
    matches!(k, Kind::Week { .. })
}

fn calendar_key(k: &Kind) -> Option<i64> {
    match k {
        Kind::Year { year } => Some(*year as i64),
        Kind::Week { week, .. } => Some(*week as i64),
        Kind::Day { date } => Some(date.to_days()),
        _ => None,
    }
}

/// Carries the additions `before` -> `after` (children, tags, fields, a checkbox) over to `old`, an
/// earlier image of the same node.
fn rebase(old: &mut Node, before: &Node, after: &Node) {
    for (i, c) in after.children.iter().enumerate() {
        if !before.children.contains(c) && !old.children.contains(c) {
            old.children.insert(i.min(old.children.len()), *c);
        }
    }
    for t in &after.tags {
        if !before.tags.contains(t) && !old.tags.contains(t) {
            old.tags.push(*t);
        }
    }
    for e in &after.fields {
        if !before.fields.iter().any(|b| b.field == e.field) && !old.fields.iter().any(|o| o.field == e.field) {
            old.fields.push(e.clone());
        }
    }
    if before.done.is_none() && old.done.is_none() {
        old.done = after.done;
    }
}

/// `old` with the changes `b` -> `a` (another copy's edits) laid over it: properties the other
/// copy changed take its value; children, tags and fields merge item by item.
// ponytail: a name both copies changed takes the merged text, so undo leaves that line as merged.
fn rebase3(old: &Node, b: &Node, a: &Node) -> Node {
    let to_map = |n: &Node| match serde_json::to_value(n) {
        Ok(serde_json::Value::Object(m)) => m,
        _ => serde_json::Map::new(),
    };
    let (mut o, bm, am) = (to_map(old), to_map(b), to_map(a));
    let keys: BTreeSet<String> = bm.keys().chain(am.keys()).cloned().collect();
    for k in keys {
        if bm.get(&k) != am.get(&k) {
            match am.get(&k) {
                Some(v) => o.insert(k, v.clone()),
                None => o.remove(&k),
            };
        }
    }
    let Ok(mut n) = serde_json::from_value::<Node>(serde_json::Value::Object(o)) else { return a.clone() };
    let merge_list = |old: &[NodeId], b: &[NodeId], a: &[NodeId]| -> Vec<NodeId> {
        let mut out: Vec<NodeId> = old.iter().filter(|x| !b.contains(x) || a.contains(x)).copied().collect();
        for (i, x) in a.iter().enumerate() {
            if !b.contains(x) && !out.contains(x) {
                out.insert(i.min(out.len()), *x);
            }
        }
        out
    };
    n.children = merge_list(&old.children, &b.children, &a.children);
    n.tags = merge_list(&old.tags, &b.tags, &a.tags);
    let mut fields: Vec<FieldEntry> = Vec::new();
    for e in &old.fields {
        let (be, ae) = (b.fields.iter().find(|x| x.field == e.field), a.fields.iter().find(|x| x.field == e.field));
        match (be, ae) {
            (Some(_), None) => {}
            (Some(be), Some(ae)) if be != ae => fields.push(ae.clone()),
            _ => fields.push(e.clone()),
        }
    }
    for ae in &a.fields {
        if !b.fields.iter().any(|x| x.field == ae.field) && !fields.iter().any(|x| x.field == ae.field) {
            fields.push(ae.clone());
        }
    }
    n.fields = fields;
    n
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
