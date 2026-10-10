//! The graph's durable store: a Loro document (a CRDT), so two copies edited apart merge.
//!
//! The [`crate::Graph`] keeps working on its in-memory nodes (fast reads, the undo journal); this
//! module mirrors them into the document and back:
//!
//! - the outline is a Loro movable tree (`tree`): one tree node per graph node, children in order
//!   (fractional indexes), so concurrent moves merge without cycles;
//! - each tree node's metadata map holds the node: `id`, `name` and `description` as Loro text
//!   (concurrent typing merges by character), `tags` as a map of tag id to its position (concurrent
//!   tags both stay), `fields` as a map of field id to its entry (concurrent edits of different
//!   fields both stay), and every other property as `p:<name>` holding JSON (last writer wins);
//! - the root map `meta` holds the built-in node ids, settings and expanded nodes.
//!
//! [`Store::flush`] writes the nodes changed since the last flush, comparing each part with what the
//! document holds so unchanged parts make no operations. [`Store::read`] turns the document back
//! into nodes (after loading or merging).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use loro::{ExportMode, LoroDoc, LoroMap, LoroTree, LoroValue, TreeID, TreeParentId, ValueOrContainer, VersionVector};
use serde_json::{Map, Value, json};

use crate::model::{Node, NodeId};
use crate::{Error, Result};

pub const FORMAT: &str = "tanacraft-loro";
const VERSION: i64 = 1;

/// Keys of a serialized [`Node`] stored in their own way (everything else is `p:<key>`).
const OWN: [&str; 7] = ["id", "name", "description", "parent", "children", "tags", "fields"];

/// New ids a copy makes live in a block of its own (from its peer id), so copies edited apart
/// never hand out the same id. Blocks hold 2^32 ids and stay below 2^53 (safe in JavaScript).
pub fn id_block(peer: u64) -> u64 {
    ((peer % ((1 << 20) - 1)) + 1) << 32
}

pub struct Store {
    pub doc: LoroDoc,
    tree: LoroTree,
    ids: HashMap<NodeId, TreeID>,
}

impl Clone for Store {
    /// A clone is an independent copy (a fork), never a second handle on the same document.
    fn clone(&self) -> Store {
        let doc = self.doc.fork();
        let tree = doc.get_tree("tree");
        Store { doc, tree, ids: self.ids.clone() }
    }
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Store({} nodes)", self.ids.len())
    }
}

fn err(e: impl std::fmt::Display) -> Error {
    Error::Format(format!("store: {e}"))
}

/// What [`Store::read`] returns: the nodes, the rest of the document, and the ids given anew.
pub type Read = (BTreeMap<NodeId, Node>, Option<Meta>, BTreeSet<NodeId>);

/// What a document holds besides the nodes.
pub struct Meta {
    pub sys: Value,
    pub settings: Value,
    pub expanded: Vec<NodeId>,
}

impl Store {
    pub fn new() -> Store {
        let doc = LoroDoc::new();
        let tree = doc.get_tree("tree");
        tree.enable_fractional_index(0);
        Store { doc, tree, ids: HashMap::new() }
    }

    pub fn peer(&self) -> u64 {
        self.doc.peer_id()
    }

    /// A document from saved bytes (a snapshot or updates).
    pub fn from_bytes(bytes: &[u8]) -> Result<Store> {
        let s = Store::new();
        s.doc.import(bytes).map_err(err)?;
        Ok(s)
    }

    /// The whole document (history and state).
    pub fn snapshot(&self) -> Result<Vec<u8>> {
        self.doc.export(ExportMode::Snapshot).map_err(err)
    }

    pub fn version(&self) -> VersionVector {
        self.doc.oplog_vv()
    }

    /// Changes made since `since` (what another copy needs to catch up).
    pub fn updates_since(&self, since: &VersionVector) -> Result<Vec<u8>> {
        self.doc.export(ExportMode::updates(since)).map_err(err)
    }

    /// Merges another copy's changes in.
    pub fn import(&self, bytes: &[u8]) -> Result<()> {
        self.doc.import(bytes).map(drop).map_err(err)
    }

    /// Merges several blobs in at once (a snapshot first, then updates in any order).
    pub fn import_all(&self, parts: &[Vec<u8>]) -> Result<()> {
        match parts {
            [] => Ok(()),
            [one] => self.import(one),
            // The first one alone: a snapshot (a shallow one too) must land in the empty
            // document before the updates that follow it.
            [first, rest @ ..] => {
                self.import(first)?;
                match rest {
                    [one] => self.import(one),
                    many => self.doc.import_batch(many).map(drop).map_err(err),
                }
            }
        }
    }

    // ---- history: compaction -------------------------------------------------------------------

    /// Drops history up to the newest version every paired copy has acknowledged (`acked`: each
    /// copy's version as last confirmed), so a copy can always merge everything it has not seen.
    /// No copies known means nothing is cut: a copy made by hand (a file on a phone, a backup) can
    /// be away for any time and still merge. Returns whether it compacted.
    pub fn compact(&mut self, acked: &[VersionVector]) -> Result<bool> {
        let Some(first) = acked.first() else { return Ok(false) };
        // What all of them (and this copy) have: the smallest counter of every peer.
        let mut common = self.doc.oplog_vv();
        for vv in std::iter::once(first).chain(&acked[1..]) {
            let peers: Vec<_> = common.keys().copied().collect();
            for peer in peers {
                let theirs = vv.get(&peer).copied().unwrap_or(0);
                if let Some(c) = common.get_mut(&peer) {
                    *c = (*c).min(theirs);
                }
            }
        }
        common.retain(|_, c| *c > 0);
        // Already cut there (or later): the version of the history's first kept change.
        let root = self.doc.frontiers_to_vv(&self.doc.shallow_since_frontiers()).unwrap_or_default();
        if common.is_empty() || (self.doc.is_shallow() && root.includes_vv(&common)) {
            return Ok(false);
        }
        let at = self.doc.vv_to_frontiers(&common);
        let bytes = self.doc.export(ExportMode::shallow_snapshot(&at)).map_err(err)?;
        let doc = LoroDoc::new();
        doc.import(&bytes).map_err(err)?;
        self.tree = doc.get_tree("tree");
        self.doc = doc;
        Ok(true)
    }

    /// Operations held (the history's size).
    pub fn ops(&self) -> usize {
        self.doc.len_ops()
    }

    // ---- writing ------------------------------------------------------------------------------

    fn meta_of(&self, t: TreeID) -> Result<LoroMap> {
        self.tree.get_meta(t).map_err(err)
    }

    fn alive(&self, t: TreeID) -> bool {
        self.tree.contains(t) && !self.tree.is_node_deleted(&t).unwrap_or(true)
    }

    /// Writes `changed` nodes (and gone ones) from `nodes` into the document.
    pub fn flush(&mut self, nodes: &BTreeMap<NodeId, Node>, changed: &BTreeSet<NodeId>, meta: Option<&Meta>) -> Result<()> {
        // Every changed node exists in the tree, under its parent.
        for id in changed.iter().filter(|id| nodes.contains_key(id)) {
            self.ensure(*id, nodes, 0)?;
        }
        // Moves go shallowest first: each node's new parent then already sits where the graph says,
        // so no move can make a loop.
        let depth = |id: &NodeId| {
            let mut d = 0usize;
            let mut at = nodes.get(id).and_then(|n| n.parent);
            while let Some(p) = at.filter(|_| d < 10_000) {
                d += 1;
                at = nodes.get(&p).and_then(|n| n.parent);
            }
            d
        };
        let mut order: Vec<NodeId> = changed.iter().copied().filter(|id| nodes.contains_key(id)).collect();
        order.sort_by_key(|id| depth(id));
        let mut parents: BTreeSet<NodeId> = BTreeSet::new();
        for id in &order {
            let Some(n) = nodes.get(id) else { continue };
            let Some(t) = self.ids.get(id).copied() else { continue };
            let want = match n.parent {
                Some(p) => self.ids.get(&p).copied().map(TreeParentId::Node),
                None => Some(TreeParentId::Root),
            };
            if let Some(want) = want
                && self.tree.parent(t) != Some(want)
            {
                match want {
                    TreeParentId::Node(p) => self.tree.mov(t, p).map_err(err)?,
                    _ => self.tree.mov(t, TreeParentId::Root).map_err(err)?,
                }
            }
            if let Some(p) = n.parent {
                parents.insert(p);
            }
            parents.insert(*id);
        }
        for p in parents {
            self.order_children(p, nodes)?;
        }
        for id in changed {
            if let (Some(n), Some(t)) = (nodes.get(id), self.ids.get(id).copied()) {
                self.write_node(t, n)?;
            }
        }
        // Gone nodes leave the tree last, after anything still alive has moved out from under them
        // (a tree delete takes the subtree with it).
        for id in changed.iter().filter(|id| !nodes.contains_key(id)) {
            if let Some(t) = self.ids.remove(id)
                && self.alive(t)
            {
                self.tree.delete(t).map_err(err)?;
            }
        }
        if let Some(m) = meta {
            let root = self.doc.get_map("meta");
            set_if(&root, "format", LoroValue::from(FORMAT))?;
            set_if(&root, "version", LoroValue::from(VERSION))?;
            set_if(&root, "sys", LoroValue::from(m.sys.to_string()))?;
            set_if(&root, "settings", LoroValue::from(m.settings.to_string()))?;
            set_if(&root, "expanded", LoroValue::from(json!(m.expanded).to_string()))?;
        }
        self.doc.commit();
        Ok(())
    }

    /// The tree node of `id`, made (with its ancestors) when missing.
    fn ensure(&mut self, id: NodeId, nodes: &BTreeMap<NodeId, Node>, depth: usize) -> Result<Option<TreeID>> {
        if let Some(t) = self.ids.get(&id).copied().filter(|t| self.alive(*t)) {
            return Ok(Some(t));
        }
        if depth > 10_000 {
            return Err(Error::Format("store: ancestor chain too deep".into()));
        }
        let Some(n) = nodes.get(&id) else { return Ok(None) };
        let parent = match n.parent {
            Some(p) => match self.ensure(p, nodes, depth + 1)? {
                Some(pt) => TreeParentId::Node(pt),
                None => TreeParentId::Root,
            },
            None => TreeParentId::Root,
        };
        let t = self.tree.create(parent).map_err(err)?;
        self.meta_of(t)?.insert("id", id.0 as i64).map_err(err)?;
        self.ids.insert(id, t);
        Ok(Some(t))
    }

    /// Puts the tree children of `p` in the order of the node's children.
    fn order_children(&mut self, p: NodeId, nodes: &BTreeMap<NodeId, Node>) -> Result<()> {
        let (Some(n), Some(pt)) = (nodes.get(&p), self.ids.get(&p).copied()) else { return Ok(()) };
        let want: Vec<TreeID> = n.children.iter().filter_map(|c| self.ids.get(c).copied()).collect();
        let mut have = self.tree.children(pt).unwrap_or_default();
        if have == want {
            return Ok(());
        }
        for (i, t) in want.iter().enumerate() {
            if have.get(i) == Some(t) {
                continue;
            }
            match i.checked_sub(1).and_then(|j| want.get(j)) {
                Some(prev) => self.tree.mov_after(*t, *prev).map_err(err)?,
                None => match have.first() {
                    Some(first) if first != t => self.tree.mov_before(*t, *first).map_err(err)?,
                    _ => {}
                },
            }
            have = self.tree.children(pt).unwrap_or_default();
        }
        Ok(())
    }

    fn write_node(&self, t: TreeID, n: &Node) -> Result<()> {
        let m = self.meta_of(t)?;
        set_if(&m, "id", LoroValue::from(n.id.0 as i64))?;
        set_text(&m, "name", &n.name)?;
        // Containers cost load time, so empty parts get none (until they first hold something).
        if !n.description.is_empty() || m.get("description").is_some() {
            set_text(&m, "description", &n.description)?;
        }
        if n.tags.is_empty() && n.fields.is_empty() && m.get("tags").is_none() && m.get("fields").is_none() {
            return self.write_props(&m, n);
        }
        // Tags: tag id -> position.
        let tags = m.ensure_mergeable_map("tags").map_err(err)?;
        let have: Vec<String> = tags.keys().map(|k| k.to_string()).collect();
        for k in &have {
            if !n.tags.iter().any(|t| t.0.to_string() == *k) {
                tags.delete(k).map_err(err)?;
            }
        }
        for (i, tag) in n.tags.iter().enumerate() {
            set_if(&tags, &tag.0.to_string(), LoroValue::from(i as i64))?;
        }
        // Fields: field id -> {"at": position, "entry": the entry as JSON}.
        let fields = m.ensure_mergeable_map("fields").map_err(err)?;
        let have: Vec<String> = fields.keys().map(|k| k.to_string()).collect();
        for k in &have {
            if !n.fields.iter().any(|f| f.field.0.to_string() == *k) {
                fields.delete(k).map_err(err)?;
            }
        }
        for (i, e) in n.fields.iter().enumerate() {
            let v = json!({"at": i, "entry": e}).to_string();
            set_if(&fields, &e.field.0.to_string(), LoroValue::from(v))?;
        }
        self.write_props(&m, n)
    }

    /// Every other property, one key each.
    fn write_props(&self, m: &LoroMap, n: &Node) -> Result<()> {
        let obj = match serde_json::to_value(n).map_err(err)? {
            Value::Object(o) => o,
            _ => Map::new(),
        };
        let keys: Vec<String> = m.keys().map(|k| k.to_string()).filter(|k| k.starts_with("p:")).collect();
        for k in keys {
            if !obj.contains_key(&k[2..]) {
                m.delete(&k).map_err(err)?;
            }
        }
        for (k, v) in obj.iter().filter(|(k, _)| !OWN.contains(&k.as_str())) {
            set_if(m, &format!("p:{k}"), LoroValue::from(v.to_string()))?;
        }
        Ok(())
    }

    // ---- reading ------------------------------------------------------------------------------

    /// Every node in the document, with parents and children from the tree. Ids claimed twice
    /// (two copies that made the same id) keep the first; the other is given `fresh` ids.
    pub fn read(&mut self, fresh: &mut dyn FnMut() -> NodeId) -> Result<Read> {
        use loro::ToJson;
        self.ids.clear();
        let mut renamed = BTreeSet::new();
        let mut nodes = BTreeMap::new();
        // One deep read of the whole tree (much faster than asking node by node).
        let forest = self.tree.get_value_with_meta().to_json_value();
        let mut stack: Vec<(&Value, Option<NodeId>)> = forest.as_array().map(|a| a.iter().map(|v| (v, None)).collect()).unwrap_or_default();
        stack.reverse();
        // Children lists are filled as each child is read (in tree order).
        let mut kids: BTreeMap<NodeId, Vec<NodeId>> = BTreeMap::new();
        while let Some((v, parent)) = stack.pop() {
            let Some(tid) = v["id"].as_str().and_then(|s| TreeID::try_from(s).ok()) else { continue };
            let meta = &v["meta"];
            let Some(mut id) = meta["id"].as_i64().map(|i| NodeId(i.max(0) as u64)) else { continue };
            if self.ids.contains_key(&id) {
                id = fresh();
                self.meta_of(tid)?.insert("id", id.0 as i64).map_err(err)?;
                renamed.insert(id);
            }
            self.ids.insert(id, tid);
            let mut obj = Map::new();
            if let Some(m) = meta.as_object() {
                for (k, val) in m {
                    if let Some(p) = k.strip_prefix("p:")
                        && let Some(s) = val.as_str()
                        && let Ok(x) = serde_json::from_str::<Value>(s)
                    {
                        obj.insert(p.to_string(), x);
                    }
                }
            }
            obj.insert("id".into(), json!(id.0));
            obj.insert("name".into(), json!(meta["name"].as_str().unwrap_or_default()));
            obj.insert("description".into(), json!(meta["description"].as_str().unwrap_or_default()));
            let mut tags: Vec<(i64, u64)> = meta["tags"]
                .as_object()
                .map(|m| m.iter().filter_map(|(k, at)| Some((at.as_i64().unwrap_or(0), k.parse().ok()?))).collect())
                .unwrap_or_default();
            tags.sort();
            obj.insert("tags".into(), json!(tags.into_iter().map(|(_, t)| t).collect::<Vec<_>>()));
            let mut fields: Vec<(u64, Value)> = meta["fields"]
                .as_object()
                .map(|m| {
                    m.values()
                        .filter_map(|x| x.as_str().and_then(|s| serde_json::from_str::<Value>(s).ok()))
                        .map(|x| (x["at"].as_u64().unwrap_or(0), x["entry"].clone()))
                        .collect()
                })
                .unwrap_or_default();
            fields.sort_by_key(|(at, _)| *at);
            obj.insert("fields".into(), Value::Array(fields.into_iter().map(|(_, e)| e).collect()));
            let mut node: Node = match serde_json::from_value(Value::Object(obj)) {
                Ok(n) => n,
                Err(_) => Node::new(id, meta["name"].as_str().unwrap_or_default(), None),
            };
            node.parent = parent;
            node.children.clear();
            if let Some(p) = parent {
                kids.entry(p).or_default().push(id);
            }
            nodes.insert(id, node);
            if let Some(cs) = v["children"].as_array() {
                for c in cs.iter().rev() {
                    stack.push((c, Some(id)));
                }
            }
        }
        for (p, cs) in kids {
            if let Some(n) = nodes.get_mut(&p) {
                n.children = cs;
            }
        }
        let root = self.doc.get_map("meta");
        let meta = get_str(&root, "sys").and_then(|s| serde_json::from_str::<Value>(&s).ok()).map(|sys| Meta {
            sys,
            settings: get_str(&root, "settings").and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null),
            expanded: get_str(&root, "expanded").and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default(),
        });
        Ok((nodes, meta, renamed))
    }

    /// Whether these bytes look like a document of ours (not JSON).
    pub fn is_ours(bytes: &[u8]) -> bool {
        bytes.iter().find(|b| !b.is_ascii_whitespace()) != Some(&b'{')
    }
}

impl Default for Store {
    fn default() -> Self {
        Store::new()
    }
}

fn set_if(m: &LoroMap, key: &str, v: LoroValue) -> Result<()> {
    let same = matches!(m.get(key), Some(ValueOrContainer::Value(old)) if old == v);
    if !same {
        m.insert(key, v).map_err(err)?;
    }
    Ok(())
}

fn set_text(m: &LoroMap, key: &str, s: &str) -> Result<()> {
    let t = m.ensure_mergeable_text(key).map_err(err)?;
    if t.to_string() != s {
        t.update(s, Default::default()).map_err(|e| Error::Format(format!("store: text update: {e:?}")))?;
    }
    Ok(())
}

fn get_str(m: &LoroMap, key: &str) -> Option<String> {
    match m.get(key) {
        Some(ValueOrContainer::Value(LoroValue::String(s))) => Some(s.to_string()),
        _ => None,
    }
}
