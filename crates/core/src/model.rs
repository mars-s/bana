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
}

/// Supertag colours: twelve hues plus grey. Rendered from the UI theme, not stored as RGB.
pub const TAG_COLORS: [&str; 13] = ["grey", "red", "orange", "amber", "yellow", "lime", "green", "teal", "cyan", "blue", "indigo", "violet", "pink"];

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
}

impl FieldType {
    pub const ALL: [FieldType; 8] = [
        FieldType::Plain,
        FieldType::Options,
        FieldType::OptionsFromTag,
        FieldType::Date,
        FieldType::Number,
        FieldType::Url,
        FieldType::Email,
        FieldType::Checkbox,
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
    Checkbox(bool),
    Url(String),
    Email(String),
    /// A chosen option (`Options`).
    Choice(String),
    /// A node (`OptionsFromTag`, or any node reference).
    Node(NodeId),
}

impl FieldValue {
    /// Parses typed text into a value of `ty`. `Node` values are resolved by the graph.
    pub fn parse(ty: FieldType, s: &str) -> Result<FieldValue, String> {
        let s = s.trim();
        Ok(match ty {
            FieldType::Plain | FieldType::OptionsFromTag => FieldValue::Text(s.to_string()),
            FieldType::Options => FieldValue::Choice(s.to_string()),
            FieldType::Number => FieldValue::Number(s.parse::<f64>().ok().filter(|f| f.is_finite()).ok_or_else(|| format!("`{s}` is not a number"))?),
            FieldType::Date => FieldValue::Date(Date::parse(s).ok_or_else(|| format!("`{s}` is not a date (YYYY-MM-DD)"))?),
            FieldType::Checkbox => FieldValue::Checkbox(match s.to_ascii_lowercase().as_str() {
                "yes" | "true" | "1" | "x" | "on" => true,
                "no" | "false" | "0" | "" | "off" => false,
                _ => return Err(format!("`{s}` is not yes/no")),
            }),
            FieldType::Url => FieldValue::Url(s.to_string()),
            FieldType::Email if s.contains('@') => FieldValue::Email(s.to_string()),
            FieldType::Email => return Err(format!("`{s}` is not an email address")),
        })
    }
}

/// One field on a node: which field and its value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldEntry {
    pub field: NodeId,
    pub value: Option<FieldValue>,
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
            pinned: false,
            progress: false,
            created,
            edited: created,
            done_at: None,
        }
    }

    pub fn reference_target(&self) -> Option<NodeId> {
        match self.kind {
            Kind::Reference { target } => Some(target),
            _ => None,
        }
    }

    pub fn field_value(&self, field: NodeId) -> Option<&FieldValue> {
        self.fields.iter().find(|f| f.field == field).and_then(|f| f.value.as_ref())
    }
}
