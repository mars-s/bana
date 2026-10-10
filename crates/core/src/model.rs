//! Node graph types.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::date::Date;

/// A node id. Ids are never reused within a graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(pub u64);

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for NodeId {
    type Err = String;
    fn from_str(s: &str) -> Result<NodeId, String> {
        s.trim().trim_start_matches('n').parse().map(NodeId).map_err(|_| format!("bad node id `{s}`"))
    }
}

/// What a node is. Most nodes are `Plain`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
// ponytail: tag nodes are few; boxing TagDef would touch every match for no measurable gain.
#[allow(clippy::large_enum_variant)]
pub enum Kind {
    #[default]
    Plain,
    /// A mirror of another node: shows and edits the target.
    Reference {
        target: NodeId,
    },
    /// A supertag definition.
    Tag(TagDef),
    /// A field definition.
    Field(FieldDef),
    /// A live search: its items are the query's results, not stored children.
    Search(crate::query::Query),
    /// A calendar node.
    Day {
        date: Date,
    },
    Week {
        year: i32,
        week: u32,
    },
    Year {
        year: i32,
    },
    /// A further workspace beside the main one (spec 07 §2): its own Library under it. The main
    /// workspace is the graph root.
    Workspace,
    /// A command node (spec 01 §10): its children are command lines (`tag.add {"tag": "task"}`)
    /// run in order on a node, as one step.
    Command,
}

/// Supertag colours: "none", twelve hues and grey. Rendered from the UI theme, not stored as RGB.
/// A custom colour is stored as `#rrggbb` (see [`is_custom_color`]).
pub const TAG_COLORS: [&str; 14] =
    ["none", "grey", "red", "orange", "amber", "yellow", "lime", "green", "teal", "cyan", "blue", "indigo", "violet", "pink"];

/// `#rrggbb`.
pub fn is_custom_color(c: &str) -> bool {
    c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|x| x.is_ascii_hexdigit())
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TagDef {
    /// One of [`TAG_COLORS`].
    #[serde(default)]
    pub color: String,
    /// Supertags this one extends: their template fields come first.
    #[serde(default)]
    pub extends: Vec<NodeId>,
    /// Template fields (field definition ids), in order.
    #[serde(default)]
    pub fields: Vec<NodeId>,
    /// Tagged nodes show a checkbox.
    #[serde(default)]
    pub checkbox: bool,
    /// Checking / unchecking a tagged node sets a field (and setting the field moves the checkbox).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub done_map: Option<DoneMap>,
    /// New children of a tagged node get this supertag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child_tag: Option<NodeId>,
    /// Plain template nodes (children of the tag node) copied onto newly tagged nodes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub template: Vec<NodeId>,
    /// A built-in semantic type (task, meeting, person...), one of [`BASE_TYPES`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_type: Option<String>,
    /// Search nodes shown as view tabs on the supertag's page (children of the tag node).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub views: Vec<NodeId>,
    /// Related content: search nodes shown beside the page of every tagged node (the day page's
    /// "Open tasks" card).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related: Vec<NodeId>,
    /// Optional fields: offered on tagged nodes as "+ Field" buttons, not added up front.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub optional: Vec<NodeId>,
    /// Template fields marked pinned (offered first in view menus, spec 03 §9).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pinned_fields: Vec<NodeId>,
    /// Command nodes run on a node when it gets this supertag (UC-02.20).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_added: Vec<NodeId>,
    /// Command nodes run on a tagged node when it is checked done (UC-02.20).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_done: Vec<NodeId>,
    /// Command nodes shown as buttons on tagged nodes; a click runs one on the node (UC-06.33).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub buttons: Vec<NodeId>,
    /// Builds tagged nodes' titles from fields: `${name}` then text and `${Field}` placeholders.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_expr: Option<String>,
    /// Building block: suggested first in the `#` menu on nodes of these base types (spec 03 §5).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub promote_for: Vec<String>,
    /// Building block: left out of searches and menus.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
}

/// Base types listed in the spec (03 §5).
pub const BASE_TYPES: [&str; 13] =
    ["task", "meeting", "person", "organization", "location", "project", "topic", "event", "article", "memo", "reflection", "day", "week"];

/// Done-state mapping: checked means `field = checked`; unchecked means `field = unchecked`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DoneMap {
    pub field: NodeId,
    pub checked: String,
    pub unchecked: String,
}

/// When a field row is hidden (spec 03 §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HideMode {
    #[default]
    Never,
    WhenEmpty,
    WhenNotEmpty,
    WhenDefault,
    Always,
}

impl HideMode {
    pub const ALL: [HideMode; 5] = [HideMode::Never, HideMode::WhenEmpty, HideMode::WhenNotEmpty, HideMode::WhenDefault, HideMode::Always];
    pub fn name(self) -> &'static str {
        match self {
            HideMode::Never => "never",
            HideMode::WhenEmpty => "when_empty",
            HideMode::WhenNotEmpty => "when_not_empty",
            HideMode::WhenDefault => "when_default",
            HideMode::Always => "always",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            HideMode::Never => "Never",
            HideMode::WhenEmpty => "When empty",
            HideMode::WhenNotEmpty => "When not empty",
            HideMode::WhenDefault => "When value is default",
            HideMode::Always => "Always",
        }
    }
    pub fn parse(s: &str) -> Option<HideMode> {
        HideMode::ALL.into_iter().find(|h| h.name() == s.trim().to_ascii_lowercase().replace(' ', "_"))
    }
}

/// How a field fills itself when a supertag adds it (spec 03 §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoInit {
    /// Today's date (date fields).
    CurrentDate,
    /// The date of the day node the item lives under.
    AncestorDay,
    /// The value of the nearest ancestor that has this field.
    AncestorValue,
    /// A random node with the field's source supertag (options from supertag fields; journal
    /// prompts, UC-01.09).
    RandomNode,
}

/// The value types a field can hold (public docs: Plain, Options, Date, Number, URL, Email, Checkbox,
/// Options from supertag).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    #[default]
    Plain,
    Options,
    OptionsFromTag,
    Date,
    Number,
    Url,
    Email,
    Checkbox,
    /// A person in the workspace (here: a name; the picker offers the names already used).
    User,
}

impl FieldType {
    pub const ALL: [FieldType; 9] = [
        FieldType::Plain,
        FieldType::Options,
        FieldType::OptionsFromTag,
        FieldType::Date,
        FieldType::Number,
        FieldType::Url,
        FieldType::Email,
        FieldType::Checkbox,
        FieldType::User,
    ];

    pub fn name(self) -> &'static str {
        match self {
            FieldType::Plain => "plain",
            FieldType::Options => "options",
            FieldType::OptionsFromTag => "options_from_tag",
            FieldType::Date => "date",
            FieldType::Number => "number",
            FieldType::Url => "url",
            FieldType::Email => "email",
            FieldType::Checkbox => "checkbox",
            FieldType::User => "user",
        }
    }

    pub fn parse(s: &str) -> Option<FieldType> {
        FieldType::ALL.into_iter().find(|t| t.name().eq_ignore_ascii_case(s.trim()))
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct FieldDef {
    #[serde(default)]
    pub field_type: FieldType,
    /// Choices for `Options` fields.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
    /// Source supertag for `OptionsFromTag`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_tag: Option<NodeId>,
    /// Value a node gets when a supertag adds this field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<FieldValue>,
    /// Show a warning while empty.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub required: bool,
    #[serde(default, skip_serializing_if = "is_never")]
    pub hide: HideMode,
    /// Number fields: allowed range.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_init: Option<AutoInit>,
    /// Sources of options: nodes whose children are offered as options too.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<NodeId>,
    /// New typed values are not added to the options ("Auto-collect values" off).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub no_auto_collect: bool,
    /// Command nodes shown as buttons on the field's rows; a click runs one on the field's node
    /// (UC-06.33).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub buttons: Vec<NodeId>,
}

fn is_never(h: &HideMode) -> bool {
    *h == HideMode::Never
}

/// A field value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum FieldValue {
    Text(String),
    Number(f64),
    Date(Date),
    /// A date with a time range (minutes after midnight, `start < end <= 1440`).
    Timed {
        date: Date,
        start: u16,
        end: u16,
    },
    /// A range of whole days, `start < end` ("Add end" in the date picker, spec 03 §8).
    Span {
        start: Date,
        end: Date,
    },
    Checkbox(bool),
    Url(String),
    Email(String),
    /// A chosen option (`Options`).
    Choice(String),
    /// A node (`OptionsFromTag`, or any node reference).
    Node(NodeId),
}

/// `HH:MM` as minutes after midnight (`24:00` allowed as an end).
pub fn parse_hm(s: &str) -> Option<u16> {
    let (h, m) = s.trim().split_once(':')?;
    let (h, m): (u16, u16) = (h.parse().ok()?, m.parse().ok()?);
    (m < 60 && (h < 24 || (h == 24 && m == 0))).then_some(h * 60 + m)
}

/// `HH:MM`.
pub fn fmt_hm(min: u16) -> String {
    format!("{:02}:{:02}", min / 60, min % 60)
}

/// Days a span may cover (a hostile file cannot make a calendar loop for years).
pub const MAX_SPAN_DAYS: i64 = 3660;

/// A date, optionally followed by `HH:MM` or `HH:MM-HH:MM` (one hour when no end is given), or a
/// span of days `YYYY-MM-DD/YYYY-MM-DD`.
fn parse_date_value(s: &str) -> Result<FieldValue, String> {
    let bad = || format!("`{s}` is not a date (YYYY-MM-DD, optionally HH:MM-HH:MM, or YYYY-MM-DD/YYYY-MM-DD)");
    if let Some((a, b)) = s.split_once('/') {
        let (start, end) = (Date::parse(a.trim()).ok_or_else(bad)?, Date::parse(b.trim()).ok_or_else(bad)?);
        if end <= start {
            return Err(format!("`{s}`: the end must be after the start"));
        }
        if end.to_days() - start.to_days() > MAX_SPAN_DAYS {
            return Err(format!("`{s}`: a span is at most ten years"));
        }
        return Ok(FieldValue::Span { start, end });
    }
    if let Some((day, time)) = s.rsplit_once(' ')
        && time.contains(':')
    {
        let date = Date::parse(day).ok_or_else(bad)?;
        let (a, b) = match time.split_once('-') {
            Some((a, b)) => (parse_hm(a).ok_or_else(bad)?, parse_hm(b).ok_or_else(bad)?),
            None => {
                let a = parse_hm(time).ok_or_else(bad)?;
                (a, (a + 60).min(1440))
            }
        };
        if a >= b || a >= 1440 {
            return Err(format!("`{time}` must start before it ends"));
        }
        return Ok(FieldValue::Timed { date, start: a, end: b });
    }
    Ok(FieldValue::Date(Date::parse(s).ok_or_else(bad)?))
}

impl FieldValue {
    /// Whether a URL or e-mail value looks right (the UI warns otherwise; spec 03 §8).
    pub fn is_valid(&self) -> bool {
        match self {
            FieldValue::Url(u) => (u.starts_with("https://") || u.starts_with("http://")) && u.len() > 8 && !u.contains(' '),
            FieldValue::Email(e) => {
                let (user, host) = e.split_once('@').unwrap_or(("", ""));
                !user.is_empty() && host.contains('.') && !host.starts_with('.') && !host.ends_with('.') && !e.contains(' ')
            }
            _ => true,
        }
    }

    /// The day of a date value (timed or not; a span's first day).
    pub fn date(&self) -> Option<Date> {
        match self {
            FieldValue::Date(d) | FieldValue::Timed { date: d, .. } | FieldValue::Span { start: d, .. } => Some(*d),
            _ => None,
        }
    }

    /// Whether a date value falls on `d` (a span covers every day from start to end).
    pub fn covers(&self, d: Date) -> bool {
        match self {
            FieldValue::Span { start, end } => *start <= d && d <= *end,
            other => other.date() == Some(d),
        }
    }

    /// Whether a date value falls on any day from `a` to `b` (a span that overlaps counts).
    pub fn within(&self, a: Date, b: Date) -> bool {
        match self {
            FieldValue::Span { start, end } => *start <= b && *end >= a,
            other => other.date().is_some_and(|d| a <= d && d <= b),
        }
    }

    /// Parses typed text into a value of `ty`. `Node` values are resolved by the graph.
    pub fn parse(ty: FieldType, s: &str) -> Result<FieldValue, String> {
        let s = s.trim();
        Ok(match ty {
            FieldType::Plain | FieldType::OptionsFromTag | FieldType::User => FieldValue::Text(s.to_string()),
            FieldType::Options => FieldValue::Choice(s.to_string()),
            FieldType::Number => FieldValue::Number(s.parse::<f64>().ok().filter(|f| f.is_finite()).ok_or_else(|| format!("`{s}` is not a number"))?),
            FieldType::Date => parse_date_value(s)?,
            FieldType::Checkbox => FieldValue::Checkbox(match s.to_ascii_lowercase().as_str() {
                "yes" | "true" | "1" | "x" | "on" => true,
                "no" | "false" | "0" | "" | "off" => false,
                _ => return Err(format!("`{s}` is not yes/no")),
            }),
            FieldType::Url => FieldValue::Url(s.to_string()),
            // Kept as typed; an invalid address only gets a warning (spec 03 §8).
            FieldType::Email => FieldValue::Email(s.to_string()),
        })
    }
}

/// One field on a node: which field and its values (a field can hold several, spec 01 §5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldEntry {
    pub field: NodeId,
    /// The first value.
    pub value: Option<FieldValue>,
    /// Further values, after `value`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub more: Vec<FieldValue>,
    /// The child the field row is shown after, when it was added among the children (`>` on a
    /// line, spec 02 §11). `None`, or a child that has gone: above the children.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<NodeId>,
}

impl FieldEntry {
    pub fn new(field: NodeId, value: Option<FieldValue>) -> FieldEntry {
        FieldEntry { field, value, more: Vec::new(), after: None }
    }

    /// All values in order.
    pub fn values(&self) -> impl Iterator<Item = &FieldValue> {
        self.value.iter().chain(self.more.iter())
    }

    /// Replaces all values.
    pub fn set_values(&mut self, mut vals: Vec<FieldValue>) {
        self.value = if vals.is_empty() { None } else { Some(vals.remove(0)) };
        self.more = vals;
    }
}

/// Where a trashed node came from, so it can be restored.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TrashedFrom {
    pub parent: NodeId,
    pub index: usize,
}

/// A node: every bullet is one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    /// The node's content (one line, no line breaks).
    #[serde(default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "is_plain")]
    pub kind: Kind,
    /// The owner. `None` only for the workspace root.
    #[serde(default)]
    pub parent: Option<NodeId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<NodeId>,
    /// Supertags applied to this node.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<NodeId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<FieldEntry>,
    /// `Some(done)` shows a checkbox.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub done: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trashed_from: Option<TrashedFrom>,
    /// Shows a done/total progress bar of its checkbox children.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub progress: bool,
    /// Pinned to the sidebar.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pinned: bool,
    /// Shown as a heading (bigger, bold).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub heading: bool,
    /// Locked (UC-06.35): its text, tags, fields and children cannot be changed until unlocked.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub locked: bool,
    /// Command nodes run on each new child of this node (UC-02.20).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_child: Vec<NodeId>,
    /// The page's content width: `full` across the panel; absent is the medium column (UC-06.44).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<String>,
    /// An attached file (UC-05.08): its name in the graph's files folder (see `files_dir`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// Page decorations: an icon (a short text, usually one emoji) and a banner colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub banner: Option<String>,
    /// How the node shows its items (list when absent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<crate::view::ViewDef>,
    /// Unix seconds.
    #[serde(default)]
    pub created: u64,
    /// Last change (Unix seconds).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub edited: u64,
    /// When the checkbox was checked (Unix seconds).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub done_at: Option<u64>,
    /// The assistant (MCP client) whose edit made or last changed this node's content; `None`
    /// when a person did (see [`crate::Graph::stamp_by`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
}

fn is_zero(x: &u64) -> bool {
    *x == 0
}

/// Unix seconds now.
pub fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn is_plain(k: &Kind) -> bool {
    *k == Kind::Plain
}

impl Node {
    pub fn new(id: NodeId, name: impl Into<String>, parent: Option<NodeId>) -> Node {
        let created = now_secs();
        Node {
            id,
            name: name.into(),
            description: String::new(),
            kind: Kind::Plain,
            parent,
            children: Vec::new(),
            tags: Vec::new(),
            fields: Vec::new(),
            done: None,
            trashed_from: None,
            view: None,
            heading: false,
            locked: false,
            on_child: Vec::new(),
            width: None,
            file: None,
            icon: None,
            banner: None,
            pinned: false,
            progress: false,
            created,
            edited: created,
            done_at: None,
            by: None,
        }
    }

    pub fn reference_target(&self) -> Option<NodeId> {
        match self.kind {
            Kind::Reference { target } => Some(target),
            _ => None,
        }
    }

    /// The first value of `field`.
    pub fn field_value(&self, field: NodeId) -> Option<&FieldValue> {
        self.fields.iter().find(|f| f.field == field).and_then(|f| f.value.as_ref())
    }

    /// Every value of `field`.
    pub fn field_values(&self, field: NodeId) -> Vec<&FieldValue> {
        self.fields.iter().find(|f| f.field == field).map(|f| f.values().collect()).unwrap_or_default()
    }

    /// True when any field holds a date (timed or not) on `d`.
    pub fn has_date(&self, d: Date) -> bool {
        self.fields.iter().any(|f| f.values().any(|x| x.covers(d)))
    }

    /// True when any field holds `v`.
    pub fn has_value(&self, v: &FieldValue) -> bool {
        self.fields.iter().any(|f| f.values().any(|x| x == v))
    }
}
