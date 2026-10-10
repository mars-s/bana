//! Markdown and JSON export of a subtree.

use serde_json::{Value, json};

use crate::graph::Graph;
use crate::model::{FieldValue, Kind, NodeId};

const MAX_EXPORT_DEPTH: usize = 200;

/// A field value as people read it.
pub fn display_value(g: &Graph, v: &FieldValue) -> String {
    match v {
        FieldValue::Text(s) | FieldValue::Url(s) | FieldValue::Email(s) | FieldValue::Choice(s) => s.clone(),
        FieldValue::Number(n) => format!("{n}"),
        FieldValue::Date(d) => d.to_string(),
        FieldValue::Timed { date, start, end } => format!("{date} {}-{}", crate::model::fmt_hm(*start), crate::model::fmt_hm(*end)),
        FieldValue::Span { start, end } => format!("{start}/{end}"),
        FieldValue::Checkbox(b) => if *b { "Yes" } else { "No" }.to_string(),
        FieldValue::Node(id) => g.name(*id).to_string(),
    }
}

/// A value as shown in the UI: dates read "Thu, 15 Oct" (with the year when it is not this year).
pub fn shown_value(g: &Graph, v: &FieldValue) -> String {
    match v {
        FieldValue::Date(d) if d.year == crate::Date::today().year => d.day_title(),
        FieldValue::Date(d) => format!("{}, {}", d.day_title(), d.year),
        FieldValue::Timed { date, start, end } => {
            let day = shown_value(g, &FieldValue::Date(*date));
            format!("{day}, {} - {}", g.fmt_time(*start), g.fmt_time(*end))
        }
        FieldValue::Span { start, end } => {
            format!("{} - {}", shown_value(g, &FieldValue::Date(*start)), shown_value(g, &FieldValue::Date(*end)))
        }
        other => display_value(g, other),
    }
}

/// One line of a node: checkbox, name (or `[[target]]` for a reference) and `#tags`.
pub fn node_line(g: &Graph, id: NodeId) -> String {
    let Some(n) = g.get(id) else { return String::new() };
    let t = g.get(g.resolve(id)).unwrap_or(n);
    let mut s = String::new();
    match g.checkbox(id) {
        Some(true) => s.push_str("[x] "),
        Some(false) => s.push_str("[ ] "),
        None => {}
    }
    if n.reference_target().is_some() {
        s.push_str(&format!("[[{}]]", t.name));
    } else {
        s.push_str(&inline_refs_as_names(g, &t.name));
    }
    for tag in &t.tags {
        s.push_str(&format!(" #{}", tag_token(g.name(*tag))));
    }
    s
}

fn tag_token(name: &str) -> String {
    if name.contains(' ') { format!("[[{name}]]") } else { name.to_string() }
}

/// `[[id:42]]` becomes `[[Name]]` in exports, `[[id:42|alias]]` becomes `[[Name|alias]]`.
fn inline_refs_as_names(g: &Graph, s: &str) -> String {
    crate::format::map_refs(s, |r, alias| match alias {
        Some(a) => format!("[[{}|{a}]]", g.name(NodeId(r))),
        None => format!("[[{}]]", g.name(NodeId(r))),
    })
}

/// Markdown outline of `id` and its subtree. The root line is included unless `children_only`.
pub fn to_markdown(g: &Graph, id: NodeId, children_only: bool) -> String {
    let mut out = String::new();
    if children_only {
        for c in g.children(id) {
            md_node(g, *c, 0, &mut out);
        }
    } else {
        md_node(g, id, 0, &mut out);
    }
    out
}

fn md_node(g: &Graph, id: NodeId, depth: usize, out: &mut String) {
    if depth > MAX_EXPORT_DEPTH {
        return;
    }
    let pad = "  ".repeat(depth);
    out.push_str(&format!("{pad}- {}\n", node_line(g, id)));
    // A reference's content is exported where it lives; its notes and fields for this place
    // (spec 01 §3) are exported here, as its children.
    let is_ref = g.get(id).is_some_and(|n| n.reference_target().is_some());
    let Some(n) = g.get(id) else { return };
    let kids = if is_ref { context_notes(g, id) } else { g.children(id).to_vec() };
    if is_ref && !n.description.is_empty() {
        out.push_str(&format!("{pad}  - {}\n", n.description));
    } else if !n.description.is_empty() {
        // A description is a line of text under its node, not a child (spec 02 §13).
        out.push_str(&format!("{pad}  {}\n", n.description));
    }
    // Field rows sit where they are in the outline: after the child they were typed below, or
    // above the children.
    let field_line = |f: &crate::FieldEntry, out: &mut String| {
        let v = entry_text(g, f);
        let v = if v.is_empty() { v } else { format!(" {v}") };
        out.push_str(&format!("{pad}  - {}::{v}\n", g.name(f.field)));
    };
    let anchored = |f: &crate::FieldEntry| f.after.filter(|a| kids.contains(a));
    for f in n.fields.iter().filter(|f| anchored(f).is_none()) {
        field_line(f, out);
    }
    for c in &kids {
        md_node(g, *c, depth + 1, out);
        for f in n.fields.iter().filter(|f| anchored(f) == Some(*c)) {
            field_line(f, out);
        }
    }
}

/// The lines of a reference's notes for this place (empty for anything else).
pub fn context_notes(g: &Graph, reference: NodeId) -> Vec<NodeId> {
    g.context_of(reference).map(|h| g.children(h).to_vec()).unwrap_or_default()
}

/// The rich clipboard flavour (spec 02 §12, DIFFS N21): each copied node is a paragraph with its
/// formatting as HTML (`<b>`, `<i>`...), its fields and children a nested list under it.
pub fn to_html(g: &Graph, ids: &[NodeId]) -> String {
    let mut out = String::new();
    for id in ids {
        out.push_str(&format!("<p>{}</p>", html_line(g, *id)));
        let inner = html_children(g, *id, 0);
        out.push_str(&inner);
    }
    out
}

/// The plain clipboard flavour: the copied lines without dashes or format marks, their children as
/// an indented `- ` list.
pub fn to_plain(g: &Graph, ids: &[NodeId]) -> String {
    let mut out = String::new();
    for id in ids {
        out.push_str(&plain_line(g, *id));
        out.push('\n');
        plain_children(g, *id, 0, &mut out);
    }
    out
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// A node's line with the format marks applied (`html`) or dropped (plain text).
fn shown_line(g: &Graph, id: NodeId, html: bool) -> String {
    let Some(n) = g.get(id) else { return String::new() };
    let t = g.get(g.resolve(id)).unwrap_or(n);
    let mut s = String::new();
    match g.checkbox(id) {
        Some(true) => s.push_str("[x] "),
        Some(false) => s.push_str("[ ] "),
        None => {}
    }
    for sp in crate::format::spans(&t.name) {
        let text = match (sp.node, &sp.alias) {
            (Some(_), Some(a)) => a.clone(),
            (Some(r), None) => g.name(NodeId(r)).to_string(),
            (None, _) => sp.text.clone(),
        };
        if !html {
            s.push_str(&text);
            continue;
        }
        let mut piece = escape(&text);
        if let Some(u) = &sp.url {
            piece = format!("<a href=\"{}\">{piece}</a>", escape(u));
        }
        for (on, tag) in [(sp.code, "code"), (sp.highlight, "mark"), (sp.strike, "s"), (sp.underline, "u"), (sp.italic, "i"), (sp.bold, "b")] {
            if on {
                piece = format!("<{tag}>{piece}</{tag}>");
            }
        }
        s.push_str(&piece);
    }
    for tag in &t.tags {
        let tag = format!(" #{}", tag_token(g.name(*tag)));
        s.push_str(&if html { escape(&tag) } else { tag });
    }
    s
}

fn html_line(g: &Graph, id: NodeId) -> String {
    shown_line(g, id, true)
}

fn plain_line(g: &Graph, id: NodeId) -> String {
    shown_line(g, id, false)
}

/// Fields and children of `id` (not of a reference line, whose content lives elsewhere).
fn content(g: &Graph, id: NodeId) -> (Vec<crate::FieldEntry>, Vec<NodeId>) {
    if g.get(id).is_none_or(|n| n.reference_target().is_some()) {
        return (Vec::new(), Vec::new());
    }
    (g.get(id).map(|n| n.fields.clone()).unwrap_or_default(), g.children(id).to_vec())
}

fn html_children(g: &Graph, id: NodeId, depth: usize) -> String {
    let (fields, kids) = content(g, id);
    if depth > MAX_EXPORT_DEPTH || fields.is_empty() && kids.is_empty() {
        return String::new();
    }
    let mut out = String::from("<ul>");
    for f in &fields {
        out.push_str(&format!("<li>{}:: {}</li>", escape(g.name(f.field)), escape(&entry_text(g, f))));
    }
    for c in kids {
        out.push_str(&format!("<li>{}{}</li>", html_line(g, c), html_children(g, c, depth + 1)));
    }
    out.push_str("</ul>");
    out
}

fn plain_children(g: &Graph, id: NodeId, depth: usize, out: &mut String) {
    if depth > MAX_EXPORT_DEPTH {
        return;
    }
    let (fields, kids) = content(g, id);
    let pad = "  ".repeat(depth);
    for f in &fields {
        out.push_str(&format!("{pad}- {}:: {}\n", g.name(f.field), entry_text(g, f)));
    }
    for c in kids {
        out.push_str(&format!("{pad}- {}\n", plain_line(g, c)));
        plain_children(g, c, depth + 1, out);
    }
}

/// Nested JSON of `id` and its subtree, with names resolved (for agents and the CLI).
pub fn to_json_tree(g: &Graph, id: NodeId) -> Value {
    json_node(g, id, 0)
}

fn json_node(g: &Graph, id: NodeId, depth: usize) -> Value {
    let Some(n) = g.get(id) else { return Value::Null };
    let fields = || -> Value {
        let map: serde_json::Map<String, Value> = n
            .fields
            .iter()
            .map(|f| {
                let vals: Vec<String> = f.values().map(|x| display_value(g, x)).collect();
                let v = match vals.len() {
                    0 => Value::Null,
                    1 => json!(vals[0]),
                    _ => json!(vals),
                };
                (g.name(f.field).to_string(), v)
            })
            .collect();
        Value::Object(map)
    };
    if let Some(t) = n.reference_target() {
        // A reference's notes and fields for this place (spec 01 §3) go with it.
        let mut v = json!({"id": id.0, "ref": t.0, "name": g.name(t)});
        if !n.description.is_empty() {
            v["note"] = json!(n.description);
        }
        if !n.fields.is_empty() {
            v["fields"] = fields();
        }
        let notes = context_notes(g, id);
        if !notes.is_empty() && depth < MAX_EXPORT_DEPTH {
            v["notes"] = Value::Array(notes.iter().map(|c| json_node(g, *c, depth + 1)).collect());
        }
        return v;
    }
    let mut v = json!({"id": id.0, "name": n.name});
    if !n.description.is_empty() {
        v["description"] = json!(n.description);
    }
    if let Some(by) = &n.by {
        v["by"] = json!(by);
    }
    if !n.tags.is_empty() {
        v["tags"] = json!(n.tags.iter().map(|t| g.name(*t)).collect::<Vec<_>>());
    }
    if !n.fields.is_empty() {
        v["fields"] = fields();
    }
    if let Some(d) = n.done {
        v["done"] = json!(d);
    }
    // Command nodes run on its new children (UC-02.20).
    if !n.on_child.is_empty() {
        v["onChild"] = json!(n.on_child.iter().map(|c| g.name(*c)).collect::<Vec<_>>());
    }
    match &n.kind {
        Kind::Day { date } => v["date"] = json!(date.to_string()),
        Kind::Tag(_) => v["kind"] = json!("supertag"),
        Kind::Search(q) => {
            v["kind"] = json!("search");
            v["query"] = json!(q.describe(g));
            v["results"] = json!(q.run(g, id).iter().map(|r| r.0).collect::<Vec<_>>());
        }
        Kind::Field(f) => v["kind"] = json!(format!("field:{}", f.field_type.name())),
        _ => {}
    }
    if !n.children.is_empty() && depth < MAX_EXPORT_DEPTH {
        v["children"] = Value::Array(n.children.iter().map(|c| json_node(g, *c, depth + 1)).collect());
    }
    v
}

/// All values of a field entry as text, comma separated.
pub fn entry_text(g: &Graph, e: &crate::FieldEntry) -> String {
    e.values().map(|v| display_value(g, v)).collect::<Vec<_>>().join(", ")
}

/// A content snippet for a search hit (spec 05 §4): the node's description, its own long text, or
/// a child line that holds every query word.
pub fn snippet(g: &Graph, id: NodeId, query: &str) -> Option<String> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return None;
    }
    let n = g.get(id)?;
    std::iter::once(n.description.clone())
        .chain(std::iter::once(g.text(id)).filter(|t| t.chars().count() > 48))
        .chain(g.children(id).iter().take(200).map(|c| g.text(*c)))
        .find(|t| q.split_whitespace().all(|w| t.to_lowercase().contains(w)))
}

/// Every page of workspace `ws` as Markdown files, `(path, text)` (UC-08.13). `shape`:
/// - `outline`: the Library's pages in `Library/`, day pages in `Daily notes/` (main workspace);
/// - `tags`: a folder per supertag with one file per tagged node;
/// - `vault`: as `outline`, each file starting with front matter (tags and fields), for Obsidian.
///
/// Inline references are `[[Name]]` links, so a vault's pages link to each other.
pub fn markdown_files(g: &Graph, ws: NodeId, shape: &str) -> Vec<(String, String)> {
    let mut files: Vec<(String, String)> = Vec::new();
    let mut taken = std::collections::HashSet::new();
    let mut add = |dir: &str, name: &str, text: String| {
        let base = file_name(name);
        let mut path = format!("{dir}/{base}.md");
        let mut n = 2;
        while !taken.insert(path.to_lowercase()) {
            path = format!("{dir}/{base} ({n}).md");
            n += 1;
        }
        files.push((path, text));
    };
    let page = |id: NodeId, front: bool| {
        let mut s = String::new();
        if front {
            s.push_str(&front_matter(g, id));
        }
        s.push_str(&format!("# {}\n\n", inline_refs_as_names(g, g.name(id))));
        s.push_str(&to_markdown(g, id, true));
        s
    };
    if shape == "tags" {
        for tag in g.tags().into_iter().filter(|t| g.workspace_of(*t) == ws && *t != g.sys.day_tag) {
            let dir = format!("#{}", file_name(g.name(tag)));
            for id in g.tagged(tag).into_iter().filter(|n| !g.in_trash(*n)) {
                add(&dir, &g.text(id), page(id, false));
            }
        }
        return files;
    }
    let front = shape == "vault";
    for id in g.children(g.workspace_library(ws)) {
        add("Library", &g.text(*id), page(*id, front));
    }
    if ws == g.sys.root {
        let mut days: Vec<(crate::date::Date, NodeId)> = g
            .iter()
            .filter_map(|n| match n.kind {
                crate::Kind::Day { date } if !g.children(n.id).is_empty() && !g.in_trash(n.id) => Some((date, n.id)),
                _ => None,
            })
            .collect();
        days.sort();
        for (date, id) in days {
            add("Daily notes", &date.to_string(), page(id, front));
        }
    }
    files
}

/// A file name from a node's text: no path separators or characters systems refuse, at most 80
/// characters, never empty.
fn file_name(name: &str) -> String {
    let clean: String = name.chars().map(|c| if c.is_control() || "/\\:*?\"<>|".contains(c) { ' ' } else { c }).collect();
    let clean = clean.split_whitespace().collect::<Vec<_>>().join(" ");
    let clean: String = clean.trim_start_matches('.').chars().take(80).collect();
    if clean.trim().is_empty() { "Untitled".into() } else { clean.trim().to_string() }
}

/// YAML front matter: the node's supertags and fields (Obsidian properties).
fn front_matter(g: &Graph, id: NodeId) -> String {
    let Some(n) = g.get(id) else { return String::new() };
    let yaml = |s: &str| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""));
    let mut s = String::from("---\n");
    if !n.tags.is_empty() {
        let tags: Vec<String> = n.tags.iter().map(|t| yaml(g.name(*t))).collect();
        s.push_str(&format!("tags: [{}]\n", tags.join(", ")));
    }
    for e in &n.fields {
        s.push_str(&format!("{}: {}\n", yaml(g.name(e.field)), yaml(&entry_text(g, e))));
    }
    s.push_str("---\n\n");
    if s == "---\n---\n\n" { String::new() } else { s }
}
