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
    // A reference whose node is gone still draws as a full row (a damaged file, or the other
    // copy deleted it): no text, no tags, marked missing.
    let Some(n) = g.get(target) else {
        return json!({"id": id.0, "target": target.0, "parent": g.parent(id).map(|p| p.0), "index": g.index_in_parent(id),
                      "pinned": false, "inTrash": false, "name": "", "ref": true, "kind": "missing", "tags": [], "done": null,
                      "hasKids": false, "expanded": false, "heading": false, "description": "", "icon": null, "link": false,
                      "suffix": [], "notes": false, "notesHead": null, "progress": null, "created": 0, "edited": 0, "missing": true});
    };
    for r in format::refs(&n.name) {
        refs.entry(r).or_insert_with(|| g.name(NodeId(r)).to_string());
    }
    let has_kids = !g.children(id).is_empty() || !n.fields.is_empty() || g.query(target).is_some();
    let suffix: Vec<Value> = g.title_suffix(id).into_iter().map(|(t, ph)| json!([t, ph])).collect();
    let tags: Vec<Value> = n.tags.iter().map(|t| tag_chip(g, *t)).collect();
    let notes = g.context_of(id).is_some_and(|c| !g.children(c).is_empty());
    // The first line of a reference's notes for this place carries their heading (DIFFS N22, N30).
    let notes_head = g.parent(id).filter(|h| g.is_context(*h) && g.children(*h).first() == Some(&id)).map(|holder| {
        let reference = g.parent(holder).unwrap_or(holder);
        let place = g.parent(reference).map(|p| page_title(s, p)).unwrap_or_default();
        format!("Notes and fields about {}, stored in {place}", g.name(reference))
    });
    json!({
        "id": id.0,
        "target": target.0,
        "parent": g.parent(id).map(|p| p.0),
        "index": g.index_in_parent(id),
        "pinned": n.pinned,
        "inTrash": g.in_trash(target),
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
        "notesHead": notes_head,
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
    // Fields that come from the owner's supertags sit in a soft box; free-standing ones do not.
    let boxed = g.get(owner).is_some_and(|n| n.tags.iter().any(|t| g.template_fields(*t).contains(&e.field)));
    json!({
        "boxed": boxed,
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
    /// The empty child line of an expanded node with no children (spec 02 §7): parent, depth.
    Empty(NodeId, usize),
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
        // A reference's own fields and notes are about this place only: always shown under it.
        if g.resolve(c) != c {
            for e in g.get(c).map(|r| r.fields.as_slice()).unwrap_or_default() {
                out.push(Item::Field(c, e.field, depth + 1));
            }
        }
        if let Some(notes) = g.context_of(c) {
            walk_plain(s, notes, depth + 1, path, out);
        }
        let target = g.resolve(c);
        if g.is_expanded(c) && !path.contains(&target) {
            path.push(target);
            let before = out.len();
            walk(s, c, depth + 1, path, out);
            path.pop();
            if out.len() == before && g.query(target).is_none() {
                out.push(Item::Empty(c, depth + 1));
            }
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
        Item::Empty(parent, depth) => json!({"k": "empty", "key": format!("e{}", parent.0), "parent": parent.0, "depth": depth}),
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
    let mut window: Vec<Value> = all.get(start..end).unwrap_or(&[]).iter().map(|it| item_json(s, it, &mut refs)).collect();
    // The reference counter (a setting): how many places point at each line's node.
    if g.settings.ref_counter {
        let counts = g.reference_counts();
        for v in &mut window {
            let n = v["target"].as_u64().and_then(|t| counts.get(&NodeId(t)).copied()).unwrap_or(0);
            if n > 0 {
                v["uses"] = json!(n);
            }
        }
    }
    // Where the focused row is, so the front end can scroll it into view (DIFFS N32).
    let focus_index = s.focus.and_then(|f| all.iter().position(|it| matches!(it, Item::Node(id, _) if *id == f)));
    let mut crumbs: Vec<Value> = s
        .breadcrumbs()
        .into_iter()
        .map(|c| json!({"id": c.0, "name": if c == g.sys.schema { "Schema".to_string() } else { page_title(s, c) }}))
        .collect();
    // A field definition reads Schema / #<first supertag using it> / <field> (spec 12).
    if g.field_def(target).is_some() {
        if let Some(t) = g.tags().into_iter().find(|t| g.tag_def(*t).is_some_and(|d| d.fields.contains(&target))) {
            crumbs.push(json!({"id": t.0, "name": format!("#{}", g.name(t))}));
        }
        crumbs.push(json!({"id": target.0, "name": g.name(target)}));
    }
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
    // Nodes that point here: a reference shows where it sits, a field reference its holder.
    let references: Vec<Value> = g
        .references_to(target)
        .into_iter()
        .take(200)
        .map(|r| {
            let open = if g.get(r).and_then(|n| n.reference_target()).is_some() { g.parent(r).unwrap_or(r) } else { r };
            let path: Vec<String> = g.ancestors(open).into_iter().filter(|a| *a != g.sys.root && *a != open).map(|a| page_title(s, a)).collect();
            json!({"id": open.0, "name": page_title(s, open), "path": path.join(" / ")})
        })
        .collect();
    let is_view = g.query(target).is_some() || node.is_some_and(|n| n.view.is_some()) || g.tag_def(target).is_some();
    json!({
        "zoom": zoom.0,
        "target": target.0,
        "title": page_title(s, zoom),
        "name": g.name(zoom),
        "titleEditable": title_editable(g, zoom),
        "done": g.checkbox(target),
        "kind": kind_name(g, target),
        "tagColor": g.tag_def(target).map(|d| d.color.clone()),
        "fieldType": g.field_def(target).map(|d| d.field_type.name()),
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
        "references": references,
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
        "todayDate": s.today.to_string(),
        "zoom": s.zoom.0,
        "pinned": pinned,
        "workspaces": workspaces,
        "journal": g.sys.journal.0,
        "schema": g.sys.schema.0,
        "hidden": g.settings.hidden,
        "keybindings": g.settings.keybindings,
        "warnReferenced": g.settings.warn_referenced,
        "urlEmbeds": g.settings.url_embeds,
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

/// What a picker offers (`#`, `>`, `@`, `/` and the field value menus). Params: `mode` (`tag`,
/// `field`, `reference`, `inlineRef`, `slash`, `choice`, `moveTo`, `searchTag`, `column`), `node`,
/// `query`, and `field` for `choice`. Each item is `{id, label}` or `{create: name}`.
pub fn choices(s: &Session, params: &Value) -> Value {
    let g = &s.graph;
    let mode = params.get("mode").and_then(Value::as_str).unwrap_or_default();
    let node = tanacraft_commands::p::node(params, "node").ok().flatten().filter(|n| g.get(*n).is_some()).unwrap_or(s.zoom);
    let raw = params.get("query").and_then(Value::as_str).unwrap_or_default();
    let q = raw.trim().trim_start_matches('#').to_lowercase();
    let item = |id: NodeId, label: String| json!({"id": id.0, "label": label});
    let line = |id: NodeId| export::node_line(g, id);
    let mut out: Vec<Value> = match mode {
        "tag" | "searchTag" | "sourceTag" => g
            .menu_tags(node)
            .into_iter()
            .filter(|t| g.name(*t).to_lowercase().contains(&q))
            .map(|t| {
                let mut v = item(t, g.name(t).to_string());
                v["color"] = json!(g.tag_def(t).map(|d| d.color.clone()).unwrap_or_default());
                v
            })
            .collect(),
        "field" | "column" | "templateField" => g
            .iter()
            .filter(|n| g.field_def(n.id).is_some() && !g.in_trash(n.id) && n.name.to_lowercase().contains(&q))
            .map(|n| {
                let mut v = item(n.id, n.name.clone());
                v["type"] = json!(g.field_def(n.id).map(|d| d.field_type.name()).unwrap_or("plain"));
                v
            })
            .collect(),
        "choice" => {
            let field = tanacraft_commands::p::node(params, "field").ok().flatten();
            let current: Vec<String> = field
                .and_then(|f| Some(g.get(g.resolve(node))?.field_values(f).into_iter().map(|v| export::display_value(g, v)).collect()))
                .unwrap_or_default();
            let def = field.and_then(|f| g.field_def(f));
            match (def.map(|d| d.field_type), def.and_then(|d| d.source_tag), field) {
                (Some(FieldType::User), _, _) => g
                    .user_names()
                    .into_iter()
                    .filter(|n| !current.contains(n) && n.to_lowercase().contains(&q))
                    .map(|n| json!({"id": 0, "label": n}))
                    .collect(),
                (Some(FieldType::OptionsFromTag), Some(tag), _) => g
                    .tagged(tag)
                    .into_iter()
                    .filter(|n| !g.in_trash(*n) && !current.contains(&g.text(*n)) && g.text(*n).to_lowercase().contains(&q))
                    .map(|n| item(n, g.text(n)))
                    .collect(),
                (_, _, Some(f)) => g
                    .field_options(f)
                    .into_iter()
                    .filter(|o| !current.contains(o) && o.to_lowercase().contains(&q))
                    .map(|o| json!({"id": 0, "label": o}))
                    .collect(),
                _ => Vec::new(),
            }
        }
        "slash" => ["Search node", "Field", "Reference", "Heading", "Checkbox", "Checklist", "Command line"]
            .into_iter()
            .filter(|l| l.to_lowercase().contains(&q))
            .map(|l| json!({"id": 0, "label": l}))
            .collect(),
        "inlineRef" | "moveTo" if q.is_empty() => {
            s.recents.iter().copied().filter(|r| *r != node && g.get(*r).is_some()).take(8).map(|n| item(n, line(n))).collect()
        }
        "inlineRef" => g.fuzzy(&q, 10).into_iter().filter(|n| *n != node).map(|n| item(n, line(n))).collect(),
        "moveTo" => {
            let mut v: Vec<Value> = [g.sys.library, g.sys.journal]
                .into_iter()
                .filter(|n| g.name(*n).to_lowercase().contains(&q))
                .map(|n| item(n, g.name(n).to_string()))
                .collect();
            v.extend(g.fuzzy(&q, 10).into_iter().filter(|n| !g.is_ancestor_or_self(node, *n)).map(|n| item(n, line(n))));
            v
        }
        "reference" if q.is_empty() => Vec::new(),
        "reference" => g.search(&q).into_iter().filter(|n| *n != node).take(10).map(|n| item(n, line(n))).collect(),
        _ => Vec::new(),
    };
    out.truncate(10);
    let exact = out.iter().any(|c| c["label"].as_str().is_some_and(|l| l.trim_start_matches('#').eq_ignore_ascii_case(&q)));
    if !q.is_empty() && !exact && !matches!(mode, "slash" | "moveTo" | "searchTag" | "sourceTag") {
        out.push(json!({"create": raw.trim().trim_start_matches('#')}));
    }
    // A new supertag gets the next colour of the palette, so tags are told apart at a glance.
    let next = tanacraft_core::TAG_COLORS.get(2 + g.tags().len() % 12).copied().unwrap_or("grey");
    json!({"items": out, "nextColor": next})
}

/// Whether `id` shows its items through a view (search results, tagged nodes, or a set view).
pub fn is_view(g: &Graph, id: NodeId) -> bool {
    let t = g.resolve(id);
    g.query(t).is_some() || g.tag_def(t).is_some() || g.get(t).is_some_and(|n| n.view.is_some())
}

/// A search node's query as a tree the query builder draws: each group is AND or OR, each clause
/// reads as text, a NOT group is a clause with `not` set.
fn query_tree(g: &Graph, q: &tanacraft_core::Query, depth: usize) -> Value {
    use tanacraft_core::query::{Clause, ClauseKind};
    let clauses: Vec<Value> = q
        .clauses
        .iter()
        .take(200)
        .map(|c| match &c.kind {
            ClauseKind::Group { query } if depth < 8 => json!({"not": c.not, "group": query_tree(g, query, depth + 1)}),
            _ => {
                let one = tanacraft_core::Query { any: false, clauses: vec![Clause { not: false, kind: c.kind.clone() }] };
                json!({"not": c.not, "text": one.describe(g)})
            }
        })
        .collect();
    json!({"any": q.any, "clauses": clauses})
}

/// Everything a view (table, cards, board, list, calendar...) needs: its items in groups, with the
/// values of each column, and its settings. Params: `node` (default: the page), `week` (a date in
/// the week a calendar shows).
pub fn view(s: &Session, params: &Value) -> Value {
    let g = &s.graph;
    let node = tanacraft_commands::p::node(params, "node").ok().flatten().filter(|n| g.get(*n).is_some()).unwrap_or(s.zoom);
    let def = g.view_def(node);
    let cols = g.view_columns(node);
    let mut refs = BTreeMap::new();
    let date_field = g.calendar_field(node);
    let groups: Vec<Value> = g
        .view_groups(node)
        .into_iter()
        .map(|gr| {
            let items: Vec<Value> = gr
                .items
                .iter()
                .take(2_000)
                .map(|i| {
                    let mut v = node_info(s, *i, &mut refs);
                    let target = g.resolve(*i);
                    let n = g.get(target);
                    let cells: serde_json::Map<String, Value> = cols
                        .iter()
                        .map(|c| {
                            let vals: Vec<String> =
                                n.map(|n| n.field_values(*c).into_iter().map(|x| export::shown_value(g, x)).collect()).unwrap_or_default();
                            (c.0.to_string(), json!(vals))
                        })
                        .collect();
                    v["cells"] = Value::Object(cells);
                    v["line"] = json!(g.display_line(node, *i));
                    v["dates"] = json!(g.item_dates(*i, date_field).iter().take(62).map(|d| d.to_string()).collect::<Vec<_>>());
                    v["time"] = match g.item_time(*i, date_field) {
                        Some((a, b)) => json!([a, b]),
                        None => Value::Null,
                    };
                    v
                })
                .collect();
            json!({"label": gr.label, "count": gr.items.len(), "items": items})
        })
        .collect();
    let all: Vec<NodeId> = g.view_groups(node).into_iter().flat_map(|gr| gr.items).collect();
    let columns: Vec<Value> = cols
        .iter()
        .map(|c| {
            let calc = def.calcs.iter().find(|(f, _)| f == c).map(|(_, k)| k.clone());
            let calc_value = calc.as_deref().and_then(|k| g.column_calc(&all, *c, k));
            json!({"id": c.0, "name": g.name(*c), "type": g.field_def(*c).map(|d| d.field_type.name()).unwrap_or("plain"),
                   "width": g.column_width(node, *c), "calc": calc, "calcValue": calc_value})
        })
        .collect();
    let today = s.today;
    let (y, m) = s.calendar_month.unwrap_or((today.year, today.month));
    let mode = ["month", "week", "day"].get(s.calendar_mode as usize).copied().unwrap_or("month");
    let tabs: Vec<Value> = match g.tag_def(g.resolve(node)) {
        Some(d) => d.views.iter().filter(|v| g.get(**v).is_some()).map(|v| json!({"id": v.0, "name": g.name(*v)})).collect(),
        None => Vec::new(),
    };
    json!({
        "node": node.0,
        "kind": def.kind.name(),
        "toolbar": !def.hide_toolbar,
        "groups": groups,
        "grouped": def.group.is_some(),
        "columns": columns,
        "titleWidth": g.column_width(node, NodeId(0)),
        "sorted": !def.sort.is_empty(),
        "filtered": !def.filter.is_empty(),
        // The rules behind the toolbar (N48): sort rules in order, filter conditions as chips.
        "sorts": def.sort.iter().map(|r| {
            let (by, label) = match r.by {
                tanacraft_core::SortBy::Name => ("name".to_string(), "Name".to_string()),
                tanacraft_core::SortBy::Created => ("created".into(), "Created time".into()),
                tanacraft_core::SortBy::Edited => ("edited".into(), "Last edited time".into()),
                tanacraft_core::SortBy::Done => ("done".into(), "Done".into()),
                tanacraft_core::SortBy::DoneTime => ("done_time".into(), "Done time".into()),
                tanacraft_core::SortBy::Field(f) => (f.0.to_string(), g.name(f).to_string()),
            };
            json!({"by": by, "label": label, "desc": r.desc})
        }).collect::<Vec<_>>(),
        "filters": def.filter.clauses.iter().map(|c| tanacraft_core::query::Query { any: false, clauses: vec![c.clone()] }.describe(g)).collect::<Vec<_>>(),
        "system": def.system.iter().map(|f| f.label()).collect::<Vec<_>>(),
        "filteredOut": g.view_filtered_out(node),
        "hiddenGroups": def.hidden_groups,
        "dateField": date_field.map(|f| json!({"id": f.0, "name": g.name(f)})),
        "calendar": {"year": y, "month": m, "mode": mode,
                      "today": today.to_string(), "weekStart": g.week_start(today).to_string(), "weekdays": g.weekday_names()},
        "selected": s.nav_selected.get(&node).map(|n| n.0),
        "tabs": tabs,
        "name": g.name(node),
        "description": g.get(node).map(|n| n.description.clone()).unwrap_or_default(),
        "query": g.query(g.resolve(node)).map(|q| json!({"tree": query_tree(g, q, 0), "text": q.describe(g), "sentence": q.sentence(g), "held": g.held_results(g.resolve(node)).is_some()})),
        "refs": refs.into_iter().map(|(k, v)| (k.to_string(), json!(v))).collect::<serde_json::Map<_, _>>(),
    })
}

/// The Supertags page: every supertag with its colour, whether it is pinned, and its workspace.
pub fn tags(s: &Session) -> Value {
    let g = &s.graph;
    let tags: Vec<Value> = g
        .tags()
        .into_iter()
        .map(|t| {
            json!({"id": t.0, "name": g.name(t), "color": g.tag_def(t).map(|d| d.color.clone()).unwrap_or_default(),
                   "pinned": g.get(t).is_some_and(|n| n.pinned), "workspace": g.workspace_of(t).0, "count": g.tagged(t).len()})
        })
        .collect();
    let workspaces: Vec<Value> =
        g.workspaces().into_iter().map(|w| json!({"id": w.0, "name": if g.name(w).is_empty() { "Workspace" } else { g.name(w) }})).collect();
    json!({"tags": tags, "workspaces": workspaces})
}

/// How many places outside `nodes` (default: the selection, else the focused line) point at them:
/// deleting such nodes asks first (DIFFS N20). Also returns the first node's name.
pub fn used_elsewhere(s: &Session, params: &Value) -> Value {
    let g = &s.graph;
    let ids: Vec<NodeId> = match params.get("nodes").and_then(Value::as_array) {
        Some(a) => a.iter().filter_map(|v| v.as_u64()).map(NodeId).filter(|n| g.get(*n).is_some()).collect(),
        None => {
            let sel = s.selected();
            if sel.is_empty() { s.focus.into_iter().collect() } else { sel }
        }
    };
    let inside = |r: NodeId| ids.iter().any(|d| g.is_ancestor_or_self(*d, r));
    let used: std::collections::BTreeSet<NodeId> =
        ids.iter().filter(|i| g.resolve(**i) == **i).flat_map(|i| g.references_to(*i)).filter(|r| !inside(*r)).collect();
    json!({"count": used.len(), "name": ids.first().map(|i| g.name(*i)).unwrap_or_default(), "nodes": ids.iter().map(|i| i.0).collect::<Vec<_>>()})
}

/// The definition behind a supertag or field page: what the configure panel and the field page
/// show and change. Params: `node` (default: the page).
pub fn schema(s: &Session, params: &Value) -> Value {
    let g = &s.graph;
    let node = g.resolve(tanacraft_commands::p::node(params, "node").ok().flatten().filter(|n| g.get(*n).is_some()).unwrap_or(s.zoom));
    let named = |id: NodeId| json!({"id": id.0, "name": g.name(id)});
    if let Some(d) = g.field_def(node) {
        let (nodes, tags) = g.field_usage(node);
        return json!({
            "kind": "field",
            "id": node.0,
            "type": d.field_type.name(),
            "options": g.field_options(node),
            "ownOptions": d.options,
            "sourceTag": d.source_tag.map(named),
            "required": d.required,
            "hide": d.hide.name(),
            "autoInit": match d.auto_init {
                Some(tanacraft_core::AutoInit::CurrentDate) => "current_date",
                Some(tanacraft_core::AutoInit::AncestorDay) => "ancestor_day",
                Some(tanacraft_core::AutoInit::AncestorValue) => "ancestor_value",
                None => "none",
            },
            "default": d.default.as_ref().map(|v| export::display_value(g, v)),
            "min": d.min, "max": d.max,
            "autoCollect": !d.no_auto_collect,
            "usedIn": {"nodes": nodes, "tags": tags},
        });
    }
    if let Some(d) = g.tag_def(node) {
        let fields: Vec<Value> = g
            .template_fields(node)
            .into_iter()
            .map(|f| {
                json!({"id": f.0, "name": g.name(f), "type": g.field_def(f).map(|x| x.field_type.name()).unwrap_or("plain"),
                       "own": d.fields.contains(&f), "pinned": d.pinned_fields.contains(&f)})
            })
            .collect();
        return json!({
            "kind": "tag",
            "id": node.0,
            "color": d.color,
            "checkbox": d.checkbox,
            "extends": d.extends.iter().map(|t| named(*t)).collect::<Vec<_>>(),
            "fields": fields,
            "optional": d.optional.iter().map(|f| named(*f)).collect::<Vec<_>>(),
            "childTag": d.child_tag.map(named),
            "baseType": d.base_type,
            "baseTypes": tanacraft_core::BASE_TYPES,
            "titleExpr": d.title_expr,
            "doneMap": d.done_map.as_ref().map(|m| json!({"field": named(m.field), "checked": m.checked, "unchecked": m.unchecked})),
            "colors": tanacraft_core::TAG_COLORS,
            "used": g.tagged(node).len(),
        });
    }
    json!({"kind": "none"})
}
