//! Every command. Grouped by location; ids are stable (CLI, MCP and saved shortcuts use them).
//!
//! Shortcut sources: `Ctrl+Shift+D` for today is from Tana's public docs. The other bindings are
//! common outliner conventions chosen here (spec-gap until the behaviour spec names them).

use serde_json::{Value, json};
use tanacraft_core::format::Mark;
use tanacraft_core::query::{Clause, Query};
use tanacraft_core::{Date, FieldType, GroupBy, NodeId, Sort, SortBy, TAG_COLORS, ViewKind, export};

const FMT: &str = r#"{"node"?: id, "start"?: chars, "end"?: chars}  (default: the selection, or the whole text)"#;

use crate::{CmdError, CmdResult, CommandSpec as C, Registry, Session, p};

pub fn registry() -> Registry {
    Registry::new(vec![
        // Editing the outline.
        C::new("node.enter", "New node (split at caret)", "Node", enter).key("Enter").params(r#"{"node"?: id, "caret"?: chars}"#),
        C::new("node.indent", "Indent", "Node", indent).key("Tab").params(r#"{"node"?: id}"#),
        C::new("node.outdent", "Outdent", "Node", outdent).key("Shift+Tab").params(r#"{"node"?: id}"#),
        C::new("node.backspace", "Merge with the node above", "Node", backspace).key("Backspace").params(r#"{"node"?: id}"#),
        C::new("node.moveUp", "Move up", "Node", |s, v| move_sibling(s, v, true)).key("Mod+Shift+Up").params(r#"{"node"?: id}"#),
        C::new("node.moveDown", "Move down", "Node", |s, v| move_sibling(s, v, false)).key("Mod+Shift+Down").params(r#"{"node"?: id}"#),
        C::new("node.toggleDone", "Toggle checkbox", "Node", toggle_done).key("Mod+Enter").params(r#"{"node"?: id}"#),
        C::new("node.delete", "Delete (move to trash)", "Node", delete).key("Mod+Shift+Backspace").params(r#"{"node"?: id}"#),
        C::new("node.duplicate", "Duplicate", "Node", duplicate).key("Mod+Shift+D").params(r#"{"node"?: id}"#),
        C::new("node.createNew", "Create new", "Node", create_new).key("Mod+N"),
        C::new("node.add", "Add node", "Node", add).params(r#"{"name": string, "parent"?: id, "index"?: number}"#),
        C::new("node.setName", "Rename node", "Node", set_name).params(r#"{"node"?: id, "name": string, "caret"?: chars}"#),
        C::new("node.setDescription", "Set description", "Node", set_description).params(r#"{"node"?: id, "text": string}"#),
        C::new("node.move", "Move to", "Node", move_to).params(r#"{"node"?: id, "parent": id, "index"?: number}"#),
        C::new("node.reference", "Add reference", "Node", reference)
            .params(r#"{"target": id, "parent"?: id, "index"?: number, "replace"?: id}  (replace: an empty node the reference takes the place of)"#),
        C::new("node.restore", "Restore from trash", "Node", restore).params(r#"{"node": id}"#),
        C::new("trash.empty", "Empty trash", "Node", empty_trash),
        C::new("node.insert", "Insert node", "Node", insert).key("Shift+Enter").params(r#"{"node"?: id}"#),
        C::new("node.quickAdd", "Quick add to today", "Node", quick_add).params(r#"{"name": string, "tag"?: string}"#),
        // Inline formatting (marks in the text; see core::format).
        C::new("format.bold", "Bold", "Format", |s, v| format_mark(s, v, Mark::Bold)).key("Mod+B").params(FMT),
        C::new("format.italic", "Italic", "Format", |s, v| format_mark(s, v, Mark::Italic)).key("Mod+I").params(FMT),
        C::new("format.underline", "Underline", "Format", |s, v| format_mark(s, v, Mark::Underline)).key("Mod+U").params(FMT),
        C::new("format.highlight", "Highlight", "Format", |s, v| format_mark(s, v, Mark::Highlight)).key("Ctrl+H").params(FMT),
        C::new("format.code", "Code", "Format", |s, v| format_mark(s, v, Mark::Code)).params(FMT),
        // Search nodes.
        C::new("search.create", "New search node", "Search", search_create)
            .params(r#"{"query": string, "name"?: string, "parent"?: id}  (query syntax: see core::query)"#),
        C::new("search.setQuery", "Set search query", "Search", search_set_query).params(r#"{"node"?: id, "query": string}"#),
        C::new("search.addClause", "Add search condition", "Search", search_add_clause).params(r#"{"node"?: id, "clause": string, "not"?: bool}"#),
        C::new("search.removeClause", "Remove search condition", "Search", search_remove_clause).params(r#"{"node"?: id, "index": number}"#),
        C::new("search.toggleNot", "Negate search condition", "Search", search_toggle_not).params(r#"{"node"?: id, "index": number}"#),
        C::new("search.setMode", "Match all or any", "Search", search_set_mode).params(r#"{"node"?: id, "any": bool}"#),
        // Views.
        C::new("view.set", "View as", "Views", view_set).params(r#"{"node"?: id, "kind": "list|table|cards|board|calendar"}"#),
        C::new("view.sort", "Sort by", "Views", view_sort).params(r#"{"node"?: id, "by": "none|name|created|done|<field>", "desc"?: bool}"#),
        C::new("view.group", "Group by", "Views", view_group).params(r#"{"node"?: id, "by": "none|tags|done|<field>"}"#),
        C::new("view.filter", "Filter", "Views", view_filter).params(r#"{"node"?: id, "query": string}  (empty clears)"#),
        C::new("view.addColumn", "Add column", "Views", view_add_column).params(r#"{"node"?: id, "field": name | id, "type"?: string}"#),
        C::new("view.removeColumn", "Remove column", "Views", view_remove_column).params(r#"{"node"?: id, "field": name | id}"#),
        C::new("view.dateField", "Calendar date field", "Views", view_date_field).params(r#"{"node"?: id, "field": name | id}"#),
        C::new("view.month", "Calendar month", "Views", view_month)
            .params(r#"{"step"?: number, "month"?: "YYYY-MM"}  (step -1/1 moves; nothing: this month)"#)
            .pure(),
        C::new("graph.results", "View results", "Graph", results).params(r#"{"node"?: id}"#).pure(),
        // Expand, collapse, focus.
        C::new("node.expand", "Expand", "View", |s, v| set_expanded(s, v, Some(true))).key("Mod+Down").params(r#"{"node"?: id}"#).pure(),
        C::new("node.collapse", "Collapse", "View", |s, v| set_expanded(s, v, Some(false))).key("Mod+Up").params(r#"{"node"?: id}"#).pure(),
        C::new("node.toggleExpand", "Expand or collapse", "View", |s, v| set_expanded(s, v, None)).params(r#"{"node"?: id}"#).pure(),
        C::new("view.expandAll", "Expand all", "View", |s, _| expand_all(s, true)).key("Ctrl+Mod+Down").pure(),
        C::new("view.collapseAll", "Collapse all", "View", |s, _| expand_all(s, false)).key("Ctrl+Mod+Up").pure(),
        C::new("focus.up", "Focus previous node", "View", |s, _| focus_step(s, -1)).key("Up").pure(),
        C::new("focus.down", "Focus next node", "View", |s, _| focus_step(s, 1)).key("Down").pure(),
        C::new("select.up", "Extend selection up", "View", |s, _| select_step(s, -1)).key("Shift+Up").pure(),
        C::new("select.down", "Extend selection down", "View", |s, _| select_step(s, 1)).key("Shift+Down").pure(),
        C::new("select.set", "Select nodes", "View", select_set).params(r#"{"from": id, "to"?: id}  (rows between, inclusive)"#).pure(),
        C::new("select.clear", "Clear selection", "View", |s, _| {
            s.sel = None;
            Ok(json!({}))
        })
        .pure(),
        C::new("focus.set", "Focus node", "View", focus_set).params(r#"{"node": id, "caret"?: chars}"#).pure(),
        // Navigation.
        C::new("nav.zoomIn", "Zoom in", "Navigate", zoom_in).key("Mod+.").params(r#"{"node"?: id}"#).pure(),
        C::new("nav.zoomOut", "Zoom out", "Navigate", zoom_out).key("Mod+,").pure(),
        C::new("nav.open", "Open node", "Navigate", open).params(r#"{"node": id}"#).pure(),
        C::new("nav.back", "Back", "Navigate", |s, _| Ok(json!({"moved": s.go_back()}))).key("Mod+[").pure(),
        C::new("nav.forward", "Forward", "Navigate", |s, _| Ok(json!({"moved": s.go_forward()}))).key("Mod+]").pure(),
        C::new("nav.today", "Go to today", "Navigate", |s, _| open_day(s, s.today)).key("Ctrl+Shift+D").pure(),
        C::new("nav.day", "Go to day", "Navigate", |s, v| {
            open_day(s, Date::parse(p::req_str(v, "date")?).ok_or_else(|| CmdError::Params("`date` must be YYYY-MM-DD".into()))?)
        })
        .params(r#"{"date": "YYYY-MM-DD" | "today" | "yesterday" | "tomorrow"}"#)
        .pure(),
        C::new("nav.previousDay", "Previous day", "Navigate", |s, _| step_day(s, -1)).pure(),
        C::new("nav.nextDay", "Next day", "Navigate", |s, _| step_day(s, 1)).pure(),
        C::new("nav.home", "Go to Home", "Navigate", |s, _| open_sys(s, s.graph.sys.library)).pure(),
        C::new("nav.supertags", "Go to Supertags", "Navigate", |s, _| open_sys(s, s.graph.sys.schema)).pure(),
        C::new("nav.dailyNotes", "Go to Daily notes", "Navigate", |s, _| open_sys(s, s.graph.sys.journal)).pure(),
        C::new("nav.trash", "Go to Trash", "Navigate", |s, _| open_sys(s, s.graph.sys.trash)).pure(),
        // Supertags.
        C::new("tag.add", "Add supertag", "Supertag", tag_add)
            .key("#")
            .params(r##"{"node"?: id, "tag": name | id, "color"?: string}  (creates the supertag if missing)"##),
        C::new("tag.remove", "Remove supertag", "Supertag", tag_remove).params(r#"{"node"?: id, "tag": name | id}"#),
        C::new("tag.create", "Create supertag", "Supertag", tag_create).params(r#"{"name": string, "color"?: string}"#),
        C::new("tag.setColor", "Set supertag colour", "Supertag", tag_set_color).params(r#"{"tag": name | id, "color": string}"#),
        C::new("tag.extend", "Extend supertag", "Supertag", tag_extend).params(r#"{"tag": name | id, "base": name | id}"#),
        C::new("tag.addField", "Add field to supertag", "Supertag", tag_add_field)
            .params(r#"{"tag": name | id, "field": name | id, "type"?: string}"#),
        C::new("tag.setCheckbox", "Supertag shows checkbox", "Supertag", tag_set_checkbox).params(r#"{"tag": name | id, "on": bool}"#),
        // Fields.
        C::new("field.add", "Add field", "Field", field_add)
            .key(">")
            .params(r#"{"node"?: id, "field": name | id, "type"?: string, "value"?: string}"#),
        C::new("field.set", "Set field value", "Field", field_set).params(r#"{"node"?: id, "field": name | id, "value": string}"#),
        C::new("field.remove", "Remove field", "Field", field_remove).params(r#"{"node"?: id, "field": name | id}"#),
        C::new("field.create", "Create field", "Field", field_create)
            .params(r#"{"name": string, "type"?: "plain|options|options_from_tag|date|number|url|email|checkbox"}"#),
        C::new("field.setOptions", "Set field options", "Field", field_set_options).params(r#"{"field": name | id, "options": [string]}"#),
        // Edit and file.
        C::new("edit.undo", "Undo", "Edit", |s, _| Ok(json!({"undone": s.graph.undo()}))).key("Mod+Z").pure(),
        C::new("edit.redo", "Redo", "Edit", |s, _| Ok(json!({"redone": s.graph.redo()}))).key("Mod+Shift+Z").pure(),
        C::new("file.save", "Save", "File", save).key("Mod+S").params(r#"{"path"?: string}"#).pure(),
        C::new("file.open", "Open graph file", "File", open_file).params(r#"{"path": string}"#).pure(),
        // Reading (for agents and the CLI).
        C::new("graph.inspect", "Inspect", "Graph", inspect).params(r#"{"node"?: id}  (default: the zoomed node)"#).pure(),
        C::new("graph.search", "Search", "Graph", search).key("Mod+F").params(r#"{"query": string}"#).pure(),
        C::new("graph.tags", "List supertags", "Graph", list_tags).pure(),
        C::new("graph.view", "Visible outline", "Graph", view).pure(),
        C::new("export.markdown", "Export as Markdown", "Export", |s, v| {
            Ok(json!({"markdown": export::to_markdown(&s.graph, node_or_zoom(s, v)?, false)}))
        })
        .params(r#"{"node"?: id}"#)
        .pure(),
        C::new("export.json", "Export as JSON", "Export", |s, v| Ok(export::to_json_tree(&s.graph, node_or_zoom(s, v)?)))
            .params(r#"{"node"?: id}"#)
            .pure(),
    ])
}

// ---- helpers ------------------------------------------------------------------------------------

fn node_or_zoom(s: &Session, v: &Value) -> Result<NodeId, CmdError> {
    let id = p::node(v, "node")?.unwrap_or(s.zoom);
    s.graph.node(id)?;
    Ok(id)
}

fn len(s: &Session, id: NodeId) -> usize {
    s.graph.name(id).chars().count()
}

fn row_neighbour(s: &Session, id: NodeId, step: isize) -> Option<NodeId> {
    let rows = s.rows();
    let at = rows.iter().position(|r| r.id == id)?;
    let j = at.checked_add_signed(step)?;
    rows.get(j).map(|r| r.id)
}

/// Accepts a node id or a name; looks names up with `find`.
fn resolve_named(s: &Session, v: &Value, key: &str, find: fn(&tanacraft_core::Graph, &str) -> Option<NodeId>) -> Result<Option<NodeId>, CmdError> {
    match v.get(key) {
        Some(Value::String(name)) if name.trim().parse::<NodeId>().is_err() => Ok(find(&s.graph, name)),
        _ => {
            let id = p::node(v, key)?.ok_or_else(|| CmdError::Params(format!("`{key}` is required")))?;
            s.graph.node(id)?;
            Ok(Some(id))
        }
    }
}

fn tag_param(s: &mut Session, v: &Value, key: &str, create: bool) -> Result<NodeId, CmdError> {
    if let Some(t) = resolve_named(s, v, key, |g, n| g.find_tag(n))? {
        s.graph.tag_def(t).ok_or_else(|| CmdError::Params(format!("node {t} is not a supertag")))?;
        return Ok(t);
    }
    let name = p::req_str(v, key)?;
    if !create {
        return Err(CmdError::Failed(format!("no supertag #{}", name.trim_start_matches('#'))));
    }
    Ok(s.graph.create_tag(name, p::str(v, "color").unwrap_or("grey"))?)
}

fn field_type(v: &Value) -> Result<FieldType, CmdError> {
    match p::str(v, "type") {
        None => Ok(FieldType::Plain),
        Some(t) => FieldType::parse(t).ok_or_else(|| CmdError::Params(format!("unknown field type `{t}`"))),
    }
}

fn field_param(s: &mut Session, v: &Value, create: bool) -> Result<NodeId, CmdError> {
    if let Some(f) = resolve_named(s, v, "field", |g, n| g.find_field(n))? {
        s.graph.field_def(f).ok_or_else(|| CmdError::Params(format!("node {f} is not a field")))?;
        return Ok(f);
    }
    let name = p::req_str(v, "field")?;
    if !create {
        return Err(CmdError::Failed(format!("no field `{name}`")));
    }
    let ty = field_type(v)?;
    Ok(s.graph.create_field(name, ty)?)
}

fn char_split(s: &str, at: usize) -> (String, String) {
    let i = s.char_indices().nth(at).map(|(i, _)| i).unwrap_or(s.len());
    (s[..i].to_string(), s[i..].to_string())
}

// ---- editing ------------------------------------------------------------------------------------

/// Enter. At the end of an expanded node with children: a new first child. At the start of a
/// non-empty node: an empty node above (focus stays). Otherwise the text after the caret moves to a
/// new sibling below, which takes focus.
fn enter(s: &mut Session, v: &Value) -> CmdResult {
    let id = s.target(v)?;
    let n = len(s, id);
    let caret = p::usize(v, "caret").unwrap_or(if s.focus == Some(id) { s.caret } else { n }).min(n);
    if id == s.zoom {
        let new = s.graph.create(id, Some(0), "")?;
        s.set_focus(new, 0);
        return Ok(s.focus_json());
    }
    let parent = s.graph.parent(id).ok_or_else(|| CmdError::Failed("this node has no parent".into()))?;
    let at = s.graph.index_in_parent(id).unwrap_or(0);
    let is_ref = s.graph.get(id).is_some_and(|x| x.reference_target().is_some());
    if caret == n && s.graph.is_expanded(id) && !s.graph.children(id).is_empty() {
        let new = s.graph.create(id, Some(0), "")?;
        s.set_focus(new, 0);
    } else if caret == 0 && n > 0 && !is_ref {
        s.graph.create(parent, Some(at), "")?;
        s.set_focus(id, 0);
    } else if is_ref {
        let new = s.graph.create(parent, Some(at + 1), "")?;
        s.set_focus(new, 0);
    } else {
        let (head, tail) = char_split(s.graph.name(id), caret);
        s.graph.set_name(id, &head)?;
        let new = s.graph.create(parent, Some(at + 1), &tail)?;
        s.set_focus(new, 0);
    }
    Ok(s.focus_json())
}

// Selections: indent and move-up go top-down, outdent and move-down bottom-up, so the selected
// nodes keep their order.
fn indent(s: &mut Session, v: &Value) -> CmdResult {
    for id in s.targets(v)? {
        s.graph.indent(id)?;
    }
    Ok(s.focus_json())
}

fn outdent(s: &mut Session, v: &Value) -> CmdResult {
    for id in s.targets(v)?.into_iter().rev() {
        if s.graph.parent(id) == Some(s.graph.resolve(s.zoom)) {
            return Err(CmdError::Failed("already at the top of this view".into()));
        }
        s.graph.outdent(id)?;
    }
    Ok(s.focus_json())
}

/// Backspace at the start of a node: an empty leaf is deleted; otherwise the text joins the node
/// above. Nodes with supertags or fields are not merged (that would lose them).
fn backspace(s: &mut Session, v: &Value) -> CmdResult {
    let id = s.target(v)?;
    let prev = row_neighbour(s, id, -1);
    let node = s.graph.node(id)?.clone();
    let is_ref = node.reference_target().is_some();
    if !is_ref && (!node.tags.is_empty() || !node.fields.is_empty()) {
        return Err(CmdError::Failed("remove the supertags and fields before merging this node".into()));
    }
    let empty_leaf = is_ref || (node.name.is_empty() && node.children.is_empty());
    match prev {
        Some(prev) if empty_leaf => {
            s.graph.delete_permanently(id)?;
            let end = len(s, prev);
            s.set_focus(prev, end);
        }
        Some(prev) if s.graph.get(prev).is_some_and(|p| p.reference_target().is_none()) => {
            let caret = s.graph.merge_into(id, prev)?;
            s.set_focus(prev, caret);
        }
        Some(_) => return Err(CmdError::Failed("cannot merge into a reference".into())),
        None => return Err(CmdError::Failed("nothing above to merge with".into())),
    }
    Ok(s.focus_json())
}

fn move_sibling(s: &mut Session, v: &Value, up: bool) -> CmdResult {
    let mut ids = s.targets(v)?;
    if !up {
        ids.reverse();
    }
    for id in ids {
        s.graph.move_sibling(id, up)?;
    }
    Ok(s.focus_json())
}

/// No checkbox, then unchecked, then checked, then back (to unchecked when a supertag asks for a
/// checkbox, else to none).
fn toggle_done(s: &mut Session, v: &Value) -> CmdResult {
    let ids: Vec<NodeId> = s.targets(v)?.into_iter().map(|i| s.graph.resolve(i)).collect();
    // A selection toggles together, following the first node.
    let first = ids.first().copied().ok_or_else(|| CmdError::Params("no node".into()))?;
    let next = match s.graph.checkbox(first) {
        None => Some(false),
        Some(false) => Some(true),
        Some(true) if s.graph.tag_wants_checkbox(first) => Some(false),
        Some(true) => None,
    };
    for id in &ids {
        s.graph.set_done(*id, next)?;
    }
    Ok(json!({"node": first.0, "count": ids.len(), "done": next}))
}

fn delete(s: &mut Session, v: &Value) -> CmdResult {
    let ids = s.targets(v)?;
    if ids.len() > 1 {
        for id in &ids {
            s.graph.trash(*id)?;
        }
        s.sel = None;
        return Ok(json!({"trashed": ids.iter().map(|i| i.0).collect::<Vec<_>>()}));
    }
    let id = s.target(v)?;
    let next_focus = row_neighbour(s, id, -1).or_else(|| row_neighbour(s, id, 1));
    s.graph.trash(id)?;
    if s.focus == Some(id) {
        match next_focus {
            Some(f) => {
                let end = len(s, f);
                s.set_focus(f, end);
            }
            None => s.focus = None,
        }
    }
    Ok(json!({"trashed": id.0}))
}

fn duplicate(s: &mut Session, v: &Value) -> CmdResult {
    let id = s.target(v)?;
    let copy = s.graph.duplicate(id)?;
    s.set_focus(copy, usize::MAX);
    Ok(json!({"id": copy.0}))
}

/// A new empty node at the end of today's page, focused.
// spec-gap: where "Create new" puts the node; today's page is our choice.
fn create_new(s: &mut Session, _: &Value) -> CmdResult {
    let day = s.graph.day(s.today)?;
    let id = s.graph.create(day, None, "")?;
    s.open(day)?;
    s.set_focus(id, 0);
    Ok(json!({"id": id.0}))
}

fn add(s: &mut Session, v: &Value) -> CmdResult {
    let parent = p::node(v, "parent")?.unwrap_or(s.zoom);
    let name = p::str(v, "name").unwrap_or("");
    let id = s.graph.create(parent, p::usize(v, "index"), name)?;
    Ok(json!({"id": id.0}))
}

fn set_name(s: &mut Session, v: &Value) -> CmdResult {
    let id = s.target(v)?;
    s.graph.set_name(id, p::req_str(v, "name")?)?;
    match p::usize(v, "caret") {
        Some(c) => s.set_focus(id, c),
        None if s.focus == Some(id) => s.set_focus(id, usize::MAX),
        None => {}
    }
    Ok(json!({"node": id.0, "name": s.graph.name(id)}))
}

fn set_description(s: &mut Session, v: &Value) -> CmdResult {
    let id = s.target(v)?;
    s.graph.set_description(id, p::req_str(v, "text")?)?;
    Ok(json!({"node": id.0}))
}

fn move_to(s: &mut Session, v: &Value) -> CmdResult {
    let id = s.target(v)?;
    let parent = p::node(v, "parent")?.ok_or_else(|| CmdError::Params("`parent` is required".into()))?;
    s.graph.move_to(id, parent, p::usize(v, "index"))?;
    Ok(json!({"node": id.0, "parent": s.graph.parent(id).map(|x| x.0)}))
}

fn reference(s: &mut Session, v: &Value) -> CmdResult {
    let target = p::node(v, "target")?.ok_or_else(|| CmdError::Params("`target` is required".into()))?;
    let mut parent = p::node(v, "parent")?.unwrap_or(s.zoom);
    let mut index = p::usize(v, "index");
    if let Some(r) = p::node(v, "replace")? {
        let n = s.graph.node(r)?;
        if !n.name.is_empty() || !n.children.is_empty() || s.graph.is_system(r) {
            return Err(CmdError::Failed("only an empty node can become a reference".into()));
        }
        parent = s.graph.parent(r).ok_or_else(|| CmdError::Failed("no parent".into()))?;
        index = s.graph.index_in_parent(r);
        s.graph.delete_permanently(r)?;
    }
    let id = s.graph.create_reference(parent, index, target)?;
    if s.focus.is_some_and(|f| s.graph.get(f).is_none()) {
        s.set_focus(id, 0);
    }
    Ok(json!({"id": id.0, "target": s.graph.resolve(id).0}))
}

fn restore(s: &mut Session, v: &Value) -> CmdResult {
    let id = s.target(v)?;
    let parent = s.graph.restore(id)?;
    Ok(json!({"node": id.0, "parent": parent.0}))
}

fn empty_trash(s: &mut Session, _: &Value) -> CmdResult {
    Ok(json!({"deleted": s.graph.empty_trash()?}))
}

// ---- view ---------------------------------------------------------------------------------------

fn set_expanded(s: &mut Session, v: &Value, on: Option<bool>) -> CmdResult {
    let id = s.target(v)?;
    let on = on.unwrap_or(!s.graph.is_expanded(id));
    s.graph.set_expanded(id, on);
    Ok(json!({"node": id.0, "expanded": on}))
}

fn expand_all(s: &mut Session, on: bool) -> CmdResult {
    // ponytail: walks the zoomed subtree breadth-first with a cap; fine for outlines people read.
    let mut stack = vec![(s.zoom, 0usize)];
    let mut count = 0;
    while let Some((id, depth)) = stack.pop() {
        if depth > 32 || count > 50_000 {
            break;
        }
        for c in s.graph.children(id).to_vec() {
            if !s.graph.children(c).is_empty() {
                s.graph.set_expanded(c, on);
                count += 1;
                if s.graph.get(c).is_some_and(|n| n.reference_target().is_none()) {
                    stack.push((c, depth + 1));
                }
            }
        }
    }
    Ok(json!({"changed": count}))
}

fn focus_step(s: &mut Session, step: isize) -> CmdResult {
    let rows = s.rows();
    let next = match s.focus {
        Some(f) => row_neighbour(s, f, step),
        None if step > 0 => rows.first().map(|r| r.id),
        None => rows.last().map(|r| r.id),
    };
    if let Some(n) = next {
        let caret = s.caret;
        s.set_focus(n, caret);
    }
    Ok(s.focus_json())
}

/// Shift+Up/Down: starts a selection at the focused bullet (leaving text editing) or grows it.
fn select_step(s: &mut Session, step: isize) -> CmdResult {
    let (anchor, head) = match (s.sel, s.focus) {
        (Some(sel), _) => sel,
        (None, Some(f)) => (f, f),
        (None, None) => return Err(CmdError::Failed("focus a bullet first".into())),
    };
    let head = row_neighbour(s, head, step).unwrap_or(head);
    s.sel = Some((anchor, head));
    s.focus = None;
    Ok(json!({"selected": s.selected().iter().map(|i| i.0).collect::<Vec<_>>()}))
}

fn select_set(s: &mut Session, v: &Value) -> CmdResult {
    let from = p::node(v, "from")?.ok_or_else(|| CmdError::Params("`from` is required".into()))?;
    let to = p::node(v, "to")?.unwrap_or(from);
    if !s.rows().iter().any(|r| r.id == from) || !s.rows().iter().any(|r| r.id == to) {
        return Err(CmdError::Params("both ends must be visible bullets".into()));
    }
    s.sel = Some((from, to));
    s.focus = None;
    Ok(json!({"selected": s.selected().iter().map(|i| i.0).collect::<Vec<_>>()}))
}

fn focus_set(s: &mut Session, v: &Value) -> CmdResult {
    let id = s.target(v)?;
    let caret = p::usize(v, "caret").unwrap_or(usize::MAX);
    s.set_focus(id, caret);
    Ok(s.focus_json())
}

// ---- navigation ---------------------------------------------------------------------------------

fn zoom_in(s: &mut Session, v: &Value) -> CmdResult {
    let id = s.target(v)?;
    s.open(id)?;
    Ok(s.focus_json())
}

fn zoom_out(s: &mut Session, _: &Value) -> CmdResult {
    let current = s.zoom;
    let parent = s.graph.parent(current).filter(|p| *p != s.graph.sys.root).ok_or_else(|| CmdError::Failed("already at the top".into()))?;
    s.open(parent)?;
    s.set_focus(current, usize::MAX);
    Ok(s.focus_json())
}

fn open(s: &mut Session, v: &Value) -> CmdResult {
    let id = p::node(v, "node")?.ok_or_else(|| CmdError::Params("`node` is required".into()))?;
    s.open(id)?;
    Ok(s.focus_json())
}

fn open_sys(s: &mut Session, id: NodeId) -> CmdResult {
    s.open(id)?;
    Ok(s.focus_json())
}

/// Opens a day node (created when missing; creating a day is not an undo step).
fn open_day(s: &mut Session, date: Date) -> CmdResult {
    let day = s.graph.day(date)?;
    s.open(day)?;
    Ok(json!({"zoom": day.0, "date": date.to_string()}))
}

fn step_day(s: &mut Session, step: i64) -> CmdResult {
    let date = match s.graph.get(s.zoom).map(|n| n.kind.clone()) {
        Some(tanacraft_core::Kind::Day { date }) => date,
        _ => s.today,
    };
    open_day(s, date.add_days(step))
}

// ---- supertags and fields -----------------------------------------------------------------------

fn tag_add(s: &mut Session, v: &Value) -> CmdResult {
    let ids = s.targets(v)?;
    let tag = tag_param(s, v, "tag", true)?;
    for id in &ids {
        s.graph.add_tag(*id, tag)?;
    }
    Ok(json!({"node": ids.first().map(|i| i.0), "count": ids.len(), "tag": tag.0}))
}

fn tag_remove(s: &mut Session, v: &Value) -> CmdResult {
    let id = s.target(v)?;
    let tag = tag_param(s, v, "tag", false)?;
    s.graph.remove_tag(id, tag)?;
    Ok(json!({"node": id.0, "tag": tag.0}))
}

fn tag_create(s: &mut Session, v: &Value) -> CmdResult {
    let id = s.graph.create_tag(p::req_str(v, "name")?, p::str(v, "color").unwrap_or("grey"))?;
    Ok(json!({"tag": id.0}))
}

fn tag_set_color(s: &mut Session, v: &Value) -> CmdResult {
    let tag = tag_param(s, v, "tag", false)?;
    let color = p::req_str(v, "color")?;
    if !TAG_COLORS.contains(&color) {
        return Err(CmdError::Params(format!("`color` must be one of {}", TAG_COLORS.join(", "))));
    }
    s.graph.set_tag_color(tag, color)?;
    Ok(json!({"tag": tag.0, "color": color}))
}

fn tag_extend(s: &mut Session, v: &Value) -> CmdResult {
    let tag = tag_param(s, v, "tag", false)?;
    let base = tag_param(s, v, "base", false)?;
    s.graph.tag_extend(tag, base)?;
    Ok(json!({"tag": tag.0, "base": base.0}))
}

fn tag_add_field(s: &mut Session, v: &Value) -> CmdResult {
    let tag = tag_param(s, v, "tag", false)?;
    let field = field_param(s, v, true)?;
    s.graph.tag_add_field(tag, field)?;
    Ok(json!({"tag": tag.0, "field": field.0}))
}

fn tag_set_checkbox(s: &mut Session, v: &Value) -> CmdResult {
    let tag = tag_param(s, v, "tag", false)?;
    let on = p::bool(v, "on").ok_or_else(|| CmdError::Params("`on` (bool) is required".into()))?;
    s.graph.set_tag_checkbox(tag, on)?;
    Ok(json!({"tag": tag.0, "checkbox": on}))
}

fn field_add(s: &mut Session, v: &Value) -> CmdResult {
    let id = s.target(v)?;
    let field = field_param(s, v, true)?;
    match p::str(v, "value") {
        Some(text) => s.graph.set_field_text(id, field, text)?,
        None if s.graph.get(s.graph.resolve(id)).is_some_and(|n| n.fields.iter().any(|f| f.field == field)) => {}
        None => s.graph.set_field(id, field, None)?,
    }
    Ok(json!({"node": id.0, "field": field.0}))
}

fn field_set(s: &mut Session, v: &Value) -> CmdResult {
    let id = s.target(v)?;
    let field = field_param(s, v, false)?;
    s.graph.set_field_text(id, field, p::req_str(v, "value")?)?;
    let value = s.graph.get(s.graph.resolve(id)).and_then(|n| n.field_value(field)).map(|x| export::display_value(&s.graph, x));
    Ok(json!({"node": id.0, "field": field.0, "value": value}))
}

fn field_remove(s: &mut Session, v: &Value) -> CmdResult {
    let id = s.target(v)?;
    let field = field_param(s, v, false)?;
    s.graph.remove_field(id, field)?;
    Ok(json!({"node": id.0, "field": field.0}))
}

fn field_create(s: &mut Session, v: &Value) -> CmdResult {
    let ty = field_type(v)?;
    let id = s.graph.create_field(p::req_str(v, "name")?, ty)?;
    Ok(json!({"field": id.0, "type": ty.name()}))
}

fn field_set_options(s: &mut Session, v: &Value) -> CmdResult {
    let field = field_param(s, v, false)?;
    let options: Vec<String> = v
        .get("options")
        .and_then(Value::as_array)
        .ok_or_else(|| CmdError::Params("`options` (array of strings) is required".into()))?
        .iter()
        .filter_map(|o| o.as_str().map(str::to_string))
        .take(1000)
        .collect();
    s.graph.field_def_mut(field)?.options = options;
    Ok(json!({"field": field.0}))
}

// ---- file ---------------------------------------------------------------------------------------

fn save(s: &mut Session, v: &Value) -> CmdResult {
    if let Some(p) = p::str(v, "path") {
        s.path = Some(p.into());
    }
    let path = s.path.clone().ok_or_else(|| CmdError::Params("no file yet: pass `path`".into()))?;
    s.graph.save(&path)?;
    Ok(json!({"saved": path.display().to_string()}))
}

fn open_file(s: &mut Session, v: &Value) -> CmdResult {
    let path = std::path::PathBuf::from(p::req_str(v, "path")?);
    s.graph = tanacraft_core::Graph::load(&path)?;
    s.path = Some(path);
    s.zoom = s.graph.sys.library;
    s.focus = None;
    Ok(json!({"nodes": s.graph.len()}))
}

// ---- reading ------------------------------------------------------------------------------------

fn inspect(s: &mut Session, v: &Value) -> CmdResult {
    let id = node_or_zoom(s, v)?;
    let mut tree = export::to_json_tree(&s.graph, id);
    tree["breadcrumbs"] = json!(s.graph.ancestors(id).iter().map(|a| json!({"id": a.0, "name": s.graph.name(*a)})).collect::<Vec<_>>());
    Ok(tree)
}

fn search(s: &mut Session, v: &Value) -> CmdResult {
    let q = p::req_str(v, "query")?;
    let hits: Vec<Value> = s.graph.search(q).into_iter().take(200).map(|id| json!({"id": id.0, "line": export::node_line(&s.graph, id)})).collect();
    Ok(Value::Array(hits))
}

fn list_tags(s: &mut Session, _: &Value) -> CmdResult {
    let g = &s.graph;
    Ok(Value::Array(
        g.tags()
            .into_iter()
            .map(|t| {
                let def = g.tag_def(t).cloned().unwrap_or_default();
                json!({
                    "id": t.0,
                    "name": g.name(t),
                    "color": def.color,
                    "extends": def.extends.iter().map(|x| g.name(*x)).collect::<Vec<_>>(),
                    "fields": g.template_fields(t).iter().map(|f| json!({"id": f.0, "name": g.name(*f), "type": g.field_def(*f).map(|d| d.field_type.name())})).collect::<Vec<_>>(),
                    "checkbox": def.checkbox,
                    "count": g.tagged(t).len(),
                })
            })
            .collect(),
    ))
}

/// What the panel shows right now, as text lines (handy for agents and tests).
fn view(s: &mut Session, _: &Value) -> CmdResult {
    let lines: Vec<String> = s
        .rows()
        .iter()
        .map(|r| {
            let mark = if s.focus == Some(r.id) { ">" } else { " " };
            format!("{mark} {}- {}", "  ".repeat(r.depth), export::node_line(&s.graph, r.id))
        })
        .collect();
    Ok(json!({"title": s.graph.name(s.zoom), "zoom": s.zoom.0, "lines": lines}))
}

// ---- insert, quick add, formatting --------------------------------------------------------------

/// Shift+Enter: a new empty node right after this one, leaving the text whole.
// spec-gap: the spec lists "Insert node" for Shift+Enter without saying where; we add below.
fn insert(s: &mut Session, v: &Value) -> CmdResult {
    let id = s.target(v)?;
    let parent = s.graph.parent(id).ok_or_else(|| CmdError::Failed("this node has no parent".into()))?;
    let at = s.graph.index_in_parent(id).unwrap_or(0);
    let new = s.graph.create(parent, Some(at + 1), "")?;
    s.set_focus(new, 0);
    Ok(json!({"id": new.0}))
}

/// Adds a node to today's page without leaving the current page.
fn quick_add(s: &mut Session, v: &Value) -> CmdResult {
    let name = p::req_str(v, "name")?;
    if name.trim().is_empty() {
        return Err(CmdError::Params("`name` is empty".into()));
    }
    let day = s.graph.day(s.today)?;
    let id = s.graph.create(day, None, name)?;
    if let Some(t) = p::str(v, "tag") {
        let tag = match s.graph.find_tag(t) {
            Some(t) => t,
            None => s.graph.create_tag(t, "grey")?,
        };
        s.graph.add_tag(id, tag)?;
    }
    Ok(json!({"id": id.0, "day": day.0}))
}

fn format_mark(s: &mut Session, v: &Value, mark: Mark) -> CmdResult {
    let id = s.graph.resolve(s.target(v)?);
    let focused = s.focus.map(|f| s.graph.resolve(f)) == Some(id);
    let (a, b) = match (p::usize(v, "start"), p::usize(v, "end")) {
        (Some(a), Some(b)) => (a, b),
        _ if focused => (s.anchor, s.caret),
        _ => (0, 0),
    };
    let (text, a2, b2) = tanacraft_core::format::toggle(s.graph.name(id), a, b, mark);
    s.graph.set_name(id, &text)?;
    if focused {
        s.caret = b2;
        s.anchor = a2;
    }
    Ok(json!({"node": id.0, "name": text, "start": a2, "end": b2}))
}

// ---- search nodes -------------------------------------------------------------------------------

/// The search node a command acts on: `node`, else the focused bullet or the open page if either is one.
fn search_target(s: &Session, v: &Value) -> Result<NodeId, CmdError> {
    let candidates = [p::node(v, "node")?, s.focus, Some(s.zoom)];
    candidates
        .into_iter()
        .flatten()
        .map(|c| s.graph.resolve(c))
        .find(|c| s.graph.query(*c).is_some())
        .ok_or_else(|| CmdError::Params("no search node: pass `node`".into()))
}

fn parse_query(s: &Session, text: &str) -> Result<Query, CmdError> {
    Query::parse(&s.graph, text).map_err(|e| CmdError::Params(e.to_string()))
}

fn search_create(s: &mut Session, v: &Value) -> CmdResult {
    let q = parse_query(s, p::req_str(v, "query")?)?;
    let parent = p::node(v, "parent")?.unwrap_or(s.zoom);
    let name = p::str(v, "name").map(str::to_string).unwrap_or_else(|| q.describe(&s.graph));
    let id = s.graph.create_search(parent, p::usize(v, "index"), &name, q)?;
    s.graph.set_expanded(id, true);
    Ok(json!({"id": id.0, "results": s.graph.view_items(id).len()}))
}

fn query_result(s: &Session, id: NodeId) -> CmdResult {
    let q = s.graph.query(id).map(|q| q.describe(&s.graph)).unwrap_or_default();
    Ok(json!({"node": id.0, "query": q, "results": s.graph.view_items(id).len()}))
}

fn search_set_query(s: &mut Session, v: &Value) -> CmdResult {
    let id = search_target(s, v)?;
    let q = parse_query(s, p::req_str(v, "query")?)?;
    s.graph.set_query(id, q)?;
    query_result(s, id)
}

fn edit_query(s: &mut Session, v: &Value, f: impl FnOnce(&mut Query) -> Result<(), CmdError>) -> CmdResult {
    let id = search_target(s, v)?;
    let mut q = s.graph.query(id).cloned().unwrap_or_default();
    f(&mut q)?;
    s.graph.set_query(id, q)?;
    query_result(s, id)
}

fn search_add_clause(s: &mut Session, v: &Value) -> CmdResult {
    let parsed = parse_query(s, p::req_str(v, "clause")?)?;
    let not = p::bool(v, "not").unwrap_or(false);
    edit_query(s, v, |q| {
        q.clauses.extend(parsed.clauses.into_iter().map(|c| Clause { not: c.not != not, kind: c.kind }));
        Ok(())
    })
}

fn clause_index(v: &Value, q: &Query) -> Result<usize, CmdError> {
    p::usize(v, "index").filter(|i| *i < q.clauses.len()).ok_or_else(|| CmdError::Params("`index` must name an existing condition".into()))
}

fn search_remove_clause(s: &mut Session, v: &Value) -> CmdResult {
    edit_query(s, v, |q| {
        let i = clause_index(v, q)?;
        q.clauses.remove(i);
        Ok(())
    })
}

fn search_toggle_not(s: &mut Session, v: &Value) -> CmdResult {
    edit_query(s, v, |q| {
        let i = clause_index(v, q)?;
        if let Some(c) = q.clauses.get_mut(i) {
            c.not = !c.not;
        }
        Ok(())
    })
}

fn search_set_mode(s: &mut Session, v: &Value) -> CmdResult {
    let any = p::bool(v, "any").ok_or_else(|| CmdError::Params("`any` (bool) is required".into()))?;
    edit_query(s, v, |q| {
        q.any = any;
        Ok(())
    })
}

// ---- views --------------------------------------------------------------------------------------

/// The node whose view a command changes: `node`, else the open page.
fn view_target(s: &Session, v: &Value) -> Result<NodeId, CmdError> {
    let id = p::node(v, "node")?.unwrap_or(s.zoom);
    s.graph.node(id)?;
    Ok(s.graph.resolve(id))
}

fn view_json(s: &Session, id: NodeId) -> CmdResult {
    let d = s.graph.view_def(id);
    Ok(json!({"node": id.0, "kind": d.kind.name(), "groups": s.graph.view_groups(id).len()}))
}

fn view_set(s: &mut Session, v: &Value) -> CmdResult {
    let id = view_target(s, v)?;
    let kind = p::req_str(v, "kind")?;
    let kind = ViewKind::parse(kind).ok_or_else(|| CmdError::Params(format!("unknown view `{kind}`")))?;
    let needs_group = kind == ViewKind::Board && s.graph.view_def(id).group.is_none();
    // A board needs lanes: default to the first options field, else done / not done.
    let default_group = needs_group.then(|| {
        s.graph
            .view_columns(id)
            .into_iter()
            .find(|f| s.graph.field_def(*f).is_some_and(|d| d.field_type == FieldType::Options))
            .map(GroupBy::Field)
            .unwrap_or(GroupBy::Done)
    });
    let view = s.graph.view_mut(id)?;
    view.kind = kind;
    if let Some(g) = default_group {
        view.group = Some(g);
    }
    view_json(s, id)
}

fn field_or_keyword(s: &mut Session, v: &Value, key: &str) -> Result<Result<NodeId, String>, CmdError> {
    let raw = p::req_str(v, key)?.to_string();
    let lower = raw.trim().to_ascii_lowercase();
    if ["none", "name", "created", "done", "tags"].contains(&lower.as_str()) {
        return Ok(Err(lower));
    }
    let probe = json!({"field": raw});
    Ok(Ok(field_param(s, &probe, false)?))
}

fn view_sort(s: &mut Session, v: &Value) -> CmdResult {
    let id = view_target(s, v)?;
    let desc = p::bool(v, "desc").unwrap_or(false);
    let by = match field_or_keyword(s, v, "by")? {
        Ok(f) => Some(SortBy::Field(f)),
        Err(k) => match k.as_str() {
            "none" => None,
            "name" => Some(SortBy::Name),
            "created" => Some(SortBy::Created),
            "done" => Some(SortBy::Done),
            other => return Err(CmdError::Params(format!("cannot sort by `{other}`"))),
        },
    };
    s.graph.view_mut(id)?.sort = by.map(|by| Sort { by, desc });
    view_json(s, id)
}

fn view_group(s: &mut Session, v: &Value) -> CmdResult {
    let id = view_target(s, v)?;
    let by = match field_or_keyword(s, v, "by")? {
        Ok(f) => Some(GroupBy::Field(f)),
        Err(k) => match k.as_str() {
            "none" => None,
            "tags" => Some(GroupBy::Tags),
            "done" => Some(GroupBy::Done),
            other => return Err(CmdError::Params(format!("cannot group by `{other}`"))),
        },
    };
    s.graph.view_mut(id)?.group = by;
    view_json(s, id)
}

fn view_filter(s: &mut Session, v: &Value) -> CmdResult {
    let id = view_target(s, v)?;
    let q = parse_query(s, p::str(v, "query").unwrap_or(""))?;
    s.graph.view_mut(id)?.filter = q;
    view_json(s, id)
}

fn view_add_column(s: &mut Session, v: &Value) -> CmdResult {
    let id = view_target(s, v)?;
    let f = field_param(s, v, true)?;
    let mut cols = s.graph.view_columns(id);
    if !cols.contains(&f) {
        cols.push(f);
    }
    s.graph.view_mut(id)?.columns = cols;
    view_json(s, id)
}

fn view_remove_column(s: &mut Session, v: &Value) -> CmdResult {
    let id = view_target(s, v)?;
    let f = field_param(s, v, false)?;
    let cols: Vec<NodeId> = s.graph.view_columns(id).into_iter().filter(|c| *c != f).collect();
    s.graph.view_mut(id)?.columns = cols;
    view_json(s, id)
}

fn view_date_field(s: &mut Session, v: &Value) -> CmdResult {
    let id = view_target(s, v)?;
    let f = field_param(s, v, false)?;
    if s.graph.field_def(f).is_none_or(|d| d.field_type != FieldType::Date) {
        return Err(CmdError::Params("the calendar needs a date field".into()));
    }
    s.graph.view_mut(id)?.date_field = Some(f);
    view_json(s, id)
}

fn view_month(s: &mut Session, v: &Value) -> CmdResult {
    let (mut y, mut m) = s.calendar_month.unwrap_or((s.today.year, s.today.month));
    if let Some(text) = p::str(v, "month") {
        let d = Date::parse(&format!("{text}-01")).ok_or_else(|| CmdError::Params("`month` must be YYYY-MM".into()))?;
        (y, m) = (d.year, d.month);
    } else if let Some(step) = v.get("step").and_then(Value::as_i64) {
        let total = (y as i64 * 12 + (m as i64 - 1)).saturating_add(step.clamp(-1200, 1200));
        y = total.div_euclid(12) as i32;
        m = (total.rem_euclid(12) + 1) as u8;
    } else {
        (y, m) = (s.today.year, s.today.month);
    }
    if Date::new(y, m, 1).is_none() {
        return Err(CmdError::Params("month out of range".into()));
    }
    s.calendar_month = Some((y, m));
    Ok(json!({"month": format!("{y:04}-{m:02}")}))
}

fn results(s: &mut Session, v: &Value) -> CmdResult {
    let id = view_target(s, v)?;
    let groups: Vec<Value> = s
        .graph
        .view_groups(id)
        .into_iter()
        .map(|g| json!({"label": g.label, "items": g.items.iter().map(|i| json!({"id": i.0, "line": export::node_line(&s.graph, *i)})).collect::<Vec<_>>()}))
        .collect();
    let d = s.graph.view_def(id);
    Ok(json!({"node": id.0, "view": d.kind.name(), "query": s.graph.query(id).map(|q| q.describe(&s.graph)), "groups": groups}))
}
