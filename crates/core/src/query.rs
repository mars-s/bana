//! Search queries: what a search node finds, and the filters of a view.
//!
//! A query is a flat list of clauses joined by AND (or by OR), each optionally negated. It has a
//! text form, used by the CLI, MCP and the query editor (our own syntax):
//!
//! | Text | Matches |
//! |---|---|
//! | `#task`, `#"weekly review"` | tagged with the supertag (or one that extends it) |
//! | `seeds`, `"seed swap"` | node text contains the words |
//! | `TODO`, `DONE` | unchecked / checked checkbox |
//! | `Status:"In progress"` | field value equals (case-insensitive) |
//! | `Due:*` | field has a value |
//! | `Effort>2`, `Due<2026-10-20`, `Due<today` | number or date comparison |
//! | `CREATED:7` | created in the last 7 days |
//! | `UNDER:42` | inside node 42 |
//! | `NOT clause` | negation; `OR` between clauses switches to any-of |

use serde::{Deserialize, Serialize};

use crate::date::Date;
use crate::export::display_value;
use crate::graph::Graph;
use crate::model::{FieldValue, Kind, NodeId};
use crate::{Error, Result};

/// Search results are capped (a search should never stall the app).
pub const MAX_RESULTS: usize = 5000;

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Query {
    /// `true`: any clause matches (OR). `false`: all must match (AND).
    #[serde(default)]
    pub any: bool,
    #[serde(default)]
    pub clauses: Vec<Clause>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Clause {
    #[serde(default)]
    pub not: bool,
    pub kind: ClauseKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    /// Has any value.
    Set,
    Eq,
    Lt,
    Gt,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClauseKind {
    Tagged {
        tag: NodeId,
    },
    Text {
        text: String,
    },
    Done,
    Todo,
    Field {
        field: NodeId,
        op: Op,
        value: String,
    },
    CreatedWithin {
        days: u32,
    },
    EditedWithin {
        days: u32,
    },
    DoneWithin {
        days: u32,
    },
    Under {
        node: NodeId,
    },
    /// Has any supertag / any field.
    HasTag,
    HasField,
    /// Node type: tag, field, search or calendar node.
    Is {
        what: NodeType,
    },
    InLibrary,
    /// Lives (somewhere) under a day node.
    OnDay,
    /// A date field equals the date, or the node lives on that day (`today` etc. are relative).
    ForDate {
        date: String,
    },
    /// References the node (reference node or node-valued field).
    LinksTo {
        node: NodeId,
    },
    /// A nested group, in parentheses.
    Group {
        query: Query,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    Tag,
    Field,
    Search,
    Calendar,
}

impl NodeType {
    fn name(self) -> &'static str {
        match self {
            NodeType::Tag => "TAG",
            NodeType::Field => "FIELD",
            NodeType::Search => "SEARCH",
            NodeType::Calendar => "CALENDAR",
        }
    }
}

impl Query {
    pub fn is_empty(&self) -> bool {
        self.clauses.is_empty()
    }

    /// Parses the text form against `g` (supertag and field names must exist).
    pub fn parse(g: &Graph, text: &str) -> Result<Query> {
        let toks = tokenize(text)?;
        let mut pos = 0;
        let q = parse_group(g, &toks, &mut pos, 0)?;
        if pos < toks.len() {
            return Err(Error::Invalid("unmatched `)` in query".into()));
        }
        Ok(q)
    }

    /// Whether any clause asks for a node type (then tags, fields... become candidates).
    fn wants_types(&self) -> bool {
        self.clauses.iter().any(|c| match &c.kind {
            ClauseKind::Is { .. } => !c.not,
            ClauseKind::Group { query } => query.wants_types(),
            _ => false,
        })
    }
}

fn parse_group(g: &Graph, toks: &[Token], pos: &mut usize, depth: usize) -> Result<Query> {
    if depth > 16 {
        return Err(Error::Invalid("query nested too deeply".into()));
    }
    let mut q = Query::default();
    let mut negate = false;
    let mut saw_and = false;
    while let Some(tok) = toks.get(*pos) {
        *pos += 1;
        if !tok.quoted && !tok.hash && tok.text == "(" {
            let inner = parse_group(g, toks, pos, depth + 1)?;
            match toks.get(*pos) {
                Some(t) if !t.quoted && t.text == ")" => *pos += 1,
                _ => return Err(Error::Invalid("missing `)` in query".into())),
            }
            push(&mut q, &mut negate, ClauseKind::Group { query: inner });
            continue;
        }
        if !tok.quoted && !tok.hash && tok.text == ")" {
            *pos -= 1;
            break;
        }
        {
            {
                match (tok.text.as_str(), tok.quoted, tok.op) {
                    ("AND", false, None) => saw_and = true,
                    ("OR", false, None) => q.any = true,
                    ("NOT", false, None) => negate = !negate,
                    ("DONE", false, None) => push(&mut q, &mut negate, ClauseKind::Done),
                    ("TODO", false, None) => push(&mut q, &mut negate, ClauseKind::Todo),
                    _ => {
                        let kind = clause(g, tok)?;
                        push(&mut q, &mut negate, kind);
                    }
                }
            }
        }
    }
    if q.any && saw_and {
        return Err(Error::Invalid("mixing AND and OR in one group: use parentheses, e.g. (a OR b) AND c".into()));
    }
    Ok(q)
}

impl Query {
    /// The text form (parses back to the same query).
    pub fn describe(&self, g: &Graph) -> String {
        let parts: Vec<String> = self
            .clauses
            .iter()
            .map(|c| {
                let body = match &c.kind {
                    ClauseKind::Tagged { tag } => format!("#{}", quote(g.name(*tag))),
                    ClauseKind::Text { text } => quote(text),
                    ClauseKind::Done => "DONE".into(),
                    ClauseKind::Todo => "TODO".into(),
                    ClauseKind::Field { field, op, value } => {
                        let name = quote(g.name(*field));
                        match op {
                            Op::Set => format!("{name}:*"),
                            Op::Eq => format!("{name}:{}", quote(value)),
                            Op::Lt => format!("{name}<{}", quote(value)),
                            Op::Gt => format!("{name}>{}", quote(value)),
                        }
                    }
                    ClauseKind::CreatedWithin { days } => format!("CREATED:{days}"),
                    ClauseKind::EditedWithin { days } => format!("EDITED:{days}"),
                    ClauseKind::DoneWithin { days } => format!("DONE:{days}"),
                    ClauseKind::Under { node } => format!("UNDER:{node}"),
                    ClauseKind::HasTag => "HAS:TAG".into(),
                    ClauseKind::HasField => "HAS:FIELD".into(),
                    ClauseKind::Is { what } => format!("IS:{}", what.name()),
                    ClauseKind::InLibrary => "IN:LIBRARY".into(),
                    ClauseKind::OnDay => "ON:DAY".into(),
                    ClauseKind::ForDate { date } => format!("DATE:{}", quote(date)),
                    ClauseKind::LinksTo { node } => format!("LINKS:{node}"),
                    ClauseKind::Group { query } => format!("({})", query.describe(g)),
                };
                if c.not { format!("NOT {body}") } else { body }
            })
            .collect();
        parts.join(if self.any { " OR " } else { " AND " })
    }

    /// Whether node `id` matches. An empty query matches nothing (a search must ask for something).
    pub fn matches(&self, g: &Graph, id: NodeId) -> bool {
        if self.clauses.is_empty() {
            return false;
        }
        let one = |c: &Clause| clause_matches(g, id, &c.kind) != c.not;
        if self.any { self.clauses.iter().any(one) } else { self.clauses.iter().all(one) }
    }

    /// Like [`Query::matches`], but an empty query matches everything (view filters).
    pub fn filter(&self, g: &Graph, id: NodeId) -> bool {
        self.clauses.is_empty() || self.matches(g, id)
    }

    /// Every searchable node that matches, oldest first.
    pub fn run(&self, g: &Graph, exclude: NodeId) -> Vec<NodeId> {
        let types = self.wants_types();
        let ok = |id: NodeId| if types { candidate(g, id) } else { searchable(g, id) };
        let mut hits: Vec<(u64, NodeId)> =
            g.iter().filter(|n| ok(n.id) && n.id != exclude && self.matches(g, n.id)).map(|n| (n.created, n.id)).take(MAX_RESULTS).collect();
        hits.sort();
        hits.into_iter().map(|(_, id)| id).collect()
    }
}

fn push(q: &mut Query, negate: &mut bool, kind: ClauseKind) {
    q.clauses.push(Clause { not: *negate, kind });
    *negate = false;
}

/// Content nodes: not built-in, not definitions, not calendar nodes, not references, not trashed.
pub fn searchable(g: &Graph, id: NodeId) -> bool {
    match g.get(id) {
        Some(n) => matches!(n.kind, Kind::Plain) && !g.is_system(id) && !g.in_trash(id),
        None => false,
    }
}

/// Any node a type operator may find: not built-in, not a reference, not trashed.
fn candidate(g: &Graph, id: NodeId) -> bool {
    g.get(id).is_some_and(|n| n.reference_target().is_none()) && !g.is_system(id) && !g.in_trash(id)
}

fn within(t: u64, days: u32) -> bool {
    t + u64::from(days) * 86_400 >= crate::model::now_secs()
}

fn clause_matches(g: &Graph, id: NodeId, k: &ClauseKind) -> bool {
    let Some(n) = g.get(id) else { return false };
    match k {
        ClauseKind::EditedWithin { days } => within(n.edited.max(n.created), *days),
        ClauseKind::DoneWithin { days } => g.checkbox(id) == Some(true) && n.done_at.is_some_and(|t| within(t, *days)),
        ClauseKind::HasTag => n.tags.iter().any(|t| *t != g.sys.day_tag),
        ClauseKind::HasField => !n.fields.is_empty(),
        ClauseKind::Is { what } => match what {
            NodeType::Tag => matches!(n.kind, Kind::Tag(_)),
            NodeType::Field => matches!(n.kind, Kind::Field(_)),
            NodeType::Search => matches!(n.kind, Kind::Search(_)),
            NodeType::Calendar => matches!(n.kind, Kind::Day { .. } | Kind::Week { .. } | Kind::Year { .. }),
        },
        ClauseKind::InLibrary => g.is_ancestor_or_self(g.sys.library, id),
        ClauseKind::OnDay => g.ancestors(id).iter().any(|a| matches!(g.get(*a).map(|x| &x.kind), Some(Kind::Day { .. }))),
        ClauseKind::ForDate { date } => {
            let Some(d) = Date::parse(date) else { return false };
            n.fields.iter().any(|f| f.value == Some(FieldValue::Date(d)))
                || g.ancestors(id).iter().any(|a| g.get(*a).is_some_and(|x| x.kind == Kind::Day { date: d }))
        }
        ClauseKind::LinksTo { node } => {
            n.reference_target() == Some(*node)
                || crate::format::refs(&n.name).contains(&node.0)
                || n.fields.iter().any(|f| f.value == Some(FieldValue::Node(*node)))
        }
        ClauseKind::Group { query } => query.matches(g, id),
        ClauseKind::Tagged { tag } => n.tags.iter().any(|t| g.tag_lineage(*t).contains(tag)),
        ClauseKind::Text { text } => g.text(id).to_lowercase().contains(&text.to_lowercase()),
        ClauseKind::Done => g.checkbox(id) == Some(true),
        ClauseKind::Todo => g.checkbox(id) == Some(false),
        ClauseKind::CreatedWithin { days } => {
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            n.created + u64::from(*days) * 86_400 >= now
        }
        ClauseKind::Under { node } => *node != id && g.is_ancestor_or_self(*node, id),
        ClauseKind::Field { field, op, value } => {
            let v = n.field_value(*field);
            match (op, v) {
                (Op::Set, v) => v.is_some_and(|v| !display_value(g, v).is_empty()),
                (_, None) => false,
                (Op::Eq, Some(v)) => display_value(g, v).eq_ignore_ascii_case(value.trim()),
                (Op::Lt, Some(v)) => compare(g, v, value).is_some_and(|o| o.is_lt()),
                (Op::Gt, Some(v)) => compare(g, v, value).is_some_and(|o| o.is_gt()),
            }
        }
    }
}

/// Compares a value with typed text: numbers as numbers, dates as dates, else text.
pub fn compare(g: &Graph, v: &FieldValue, text: &str) -> Option<std::cmp::Ordering> {
    match v {
        FieldValue::Number(x) => text.trim().parse::<f64>().ok().and_then(|y| x.partial_cmp(&y)),
        FieldValue::Date(d) => Date::parse(text).map(|t| d.cmp(&t)),
        other => Some(display_value(g, other).to_lowercase().cmp(&text.trim().to_lowercase())),
    }
}

struct Token {
    text: String,
    quoted: bool,
    /// Position (in chars of `text`) and kind of the first unquoted `:`, `<` or `>`.
    op: Option<(usize, char)>,
    /// `#` prefix (outside quotes).
    hash: bool,
}

fn tokenize(s: &str) -> Result<Vec<Token>> {
    let mut out = Vec::new();
    let mut chars = s.chars().peekable();
    loop {
        while chars.peek().is_some_and(|c| c.is_whitespace()) {
            chars.next();
        }
        if chars.peek().is_none() {
            break;
        }
        let mut t = Token { text: String::new(), quoted: false, op: None, hash: false };
        if let Some(&c) = chars.peek()
            && (c == '(' || c == ')')
        {
            chars.next();
            t.text.push(c);
            out.push(t);
            continue;
        }
        if chars.peek() == Some(&'#') {
            chars.next();
            t.hash = true;
        }
        while let Some(&c) = chars.peek() {
            if c.is_whitespace() || c == ')' {
                break;
            }
            chars.next();
            if c == '"' {
                t.quoted = true;
                let mut closed = false;
                for q in chars.by_ref() {
                    if q == '"' {
                        closed = true;
                        break;
                    }
                    t.text.push(q);
                }
                if !closed {
                    return Err(Error::Invalid("unclosed quote in query".into()));
                }
            } else if t.op.is_none() && matches!(c, ':' | '<' | '>') && !t.text.is_empty() {
                t.op = Some((t.text.chars().count(), c));
                t.text.push(c);
            } else {
                t.text.push(c);
            }
        }
        out.push(t);
    }
    Ok(out)
}

fn clause(g: &Graph, tok: &Token) -> Result<ClauseKind> {
    if tok.hash {
        let tag = g.find_tag(&tok.text).ok_or_else(|| Error::Invalid(format!("no supertag #{}", tok.text)))?;
        return Ok(ClauseKind::Tagged { tag });
    }
    let Some((at, op)) = tok.op else {
        return Ok(ClauseKind::Text { text: tok.text.clone() });
    };
    let name: String = tok.text.chars().take(at).collect();
    let value: String = tok.text.chars().skip(at + 1).collect();
    match (name.as_str(), op) {
        ("CREATED", ':') => {
            let days = value.trim().parse().map_err(|_| Error::Invalid(format!("CREATED needs a number of days, not `{value}`")))?;
            return Ok(ClauseKind::CreatedWithin { days });
        }
        ("EDITED", ':') | ("DONE", ':') => {
            let days = value.trim().parse().map_err(|_| Error::Invalid(format!("{name} needs a number of days, not `{value}`")))?;
            return Ok(if name == "EDITED" { ClauseKind::EditedWithin { days } } else { ClauseKind::DoneWithin { days } });
        }
        ("HAS", ':') => {
            return match value.as_str() {
                "TAG" => Ok(ClauseKind::HasTag),
                "FIELD" => Ok(ClauseKind::HasField),
                other => Err(Error::Invalid(format!("HAS:{other}: use HAS:TAG or HAS:FIELD"))),
            };
        }
        ("IS", ':') => {
            let what = match value.as_str() {
                "TAG" => NodeType::Tag,
                "FIELD" => NodeType::Field,
                "SEARCH" => NodeType::Search,
                "CALENDAR" => NodeType::Calendar,
                other => return Err(Error::Invalid(format!("IS:{other}: use IS:TAG, IS:FIELD, IS:SEARCH or IS:CALENDAR"))),
            };
            return Ok(ClauseKind::Is { what });
        }
        ("IN", ':') if value == "LIBRARY" => return Ok(ClauseKind::InLibrary),
        ("ON", ':') if value == "DAY" => return Ok(ClauseKind::OnDay),
        ("DATE", ':') => {
            Date::parse(&value).ok_or_else(|| Error::Invalid(format!("DATE:{value} is not a date")))?;
            return Ok(ClauseKind::ForDate { date: value });
        }
        ("LINKS", ':') => {
            let node: NodeId = value.parse().map_err(Error::Invalid)?;
            g.node(node)?;
            return Ok(ClauseKind::LinksTo { node });
        }
        ("UNDER", ':') => {
            let node: NodeId = value.parse().map_err(Error::Invalid)?;
            g.node(node)?;
            return Ok(ClauseKind::Under { node });
        }
        _ => {}
    }
    let Some(field) = g.find_field(&name) else {
        // Not a field: treat the whole token as text (e.g. "10:30").
        return Ok(ClauseKind::Text { text: tok.text.clone() });
    };
    let op = match op {
        ':' if value == "*" => Op::Set,
        ':' => Op::Eq,
        '<' => Op::Lt,
        _ => Op::Gt,
    };
    Ok(ClauseKind::Field { field, op, value })
}

fn quote(s: &str) -> String {
    let plain = !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | '/' | '@'));
    let keyword = matches!(s, "AND" | "OR" | "NOT" | "DONE" | "TODO");
    if plain && !keyword { s.to_string() } else { format!("\"{}\"", s.replace('"', "'")) }
}
