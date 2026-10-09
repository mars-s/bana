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
    #[default]
    List,
    Table,
    Cards,
    /// Lanes: one column per group.
    Board,
    Calendar,
}

impl ViewKind {
    pub const ALL: [ViewKind; 5] = [ViewKind::List, ViewKind::Table, ViewKind::Cards, ViewKind::Board, ViewKind::Calendar];

    pub fn name(self) -> &'static str {
        match self {
            ViewKind::List => "list",
            ViewKind::Table => "table",
            ViewKind::Cards => "cards",
            ViewKind::Board => "board",
            ViewKind::Calendar => "calendar",
        }
    }

    pub fn parse(s: &str) -> Option<ViewKind> {
        let s = s.trim().to_ascii_lowercase();
        ViewKind::ALL.into_iter().find(|k| k.name() == s || (s == "lanes" && *k == ViewKind::Board))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "by", content = "field", rename_all = "snake_case")]
pub enum SortBy {
    Name,
    Created,
    Done,
    Field(NodeId),
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
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ViewDef {
    #[serde(default)]
    pub kind: ViewKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<Sort>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<GroupBy>,
    /// Extra conditions on the items (ANDed with the search query).
    #[serde(default, skip_serializing_if = "Query::is_empty")]
    pub filter: Query,
    /// Fields shown as table columns and card lines. Empty: every field the items have.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<NodeId>,
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

pub const NO_VALUE: &str = "No value";

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

    pub fn view_def(&self, id: NodeId) -> ViewDef {
        self.get(self.resolve(id)).and_then(|n| n.view.clone()).unwrap_or_default()
    }

    /// Items after the view's filter and sort, split into groups.
    pub fn view_groups(&self, id: NodeId) -> Vec<Group> {
        let v = self.view_def(id);
        let mut items: Vec<NodeId> = self.view_items(id).into_iter().filter(|i| v.filter.filter(self, self.resolve(*i))).collect();
        if let Some(sort) = v.sort {
            items.sort_by(|a, b| {
                let o = self.sort_key_cmp(self.resolve(*a), self.resolve(*b), sort.by);
                if sort.desc { o.reverse() } else { o }
            });
        }
        let Some(group) = v.group else {
            return vec![Group { label: String::new(), items }];
        };
        let mut groups: Vec<Group> = Vec::new();
        for i in items {
            let label = self.group_label(self.resolve(i), group);
            match groups.iter_mut().find(|g| g.label == label) {
                Some(g) => g.items.push(i),
                None => groups.push(Group { label, items: vec![i] }),
            }
        }
        // Options fields keep their option order; everything else is alphabetical. "No value" last.
        let order: Vec<String> = match group {
            GroupBy::Field(f) => self.field_def(f).map(|d| d.options.clone()).unwrap_or_default(),
            GroupBy::Done => vec!["Not done".into(), "Done".into()],
            GroupBy::Tags => Vec::new(),
        };
        groups.sort_by_key(|g| {
            let none = g.label == NO_VALUE;
            let pos = order.iter().position(|o| *o == g.label).unwrap_or(usize::MAX);
            (none, pos, g.label.to_lowercase())
        });
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
        };
        if label.is_empty() { NO_VALUE.to_string() } else { label }
    }

    fn sort_key_cmp(&self, a: NodeId, b: NodeId, by: SortBy) -> std::cmp::Ordering {
        let (na, nb) = (self.get(a), self.get(b));
        match by {
            SortBy::Name => crate::format::plain(self.name(a)).to_lowercase().cmp(&crate::format::plain(self.name(b)).to_lowercase()),
            SortBy::Created => na.map(|n| n.created).cmp(&nb.map(|n| n.created)).then(a.cmp(&b)),
            SortBy::Done => self.checkbox(a).cmp(&self.checkbox(b)),
            SortBy::Field(f) => {
                let va = na.and_then(|n| n.field_value(f));
                let vb = nb.and_then(|n| n.field_value(f));
                match (va, vb) {
                    (None, None) => std::cmp::Ordering::Equal,
                    // Empty values sort last.
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (Some(FieldValue::Number(x)), Some(FieldValue::Number(y))) => x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal),
                    (Some(FieldValue::Date(x)), Some(FieldValue::Date(y))) => x.cmp(y),
                    (Some(x), Some(y)) => display_value(self, x).to_lowercase().cmp(&display_value(self, y).to_lowercase()),
                }
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
        match self.get(self.resolve(item)).and_then(|n| n.field_value(field?)) {
            Some(FieldValue::Date(d)) => Some(*d),
            _ => None,
        }
    }
}
