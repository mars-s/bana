//! View snapshots: everything a front end needs to draw a panel or the sidebar, as JSON. The
//! front end draws exactly this and holds no data of its own.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use tanacraft_commands::Session;
use tanacraft_core::{FieldType, FieldValue, Graph, HideMode, Kind, NodeId, export, format};

/// Rows sent per page request when the front end does not say (it asks for the window it shows).
const DEFAULT_COUNT: usize = 400;
/// At most this many rows per request.
const MAX_COUNT: usize = 5_000;
const MAX_DEPTH: usize = 64;

/// A page's title as people read it (`Today, Sat, 10 Oct` for today).
pub fn page_title(s: &Session, id: NodeId) -> String {
    let g = &s.graph;
    match g.get(id).map(|n| &n.kind) {
        Some(Kind::Day { date }) if *date == s.today => format!("Today, {}", g.name(id)),
        Some(Kind::Day { date }) if *date == s.today.add_days(-1) => format!("Yesterday, {}", g.name(id)),
        Some(Kind::Day { date }) if *date == s.today.add_days(1) => format!("Tomorrow, {}", g.name(id)),
        _ if id == g.sys.schema => "Supertags".to_string(),
        _ => g.name(id).to_string(),
    }
}

pub fn title_editable(g: &Graph, zoom: NodeId) -> bool {
    !g.is_system(zoom) && !matches!(g.get(zoom).map(|n| &n.kind), Some(Kind::Day { .. } | Kind::Week { .. } | Kind::Year { .. }))
}

fn kind_name(g: &Graph, id: NodeId) -> &'static str {
    let s = &g.sys;
    if id == s.schema {
        return "schema";
    }
    if id == s.journal {
        return "journal";
    }
    if id == s.library {
        return "library";
    }
    if Some(id) == s.inbox {
        return "inbox";
    }
    if id == g.workspace_trash(g.workspace_of(id)) {
        return "trash";
    }
    match g.get(id).map(|n| &n.kind) {
        Some(Kind::Reference { .. }) => "reference",
        Some(Kind::Tag(_)) => "tag",
        Some(Kind::Field(_)) => "field",
        Some(Kind::Search(_)) => "search",
        Some(Kind::Day { .. }) => "day",
        Some(Kind::Week { .. }) => "week",
        Some(Kind::Year { .. }) => "year",
        Some(Kind::Workspace) => "workspace",
        Some(Kind::Command) => "command",
        _ => "plain",
    }
}

pub fn tag_chip(g: &Graph, tag: NodeId) -> Value {
    let color = g.tag_def(tag).map(|d| d.color.clone()).unwrap_or_default();
    json!({"id": tag.0, "name": g.name(tag), "color": color})
}

fn is_link_only(name: &str) -> bool {
    let sp = format::spans(name.trim());
    sp.len() == 1 && sp.first().is_some_and(|x| x.url.is_some())
}

/// What a bullet shows: its text, tags, checkbox and state. `id` may be a reference.
pub fn node_info(s: &Session, id: NodeId, refs: &mut BTreeMap<u64, String>) -> Value {
    let g = &s.graph;
    let target = g.resolve(id);
    let Some(n) = g.get(target) else { return json!({"id": id.0, "missing": true}) };
    for r in format::refs(&n.name) {
        refs.entry(r).or_insert_with(|| g.name(NodeId(r)).to_string());
    }
    let has_kids = !g.children(id).is_empty() || !n.fields.is_empty() || g.query(target).is_some();
    let suffix: Vec<Value> = g.title_suffix(id).into_iter().map(|(t, ph)| json!([t, ph])).collect();
    let tags: Vec<Value> = n.tags.iter().map(|t| tag_chip(g, *t)).collect();
    let notes = g.context_of(id).is_some_and(|c| !g.children(c).is_empty());
    json!({
        "id": id.0,
        "target": target.0,
        "name": n.name,
        "ref": target != id,
        "kind": kind_name(g, target),
        "tags": tags,
        "done": g.checkbox(target),
        "hasKids": has_kids,
        "expanded": g.is_expanded(id),
        "heading": n.heading,
        "description": n.description,
        "icon": n.icon,
        "link": is_link_only(&n.name),
        "suffix": suffix,
        "notes": notes,
        "progress": if n.progress { let (d, t) = g.progress(target); json!([d, t]) } else { Value::Null },
        "created": n.created,
        "edited": n.edited.max(n.created),
    })
}

fn field_type_name(t: FieldType) -> &'static str {
    t.name()
}

/// One field row: name, type and values as shown.
fn field_row(g: &Graph, owner: NodeId, e: &tanacraft_core::FieldEntry, depth: usize) -> Value {
    let def = g.field_def(e.field).cloned().unwrap_or_default();
    let values: Vec<&FieldValue> = e.values().collect();
    let shown: Vec<Value> = values
        .iter()
        .map(|v| {
            let node = if let FieldValue::Node(n) = v { Some(n.0) } else { None };
            json!({"text": export::shown_value(g, v), "raw": export::display_value(g, v), "node": node,
                   "checked": if let FieldValue::Checkbox(b) = v { Some(*b) } else { None }})
        })
        .collect();
    let empty = values.iter().all(|v| export::display_value(g, v).is_empty());
    let typed_text = matches!(def.field_type, FieldType::Number | FieldType::Date | FieldType::Checkbox);
    let invalid = values.iter().any(|v| !v.is_valid() || (typed_text && matches!(v, FieldValue::Text(_))));
    json!({
        "k": "field",
        "key": format!("f{}:{}", owner.0, e.field.0),
        "owner": owner.0,
        "field": e.field.0,
        "depth": depth,
        "name": g.name(e.field),
        "type": field_type_name(def.field_type),
        "values": shown,
        "missing": def.required && empty,
        "invalid": invalid,
    })
}

/// The display list: node rows with field rows interleaved, depth-first through expanded nodes.
/// Entries are `(kind, id, depth, extra)` and are only turned into JSON for the window sent.
#[derive(Debug, Clone)]
enum Item {
    Node(NodeId, usize),
    /// A field row: owner (resolved), field, depth.
    Field(NodeId, NodeId, usize),
    /// The "+ Field" line of folded and optional fields: owner, depth, (field, name, optional).
    More(NodeId, usize, Vec<(NodeId, bool)>),
}

fn push_fields(s: &Session, owner: NodeId, slot: Option<NodeId>, depth: usize, out: &mut Vec<Item>) {
    let g = &s.graph;
    let target = g.resolve(owner);
    let Some(n) = g.get(target) else { return };
    let zoomed = target == g.resolve(s.zoom);
    let kids = g.children(target);
    let mut folded = Vec::new();
    for e in n.fields.iter().filter(|e| e.after.filter(|a| kids.contains(a)) == slot) {
        let Some(def) = g.field_def(e.field) else { continue };
        let empty = e.values().all(|v| export::display_value(g, v).is_empty());
        let hidden = match def.hide {
            HideMode::Never => false,
            HideMode::WhenEmpty => empty && !zoomed,
            HideMode::WhenNotEmpty => !empty,
            HideMode::WhenDefault => def.default.is_some() && e.value == def.default,
            HideMode::Always => true,
        };
        if hidden {
            folded.push((e.field, false));
        } else {
            out.push(Item::Field(target, e.field, depth));
        }
    }
    if slot.is_some() {
        return;
    }
    // Optional fields of the node's supertags are offered as "+ Field" until added (spec 03 §5).
    let have: Vec<NodeId> = n.fields.iter().map(|e| e.field).collect();
    for tg in n.tags.iter().flat_map(|x| g.tag_lineage(*x)) {
        for f in g.tag_def(tg).map(|d| d.optional.clone()).unwrap_or_default() {
            if !have.contains(&f) && !folded.iter().any(|(x, _)| *x == f) && g.field_def(f).is_some() {
                folded.push((f, true));
            }
        }
    }
    if !folded.is_empty() {
        out.push(Item::More(target, depth, folded));
    }
}

fn walk(s: &Session, parent: NodeId, depth: usize, path: &mut Vec<NodeId>, out: &mut Vec<Item>) {
    if depth >= MAX_DEPTH {
        return;
    }
    let g = &s.graph;
    // Field rows of a node show first under it (on its own page: above the outline).
    if g.query(g.resolve(parent)).is_none() {
        push_fields(s, parent, None, depth, out);
    }
    let plain_children = g.query(g.resolve(parent)).is_none() && g.get(g.resolve(parent)).is_some_and(|n| n.view.is_none());
    for c in s.row_children(parent) {
        out.push(Item::Node(c, depth));
        if let Some(notes) = g.context_of(c) {
            walk_plain(s, notes, depth + 1, path, out);
        }
        let target = g.resolve(c);
        if g.is_expanded(c) && !path.contains(&target) {
            path.push(target);
            walk(s, c, depth + 1, path, out);
            path.pop();
        }
        // Field rows added among the children (`>` on a line) show after their child.
        if plain_children {
            push_fields(s, parent, Some(c), depth, out);
        }
    }
}

/// Children only (a reference's notes have no fields of their own to show).
fn walk_plain(s: &Session, parent: NodeId, depth: usize, path: &mut Vec<NodeId>, out: &mut Vec<Item>) {
    if depth >= MAX_DEPTH {
        return;
    }
    for c in s.graph.children(parent).to_vec() {
        out.push(Item::Node(c, depth));
        let target = s.graph.resolve(c);
        if s.graph.is_expanded(c) && !path.contains(&target) {
            path.push(target);
            walk(s, c, depth + 1, path, out);
            path.pop();
        }
    }
}

fn items(s: &Session) -> Vec<Item> {
    let mut out = Vec::new();
    let mut path = vec![s.graph.resolve(s.zoom)];
    walk(s, s.zoom, 0, &mut path, &mut out);
    out
}

fn item_json(s: &Session, it: &Item, refs: &mut BTreeMap<u64, String>) -> Value {
    let g = &s.graph;
    match it {
        Item::Node(id, depth) => {
            let mut v = node_info(s, *id, refs);
            v["k"] = json!("node");
            v["key"] = json!(format!("n{}", id.0));
            v["depth"] = json!(depth);
            v
        }
        Item::Field(owner, field, depth) => match g.get(*owner).and_then(|n| n.fields.iter().find(|e| e.field == *field)) {
            Some(e) => field_row(g, *owner, e, *depth),
            None => json!({"k": "gone", "key": format!("f{}:{}", owner.0, field.0), "depth": depth}),
        },
        Item::More(owner, depth, fields) => json!({
            "k": "more",
            "key": format!("m{}", owner.0),
            "owner": owner.0,
            "depth": depth,
            "fields": fields.iter().map(|(f, opt)| json!({"field": f.0, "name": g.name(*f), "optional": opt})).collect::<Vec<_>>(),
        }),
    }
}

/// Selected rows (every row between the anchor and head, inclusive).
fn selection(s: &Session) -> Vec<u64> {
    let Some((a, h)) = s.sel else { return Vec::new() };
    let rows = s.rows();
    let (Some(i), Some(j)) = (rows.iter().position(|r| r.id == a), rows.iter().position(|r| r.id == h)) else { return Vec::new() };
    rows.get(i.min(j)..=i.max(j)).unwrap_or(&[]).iter().map(|r| r.id.0).collect()
}

/// The page of the active panel. Params: `start`, `count` (the window of rows to describe).
pub fn page(s: &Session, params: &Value) -> Value {
    let g = &s.graph;
    let zoom = s.zoom;
    let target = g.resolve(zoom);
    let mut refs = BTreeMap::new();
    let all = items(s);
    let total = all.len();
    let start = tanacraft_commands::p::usize(params, "start").unwrap_or(0).min(total);
    let count = tanacraft_commands::p::usize(params, "count").unwrap_or(DEFAULT_COUNT).min(MAX_COUNT);
    let end = start.saturating_add(count).min(total);
    let window: Vec<Value> = all.get(start..end).unwrap_or(&[]).iter().map(|it| item_json(s, it, &mut refs)).collect();
    // Where the focused row is, so the front end can scroll it into view (DIFFS N32).
    let focus_index = s.focus.and_then(|f| all.iter().position(|it| matches!(it, Item::Node(id, _) if *id == f)));
    let crumbs: Vec<Value> = s
        .breadcrumbs()
        .into_iter()
        .map(|c| json!({"id": c.0, "name": if c == g.sys.schema { "Schema".to_string() } else { page_title(s, c) }}))
        .collect();
    let node = g.get(target);
    let vdef = g.view_def(zoom);
    let related: Vec<Value> = g
        .related_for(zoom)
        .into_iter()
        .map(|r| {
            let items: Vec<Value> = g.view_groups(r).into_iter().flat_map(|gr| gr.items).take(50).map(|i| node_info(s, i, &mut refs)).collect();
            json!({"id": r.0, "name": g.name(r), "items": items})
        })
        .collect();
    let is_view = g.query(target).is_some() || node.is_some_and(|n| n.view.is_some()) || g.tag_def(target).is_some();
    json!({
        "zoom": zoom.0,
        "target": target.0,
        "title": page_title(s, zoom),
        "name": g.name(zoom),
        "titleEditable": title_editable(g, zoom),
        "kind": kind_name(g, target),
        "icon": node.and_then(|n| n.icon.clone()),
        "banner": node.and_then(|n| n.banner.clone()),
        "description": node.map(|n| n.description.clone()).unwrap_or_default(),
        "tags": node.map(|n| n.tags.iter().map(|t| tag_chip(g, *t)).collect::<Vec<_>>()).unwrap_or_default(),
        "crumbs": crumbs,
        "canBack": s.can_go_back(),
        "canForward": s.can_go_forward(),
        "focus": s.focus.map(|f| f.0),
        "caret": s.caret,
        "anchor": s.anchor,
        "sel": selection(s),
        "view": {"kind": vdef.kind.name(), "isView": is_view, "toolbar": !vdef.hide_toolbar},
        "related": related,
        "filteredOut": g.view_filtered_out(zoom),
        "rows": {"total": total, "start": start, "items": window, "focusIndex": focus_index},
        "refs": refs.into_iter().map(|(k, v)| (k.to_string(), json!(v))).collect::<serde_json::Map<_, _>>(),
        "status": s.status,
        "canUndo": g.can_undo(),
        "canRedo": g.can_redo(),
    })
}

/// The sidebar: today, pinned nodes, workspaces with their entries, and settings that hide rows.
pub fn sidebar(s: &Session) -> Value {
    let g = &s.graph;
    let pin = |id: NodeId| {
        let t = g.resolve(id);
        let color = g.tag_def(t).map(|d| d.color.clone());
        json!({"id": id.0, "name": page_title(s, id), "kind": kind_name(g, t), "color": color,
               "hasKids": !g.children(t).is_empty(), "icon": g.get(t).and_then(|n| n.icon.clone())})
    };
    let pinned: Vec<Value> = g.pinned().into_iter().map(pin).collect();
    let workspaces: Vec<Value> = g
        .workspaces()
        .into_iter()
        .map(|ws| {
            let name = g.name(ws).to_string();
            let lib = g.workspace_library(ws);
            let entries: Vec<Value> = g.children(lib).iter().take(200).map(|c| pin(*c)).collect();
            json!({"id": ws.0, "name": if name.is_empty() { "Workspace".to_string() } else { name },
                   "library": lib.0, "trash": g.workspace_trash(ws).0, "main": ws == g.sys.root, "entries": entries})
        })
        .collect();
    json!({
        "today": s.today.day,
        "zoom": s.zoom.0,
        "pinned": pinned,
        "workspaces": workspaces,
        "journal": g.sys.journal.0,
        "schema": g.sys.schema.0,
        "hidden": g.settings.hidden,
        "recents": s.recents.iter().filter(|r| g.get(**r).is_some()).map(|r| json!({"id": r.0, "name": page_title(s, *r), "kind": kind_name(g, *r)})).collect::<Vec<_>>(),
    })
}

/// "Search and open": fuzzy matches (or recent pages for an empty query) with where they live.
pub fn find(s: &Session, params: &Value) -> Value {
    let g = &s.graph;
    let q = params.get("query").and_then(Value::as_str).unwrap_or_default();
    let limit = tanacraft_commands::p::usize(params, "limit").unwrap_or(12).min(100);
    let ids: Vec<NodeId> =
        if q.trim().is_empty() { s.recents.iter().copied().filter(|r| g.get(*r).is_some()).take(limit).collect() } else { g.fuzzy(q, limit) };
    let mut refs = BTreeMap::new();
    Value::Array(
        ids.into_iter()
            .map(|id| {
                let mut v = node_info(s, id, &mut refs);
                let path: Vec<String> = g.ancestors(id).into_iter().filter(|a| *a != g.sys.root && *a != id).map(|a| page_title(s, a)).collect();
                v["title"] = json!(page_title(s, id));
                v["path"] = json!(path.join(" / "));
                v
            })
            .collect(),
    )
}
