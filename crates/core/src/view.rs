//! Views: how a node shows its items (its children, a search node's results, or a supertag's
//! tagged nodes) as a list, table, cards, board or calendar, with sort, group and filter.

use serde::{Deserialize, Serialize};

use crate::date::Date;
use crate::export::display_value;
use crate::graph::Graph;
use crate::model::{FieldType, FieldValue, Kind, NodeId};
use crate::query::Query;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewKind {
    /// Bullets (the default).
    #[default]
    Outline,
    Table,
    /// A grid of cards; grouped cards are laid out as board columns.
    Cards,
    /// Each item is a vertical lane listing its children.
    Lanes,
    /// Navigation view: a narrow list of items, the selected one shown beside it.
    List,
    /// Navigation view: a compact menu of items at the left of the content.
    SideMenu,
    /// Navigation view: items as tabs, the selected one shown below.
    Tabs,
    Calendar,
}

impl ViewKind {
    pub const ALL: [ViewKind; 8] = [
        ViewKind::Outline,
        ViewKind::Table,
        ViewKind::Cards,
        ViewKind::Lanes,
        ViewKind::List,
        ViewKind::SideMenu,
        ViewKind::Tabs,
        ViewKind::Calendar,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ViewKind::Outline => "outline",
            ViewKind::Table => "table",
            ViewKind::Cards => "cards",
            ViewKind::Lanes => "lanes",
            ViewKind::List => "list",
            ViewKind::SideMenu => "side_menu",
            ViewKind::Tabs => "tabs",
            ViewKind::Calendar => "calendar",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ViewKind::Outline => "Outline",
            ViewKind::Table => "Table",
            ViewKind::Cards => "Cards",
            ViewKind::Lanes => "Lanes",
            ViewKind::List => "List",
            ViewKind::SideMenu => "Side menu",
            ViewKind::Tabs => "Tabs",
            ViewKind::Calendar => "Calendar",
        }
    }

    /// Navigation views show one selected item beside or below the item list.
    pub fn is_navigation(self) -> bool {
        matches!(self, ViewKind::List | ViewKind::SideMenu | ViewKind::Tabs)
    }

    /// Accepts the names above, labels, and `board` (cards grouped into columns).
    pub fn parse(s: &str) -> Option<ViewKind> {
        let s = s.trim().to_ascii_lowercase().replace([' ', '-'], "_");
        if s == "board" {
            return Some(ViewKind::Cards);
        }
        ViewKind::ALL.into_iter().find(|k| k.name() == s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "by", content = "field", rename_all = "snake_case")]
pub enum SortBy {
    Name,
    Created,
    Done,
    Field(NodeId),
    /// Last edited time.
    Edited,
    /// When the checkbox was checked.
    DoneTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Sort {
    pub by: SortBy,
    #[serde(default)]
    pub desc: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "by", content = "field", rename_all = "snake_case")]
pub enum GroupBy {
    Field(NodeId),
    Tags,
    Done,
    /// The owner node.
    Owner,
    /// The day the node was created.
    Created,
    /// The day the node was last edited.
    Edited,
}

/// Fields every node has (spec 04 §11), usable as display fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemField {
    Created,
    Edited,
    DoneTime,
    Owner,
    Tags,
    References,
}

impl SystemField {
    pub const ALL: [SystemField; 6] =
        [SystemField::Created, SystemField::Edited, SystemField::DoneTime, SystemField::Owner, SystemField::Tags, SystemField::References];
    pub fn label(self) -> &'static str {
        match self {
            SystemField::Created => "Created time",
            SystemField::Edited => "Last edited time",
            SystemField::DoneTime => "Done time",
            SystemField::Owner => "Owner node",
            SystemField::Tags => "Tags",
            SystemField::References => "Number of references",
        }
    }
    pub fn parse(s: &str) -> Option<SystemField> {
        let s = s.trim().to_ascii_lowercase();
        SystemField::ALL
            .into_iter()
            .find(|f| f.label().to_ascii_lowercase() == s || format!("{f:?}").to_ascii_lowercase() == s.replace(['_', ' '], ""))
    }
}

/// A Unix time as a calendar day (UTC).
fn day_of(secs: u64) -> Date {
    Date::from_days((secs / 86_400) as i64)
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ViewDef {
    #[serde(default)]
    pub kind: ViewKind,
    /// Sort rules, applied in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sort: Vec<Sort>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<GroupBy>,
    /// Extra conditions on the items (ANDed with the search query).
    #[serde(default, skip_serializing_if = "Query::is_empty")]
    pub filter: Query,
    /// Display fields: shown under each item (outline, cards, lists) and as table columns. A table
    /// with none shows every field the items have.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<NodeId>,
    /// Table column widths in points (field id; `NodeId(0)` is the title column).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub widths: Vec<(NodeId, f32)>,
    /// Groups (board columns) the person hid.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden_groups: Vec<String>,
    /// The view toolbar is hidden.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hide_toolbar: bool,
    /// System fields shown under items (after the display fields).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub system: Vec<SystemField>,
    /// The date field a calendar places items by. `None`: the first date field the items have.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_field: Option<NodeId>,
}

/// One group of items (a board lane, a table section). Ungrouped views have one group, label "".
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub label: String,
    pub items: Vec<NodeId>,
}

pub const NO_VALUE: &str = "Not defined";

impl Graph {
    /// The items a node shows: a search node's results, a supertag's tagged nodes, or its children.
    pub fn view_items(&self, id: NodeId) -> Vec<NodeId> {
        let target = self.resolve(id);
        match self.get(target).map(|n| &n.kind) {
            Some(Kind::Search(q)) => q.run(self, target),
            Some(Kind::Tag(_)) => self.tagged(target),
            _ => self.children(target).to_vec(),
        }
    }

    /// A node's view settings. A supertag page shows its tagged nodes as a table by default.
    pub fn view_def(&self, id: NodeId) -> ViewDef {
        let n = self.get(self.resolve(id));
        match n.and_then(|n| n.view.clone()) {
            Some(v) => v,
            None if n.is_some_and(|n| matches!(n.kind, Kind::Tag(_))) => ViewDef { kind: ViewKind::Table, ..ViewDef::default() },
            None => ViewDef::default(),
        }
    }

    /// How many items the view's filter hides.
    pub fn view_filtered_out(&self, id: NodeId) -> usize {
        let v = self.view_def(id);
        if v.filter.is_empty() {
            return 0;
        }
        self.view_items(id).into_iter().filter(|i| !v.filter.filter(self, self.resolve(*i))).count()
    }

    /// The field values shown under an item for the view's display fields.
    pub fn display_line(&self, view: NodeId, item: NodeId) -> String {
        let v = self.view_def(view);
        let n = self.get(self.resolve(item));
        let item = self.resolve(item);
        v.columns
            .iter()
            .filter_map(|f| {
                n.and_then(|n| n.fields.iter().find(|e| e.field == *f))
                    .map(|e| e.values().map(|v| crate::export::shown_value(self, v)).collect::<Vec<_>>().join(", "))
            })
            .chain(v.system.iter().map(|f| self.system_value(item, *f)))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" · ")
    }

    /// Items after the view's filter and sort, split into groups.
    pub fn view_groups(&self, id: NodeId) -> Vec<Group> {
        let v = self.view_def(id);
        let mut items: Vec<NodeId> = self.view_items(id).into_iter().filter(|i| v.filter.filter(self, self.resolve(*i))).collect();
        if !v.sort.is_empty() {
            items.sort_by(|a, b| {
                let (a, b) = (self.resolve(*a), self.resolve(*b));
                v.sort
                    .iter()
                    .map(|s| {
                        let o = self.sort_key_cmp(a, b, s.by);
                        if s.desc { o.reverse() } else { o }
                    })
                    .find(|o| o.is_ne())
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        let Some(group) = v.group else {
            return vec![Group { label: String::new(), items }];
        };
        let hidden = v.hidden_groups.clone();
        let mut groups: Vec<Group> = Vec::new();
        // Every option of an options field gets a group, even an empty one (spec 04 §12).
        if let GroupBy::Field(f) = group {
            for o in self.field_options(f) {
                groups.push(Group { label: o, items: Vec::new() });
            }
        }
        for i in items {
            let label = self.group_label(self.resolve(i), group);
            match groups.iter_mut().find(|g| g.label == label) {
                Some(g) => g.items.push(i),
                None => groups.push(Group { label, items: vec![i] }),
            }
        }
        // Options fields keep their option order; everything else is alphabetical. "No value" last.
        let order: Vec<String> = match group {
            GroupBy::Field(f) => self.field_options(f),
            GroupBy::Done => vec!["Not done".into(), "Done".into()],
            GroupBy::Tags | GroupBy::Owner | GroupBy::Created | GroupBy::Edited => Vec::new(),
        };
        groups.sort_by_key(|g| {
            let none = g.label == NO_VALUE;
            let pos = order.iter().position(|o| *o == g.label).unwrap_or(usize::MAX);
            (none, pos, g.label.to_lowercase())
        });
        groups.retain(|g| !hidden.contains(&g.label));
        groups
    }

    fn group_label(&self, id: NodeId, by: GroupBy) -> String {
        let n = self.get(id);
        let label = match by {
            GroupBy::Field(f) => n.and_then(|n| n.field_value(f)).map(|v| display_value(self, v)).unwrap_or_default(),
            GroupBy::Tags => n.and_then(|n| n.tags.first()).map(|t| format!("#{}", self.name(*t))).unwrap_or_default(),
            GroupBy::Done => match self.checkbox(id) {
                Some(true) => "Done".into(),
                Some(false) => "Not done".into(),
                None => String::new(),
            },
            GroupBy::Owner => n.and_then(|n| n.parent).map(|p| self.text(p)).unwrap_or_default(),
            GroupBy::Created => n.map(|n| day_of(n.created).to_string()).unwrap_or_default(),
            GroupBy::Edited => n.map(|n| day_of(n.edited.max(n.created)).to_string()).unwrap_or_default(),
        };
        if label.is_empty() { NO_VALUE.to_string() } else { label }
    }

    fn sort_key_cmp(&self, a: NodeId, b: NodeId, by: SortBy) -> std::cmp::Ordering {
        let (na, nb) = (self.get(a), self.get(b));
        match by {
            SortBy::Name => self.text(a).to_lowercase().cmp(&self.text(b).to_lowercase()),
            SortBy::Created => na.map(|n| n.created).cmp(&nb.map(|n| n.created)).then(a.cmp(&b)),
            SortBy::Done => self.checkbox(a).cmp(&self.checkbox(b)),
            SortBy::Edited => na.map(|n| n.edited.max(n.created)).cmp(&nb.map(|n| n.edited.max(n.created))).then(a.cmp(&b)),
            SortBy::DoneTime => na.and_then(|n| n.done_at).cmp(&nb.and_then(|n| n.done_at)),
            SortBy::Field(f) => {
                let va = na.and_then(|n| n.field_value(f));
                let vb = nb.and_then(|n| n.field_value(f));
                match (va, vb) {
                    (None, None) => std::cmp::Ordering::Equal,
                    // Items without the value come first when ascending (observed, spec 04 §13).
                    (None, Some(_)) => std::cmp::Ordering::Less,
                    (Some(_), None) => std::cmp::Ordering::Greater,
                    (Some(FieldValue::Number(x)), Some(FieldValue::Number(y))) => x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal),
                    (Some(x), Some(y)) if x.date().is_some() && y.date().is_some() => {
                        let key = |v: &FieldValue| (v.date(), if let FieldValue::Timed { start, .. } = v { *start } else { 0 });
                        key(x).cmp(&key(y))
                    }
                    (Some(x), Some(y)) => display_value(self, x).to_lowercase().cmp(&display_value(self, y).to_lowercase()),
                }
            }
        }
    }

    /// A table column's width, if set.
    pub fn column_width(&self, id: NodeId, col: NodeId) -> Option<f32> {
        self.view_def(id).widths.iter().find(|(c, _)| *c == col).map(|(_, w)| *w)
    }

    /// A system field of a node, as text.
    pub fn system_value(&self, id: NodeId, f: SystemField) -> String {
        let Some(n) = self.get(id) else { return String::new() };
        match f {
            SystemField::Created => day_of(n.created).day_title(),
            SystemField::Edited => day_of(n.edited.max(n.created)).day_title(),
            SystemField::DoneTime => n.done_at.map(|t| day_of(t).day_title()).unwrap_or_default(),
            SystemField::Owner => n.parent.map(|p| self.text(p)).unwrap_or_default(),
            SystemField::Tags => n.tags.iter().map(|t| format!("#{}", self.name(*t))).collect::<Vec<_>>().join(" "),
            SystemField::References => {
                let c = self.references_to(id).len();
                if c == 0 { String::new() } else { format!("{c} references") }
            }
        }
    }

    /// Table columns: the view's chosen fields, else every field the items carry (first seen first).
    pub fn view_columns(&self, id: NodeId) -> Vec<NodeId> {
        let v = self.view_def(id);
        if !v.columns.is_empty() {
            return v.columns.into_iter().filter(|c| self.field_def(*c).is_some()).collect();
        }
        let mut out = Vec::new();
        for i in self.view_items(id).iter().take(500) {
            for f in self.get(self.resolve(*i)).map(|n| n.fields.as_slice()).unwrap_or(&[]) {
                if !out.contains(&f.field) {
                    out.push(f.field);
                }
            }
        }
        out
    }

    /// The date field a calendar view uses.
    pub fn calendar_field(&self, id: NodeId) -> Option<NodeId> {
        if let Some(f) = self.view_def(id).date_field {
            return Some(f);
        }
        self.view_columns(id).into_iter().find(|f| self.field_def(*f).is_some_and(|d| d.field_type == FieldType::Date))
    }

    /// The date an item sits on in a calendar view.
    pub fn item_date(&self, item: NodeId, field: Option<NodeId>) -> Option<Date> {
        self.get(self.resolve(item)).and_then(|n| n.field_value(field?)).and_then(FieldValue::date)
    }

    /// The time range of an item in a calendar view, when its date has one (minutes).
    pub fn item_time(&self, item: NodeId, field: Option<NodeId>) -> Option<(u16, u16)> {
        match self.get(self.resolve(item)).and_then(|n| n.field_value(field?)) {
            Some(FieldValue::Timed { start, end, .. }) => Some((*start, *end)),
            _ => None,
        }
    }
}
