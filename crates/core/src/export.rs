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
            format!("{day}, {} - {}", crate::model::fmt_hm(*start), crate::model::fmt_hm(*end))
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

/// `[[id:42]]` becomes `[[Name]]` in exports.
fn inline_refs_as_names(g: &Graph, s: &str) -> String {
    if !s.contains("[[id:") {
        return s.to_string();
    }
    let mut out = s.to_string();
    for r in crate::format::refs(s) {
        out = out.replace(&crate::format::ref_token(r), &format!("[[{}]]", g.name(NodeId(r))));
    }
    out
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
    let is_ref = g.get(id).is_some_and(|n| n.reference_target().is_some());
    if is_ref {
        return; // a reference's content is exported where it lives
    }
    if let Some(n) = g.get(id) {
        for f in &n.fields {
            let v = entry_text(g, f);
            let v = if v.is_empty() { v } else { format!(" {v}") };
            out.push_str(&format!("{pad}  - {}::{v}\n", g.name(f.field)));
        }
    }
    for c in g.children(id) {
        md_node(g, *c, depth + 1, out);
    }
}

/// Nested HTML list of the nodes and their subtrees (the rich clipboard flavour, spec 02 §12):
/// `<ul><li>Name<ul><li>Child</li></ul></li></ul>`, text escaped, fields as `Field:: value` items.
pub fn to_html(g: &Graph, ids: &[NodeId]) -> String {
    let mut out = String::from("<ul>");
    for id in ids {
        html_node(g, *id, 0, &mut out);
    }
    out.push_str("</ul>");
    out
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn html_node(g: &Graph, id: NodeId, depth: usize, out: &mut String) {
    if depth > MAX_EXPORT_DEPTH {
        return;
    }
    out.push_str("<li>");
    out.push_str(&escape(&node_line(g, id)));
    let is_ref = g.get(id).is_some_and(|n| n.reference_target().is_some());
    let fields = if is_ref { Vec::new() } else { g.get(id).map(|n| n.fields.clone()).unwrap_or_default() };
    let kids = if is_ref { &[][..] } else { g.children(id) };
    if !fields.is_empty() || !kids.is_empty() {
        out.push_str("<ul>");
        for f in &fields {
            out.push_str(&format!("<li>{}:: {}</li>", escape(g.name(f.field)), escape(&entry_text(g, f))));
        }
        for c in kids {
            html_node(g, *c, depth + 1, out);
        }
        out.push_str("</ul>");
    }
    out.push_str("</li>");
}

/// Nested JSON of `id` and its subtree, with names resolved (for agents and the CLI).
pub fn to_json_tree(g: &Graph, id: NodeId) -> Value {
    json_node(g, id, 0)
}

fn json_node(g: &Graph, id: NodeId, depth: usize) -> Value {
    let Some(n) = g.get(id) else { return Value::Null };
    if let Some(t) = n.reference_target() {
        return json!({"id": id.0, "ref": t.0, "name": g.name(t)});
    }
    let mut v = json!({"id": id.0, "name": n.name});
    if !n.description.is_empty() {
        v["description"] = json!(n.description);
    }
    if !n.tags.is_empty() {
        v["tags"] = json!(n.tags.iter().map(|t| g.name(*t)).collect::<Vec<_>>());
    }
    if !n.fields.is_empty() {
        let fields: serde_json::Map<String, Value> = n
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
        v["fields"] = Value::Object(fields);
    }
    if let Some(d) = n.done {
        v["done"] = json!(d);
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
