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
        FieldValue::Checkbox(b) => if *b { "Yes" } else { "No" }.to_string(),
        FieldValue::Node(id) => g.name(*id).to_string(),
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
