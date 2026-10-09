//! OPML, the outline interchange format other outliners export and import: `<outline text="..."
//! _note="...">` elements nested in `<body>`. A small tolerant reader (no XML library): it reads
//! `outline` start, empty and end tags and ignores everything else.

use crate::graph::Graph;
use crate::model::NodeId;

/// Deepest nesting read; deeper outlines are flattened onto this level.
const MAX_DEPTH: usize = 200;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Item {
    pub text: String,
    pub note: String,
    pub children: Vec<Item>,
}

/// Reads the `outline` elements of an OPML document into a tree.
pub fn parse(src: &str) -> Vec<Item> {
    // Stack of open items; the bottom one collects the top level.
    let mut stack: Vec<Item> = vec![Item::default()];
    let mut rest = src;
    while let Some(lt) = rest.find('<') {
        rest = &rest[lt + 1..];
        let Some(gt) = rest.find('>') else { break };
        let tag = &rest[..gt];
        rest = &rest[gt + 1..];
        if let Some(name) = tag.strip_prefix('/') {
            if name.trim().eq_ignore_ascii_case("outline") && stack.len() > 1 {
                let done = stack.pop().unwrap_or_default();
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(done);
                }
            }
            continue;
        }
        let lower = tag.trim_start().to_ascii_lowercase();
        if !lower.starts_with("outline") {
            continue;
        }
        let empty = tag.trim_end().ends_with('/');
        let item = Item {
            text: attr(tag, "text").or_else(|| attr(tag, "title")).unwrap_or_default(),
            note: attr(tag, "_note").unwrap_or_default(),
            children: Vec::new(),
        };
        if empty || stack.len() > MAX_DEPTH {
            if let Some(parent) = stack.last_mut() {
                parent.children.push(item);
            }
        } else {
            stack.push(item);
        }
    }
    // Unclosed outlines still count.
    while stack.len() > 1 {
        let done = stack.pop().unwrap_or_default();
        if let Some(parent) = stack.last_mut() {
            parent.children.push(done);
        }
    }
    stack.pop().map(|root| root.children).unwrap_or_default()
}

/// The value of attribute `name` in a tag body, unescaped.
fn attr(tag: &str, name: &str) -> Option<String> {
    let mut at = 0;
    while let Some(i) = tag[at..].find(name) {
        let start = at + i;
        at = start + name.len();
        // A whole attribute name: preceded by whitespace, followed by `=`.
        let before_ok = tag[..start].ends_with(|c: char| c.is_whitespace());
        let after = tag[at..].trim_start();
        if !before_ok || !after.starts_with('=') {
            continue;
        }
        let after = after[1..].trim_start();
        let quote = after.chars().next()?;
        if quote != '"' && quote != '\'' {
            return None;
        }
        let body = &after[1..];
        let end = body.find(quote)?;
        return Some(unescape(&body[..end]));
    }
    None
}

fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(semi) = rest.find(';').filter(|j| *j <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let ent = &rest[1..semi];
        let ch = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            e if e.starts_with("#x") || e.starts_with("#X") => u32::from_str_radix(&e[2..], 16).ok().and_then(char::from_u32),
            e if e.starts_with('#') => e[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &rest[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\n', "&#10;")
}

/// An OPML document of the nodes and their subtrees (names as `text`, descriptions as `_note`).
pub fn to_opml(g: &Graph, ids: &[NodeId], title: &str) -> String {
    let mut out =
        format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<opml version=\"2.0\">\n  <head><title>{}</title></head>\n  <body>\n", escape(title));
    for id in ids {
        write_node(g, *id, 2, &mut out);
    }
    out.push_str("  </body>\n</opml>\n");
    out
}

fn write_node(g: &Graph, id: NodeId, depth: usize, out: &mut String) {
    let pad = "  ".repeat(depth);
    let text = crate::export::node_line(g, id);
    let note = g.get(g.resolve(id)).map(|n| n.description.clone()).unwrap_or_default();
    let note = if note.is_empty() { String::new() } else { format!(" _note=\"{}\"", escape(&note)) };
    let is_ref = g.get(id).is_some_and(|n| n.reference_target().is_some());
    let kids = if is_ref || depth > MAX_DEPTH { &[][..] } else { g.children(id) };
    if kids.is_empty() {
        out.push_str(&format!("{pad}<outline text=\"{}\"{note}/>\n", escape(&text)));
        return;
    }
    out.push_str(&format!("{pad}<outline text=\"{}\"{note}>\n", escape(&text)));
    for c in kids {
        write_node(g, *c, depth + 1, out);
    }
    out.push_str(&format!("{pad}</outline>\n"));
}
