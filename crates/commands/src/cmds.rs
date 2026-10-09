//! Every command. Grouped by location; ids are stable (CLI, MCP and saved shortcuts use them).
//!
//! Shortcut sources: `Ctrl+Shift+D` for today is from Tana's public docs. The other bindings are
//! common outliner conventions chosen here (spec-gap until the behaviour spec names them).

use serde_json::{Value, json};
use tanacraft_core::format::Mark;
use tanacraft_core::query::{Clause, ClauseKind, Query};
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
        C::new("node.createNew", "Create new", "Node", create_new).key("Mod+N / Ctrl+N"),
        C::new("edit.insertDate", "Insert date", "Edit", |s, v| {
            let d = match p::str(v, "date") {
                Some(t) => Date::parse(t).ok_or_else(|| CmdError::Params(format!("`{t}` is not a date")))?,
                None => s.today,
            };
            let day = s.graph.day(d)?;
            insert_text(s, v, &tanacraft_core::format::ref_token(day.0))
        })
        .params(r#"{"node"?: id, "caret"?: number, "date"?: "YYYY-MM-DD | today | ..."}  (an inline link to the day)"#),
        C::new("edit.insertNow", "Insert current date and time", "Edit", |s, v| {
            let day = s.graph.day(s.today)?;
            let text = format!("{} {}", tanacraft_core::format::ref_token(day.0), tanacraft_core::fmt_hm(Date::now_minutes()));
            insert_text(s, v, &text)
        })
        .params(r#"{"node"?: id, "caret"?: number}"#),
        C::new("edit.insertTime", "Insert current time", "Edit", |s, v| insert_text(s, v, &tanacraft_core::fmt_hm(Date::now_minutes())))
            .params(r#"{"node"?: id, "caret"?: number}"#),
        C::new("node.convertToSupertag", "Convert to supertag", "Node", |s, v| {
            let id = s.target(v)?;
            let n = s.graph.tags().len();
            let color = p::str(v, "color").map(str::to_string).unwrap_or_else(|| TAG_COLORS.get(2 + n % 12).copied().unwrap_or("grey").to_string());
            let tag = s.graph.convert_to_tag(s.graph.resolve(id), &color)?;
            Ok(json!({"tag": tag.0, "name": s.graph.name(tag)}))
        })
        .params(r#"{"node"?: id, "color"?: string}"#),
        C::new("node.hardDelete", "Hard delete including references", "Node", |s, v| {
            let id = s.graph.resolve(s.target(v)?);
            let refs: Vec<NodeId> = s.graph.iter().filter(|n| n.reference_target() == Some(id)).map(|n| n.id).collect();
            for r in &refs {
                s.graph.delete_permanently(*r)?;
            }
            s.graph.trash(id)?;
            if s.focus.is_some_and(|f| s.graph.get(f).is_none() || f == id) {
                s.focus = None;
            }
            Ok(json!({"trashed": id.0, "references_removed": refs.len()}))
        })
        .params(r#"{"node"?: id}"#),
        C::new("node.editLog", "Show edit log", "Node", |s, v| {
            // ponytail: one local editor, so the log is the node's created / edited / done times.
            let id = s.graph.resolve(s.target(v)?);
            let n = s.graph.node(id)?;
            let when = Date::local_time;
            let mut log = vec![json!({"event": "created", "at": when(n.created)})];
            if n.edited > n.created {
                log.push(json!({"event": "edited", "at": when(n.edited)}));
            }
            if let Some(d) = n.done_at {
                log.push(json!({"event": "done", "at": when(d)}));
            }
            Ok(json!({"node": id.0, "log": log}))
        })
        .params(r#"{"node"?: id}"#)
        .pure(),
        C::new("edit.pasteAsCode", "Paste as code", "Edit", |s, v| {
            let text = p::req_str(v, "text")?.replace(['\n', '\r'], " ").replace('`', "'");
            insert_text(s, v, &format!("`{text}`"))
        })
        .params(r#"{"node"?: id, "caret"?: number, "text": string}  (inserted as inline code)"#),
        C::new("node.setIcon", "Set icon", "Node", |s, v| {
            let id = s.graph.resolve(s.target(v)?);
            let icon = p::str(v, "icon").map(str::trim).filter(|i| !i.is_empty()).map(|i| i.chars().take(4).collect::<String>());
            s.graph.node(id)?;
            s.graph.set_decoration(id, Some(icon.clone()), None)?;
            Ok(json!({"node": id.0, "icon": icon}))
        })
        .params(r#"{"node"?: id, "icon": string}  (an emoji or a few characters; empty removes it)"#),
        C::new("node.setBanner", "Set banner", "Node", |s, v| {
            let id = s.graph.resolve(s.target(v)?);
            let color = p::str(v, "color").map(str::trim).filter(|c| !c.is_empty()).map(str::to_string);
            if let Some(c) = &color
                && !TAG_COLORS.contains(&c.as_str())
                && !tanacraft_core::is_custom_color(c)
            {
                return Err(CmdError::Params(format!("`color` must be one of {}, or #rrggbb", TAG_COLORS.join(", "))));
            }
            s.graph.set_decoration(id, None, Some(color.clone()))?;
            Ok(json!({"node": id.0, "banner": color}))
        })
        .params(r#"{"node"?: id, "color": string}  (a colour name or #rrggbb; empty removes it)"#),
        C::new("import.markdown", "Import Markdown", "File", import_markdown)
            .params(r#"{"text": string, "parent"?: id}  (bullets nest by indent; # headings, - [ ] / - [x] checkboxes, #tags)"#),
        C::new("node.cloneReference", "Clone reference", "Node", clone_reference)
            .key("Mod+Shift+C")
            .params(r#"{"node"?: id}  (a reference becomes an independent copy of what it points at)"#),
        C::new("node.add", "Add node", "Node", add).params(r#"{"name": string, "parent"?: id, "index"?: number}"#),
        C::new("node.setName", "Rename node", "Node", set_name).params(r#"{"node"?: id, "name": string, "caret"?: chars}"#),
        C::new("node.setDescription", "Set description", "Node", set_description).params(r#"{"node"?: id, "text": string}"#),
        C::new("node.move", "Move to", "Node", move_to).params(
            r#"{"node"?: id, "parent": id, "index"?: number, "leaveReference"?: bool}  (leaveReference: a reference stays where the node was)"#,
        ),
        C::new("node.moveToTop", "Move to top", "Node", |s, v| move_end(s, v, true)).params(r#"{"node"?: id}"#),
        C::new("node.moveToBottom", "Move to bottom", "Node", |s, v| move_end(s, v, false)).params(r#"{"node"?: id}"#),
        C::new("node.setMoveTarget", "Set move target", "Node", set_move_target)
            .params(r#"{"node"?: id}  (default: the focused node or the open page)"#),
        C::new("node.moveToTarget", "Move to move target", "Node", move_to_target)
            .params(r#"{"node"?: id}  (default: the selection or focused node)"#),
        C::new("node.moveMany", "Move several nodes", "Node", move_many)
            .params(r#"{"nodes": [id], "parent": id, "index"?: number}  (keeps their order)"#),
        C::new("node.reference", "Add reference", "Node", reference)
            .params(r#"{"target": id, "parent"?: id, "index"?: number, "replace"?: id}  (replace: an empty node the reference takes the place of)"#),
        C::new("node.restore", "Restore from trash", "Node", restore).params(r#"{"node": id}"#),
        C::new("trash.empty", "Empty trash", "Node", empty_trash),
        C::new("node.joinNext", "Join with the next node", "Node", join_next).key("Delete").params(r#"{"node"?: id}"#),
        C::new("node.paste", "Paste as nodes", "Node", paste).params(r#"{"node"?: id, "text": string}"#),
        C::new("node.toggleHeading", "Toggle heading", "Node", toggle_heading).params(r#"{"node"?: id}"#),
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
        C::new("search.addClause", "Add search condition", "Search", search_add_clause)
            .params(r#"{"node"?: id, "clause": string, "not"?: bool, "path"?: [number]}  (path: indexes of nested groups)"#),
        C::new("search.addGroup", "Add AND / OR / NOT group", "Search", search_add_group)
            .params(r#"{"node"?: id, "kind": "and|or|not", "path"?: [number]}"#),
        C::new("search.removeClause", "Remove search condition", "Search", search_remove_clause)
            .params(r#"{"node"?: id, "index": number, "path"?: [number]}"#),
        C::new("search.toggleNot", "Negate search condition", "Search", search_toggle_not)
            .params(r#"{"node"?: id, "index": number, "path"?: [number]}"#),
        C::new("search.toPlainList", "Convert search node to plain list", "Search", |s, v| {
            let id = search_target(s, v)?;
            let n = s.graph.search_to_plain(id)?;
            Ok(json!({"node": id.0, "references": n}))
        })
        .params(r#"{"node"?: id}"#),
        C::new("node.sortChildren", "Sort children", "Node", sort_children).params(r#"{"node"?: id, "by": "name|created|done", "desc"?: bool}"#),
        C::new("select.all", "Select all", "View", |s, _| {
            let rows = s.rows();
            let (Some(a), Some(b)) = (rows.first(), rows.last()) else { return Ok(json!({})) };
            s.sel = Some((a.id, b.id));
            s.focus = None;
            Ok(json!({"selected": s.selected().len()}))
        })
        .key("Mod+A")
        .pure(),
        C::new("search.setMode", "Match all or any", "Search", search_set_mode).params(r#"{"node"?: id, "any": bool, "path"?: [number]}"#),
        // Views.
        C::new("view.set", "View as", "Views", view_set)
            .params(r#"{"node"?: id, "kind": "outline|table|cards|board|lanes|list|side_menu|tabs|calendar"}"#),
        C::new("view.sort", "Sort by", "Views", view_sort)
            .params(r#"{"node"?: id, "by": "none|name|created|edited|done|done_time|<field>", "desc"?: bool}"#),
        C::new("view.addFilter", "Add filter", "Views", view_add_filter).params(r#"{"node"?: id, "clause": string}"#),
        C::new("view.removeFilter", "Remove filter", "Views", view_remove_filter).params(r#"{"node"?: id, "index": number}"#),
        C::new("view.addItem", "Add item to view", "Views", view_add_item)
            .params(r#"{"node"?: id, "name"?: string, "field"?: name | id, "value"?: string, "stay"?: bool}  (stay: do not open the new item)"#),
        C::new("node.setDone", "Set done", "Node", set_done).params(r#"{"node"?: id, "done": bool | null}"#),
        C::new("view.addSort", "Add sort rule", "Views", view_add_sort).params(r#"{"node"?: id, "by": "name|created|done|<field>", "desc"?: bool}"#),
        C::new("view.removeSort", "Remove sort rule", "Views", view_remove_sort).params(r#"{"node"?: id, "index": number}"#),
        C::new("view.toggleDisplay", "Display field", "Views", view_toggle_display).params(r#"{"node"?: id, "field": name | id}"#),
        C::new("view.toggleSystemField", "Display system field", "Views", |s, v| {
            let id = view_target(s, v)?;
            let name = p::req_str(v, "field")?;
            let f = tanacraft_core::SystemField::parse(name).ok_or_else(|| CmdError::Params(format!("unknown system field `{name}`")))?;
            let view = s.graph.view_mut(id)?;
            if view.system.contains(&f) {
                view.system.retain(|x| *x != f);
            } else {
                view.system.push(f);
            }
            view_json(s, id)
        })
        .params(r#"{"node"?: id, "field": "Created time|Last edited time|Done time|Owner node|Tags|Number of references"}"#),
        C::new("view.toggleGroupHidden", "Hide or show a group", "Views", |s, v| {
            let id = view_target(s, v)?;
            let label = p::req_str(v, "group")?.to_string();
            let view = s.graph.view_mut(id)?;
            if view.hidden_groups.contains(&label) {
                view.hidden_groups.retain(|g| *g != label);
            } else {
                view.hidden_groups.push(label);
            }
            view_json(s, id)
        })
        .params(r#"{"node"?: id, "group": string}"#),
        C::new("view.setColumnWidth", "Resize table column", "Views", |s, v| {
            let id = view_target(s, v)?;
            let col = match v.get("field") {
                Some(Value::String(x)) if x == "title" => NodeId(0),
                _ => field_param(s, v, false)?,
            };
            let w = v
                .get("width")
                .and_then(Value::as_f64)
                .filter(|w| w.is_finite())
                .ok_or_else(|| CmdError::Params("`width` (number) is required".into()))?
                .clamp(60.0, 1200.0) as f32;
            let view = s.graph.view_mut(id)?;
            view.widths.retain(|(c, _)| *c != col);
            view.widths.push((col, w));
            Ok(json!({"node": id.0, "width": w}))
        })
        .params(r#"{"node"?: id, "field": name | id | "title", "width": number}"#),
        C::new("view.toggleToolbar", "Show or hide view toolbar", "Views", view_toggle_toolbar).params(r#"{"node"?: id}"#),
        C::new("view.calendarMode", "Calendar week or month", "Views", view_calendar_mode).params(r#"{"mode": "day|week|month"}"#).pure(),
        C::new("view.group", "Group by", "Views", view_group).params(r#"{"node"?: id, "by": "none|tags|done|owner|created|edited|<field>"}"#),
        C::new("view.filter", "Filter", "Views", view_filter).params(r#"{"node"?: id, "query": string}  (empty clears)"#),
        C::new("view.addColumn", "Add column", "Views", view_add_column).params(r#"{"node"?: id, "field": name | id, "type"?: string}"#),
        C::new("view.removeColumn", "Remove column", "Views", view_remove_column).params(r#"{"node"?: id, "field": name | id}"#),
        C::new("view.dayLength", "Calendar day length", "Views", |s, v| {
            let id = view_target(s, v)?;
            let hours = match (p::usize(v, "start"), p::usize(v, "end")) {
                (Some(a), Some(b)) if a < b && b <= 24 => Some((a as u8, b as u8)),
                (None, None) => None,
                _ => return Err(CmdError::Params("`start` < `end`, hours 0 to 24 (both empty resets)".into())),
            };
            s.graph.view_mut(id)?.day_hours = hours;
            Ok(json!({"node": id.0, "hours": hours}))
        })
        .params(r#"{"node"?: id, "start"?: number, "end"?: number}  (hours; none resets to the whole day)"#),
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
        C::new("nav.calendarStep", "Previous or next week or year", "Navigate", calendar_step)
            .params(r#"{"step": -1 | 1}  (on a week or year page)"#)
            .pure(),
        C::new("nav.home", "Go to Home", "Navigate", |s, _| open_sys(s, s.graph.sys.library)).pure(),
        C::new("nav.supertags", "Go to Supertags", "Navigate", |s, _| open_sys(s, s.graph.sys.schema)).pure(),
        C::new("nav.dailyNotes", "Go to Daily notes", "Navigate", |s, _| open_sys(s, s.graph.sys.journal)).pure(),
        C::new("nav.trash", "Go to Trash", "Navigate", |s, _| open_sys(s, s.graph.sys.trash)).pure(),
        C::new("nav.inbox", "Open capture inbox", "Navigate", |s, _| {
            let i = s.graph.inbox()?;
            open_sys(s, i)
        })
        .pure(),
        C::new("node.capture", "Capture to inbox", "Node", capture).params(r#"{"name": string, "tag"?: name | id}"#),
        // Workspaces: one graph file each, side by side in one folder.
        C::new("workspace.list", "List workspaces", "Workspace", workspace_list).pure(),
        C::new("workspace.create", "New workspace", "Workspace", workspace_create).params(r#"{"name": string}"#).pure(),
        C::new("workspace.switch", "Switch workspace", "Workspace", workspace_switch)
            .params(r#"{"name": string}  (a name from workspace.list)"#)
            .pure(),
        C::new("workspace.rename", "Rename workspace", "Workspace", |s, v| {
            let name = p::req_str(v, "name")?.trim();
            if name.is_empty() {
                return Err(CmdError::Params("`name` must not be empty".into()));
            }
            let root = s.graph.sys.root;
            s.graph.set_name(root, name)?;
            Ok(json!({"name": name}))
        })
        .params(r#"{"name": string}"#),
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
        C::new("tag.setDoneMap", "Map checkbox to field", "Supertag", tag_set_done_map)
            .params(r#"{"tag": name | id, "field"?: name | id, "checked"?: string, "unchecked"?: string}  (no field clears)"#),
        C::new("tag.setOptionalField", "Optional field", "Supertag", |s, v| tag_field_flag(s, v, true))
            .params(r#"{"tag": name | id, "field": name | id, "on"?: bool}  (offered as + Field on tagged nodes)"#),
        C::new("tag.setPinnedField", "Pinned field", "Supertag", |s, v| tag_field_flag(s, v, false))
            .params(r#"{"tag": name | id, "field": name | id, "on"?: bool}"#),
        C::new("tag.setTitleExpression", "Build title from fields", "Supertag", tag_set_title_expr)
            .params(r#"{"tag": name | id, "expression": string}  (starts with ${name}; ${Field} inserts a field; empty clears)"#),
        C::new("tag.addRelated", "Add related content", "Supertag", tag_add_related)
            .params(r#"{"tag": name | id, "query": string, "name"?: string}"#),
        C::new("tag.removeRelated", "Remove related content", "Supertag", tag_remove_related).params(r#"{"tag": name | id, "node": id}"#),
        C::new("tag.setChildTag", "Default child supertag", "Supertag", tag_set_child_tag).params(r#"{"tag": name | id, "child"?: name | id}"#),
        C::new("tag.setBaseType", "Base type", "Supertag", tag_set_base_type).params(r#"{"tag": name | id, "base"?: "task|meeting|person|..."}"#),
        C::new("tag.addTemplateNode", "Add template content", "Supertag", tag_add_template_node).params(r#"{"tag": name | id, "name": string}"#),
        C::new("tag.removeField", "Remove field from supertag", "Supertag", tag_remove_field).params(r#"{"tag": name | id, "field": name | id}"#),
        C::new("tag.addView", "Add view to supertag", "Supertag", tag_add_view)
            .params(r#"{"tag": name | id, "kind": "outline|table|cards|calendar|..."}"#),
        C::new("node.toggleProgress", "Show progress bar", "Node", |s, v| {
            let id = s.graph.resolve(s.target(v).unwrap_or(s.zoom));
            let on = !s.graph.node(id)?.progress;
            s.graph.set_progress(id, on)?;
            Ok(json!({"node": id.0, "progress": on}))
        })
        .params(r#"{"node"?: id}"#),
        C::new("node.insertChecklist", "Checklist", "Node", |s, v| {
            // An empty line becomes "New checklist" with a progress bar and two checkbox items.
            let id = s.target(v)?;
            let target = if s.graph.name(id).is_empty() {
                s.graph.set_name(id, "New checklist")?;
                id
            } else {
                let parent = s.graph.parent(id).ok_or_else(|| CmdError::Failed("no parent".into()))?;
                let at = s.graph.index_in_parent(id).map(|i| i + 1);
                s.graph.create(parent, at, "New checklist")?
            };
            s.graph.set_progress(target, true)?;
            for name in ["First item", "Second item"] {
                let c = s.graph.create(target, None, name)?;
                s.graph.set_done(c, Some(false))?;
            }
            s.graph.set_expanded(target, true);
            // The caret goes to the start of "New checklist" (spec 02 §11, DIFFS P2 #13).
            s.set_focus(target, 0);
            Ok(json!({"node": target.0}))
        })
        .params(r#"{"node"?: id}"#),
        C::new("node.togglePin", "Pin or unpin in sidebar", "Node", toggle_pin).params(r#"{"node"?: id}"#),
        // Fields.
        C::new("field.setType", "Field type", "Field", field_set_type).params(r#"{"field": name | id, "type": string}"#),
        C::new("field.setRequired", "Required field", "Field", field_set_required).params(r#"{"field": name | id, "on": bool}"#),
        C::new("field.setHide", "Hide field", "Field", field_set_hide)
            .params(r#"{"field": name | id, "mode": "never|when_empty|when_not_empty|when_default|always"}"#),
        C::new("field.setRange", "Number range", "Field", field_set_range).params(r#"{"field": name | id, "min"?: number, "max"?: number}"#),
        C::new("field.setAutoInit", "Auto-initialize", "Field", field_set_auto_init)
            .params(r#"{"field": name | id, "mode": "none|current_date|ancestor_day|ancestor_value"}"#),
        C::new("field.setDefault", "Default value", "Field", field_set_default).params(r#"{"field": name | id, "value": string}  (empty clears)"#),
        C::new("field.setSourceTag", "Options from supertag", "Field", field_set_source_tag).params(r#"{"field": name | id, "tag": name | id}"#),
        C::new("field.addOption", "Add option", "Field", field_add_option).params(r#"{"field": name | id, "option": string}"#),
        C::new("field.removeOption", "Remove option", "Field", field_remove_option).params(r#"{"field": name | id, "option": string}"#),
        C::new("field.addSource", "Add source of options", "Field", |s, v| field_source(s, v, true))
            .params(r#"{"field": name | id, "node": id}  (the node's children become options)"#),
        C::new("field.removeSource", "Remove source of options", "Field", |s, v| field_source(s, v, false))
            .params(r#"{"field": name | id, "node": id}"#),
        C::new("field.setAutoCollect", "Auto-collect values", "Field", |s, v| {
            let f = field_param(s, v, false)?;
            let on = p::bool(v, "on").ok_or_else(|| CmdError::Params("`on` (bool) is required".into()))?;
            s.graph.field_def_mut(f)?.no_auto_collect = !on;
            Ok(json!({"field": f.0, "on": on}))
        })
        .params(r#"{"field": name | id, "on": bool}"#),
        C::new("field.add", "Add field", "Field", field_add)
            .key(">")
            .params(r#"{"node"?: id, "field": name | id, "type"?: string, "value"?: string}"#),
        C::new("field.set", "Set field value", "Field", field_set)
            .params(r#"{"node"?: id, "field": name | id, "value": string, "index"?: number}  (index picks one of several values; empty removes it)"#),
        C::new("field.addValue", "Add field value", "Field", field_add_value).params(r#"{"node"?: id, "field": name | id, "value": string}"#),
        C::new("field.valueToNode", "Move value out of field", "Field", field_value_to_node)
            .params(r#"{"node"?: id, "field": name | id, "index": number, "text"?: string}  (text: the value being typed, if any)"#),
        C::new("field.removeValue", "Remove field value", "Field", field_remove_value)
            .params(r#"{"node"?: id, "field": name | id, "index": number}"#),
        C::new("field.remove", "Remove field", "Field", field_remove).params(r#"{"node"?: id, "field": name | id}"#),
        C::new("field.create", "Create field", "Field", field_create)
            .params(r#"{"name": string, "type"?: "plain|options|options_from_tag|date|number|url|email|checkbox|user"}"#),
        C::new("field.setOptions", "Set field options", "Field", field_set_options).params(r#"{"field": name | id, "options": [string]}"#),
        // Edit and file.
        C::new("edit.undo", "Undo", "Edit", |s, _| Ok(json!({"undone": s.graph.undo()}))).key("Mod+Z").pure(),
        C::new("edit.redo", "Redo", "Edit", |s, _| Ok(json!({"redone": s.graph.redo()}))).key("Mod+Shift+Z").pure(),
        C::new("file.save", "Save", "File", save).params(r#"{"path"?: string}"#).pure(),
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
        // An empty node above; the caret goes into it (spec 02 §2).
        let new = s.graph.create(parent, Some(at), "")?;
        s.set_focus(new, 0);
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
    let node = s.graph.node(id)?.clone();
    let is_ref = node.reference_target().is_some();
    // An empty line (or a reference) goes away; the caret lands at the end of the line above.
    if is_ref || (node.name.is_empty() && node.children.is_empty() && node.fields.is_empty()) {
        let prev = row_neighbour(s, id, -1).ok_or_else(|| CmdError::Failed("nothing above".into()))?;
        s.graph.delete_permanently(id)?;
        let end = len(s, prev);
        s.set_focus(prev, end);
        return Ok(s.focus_json());
    }
    // A non-empty leaf merges into its previous sibling (not the visual row above). Nodes with
    // children, supertags or fields stay as they are (spec 02 §4).
    if !node.children.is_empty() || !node.tags.is_empty() || !node.fields.is_empty() {
        return Ok(s.focus_json());
    }
    let parent = s.graph.parent(id).ok_or_else(|| CmdError::Failed("no parent".into()))?;
    let at = s.graph.index_in_parent(id).unwrap_or(0);
    let Some(prev) = at.checked_sub(1).and_then(|i| s.graph.children(parent).get(i).copied()) else {
        return Ok(s.focus_json());
    };
    if s.graph.get(prev).is_some_and(|p| p.reference_target().is_some()) {
        return Ok(s.focus_json());
    }
    let caret = s.graph.merge_into(id, prev)?;
    s.set_focus(prev, caret);
    Ok(s.focus_json())
}

/// Delete at the end of a line pulls the next sibling's text (and children) into this node.
fn join_next(s: &mut Session, v: &Value) -> CmdResult {
    let id = s.target(v)?;
    let parent = s.graph.parent(id).ok_or_else(|| CmdError::Failed("no parent".into()))?;
    let at = s.graph.index_in_parent(id).unwrap_or(0);
    let Some(next) = s.graph.children(parent).get(at + 1).copied() else { return Ok(s.focus_json()) };
    let nn = s.graph.node(next)?;
    if nn.reference_target().is_some()
        || !nn.tags.is_empty()
        || !nn.fields.is_empty()
        || s.graph.get(id).is_some_and(|n| n.reference_target().is_some())
    {
        return Ok(s.focus_json());
    }
    let caret = s.graph.merge_into(next, id)?;
    s.set_focus(id, caret);
    Ok(s.focus_json())
}

/// Pastes text: Markdown bullets ("- a", "  - b") nest; other lines become siblings. Into an empty
/// focused node the first line goes into that node.
/// Builds nodes from Markdown under `parent` (default: the Library): bullet lists nest by indent,
/// `#` headings become heading nodes that hold what follows, `[ ]` / `[x]` make checkboxes and
/// trailing `#tags` become supertags.
fn import_markdown(s: &mut Session, v: &Value) -> CmdResult {
    let text = p::req_str(v, "text")?;
    let root = p::node(v, "parent")?.unwrap_or(s.graph.sys.library);
    s.graph.node(root)?;
    // (indent, node): open bullets; `section` is the current heading's node.
    let mut stack: Vec<(usize, NodeId)> = Vec::new();
    let mut section = root;
    let mut made = 0usize;
    for line in text.lines().take(50_000) {
        if line.trim().is_empty() {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let t = line.trim();
        if let Some(h) = t.strip_prefix('#').filter(|_| t.starts_with("# ") || t.starts_with("## ") || t.starts_with("### ")) {
            let name = h.trim_start_matches('#').trim();
            let n = s.graph.create(root, None, name)?;
            s.graph.set_heading(n, true)?;
            section = n;
            stack.clear();
            made += 1;
            continue;
        }
        let bullet = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")).or_else(|| t.strip_prefix("+ "));
        let body = bullet.unwrap_or(t);
        while stack.last().is_some_and(|(w, _)| *w >= indent) {
            stack.pop();
        }
        let parent = stack.last().map(|(_, p)| *p).unwrap_or(section);
        let (done, body) = match body {
            b if b.starts_with("[ ] ") => (Some(false), &b[4..]),
            b if b.starts_with("[x] ") || b.starts_with("[X] ") => (Some(true), &b[4..]),
            b => (None, b),
        };
        // Trailing `#tag` words become supertags (made when missing).
        let mut words: Vec<&str> = body.split(' ').collect();
        let mut tags = Vec::new();
        while words.last().is_some_and(|w| w.len() > 1 && w.starts_with('#') && !w[1..].contains('#')) {
            tags.push(words.pop().unwrap_or_default().trim_start_matches('#').to_string());
        }
        let n = s.graph.create(parent, None, words.join(" ").trim())?;
        for tag in tags.into_iter().rev() {
            let t = match s.graph.find_tag(&tag) {
                Some(t) => t,
                None => s.graph.create_tag(&tag, "grey")?,
            };
            s.graph.add_tag(n, t)?;
        }
        if done.is_some() {
            s.graph.set_done(n, done)?;
        }
        s.graph.set_expanded(parent, true);
        stack.push((indent, n));
        made += 1;
    }
    Ok(json!({"parent": root.0, "created": made}))
}

fn paste(s: &mut Session, v: &Value) -> CmdResult {
    let text = p::req_str(v, "text")?;
    let id = s.target(v)?;
    let parent = s.graph.parent(id).ok_or_else(|| CmdError::Failed("no parent".into()))?;
    let mut at = s.graph.index_in_parent(id).unwrap_or(0);
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).take(10_000).collect();
    let bulleted = lines.iter().any(|l| {
        let t = l.trim_start();
        t.starts_with("- ") || t.starts_with("* ")
    });
    // (indent width, node) of the last node created at each depth.
    let mut stack: Vec<(usize, NodeId)> = Vec::new();
    let mut last = id;
    let mut first = true;
    for line in lines {
        let indent = if bulleted { line.len() - line.trim_start().len() } else { 0 };
        let mut t = line.trim();
        if bulleted {
            t = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")).unwrap_or(t);
        }
        while stack.last().is_some_and(|(w, _)| *w >= indent) {
            stack.pop();
        }
        let node = if first && s.graph.name(id).is_empty() && s.graph.children(id).is_empty() {
            s.graph.set_name(id, t)?;
            id
        } else if let Some((_, p)) = stack.last() {
            s.graph.set_expanded(*p, true);
            s.graph.create(*p, None, t)?
        } else {
            at += 1;
            s.graph.create(parent, Some(at), t)?
        };
        first = false;
        stack.push((indent, node));
        last = node;
    }
    s.set_focus(last, usize::MAX);
    Ok(json!({"last": last.0}))
}

fn toggle_heading(s: &mut Session, v: &Value) -> CmdResult {
    let id = s.graph.resolve(s.target(v)?);
    let on = !s.graph.node(id)?.heading;
    s.graph.set_heading(id, on)?;
    Ok(json!({"node": id.0, "heading": on}))
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
    let id = ids[0];
    let was_selected = s.sel.is_some();
    // The caret goes to the start of the next line (outside the deleted subtree), else to the end of
    // the previous one (spec 02 §4, §8).
    let rows = s.rows();
    let at = rows.iter().position(|r| r.id == id);
    let next = at.and_then(|i| rows[i + 1..].iter().find(|r| !s.graph.is_ancestor_or_self(id, r.id))).map(|r| (r.id, 0));
    let prev = row_neighbour(s, id, -1).map(|p| (p, len(s, p)));
    s.graph.trash(id)?;
    s.sel = None;
    if s.focus == Some(id) || was_selected {
        match next.or(prev) {
            Some((f, caret)) => s.set_focus(f, caret),
            None => s.focus = None,
        }
    }
    Ok(json!({"trashed": id.0}))
}

/// Inserts `text` into a node's name at the caret (default: the focused node's caret, else the end).
fn insert_text(s: &mut Session, v: &Value, text: &str) -> CmdResult {
    let id = s.target(v)?;
    let name: Vec<char> = s.graph.name(id).chars().collect();
    let at = p::usize(v, "caret").or((s.focus == Some(id)).then_some(s.caret)).unwrap_or(name.len()).min(name.len());
    let new: String = name[..at].iter().copied().chain(text.chars()).chain(name[at..].iter().copied()).collect();
    s.graph.set_name(id, &new)?;
    if s.focus == Some(id) {
        s.set_focus(id, at + text.chars().count());
    }
    Ok(json!({"node": id.0, "name": new}))
}

fn clone_reference(s: &mut Session, v: &Value) -> CmdResult {
    let id = s.target(v)?;
    let target = s.graph.node(id)?.reference_target().ok_or_else(|| CmdError::Failed("not a reference".into()))?;
    let parent = s.graph.parent(id).ok_or_else(|| CmdError::Failed("no parent".into()))?;
    let at = s.graph.index_in_parent(id).unwrap_or(0);
    // The copy goes where the reference was; the reference goes away.
    let copy = s.graph.copy_into(target, parent, at)?;
    s.graph.delete_permanently(id)?;
    if s.focus == Some(id) {
        s.set_focus(copy, 0);
    }
    Ok(json!({"id": copy.0}))
}

fn duplicate(s: &mut Session, v: &Value) -> CmdResult {
    let id = s.target(v)?;
    let copy = s.graph.duplicate(id)?;
    // The caret stays in the original (spec 02 §15).
    Ok(json!({"id": copy.0}))
}

/// "Create new" (spec 06 §8): an untitled node at the end of today's page, opened as a page.
fn create_new(s: &mut Session, _: &Value) -> CmdResult {
    let day = s.graph.day(s.today)?;
    let id = s.graph.create(day, None, "")?;
    s.open(id)?;
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
    let from = (s.graph.parent(id), s.graph.index_in_parent(id));
    s.graph.move_to(id, parent, p::usize(v, "index"))?;
    // "Move and leave reference": the old spot keeps a reference to the moved node.
    if p::bool(v, "leaveReference") == Some(true)
        && let (Some(old), index) = from
    {
        s.graph.create_reference(old, index, id)?;
    }
    Ok(json!({"node": id.0, "parent": s.graph.parent(id).map(|x| x.0)}))
}

fn move_end(s: &mut Session, v: &Value, top: bool) -> CmdResult {
    let id = s.target(v)?;
    let parent = s.graph.parent(id).ok_or_else(|| CmdError::Failed("this node has no parent".into()))?;
    s.graph.move_to(id, parent, if top { Some(0) } else { None })?;
    Ok(json!({"node": id.0, "index": s.graph.index_in_parent(id)}))
}

fn set_move_target(s: &mut Session, v: &Value) -> CmdResult {
    let id = match p::node(v, "node")? {
        Some(n) => n,
        None => s.focus.unwrap_or(s.zoom),
    };
    s.graph.node(id)?;
    s.move_target = Some(id);
    Ok(json!({"target": id.0, "name": s.graph.name(id)}))
}

fn move_to_target(s: &mut Session, v: &Value) -> CmdResult {
    let dest =
        s.move_target.filter(|t| s.graph.get(*t).is_some()).ok_or_else(|| CmdError::Failed("no move target: use Set move target first".into()))?;
    let ids = s.targets(v)?;
    for id in &ids {
        s.graph.move_to(*id, dest, None)?;
    }
    Ok(json!({"moved": ids.iter().map(|n| n.0).collect::<Vec<_>>(), "target": dest.0}))
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
    // The first press selects the current node; later presses extend (spec 02 §5).
    let (anchor, head) = match (s.sel, s.focus) {
        (Some((a, h)), _) => (a, row_neighbour(s, h, step).unwrap_or(h)),
        (None, Some(f)) => (f, f),
        (None, None) => return Err(CmdError::Failed("focus a bullet first".into())),
    };
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

/// On a week page, the previous / next week; on a year page, the previous / next year.
fn calendar_step(s: &mut Session, v: &Value) -> CmdResult {
    let step = v.get("step").and_then(Value::as_i64).unwrap_or(1).clamp(-520, 520);
    let target = match s.graph.get(s.zoom).map(|n| n.kind.clone()) {
        Some(tanacraft_core::Kind::Week { year, week }) => {
            // Monday of the ISO week: Jan 4 is always in week 1.
            let jan4 = Date::new(year, 1, 4).ok_or_else(|| CmdError::Failed("bad week".into()))?;
            let monday = jan4.add_days(-(jan4.weekday() as i64) + (week as i64 - 1) * 7 + step * 7);
            let day = s.graph.day(monday)?;
            s.graph.parent(day)
        }
        Some(tanacraft_core::Kind::Year { year }) => {
            let jan4 = Date::new(year.saturating_add(step as i32), 1, 4).ok_or_else(|| CmdError::Failed("bad year".into()))?;
            let day = s.graph.day(jan4)?;
            s.graph.parent(day).and_then(|w| s.graph.parent(w))
        }
        _ => return Err(CmdError::Failed("open a week or a year first".into())),
    };
    let t = target.ok_or_else(|| CmdError::Failed("no calendar node".into()))?;
    s.open(t)?;
    Ok(json!({"zoom": t.0, "name": s.graph.name(t)}))
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
        let name = s.graph.name(*id).to_string();
        if name.trim_end() != name {
            s.graph.set_name(*id, name.trim_end())?;
        }
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
    if !TAG_COLORS.contains(&color) && !tanacraft_core::is_custom_color(color) {
        return Err(CmdError::Params(format!("`color` must be one of {}, or #rrggbb", TAG_COLORS.join(", "))));
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

fn field_source(s: &mut Session, v: &Value, add: bool) -> CmdResult {
    let f = field_param(s, v, false)?;
    let node = p::node(v, "node")?.ok_or_else(|| CmdError::Params("`node` is required".into()))?;
    s.graph.node(node)?;
    let d = s.graph.field_def_mut(f)?;
    d.sources.retain(|x| *x != node);
    if add {
        d.sources.push(node);
    }
    Ok(json!({"field": f.0, "options": s.graph.field_options(f)}))
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
    s.graph.set_field_text_at(id, field, p::usize(v, "index").unwrap_or(0), p::req_str(v, "value")?)?;
    Ok(field_json(s, id, field))
}

fn field_json(s: &Session, id: NodeId, field: NodeId) -> Value {
    let n = s.graph.get(s.graph.resolve(id));
    let value = n.and_then(|n| n.field_value(field)).map(|x| export::display_value(&s.graph, x));
    let values: Vec<String> = n.map(|n| n.field_values(field).into_iter().map(|x| export::display_value(&s.graph, x)).collect()).unwrap_or_default();
    json!({"node": id.0, "field": field.0, "value": value, "values": values})
}

fn field_add_value(s: &mut Session, v: &Value) -> CmdResult {
    let id = s.target(v)?;
    let field = field_param(s, v, true)?;
    let count = s.graph.get(s.graph.resolve(id)).map(|n| n.field_values(field).len()).unwrap_or(0);
    s.graph.set_field_text_at(id, field, count, p::req_str(v, "value")?)?;
    Ok(field_json(s, id, field))
}

/// Takes value `index` out of the field and makes it the node's first child (Shift+Tab on a value
/// line, spec 02 §3).
fn field_value_to_node(s: &mut Session, v: &Value) -> CmdResult {
    let id = s.target(v)?;
    let field = field_param(s, v, false)?;
    let index = p::usize(v, "index").unwrap_or(0);
    let owner = s.graph.resolve(id);
    let stored = s.graph.get(owner).and_then(|n| n.field_values(field).get(index).map(|x| export::display_value(&s.graph, x)));
    let text = p::str(v, "text").map(str::to_string).or(stored.clone()).unwrap_or_default();
    if stored.is_some() {
        s.graph.set_field_at(owner, field, index, None)?;
    }
    if text.trim().is_empty() {
        return Ok(field_json(s, id, field));
    }
    let child = s.graph.create(owner, Some(0), &text)?;
    s.set_focus(child, text.chars().count());
    let mut out = field_json(s, id, field);
    out["child"] = json!(child.0);
    Ok(out)
}

fn field_remove_value(s: &mut Session, v: &Value) -> CmdResult {
    let id = s.target(v)?;
    let field = field_param(s, v, false)?;
    let index = p::usize(v, "index").ok_or_else(|| CmdError::Params("`index` (number) is required".into()))?;
    s.graph.set_field_at(id, field, index, None)?;
    Ok(field_json(s, id, field))
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

fn capture(s: &mut Session, v: &Value) -> CmdResult {
    let inbox = s.graph.inbox()?;
    let id = s.graph.create(inbox, None, p::req_str(v, "name")?)?;
    if p::str(v, "tag").is_some() {
        let tag = tag_param(s, v, "tag", true)?;
        s.graph.add_tag(id, tag)?;
    }
    Ok(json!({"id": id.0, "inbox": inbox.0}))
}

/// The folder that holds the workspaces: the current file's, else the default one.
fn workspace_dir(s: &Session) -> std::path::PathBuf {
    let p = s.path.clone().unwrap_or_else(tanacraft_core::default_graph_path);
    p.parent().map(std::path::Path::to_path_buf).unwrap_or_default()
}

fn workspace_files(s: &Session) -> Vec<std::path::PathBuf> {
    let mut v: Vec<std::path::PathBuf> = std::fs::read_dir(workspace_dir(s))
        .map(|d| d.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect())
        .unwrap_or_default();
    v.sort();
    v.truncate(200);
    v
}

fn file_name(p: &std::path::Path) -> String {
    p.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_default()
}

fn workspace_list(s: &mut Session, _: &Value) -> CmdResult {
    let current = s.path.clone();
    let list: Vec<Value> = workspace_files(s)
        .iter()
        .map(|p| json!({"name": file_name(p), "path": p.display().to_string(), "current": Some(p) == current.as_ref()}))
        .collect();
    Ok(json!({"workspaces": list, "current": s.graph.name(s.graph.sys.root)}))
}

/// Saves the open workspace, then makes `graph` (stored at `path`) the open one.
fn switch_to(s: &mut Session, graph: tanacraft_core::Graph, path: std::path::PathBuf) -> CmdResult {
    if s.path.is_some() && s.graph.is_dirty() {
        let p = s.path.clone().unwrap_or_default();
        s.graph.save(&p)?;
    }
    s.graph = graph;
    s.path = Some(path.clone());
    s.zoom = s.graph.sys.library;
    s.focus = None;
    s.sel = None;
    s.recents.clear();
    s.move_target = None;
    let day = s.graph.day(s.today)?;
    s.open(day)?;
    Ok(json!({"workspace": s.graph.name(s.graph.sys.root), "path": path.display().to_string()}))
}

fn workspace_create(s: &mut Session, v: &Value) -> CmdResult {
    let name = p::req_str(v, "name")?.trim().to_string();
    let slug: String = name.chars().map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        return Err(CmdError::Params("`name` needs a letter or digit".into()));
    }
    let path = workspace_dir(s).join(format!("{slug}.json"));
    if path.exists() {
        return Err(CmdError::Failed(format!("a workspace file `{slug}.json` already exists")));
    }
    let mut g = tanacraft_core::Graph::new();
    let root = g.sys.root;
    g.set_name(root, &name)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| CmdError::Failed(e.to_string()))?;
    }
    g.save(&path)?;
    switch_to(s, g, path)
}

fn workspace_switch(s: &mut Session, v: &Value) -> CmdResult {
    let name = p::req_str(v, "name")?;
    let path = workspace_files(s).into_iter().find(|p| file_name(p) == name).ok_or_else(|| CmdError::Failed(format!("no workspace `{name}`")))?;
    if s.path.as_ref() == Some(&path) {
        return Ok(json!({"workspace": s.graph.name(s.graph.sys.root)}));
    }
    let g = tanacraft_core::Graph::load(&path)?;
    switch_to(s, g, path)
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
    let (text, a2, b2) = if focused && a == b && p::usize(v, "start").is_none() {
        let (t, c) = tanacraft_core::format::toggle_at(s.graph.name(id), a, mark);
        (t, c, c)
    } else {
        tanacraft_core::format::toggle(s.graph.name(id), a, b, mark)
    };
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
    let mut parent = p::node(v, "parent")?.unwrap_or(s.zoom);
    let mut index = p::usize(v, "index");
    // `replace`: an empty line that becomes the search node (slash menu).
    if let Some(r) = p::node(v, "replace")? {
        let n = s.graph.node(r)?;
        if !n.name.is_empty() || !n.children.is_empty() || s.graph.is_system(r) {
            return Err(CmdError::Failed("only an empty node can become a search node".into()));
        }
        parent = s.graph.parent(r).ok_or_else(|| CmdError::Failed("no parent".into()))?;
        index = s.graph.index_in_parent(r);
        s.graph.delete_permanently(r)?;
    }
    let name = p::str(v, "name").map(str::to_string).unwrap_or_else(|| q.describe(&s.graph));
    let id = s.graph.create_search(parent, index, &name, q)?;
    s.graph.set_expanded(id, true);
    if p::node(v, "replace")?.is_some() {
        s.set_focus(id, usize::MAX);
    }
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

/// The group a clause command acts on: `path` lists clause indexes of nested groups from the top
/// (empty or absent: the top level).
fn group_at<'a>(q: &'a mut Query, v: &Value) -> Result<&'a mut Query, CmdError> {
    let path: Vec<usize> = match v.get("path") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(a)) => a.iter().take(32).map(|x| x.as_u64().map(|n| n as usize)).collect::<Option<_>>().unwrap_or_default(),
        Some(_) => return Err(CmdError::Params("`path` must be an array of condition indexes".into())),
    };
    let mut cur = q;
    for i in path {
        cur = match cur.clauses.get_mut(i).map(|c| &mut c.kind) {
            Some(ClauseKind::Group { query }) => query,
            _ => return Err(CmdError::Params("`path` must lead to a group".into())),
        };
    }
    Ok(cur)
}

fn search_add_clause(s: &mut Session, v: &Value) -> CmdResult {
    let parsed = parse_query(s, p::req_str(v, "clause")?)?;
    let not = p::bool(v, "not").unwrap_or(false);
    edit_query(s, v, |q| {
        group_at(q, v)?.clauses.extend(parsed.clauses.into_iter().map(|c| Clause { not: c.not != not, kind: c.kind }));
        Ok(())
    })
}

/// Adds an AND, OR or NOT group (spec 05 §2): a NOT group negates what it holds.
fn search_add_group(s: &mut Session, v: &Value) -> CmdResult {
    let kind = p::str(v, "kind").unwrap_or("and").to_ascii_lowercase();
    if !matches!(kind.as_str(), "and" | "or" | "not") {
        return Err(CmdError::Params("`kind` is and, or or not".into()));
    }
    edit_query(s, v, |q| {
        let g = group_at(q, v)?;
        g.clauses.push(Clause { not: kind == "not", kind: ClauseKind::Group { query: Query { any: kind == "or", clauses: Vec::new() } } });
        Ok(())
    })
}

fn clause_index(v: &Value, q: &Query) -> Result<usize, CmdError> {
    p::usize(v, "index").filter(|i| *i < q.clauses.len()).ok_or_else(|| CmdError::Params("`index` must name an existing condition".into()))
}

fn search_remove_clause(s: &mut Session, v: &Value) -> CmdResult {
    edit_query(s, v, |q| {
        let g = group_at(q, v)?;
        let i = clause_index(v, g)?;
        g.clauses.remove(i);
        Ok(())
    })
}

fn search_toggle_not(s: &mut Session, v: &Value) -> CmdResult {
    edit_query(s, v, |q| {
        let g = group_at(q, v)?;
        let i = clause_index(v, g)?;
        if let Some(c) = g.clauses.get_mut(i) {
            c.not = !c.not;
        }
        Ok(())
    })
}

fn search_set_mode(s: &mut Session, v: &Value) -> CmdResult {
    let any = p::bool(v, "any").ok_or_else(|| CmdError::Params("`any` (bool) is required".into()))?;
    edit_query(s, v, |q| {
        group_at(q, v)?.any = any;
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
    let board = kind.trim().eq_ignore_ascii_case("board");
    let kind = ViewKind::parse(kind).ok_or_else(|| CmdError::Params(format!("unknown view `{kind}`")))?;
    let needs_group = board && s.graph.view_def(id).group.is_none();
    // A board is cards grouped into columns: default to the first options field, else done.
    let default_group = needs_group.then(|| {
        s.graph
            .view_columns(id)
            .into_iter()
            .find(|f| s.graph.field_def(*f).is_some_and(|d| d.field_type == FieldType::Options))
            .map(GroupBy::Field)
            .unwrap_or(GroupBy::Done)
    });
    let current = s.graph.view_def(id);
    let view = s.graph.view_mut(id)?;
    // Keep the settings a supertag page had by default (table) when switching.
    if view.kind != current.kind {
        view.kind = current.kind;
    }
    view.kind = kind;
    if let Some(g) = default_group {
        view.group = Some(g);
    }
    view_json(s, id)
}

fn field_or_keyword(s: &mut Session, v: &Value, key: &str) -> Result<Result<NodeId, String>, CmdError> {
    let raw = p::req_str(v, key)?.to_string();
    let lower = raw.trim().to_ascii_lowercase();
    if ["none", "name", "created", "done", "tags", "edited", "done_time", "owner"].contains(&lower.as_str()) {
        return Ok(Err(lower));
    }
    let probe = json!({"field": raw});
    Ok(Ok(field_param(s, &probe, false)?))
}

fn sort_by(s: &mut Session, v: &Value) -> Result<Option<SortBy>, CmdError> {
    Ok(match field_or_keyword(s, v, "by")? {
        Ok(f) => Some(SortBy::Field(f)),
        Err(k) => match k.as_str() {
            "none" => None,
            "name" => Some(SortBy::Name),
            "created" => Some(SortBy::Created),
            "done" => Some(SortBy::Done),
            "edited" => Some(SortBy::Edited),
            "done_time" => Some(SortBy::DoneTime),
            other => return Err(CmdError::Params(format!("cannot sort by `{other}`"))),
        },
    })
}

/// Replaces the sort rules with one rule (`by: "none"` clears them).
fn view_sort(s: &mut Session, v: &Value) -> CmdResult {
    let id = view_target(s, v)?;
    let desc = p::bool(v, "desc").unwrap_or(false);
    let by = sort_by(s, v)?;
    s.graph.view_mut(id)?.sort = by.map(|by| vec![Sort { by, desc }]).unwrap_or_default();
    view_json(s, id)
}

/// Adds a sort rule after the existing ones.
fn view_add_sort(s: &mut Session, v: &Value) -> CmdResult {
    let id = view_target(s, v)?;
    let desc = p::bool(v, "desc").unwrap_or(false);
    let by = sort_by(s, v)?.ok_or_else(|| CmdError::Params("`by` must name a field or name/created/done".into()))?;
    let view = s.graph.view_mut(id)?;
    view.sort.retain(|r| r.by != by);
    view.sort.push(Sort { by, desc });
    view_json(s, id)
}

fn view_remove_sort(s: &mut Session, v: &Value) -> CmdResult {
    let id = view_target(s, v)?;
    let i = p::usize(v, "index").ok_or_else(|| CmdError::Params("`index` is required".into()))?;
    let view = s.graph.view_mut(id)?;
    if i >= view.sort.len() {
        return Err(CmdError::Params("no such sort rule".into()));
    }
    view.sort.remove(i);
    view_json(s, id)
}

fn view_toggle_display(s: &mut Session, v: &Value) -> CmdResult {
    let id = view_target(s, v)?;
    let f = field_param(s, v, false)?;
    let view = s.graph.view_mut(id)?;
    if view.columns.contains(&f) {
        view.columns.retain(|c| *c != f);
    } else {
        view.columns.push(f);
    }
    view_json(s, id)
}

fn view_toggle_toolbar(s: &mut Session, v: &Value) -> CmdResult {
    let id = view_target(s, v)?;
    let view = s.graph.view_mut(id)?;
    view.hide_toolbar = !view.hide_toolbar;
    let hidden = view.hide_toolbar;
    Ok(json!({"node": id.0, "toolbar": !hidden}))
}

fn view_calendar_mode(s: &mut Session, v: &Value) -> CmdResult {
    let mode = p::req_str(v, "mode")?;
    s.calendar_mode = match mode {
        "month" => 0,
        "week" => 1,
        "day" => 2,
        other => return Err(CmdError::Params(format!("mode must be day, week or month, not `{other}`"))),
    };
    Ok(json!({"mode": mode}))
}

fn view_group(s: &mut Session, v: &Value) -> CmdResult {
    let id = view_target(s, v)?;
    let by = match field_or_keyword(s, v, "by")? {
        Ok(f) => Some(GroupBy::Field(f)),
        Err(k) => match k.as_str() {
            "none" => None,
            "tags" => Some(GroupBy::Tags),
            "done" => Some(GroupBy::Done),
            "owner" => Some(GroupBy::Owner),
            "created" => Some(GroupBy::Created),
            "edited" => Some(GroupBy::Edited),
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
    // On a supertag's page a new column joins the supertag's template (spec 03 §9).
    if s.graph.tag_def(id).is_some() {
        s.graph.tag_add_field(id, f)?;
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

fn view_add_filter(s: &mut Session, v: &Value) -> CmdResult {
    let id = view_target(s, v)?;
    let parsed = parse_query(s, p::req_str(v, "clause")?)?;
    let view = s.graph.view_mut(id)?;
    if view.filter.any && !parsed.clauses.is_empty() {
        // Keep filters an AND list: wrap the existing OR group.
        let old = std::mem::take(&mut view.filter);
        view.filter.clauses.push(Clause { not: false, kind: tanacraft_core::query::ClauseKind::Group { query: old } });
    }
    for c in parsed.clauses {
        if !view.filter.clauses.contains(&c) {
            view.filter.clauses.push(c);
        }
    }
    view_json(s, id)
}

fn view_remove_filter(s: &mut Session, v: &Value) -> CmdResult {
    let id = view_target(s, v)?;
    let i = p::usize(v, "index").ok_or_else(|| CmdError::Params("`index` is required".into()))?;
    let view = s.graph.view_mut(id)?;
    if i >= view.filter.clauses.len() {
        return Err(CmdError::Params("no such filter".into()));
    }
    view.filter.clauses.remove(i);
    view_json(s, id)
}

fn set_done(s: &mut Session, v: &Value) -> CmdResult {
    let id = s.target(v)?;
    let done = match v.get("done") {
        Some(Value::Bool(b)) => Some(*b),
        Some(Value::Null) => None,
        _ => return Err(CmdError::Params("`done` must be true, false or null".into())),
    };
    s.graph.set_done(id, done)?;
    Ok(json!({"node": id.0, "done": done}))
}

/// A new item that fits a view (spec 05 §1, 03 §2): a child of a plain node; for a search, a node
/// in the Library carrying the query's supertags and field values; for a supertag page, a node with
/// that supertag on today's page. The new node opens with the caret in its title.
fn view_add_item(s: &mut Session, v: &Value) -> CmdResult {
    let id = view_target(s, v)?;
    let name = p::str(v, "name").unwrap_or("");
    let g = &s.graph;
    let (parent, tags, values) = if let Some(q) = g.query(id).cloned() {
        let tags: Vec<NodeId> = q
            .clauses
            .iter()
            .filter(|c| !c.not)
            .filter_map(|c| if let tanacraft_core::query::ClauseKind::Tagged { tag } = c.kind { Some(tag) } else { None })
            .collect();
        let values: Vec<(NodeId, String)> = q
            .clauses
            .iter()
            .filter(|c| !c.not)
            .filter_map(|c| match &c.kind {
                tanacraft_core::query::ClauseKind::Field { field, op: tanacraft_core::query::Op::Eq, value } => Some((*field, value.clone())),
                _ => None,
            })
            .collect();
        (g.sys.library, tags, values)
    } else if g.tag_def(id).is_some() {
        (s.graph.day(s.today)?, vec![id], Vec::new())
    } else {
        (id, Vec::new(), Vec::new())
    };
    let new = s.graph.create(parent, None, name)?;
    for t in tags {
        s.graph.add_tag(new, t)?;
    }
    for (f, val) in values {
        s.graph.set_field_text(new, f, &val)?;
    }
    if v.get("field").is_some() {
        let f = field_param(s, v, false)?;
        s.graph.set_field_text(new, f, p::str(v, "value").unwrap_or(""))?;
    }
    if p::bool(v, "stay") == Some(true) {
        // The view stays open (a calendar's New event).
    } else if parent == id {
        s.set_focus(new, 0);
    } else {
        s.open(new)?;
    }
    Ok(json!({"id": new.0, "parent": parent.0}))
}

// ---- supertag and field configuration -----------------------------------------------------------

fn opt_tag(s: &mut Session, v: &Value, key: &str) -> Result<Option<NodeId>, CmdError> {
    match v.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(x)) if x.trim().is_empty() || x == "none" => Ok(None),
        Some(_) => Ok(Some(tag_param(s, v, key, false)?)),
    }
}

fn tag_set_done_map(s: &mut Session, v: &Value) -> CmdResult {
    let tag = tag_param(s, v, "tag", false)?;
    let map = if v.get("field").is_some_and(|f| !f.is_null()) {
        let field = field_param(s, v, false)?;
        Some(tanacraft_core::DoneMap {
            field,
            checked: p::str(v, "checked").unwrap_or("Done").to_string(),
            unchecked: p::str(v, "unchecked").unwrap_or("").to_string(),
        })
    } else {
        None
    };
    s.graph.tag_set_done_map(tag, map)?;
    Ok(json!({"tag": tag.0}))
}

fn tag_field_flag(s: &mut Session, v: &Value, optional: bool) -> CmdResult {
    let tag = tag_param(s, v, "tag", false)?;
    let field = field_param(s, v, optional)?;
    let d = s.graph.tag_def(tag).cloned().unwrap_or_default();
    let list = if optional { &d.optional } else { &d.pinned_fields };
    let on = p::bool(v, "on").unwrap_or(!list.contains(&field));
    if optional {
        s.graph.tag_set_optional(tag, field, on)?;
    } else {
        s.graph.tag_set_pinned_field(tag, field, on)?;
    }
    Ok(json!({"tag": tag.0, "field": field.0, "on": on}))
}

fn tag_set_title_expr(s: &mut Session, v: &Value) -> CmdResult {
    let tag = tag_param(s, v, "tag", false)?;
    let expr = p::str(v, "expression");
    s.graph.tag_set_title_expr(tag, expr)?;
    Ok(json!({"tag": tag.0, "expression": s.graph.tag_def(tag).and_then(|d| d.title_expr.clone())}))
}

fn tag_add_related(s: &mut Session, v: &Value) -> CmdResult {
    let tag = tag_param(s, v, "tag", false)?;
    let q = parse_query(s, p::req_str(v, "query")?)?;
    let name = p::str(v, "name").map(str::to_string).unwrap_or_else(|| q.describe(&s.graph));
    let id = s.graph.tag_add_related(tag, &name, q)?;
    Ok(json!({"tag": tag.0, "node": id.0}))
}

fn tag_remove_related(s: &mut Session, v: &Value) -> CmdResult {
    let tag = tag_param(s, v, "tag", false)?;
    let node = p::node(v, "node")?.ok_or_else(|| CmdError::Params("`node` is required".into()))?;
    s.graph.tag_remove_related(tag, node)?;
    Ok(json!({"tag": tag.0}))
}

fn tag_set_child_tag(s: &mut Session, v: &Value) -> CmdResult {
    let tag = tag_param(s, v, "tag", false)?;
    let child = opt_tag(s, v, "child")?;
    s.graph.tag_set_child_tag(tag, child)?;
    Ok(json!({"tag": tag.0, "child": child.map(|c| c.0)}))
}

fn tag_set_base_type(s: &mut Session, v: &Value) -> CmdResult {
    let tag = tag_param(s, v, "tag", false)?;
    let base = p::str(v, "base").filter(|b| !b.is_empty() && *b != "none");
    s.graph.tag_set_base_type(tag, base)?;
    Ok(json!({"tag": tag.0, "base": base}))
}

fn tag_add_template_node(s: &mut Session, v: &Value) -> CmdResult {
    let tag = tag_param(s, v, "tag", false)?;
    let id = s.graph.tag_add_template_node(tag, p::req_str(v, "name")?)?;
    Ok(json!({"id": id.0}))
}

fn tag_remove_field(s: &mut Session, v: &Value) -> CmdResult {
    let tag = tag_param(s, v, "tag", false)?;
    let field = field_param(s, v, false)?;
    s.graph.tag_remove_field(tag, field)?;
    Ok(json!({"tag": tag.0, "field": field.0}))
}

fn tag_add_view(s: &mut Session, v: &Value) -> CmdResult {
    let tag = tag_param(s, v, "tag", false)?;
    let kind = p::req_str(v, "kind")?;
    let kind = ViewKind::parse(kind).ok_or_else(|| CmdError::Params(format!("unknown view `{kind}`")))?;
    let id = s.graph.tag_add_view(tag, kind)?;
    Ok(json!({"view": id.0}))
}

fn toggle_pin(s: &mut Session, v: &Value) -> CmdResult {
    let id = s.graph.resolve(p::node(v, "node")?.or(s.focus).unwrap_or(s.zoom));
    let on = !s.graph.node(id)?.pinned;
    s.graph.set_pinned(id, on)?;
    Ok(json!({"node": id.0, "pinned": on}))
}

fn field_set_type(s: &mut Session, v: &Value) -> CmdResult {
    let f = field_param(s, v, false)?;
    let ty = field_type(v)?;
    s.graph.field_def_mut(f)?.field_type = ty;
    Ok(json!({"field": f.0, "type": ty.name()}))
}

fn field_set_required(s: &mut Session, v: &Value) -> CmdResult {
    let f = field_param(s, v, false)?;
    let on = p::bool(v, "on").ok_or_else(|| CmdError::Params("`on` (bool) is required".into()))?;
    s.graph.field_def_mut(f)?.required = on;
    Ok(json!({"field": f.0, "required": on}))
}

fn field_set_hide(s: &mut Session, v: &Value) -> CmdResult {
    let f = field_param(s, v, false)?;
    let mode = p::req_str(v, "mode")?;
    let mode = tanacraft_core::HideMode::parse(mode).ok_or_else(|| CmdError::Params(format!("unknown hide mode `{mode}`")))?;
    s.graph.field_def_mut(f)?.hide = mode;
    Ok(json!({"field": f.0, "hide": mode.name()}))
}

fn field_set_range(s: &mut Session, v: &Value) -> CmdResult {
    let f = field_param(s, v, false)?;
    let num = |k: &str| v.get(k).and_then(Value::as_f64).filter(|x| x.is_finite());
    let (min, max) = (num("min"), num("max"));
    if let (Some(a), Some(b)) = (min, max)
        && a > b
    {
        return Err(CmdError::Params("`min` is above `max`".into()));
    }
    let d = s.graph.field_def_mut(f)?;
    d.min = min;
    d.max = max;
    Ok(json!({"field": f.0, "min": min, "max": max}))
}

fn field_set_auto_init(s: &mut Session, v: &Value) -> CmdResult {
    use tanacraft_core::AutoInit;
    let f = field_param(s, v, false)?;
    let mode = match p::req_str(v, "mode")? {
        "none" => None,
        "current_date" => Some(AutoInit::CurrentDate),
        "ancestor_day" => Some(AutoInit::AncestorDay),
        "ancestor_value" => Some(AutoInit::AncestorValue),
        other => return Err(CmdError::Params(format!("unknown auto-initialize mode `{other}`"))),
    };
    s.graph.field_def_mut(f)?.auto_init = mode;
    Ok(json!({"field": f.0}))
}

fn field_set_default(s: &mut Session, v: &Value) -> CmdResult {
    let f = field_param(s, v, false)?;
    let text = p::req_str(v, "value")?;
    let ty = s.graph.field_def(f).map(|d| d.field_type).unwrap_or_default();
    let value = if text.trim().is_empty() { None } else { Some(tanacraft_core::FieldValue::parse(ty, text).map_err(CmdError::Params)?) };
    s.graph.field_def_mut(f)?.default = value;
    Ok(json!({"field": f.0}))
}

fn field_set_source_tag(s: &mut Session, v: &Value) -> CmdResult {
    let f = field_param(s, v, false)?;
    let tag = tag_param(s, v, "tag", false)?;
    s.graph.field_def_mut(f)?.source_tag = Some(tag);
    Ok(json!({"field": f.0, "tag": tag.0}))
}

fn field_add_option(s: &mut Session, v: &Value) -> CmdResult {
    let f = field_param(s, v, false)?;
    let o = p::req_str(v, "option")?.trim().to_string();
    if o.is_empty() {
        return Err(CmdError::Params("`option` is empty".into()));
    }
    let d = s.graph.field_def_mut(f)?;
    if !d.options.contains(&o) {
        d.options.push(o);
    }
    Ok(json!({"field": f.0}))
}

fn field_remove_option(s: &mut Session, v: &Value) -> CmdResult {
    let f = field_param(s, v, false)?;
    let o = p::req_str(v, "option")?.to_string();
    s.graph.field_def_mut(f)?.options.retain(|x| *x != o);
    Ok(json!({"field": f.0}))
}

fn sort_children(s: &mut Session, v: &Value) -> CmdResult {
    let id = p::node(v, "node")?.or(s.focus).unwrap_or(s.zoom);
    let by = match p::req_str(v, "by")? {
        "name" => SortBy::Name,
        "created" => SortBy::Created,
        "done" => SortBy::Done,
        other => return Err(CmdError::Params(format!("sort children by name, created or done, not `{other}`"))),
    };
    s.graph.sort_children(id, by, p::bool(v, "desc").unwrap_or(false))?;
    Ok(json!({"node": id.0}))
}

/// Moves several nodes under `parent`, starting at `index`, keeping their order (one undo step).
fn move_many(s: &mut Session, v: &Value) -> CmdResult {
    let nodes = s.targets(v)?;
    let parent = p::node(v, "parent")?.ok_or_else(|| CmdError::Params("`parent` is required".into()))?;
    let parent = s.graph.resolve(parent);
    let mut index = p::usize(v, "index");
    for n in &nodes {
        s.graph.move_to(*n, parent, index)?;
        index = s.graph.index_in_parent(*n).map(|i| i + 1);
    }
    s.sel = None;
    Ok(json!({"moved": nodes.len()}))
}
