//! Search pages and views (spec 04, 05): the condition editor, the view toolbar (quick filter,
//! view as, display, group, sort, filter chips) and the table, cards / board, lanes, navigation
//! (list, side menu, tabs) and calendar layouts.

use egui::{Align2, Color32, FontId, Rect, RichText, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::json;
use tanacraft_core::query::{Clause, ClauseKind, Query};
use tanacraft_core::view::{Group, NO_VALUE};
use tanacraft_core::{Date, FieldType, GroupBy, NodeId, SortBy, ViewKind};

use crate::outline::{checkbox, chip, field_values, rich_label};
use crate::theme::{self, Tokens};
use crate::{Act, PickMode, TanaApp};

const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

/// Board cards are dragged with this payload (bullets use a bare `NodeId`).
#[derive(Debug, Clone, Copy)]
struct CardDrag(NodeId);

fn small_button(ui: &mut Ui, t: &Tokens, label: &str, active: bool) -> egui::Response {
    let b = egui::Button::new(RichText::new(label).size(13.0).color(if active { t.text } else { t.dim }))
        .fill(if active { t.hover } else { t.field_bg })
        .stroke(if active { Stroke::new(1.0, t.accent.gamma_multiply(0.8)) } else { Stroke::NONE })
        .corner_radius(5.0);
    ui.add(b)
}

fn clause_text(g: &tanacraft_core::Graph, c: &Clause) -> String {
    let one = Query { any: false, clauses: vec![Clause { not: false, kind: c.kind.clone() }] };
    one.describe(g)
}

// ---- condition editor ---------------------------------------------------------------------------

/// The query builder of a search node (spec 05 §2): a group box per AND / OR / NOT group, rows
/// for conditions, nested groups inside, an "Empty" line per group to type a condition, then
/// "+ AND / + OR / + NOT", operators, the query as text, Run once and Done.
pub fn query_builder(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, node: NodeId) {
    let Some(q) = app.session.graph.query(node).cloned() else { return };
    egui::Frame::new().fill(t.field_bg).stroke(Stroke::new(1.0, t.line)).corner_radius(8.0).inner_margin(egui::Margin::symmetric(14, 10)).show(
        ui,
        |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new("Query builder").strong().size(14.0));
                ui.label(RichText::new("Type words, #tag, DONE, TODO, Field:value, Field>value, CREATED:7").size(12.0).color(t.faint));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Label::new(RichText::new("x").color(t.dim)).sense(Sense::click())).on_hover_text("Close").clicked() {
                        app.acts.push(Act::Ui("ui.searchOptions"));
                    }
                });
            });
            ui.add_space(6.0);
            group_box(app, ui, t, node, &q, false, &[]);
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if small_button(ui, t, "+ Supertag", false).clicked() {
                    app.acts.push(Act::Pick(PickMode::SearchTag, node, ui.min_rect().left_bottom()));
                }
                ui.menu_button(RichText::new("Search operators").size(13.0).color(t.dim), |ui| {
                    for (label, clause) in [
                        ("TODO", "TODO"),
                        ("DONE", "DONE"),
                        ("DONE LAST 7 DAYS", "DONE:7"),
                        ("CREATED LAST 7 DAYS", "CREATED:7"),
                        ("EDITED LAST 7 DAYS", "EDITED:7"),
                        ("HAS TAG", "HAS:TAG"),
                        ("HAS FIELD", "HAS:FIELD"),
                        ("IS TAG", "IS:TAG"),
                        ("IS FIELD", "IS:FIELD"),
                        ("IS SEARCH NODE", "IS:SEARCH"),
                        ("IS CALENDAR NODE", "IS:CALENDAR"),
                        ("IN LIBRARY", "IN:LIBRARY"),
                        ("ON DAY NODE", "ON:DAY"),
                        ("FOR DATE today", "DATE:today"),
                    ] {
                        if ui.button(label).clicked() {
                            app.acts.push(Act::Run("search.addClause".into(), json!({"node": node.0, "clause": clause})));
                            ui.close();
                        }
                    }
                });
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let n = app.session.graph.view_items(node).len();
                ui.label(RichText::new(format!("{}   {n} results", q.describe(&app.session.graph))).size(12.5).color(t.faint));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Results follow every edit; Done also closes the builder.
                    if small_button(ui, t, "Done", true).clicked() {
                        app.acts.push(Act::Ui("ui.searchOptions"));
                    }
                    if small_button(ui, t, "Run once", false).on_hover_text("Results are live; this refreshes them").clicked() {
                        app.acts.push(Act::Run("search.setQuery".into(), json!({"node": node.0, "query": q.describe(&app.session.graph)})));
                    }
                });
            });
        },
    );
    ui.add_space(8.0);
}

/// One group: a strip on the left naming how its conditions combine, its rows on the right.
fn group_box(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, node: NodeId, q: &Query, not: bool, path: &[usize]) {
    let label = if not {
        "NOT"
    } else if q.any {
        "OR"
    } else {
        "AND"
    };
    ui.horizontal_top(|ui| {
        let strip = ui.allocate_exact_size(vec2(34.0, 24.0), Sense::click());
        let color = if not { Color32::from_rgb(232, 128, 96) } else { t.accent };
        ui.painter().text(strip.0.center(), Align2::CENTER_CENTER, label, FontId::proportional(12.0), color);
        if !not && strip.1.on_hover_text("Switch between AND and OR").clicked() {
            app.acts.push(Act::Run("search.setMode".into(), json!({"node": node.0, "any": !q.any, "path": path})));
        }
        let body = ui.vertical(|ui| {
            for (i, c) in q.clauses.iter().enumerate() {
                let mut here = path.to_vec();
                here.push(i);
                match &c.kind {
                    ClauseKind::Group { query } => {
                        egui::Frame::new().stroke(Stroke::new(1.0, t.line)).corner_radius(6.0).inner_margin(6.0).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let (word, help) = if c.not {
                                    ("NOT", "reverses what it holds")
                                } else if query.any {
                                    ("OR", "any one condition must be true")
                                } else {
                                    ("AND", "every condition must be true")
                                };
                                ui.label(RichText::new(word).strong().size(13.0));
                                ui.label(RichText::new(help).size(12.0).color(t.faint));
                                remove_button(app, ui, t, node, path, i);
                            });
                            group_box(app, ui, t, node, query, c.not, &here);
                        });
                    }
                    _ => {
                        let row = ui.horizontal(|ui| {
                            let g = &app.session.graph;
                            let (text, color) = match &c.kind {
                                ClauseKind::Tagged { tag } => {
                                    (format!("#{}", g.name(*tag)), g.tag_def(*tag).map(|d| theme::tag_color(&d.color, app.dark).0).unwrap_or(t.text))
                                }
                                _ => (clause_text(g, c), t.text),
                            };
                            if c.not {
                                ui.label(RichText::new("NOT").size(11.5).color(Color32::from_rgb(232, 128, 96)));
                            }
                            ui.label(RichText::new(text).size(13.5).color(color));
                            let hovered = ui.rect_contains_pointer(ui.max_rect());
                            let neg = ui.add(
                                egui::Label::new(RichText::new(if c.not { "un-NOT" } else { "NOT" }).size(11.0).color(if hovered {
                                    t.faint
                                } else {
                                    Color32::TRANSPARENT
                                }))
                                .sense(Sense::click()),
                            );
                            if neg.on_hover_text("Negate this condition").clicked() {
                                app.acts.push(Act::Run("search.toggleNot".into(), json!({"node": node.0, "index": i, "path": path})));
                            }
                            remove_button(app, ui, t, node, path, i);
                        });
                        let _ = row;
                    }
                }
            }
            // The "Empty" line of this group: type a condition, Enter adds it here.
            let key = egui::Id::new((app.panel_ix, "clause-input", node.0, path.to_vec()));
            let mut buf = ui.data_mut(|d| d.get_temp::<String>(key)).unwrap_or_default();
            let r = ui.add(egui::TextEdit::singleline(&mut buf).frame(egui::Frame::NONE).hint_text("Empty").desired_width(260.0));
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !buf.trim().is_empty() {
                app.acts.push(Act::Run("search.addClause".into(), json!({"node": node.0, "clause": buf, "path": path})));
                buf.clear();
                r.request_focus();
            }
            ui.data_mut(|d| d.insert_temp(key, buf));
            ui.horizontal(|ui| {
                for kind in ["and", "or", "not"] {
                    let label = format!("+ {}", kind.to_uppercase());
                    if ui
                        .add(egui::Label::new(RichText::new(label).size(12.0).color(t.dim)).sense(Sense::click()))
                        .on_hover_text("Add a group here")
                        .clicked()
                    {
                        app.acts.push(Act::Run("search.addGroup".into(), json!({"node": node.0, "kind": kind, "path": path})));
                    }
                }
            });
        });
        // The strip runs down the group's left edge.
        let r = body.response.rect;
        ui.painter().line_segment([pos2(r.left() - 8.0, r.top() + 4.0), pos2(r.left() - 8.0, r.bottom() - 4.0)], Stroke::new(1.0, t.line));
    });
}

fn remove_button(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, node: NodeId, path: &[usize], index: usize) {
    if ui.add(egui::Label::new(RichText::new("x").size(12.0).color(t.faint)).sense(Sense::click())).on_hover_text("Remove").clicked() {
        app.acts.push(Act::Run("search.removeClause".into(), json!({"node": node.0, "index": index, "path": path})));
    }
}

// ---- toolbar ------------------------------------------------------------------------------------

fn by_key(by: SortBy) -> String {
    match by {
        SortBy::Name => "name".into(),
        SortBy::Created => "created".into(),
        SortBy::Done => "done".into(),
        SortBy::Edited => "edited".into(),
        SortBy::DoneTime => "done_time".into(),
        SortBy::Field(f) => f.0.to_string(),
    }
}

/// The view toolbar of `node` (spec 04 §1).
pub fn toolbar(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, node: NodeId) {
    let g = &app.session.graph;
    let def = g.view_def(node);
    if def.hide_toolbar {
        return;
    }
    let fields = all_fields(app, node);
    // Pinned fields (a supertag's template marks them) come first in every menu (spec 03 §9).
    let pinned: Vec<NodeId> = g.tags().into_iter().filter_map(|x| g.tag_def(x)).flat_map(|d| d.pinned_fields.clone()).collect();
    let mut fields = fields;
    fields.sort_by_key(|f| !pinned.contains(f));
    let field_names: Vec<(NodeId, String, FieldType)> =
        fields.iter().map(|f| (*f, g.name(*f).to_string(), g.field_def(*f).map(|d| d.field_type).unwrap_or_default())).collect();
    let sort_label = |by: SortBy| match by {
        SortBy::Name => "Name".to_string(),
        SortBy::Created => "Created time".into(),
        SortBy::Done => "Done".into(),
        SortBy::Edited => "Last edited time".into(),
        SortBy::DoneTime => "Done time".into(),
        SortBy::Field(f) => g.name(f).to_string(),
    };
    let mut acts = Vec::new();
    ui.horizontal(|ui| {
        // Quick text filter.
        let open = app.quick_filter_open == Some(node);
        if open {
            let buf = app.quick_filter.entry(node).or_default();
            let r = ui.add(egui::TextEdit::singleline(buf).hint_text("Filter").desired_width(160.0));
            if app.quick_filter_focus {
                r.request_focus();
                app.quick_filter_focus = false;
            }
            if r.lost_focus() && buf.is_empty() {
                app.quick_filter_open = None;
            }
        } else if small_button(ui, t, "Search", app.quick_filter.get(&node).is_some_and(|s| !s.is_empty()))
            .on_hover_text("Filter the items by text")
            .clicked()
        {
            app.quick_filter_open = Some(node);
            app.quick_filter_focus = true;
        }
        ui.menu_button(RichText::new(format!("View: {}", def.kind.label())).size(13.0).color(t.dim), |ui| {
            for k in ViewKind::ALL {
                if k == ViewKind::List {
                    ui.separator();
                    ui.label(RichText::new("Navigation views").size(11.5).color(t.faint));
                }
                if ui.selectable_label(def.kind == k, k.label()).clicked() {
                    acts.push(Act::Run("view.set".into(), json!({"node": node.0, "kind": k.name()})));
                    ui.close();
                }
            }
            ui.separator();
            if ui.button("Board (cards grouped)").clicked() {
                acts.push(Act::Run("view.set".into(), json!({"node": node.0, "kind": "board"})));
                ui.close();
            }
        });
        ui.menu_button(
            RichText::new(if def.columns.is_empty() && def.system.is_empty() {
                "Display".to_string()
            } else {
                format!("Display ({})", def.columns.len() + def.system.len())
            })
            .size(13.0)
            .color(t.dim),
            |ui| {
                ui.label(RichText::new("Display fields").size(12.0).color(t.faint));
                for (f, name, _) in &field_names {
                    let mut on = def.columns.contains(f);
                    if ui.checkbox(&mut on, name).changed() {
                        acts.push(Act::Run("view.toggleDisplay".into(), json!({"node": node.0, "field": f.0})));
                    }
                }
                if field_names.is_empty() {
                    ui.label(RichText::new("The items have no fields yet").color(t.faint));
                }
            },
        );
        if def.kind != ViewKind::Table {
            let group_label = match def.group {
                None => "Group".to_string(),
                Some(GroupBy::Tags) => "Group: tags".into(),
                Some(GroupBy::Done) => "Group: done".into(),
                Some(GroupBy::Owner) => "Group: owner".into(),
                Some(GroupBy::Created) => "Group: created".into(),
                Some(GroupBy::Edited) => "Group: edited".into(),
                Some(GroupBy::Field(f)) => format!("Group: {}", g.name(f)),
            };
            ui.menu_button(RichText::new(group_label).size(13.0).color(t.dim), |ui| {
                ui.label(RichText::new("Group by").size(12.0).color(t.faint));
                for hidden in &def.hidden_groups {
                    if ui.button(format!("Show hidden group: {hidden}")).clicked() {
                        acts.push(Act::Run("view.toggleGroupHidden".into(), json!({"node": node.0, "group": hidden})));
                        ui.close();
                    }
                }
                for (label, by) in [
                    ("None", "none".to_string()),
                    ("Tags", "tags".into()),
                    ("Done", "done".into()),
                    ("Owner node", "owner".into()),
                    ("Created time", "created".into()),
                    ("Last edited time", "edited".into()),
                ] {
                    if ui.button(label).clicked() {
                        acts.push(Act::Run("view.group".into(), json!({"node": node.0, "by": by})));
                        ui.close();
                    }
                }
                for (f, name, _) in &field_names {
                    if ui.button(name).clicked() {
                        acts.push(Act::Run("view.group".into(), json!({"node": node.0, "by": f.0.to_string()})));
                        ui.close();
                    }
                }
            });
        }
        let sort_text = match def.sort.first() {
            None => "Sort".to_string(),
            Some(r) => format!("Sort: {}{}", sort_label(r.by), if def.sort.len() > 1 { format!(" +{}", def.sort.len() - 1) } else { String::new() }),
        };
        ui.menu_button(RichText::new(sort_text).size(13.0).color(t.dim), |ui| {
            ui.label(RichText::new("Sort by").size(12.0).color(t.faint));
            for (i, r) in def.sort.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(sort_label(r.by));
                    if ui.button(if r.desc { "Descending" } else { "Ascending" }).clicked() {
                        acts.push(Act::Run("view.addSort".into(), json!({"node": node.0, "by": by_key(r.by), "desc": !r.desc})));
                    }
                    if ui.button("-").on_hover_text("Remove").clicked() {
                        acts.push(Act::Run("view.removeSort".into(), json!({"node": node.0, "index": i})));
                    }
                });
            }
            ui.menu_button("+ Add sort", |ui| {
                for (label, by) in [
                    ("Name", "name".to_string()),
                    ("Created time", "created".into()),
                    ("Last edited time", "edited".into()),
                    ("Done", "done".into()),
                    ("Done time", "done_time".into()),
                ] {
                    if ui.button(label).clicked() {
                        acts.push(Act::Run("view.addSort".into(), json!({"node": node.0, "by": by})));
                        ui.close();
                    }
                }
                for (f, name, _) in &field_names {
                    if ui.button(name).clicked() {
                        acts.push(Act::Run("view.addSort".into(), json!({"node": node.0, "by": f.0.to_string()})));
                        ui.close();
                    }
                }
            });
            if !def.sort.is_empty() && ui.button(RichText::new("Reset").color(Color32::from_rgb(232, 128, 96))).clicked() {
                acts.push(Act::Run("view.sort".into(), json!({"node": node.0, "by": "none"})));
                ui.close();
            }
        });
        ui.menu_button(RichText::new("Filter by").size(13.0).color(t.dim), |ui| {
            ui.menu_button("Checked state", |ui| {
                for (label, clause) in [("Done", "DONE"), ("Not done", "TODO")] {
                    if ui.button(label).clicked() {
                        acts.push(Act::Run("view.addFilter".into(), json!({"node": node.0, "clause": clause})));
                        ui.close();
                    }
                }
            });
            ui.menu_button("Tags", |ui| {
                for tag in g.tags() {
                    if tag != g.sys.day_tag && ui.button(format!("#{}", g.name(tag))).clicked() {
                        acts.push(Act::Run("view.addFilter".into(), json!({"node": node.0, "clause": format!("#\"{}\"", g.name(tag))})));
                        ui.close();
                    }
                }
            });
            for (f, name, ty) in &field_names {
                let opts = g.field_options(*f);
                ui.menu_button(name, |ui| {
                    if ui.button("Has a value").clicked() {
                        acts.push(Act::Run("view.addFilter".into(), json!({"node": node.0, "clause": format!("\"{name}\":*")})));
                        ui.close();
                    }
                    if *ty == FieldType::Checkbox {
                        for v in ["Yes", "No"] {
                            if ui.button(v).clicked() {
                                acts.push(Act::Run("view.addFilter".into(), json!({"node": node.0, "clause": format!("\"{name}\":{v}")})));
                                ui.close();
                            }
                        }
                    }
                    for o in &opts {
                        if ui.button(o).clicked() {
                            acts.push(Act::Run("view.addFilter".into(), json!({"node": node.0, "clause": format!("\"{name}\":\"{o}\"")})));
                            ui.close();
                        }
                    }
                });
            }
        });
        if def.kind == ViewKind::Calendar {
            let date_fields: Vec<&(NodeId, String, FieldType)> = field_names.iter().filter(|(_, _, ty)| *ty == FieldType::Date).collect();
            ui.menu_button(RichText::new("Date field").size(13.0).color(t.dim), |ui| {
                for (f, name, _) in date_fields {
                    if ui.button(name).clicked() {
                        acts.push(Act::Run("view.dateField".into(), json!({"node": node.0, "field": f.0})));
                        ui.close();
                    }
                }
            });
        }
        // Active filters as chips.
        for (i, c) in def.filter.clauses.iter().enumerate() {
            let label = format!("{}{}  x", if c.not { "NOT " } else { "" }, clause_text(g, c));
            let b = egui::Button::new(RichText::new(label).size(12.5).color(t.link)).fill(t.link.gamma_multiply(0.15)).corner_radius(10.0);
            if ui.add(b).on_hover_text("Remove filter").clicked() {
                acts.push(Act::Run("view.removeFilter".into(), json!({"node": node.0, "index": i})));
            }
        }
    });
    app.acts.extend(acts);
    ui.add_space(4.0);
    let r = ui.available_rect_before_wrap();
    ui.painter().line_segment([pos2(r.left(), r.top()), pos2(r.right(), r.top())], Stroke::new(1.0, t.line));
    ui.add_space(8.0);
}

/// Every field the items carry (for menus), display fields first.
fn all_fields(app: &TanaApp, node: NodeId) -> Vec<NodeId> {
    let g = &app.session.graph;
    let mut out = g.view_def(node).columns;
    for i in g.view_items(node).iter().take(500) {
        for f in g.get(g.resolve(*i)).map(|n| n.fields.as_slice()).unwrap_or(&[]) {
            if !out.contains(&f.field) {
                out.push(f.field);
            }
        }
    }
    if let Some(tag) = g.tag_def(node).map(|_| node) {
        for f in g.template_fields(tag) {
            if !out.contains(&f) {
                out.push(f);
            }
        }
    }
    out
}

/// The view's groups after the quick text filter.
fn groups_for(app: &TanaApp, node: NodeId) -> Vec<Group> {
    let g = &app.session.graph;
    let mut groups = g.view_groups(node);
    if let Some(q) = app.quick_filter.get(&node).map(|q| q.trim().to_lowercase()).filter(|q| !q.is_empty()) {
        for gr in &mut groups {
            gr.items.retain(|i| g.text(*i).to_lowercase().contains(&q));
        }
    }
    groups
}

/// Whether the outline of a page is replaced by a view layout.
pub fn uses_layout(app: &TanaApp, node: NodeId) -> bool {
    let g = &app.session.graph;
    let d = g.view_def(node);
    d.kind != ViewKind::Outline || d.group.is_some() || g.tag_def(node).is_some() || app.quick_filter.get(&node).is_some_and(|q| !q.trim().is_empty())
}

/// Draws the items of `node` in its view.
pub fn render(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, node: NodeId) {
    let kind = app.session.graph.view_def(node).kind;
    let groups = groups_for(app, node);
    match kind {
        ViewKind::Outline => grouped_outline(app, ui, t, node, &groups),
        ViewKind::Table => table(app, ui, t, node, &groups),
        ViewKind::Cards if app.session.graph.view_def(node).group.is_some() => board(app, ui, t, node, &groups),
        ViewKind::Cards => {
            add_button(app, ui, t, node, None);
            let w = ui.available_width();
            let cols = ((w + 12.0) / 302.0).floor().max(1.0);
            let card_w = ((w + 12.0) / cols - 12.0).max(200.0);
            for g in &groups {
                group_header(ui, t, &g.label, g.items.len());
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(12.0, 12.0);
                    for i in &g.items {
                        card(app, ui, t, node, *i, card_w - 22.0, false);
                    }
                });
            }
        }
        ViewKind::Lanes => lanes(app, ui, t, node, &groups),
        ViewKind::List | ViewKind::SideMenu | ViewKind::Tabs => navigation(app, ui, t, node, kind, &groups),
        ViewKind::Calendar => calendar(app, ui, t, node, &groups),
    }
    let hidden = app.session.graph.view_filtered_out(node);
    if hidden > 0 {
        ui.add_space(6.0);
        ui.label(RichText::new(format!("{hidden} items filtered out")).size(12.5).color(t.faint));
    }
}

fn group_header(ui: &mut Ui, t: &Tokens, label: &str, n: usize) {
    if label.is_empty() {
        return;
    }
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(16.0).strong().color(t.text));
        ui.label(RichText::new(n.to_string()).size(12.5).color(t.faint));
    });
    ui.add_space(2.0);
}

/// "+ Add": a new item that fits the view. Children of a plain node; for a search or supertag page,
/// a node in the Library carrying the query's supertags and field values, opened for typing
/// (spec 05 §1). `preset` fills the board column's value.
fn add_button(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, node: NodeId, preset: Option<(GroupBy, String)>) {
    if ui.add(egui::Label::new(RichText::new("+ Add").size(13.5).color(t.dim)).sense(Sense::click())).clicked() {
        let mut p = json!({"node": node.0, "name": ""});
        if let Some((GroupBy::Field(f), v)) = &preset {
            p["field"] = json!(f.0);
            p["value"] = json!(v);
        }
        app.acts.push(Act::Run("view.addItem".into(), p));
    }
}

/// One item as a line: bullet in the tag colour, checkbox, text, tag chips. Click opens it.
pub(crate) fn item_line(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, item: NodeId, with_chips: bool) {
    let g = &app.session.graph;
    let target = g.resolve(item);
    let color = g
        .get(target)
        .and_then(|n| n.tags.iter().find(|x| **x != g.sys.day_tag))
        .and_then(|x| g.tag_def(*x))
        .map(|d| theme::tag_color(&d.color, app.dark).0)
        .unwrap_or(t.dim);
    let (rect, _) = ui.allocate_exact_size(vec2(16.0, 22.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), 3.2, color);
    if let Some(done) = g.checkbox(target)
        && checkbox(ui, t, done, 14.0).clicked()
    {
        app.acts.push(Act::Run("node.toggleDone".into(), json!({"node": target.0})));
    }
    let g = &app.session.graph;
    let done = g.checkbox(target) == Some(true);
    let mut link_acts = Vec::new();
    let clicked = rich_label(ui, &mut link_acts, g, t, g.name(target), if done { t.dim } else { t.text }, false).clicked();
    crate::outline::title_suffix(ui, g, t, target, theme::FONT);
    if !link_acts.is_empty() {
        app.acts.extend(link_acts);
    } else if clicked {
        app.acts.push(Act::Run("nav.open".into(), json!({"node": target.0})));
    }
    if with_chips {
        for tag in g.get(target).map(|n| n.tags.clone()).unwrap_or_default() {
            if tag != g.sys.day_tag && chip(ui, g, tag, app.dark).clicked() {
                app.acts.push(Act::Run("nav.open".into(), json!({"node": tag.0})));
            }
        }
    }
}

/// An item line with its display fields underneath.
fn item_block(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, view: NodeId, item: NodeId) {
    ui.horizontal(|ui| {
        ui.set_min_height(26.0);
        item_line(app, ui, t, item, true);
    });
    let line = app.session.graph.display_line(view, item);
    if !line.is_empty() {
        ui.horizontal(|ui| {
            ui.add_space(38.0);
            ui.label(RichText::new(line).size(12.5).color(t.dim));
        });
    }
}

/// Outline layout for supertag pages and grouped outlines: group headers with their items.
fn grouped_outline(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, node: NodeId, groups: &[Group]) {
    for g in groups {
        if !g.label.is_empty() {
            let key = (node, g.label.clone());
            let closed = app.closed_groups.contains(&key);
            ui.add_space(8.0);
            let r = ui.horizontal(|ui| {
                ui.label(RichText::new(if closed { ">" } else { "v" }).size(12.0).color(t.faint));
                ui.label(RichText::new(&g.label).size(16.0).strong().color(t.text));
                ui.label(RichText::new(g.items.len().to_string()).size(12.5).color(t.faint));
            });
            if r.response.interact(Sense::click()).clicked() {
                if closed {
                    app.closed_groups.remove(&key);
                } else {
                    app.closed_groups.insert(key);
                }
            }
            if closed {
                continue;
            }
        }
        if g.items.is_empty() && !g.label.is_empty() {
            let (r, _) = ui.allocate_exact_size(vec2(20.0, 22.0), Sense::hover());
            ui.painter().circle_filled(r.center(), 2.5, t.faint.gamma_multiply(0.6));
        }
        for i in g.items.iter().take(1000) {
            item_block(app, ui, t, node, *i);
        }
    }
    add_button(app, ui, t, node, None);
}

fn table(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, node: NodeId, groups: &[Group]) {
    let cols = app.session.graph.view_columns(node);
    let total = ui.available_width();
    // Column widths: saved per view, live while a header edge is dragged (spec screenshot 154).
    let width_of = |app: &TanaApp, c: NodeId, default: f32| -> f32 {
        app.col_drag
            .filter(|(v, cc, _)| *v == node && *cc == c)
            .map(|(_, _, w)| w)
            .or_else(|| app.session.graph.column_width(node, c))
            .unwrap_or(default)
    };
    let col_ws: Vec<f32> = cols.iter().map(|c| width_of(app, *c, 160.0)).collect();
    let fixed: f32 = col_ws.iter().sum();
    let name_w = width_of(app, NodeId(0), (total - fixed - 70.0).max(240.0));
    let sort = app.session.graph.view_def(node).sort;
    let tag_color = app.session.graph.tag_def(node).map(|d| theme::tag_color(&d.color, app.dark).0);
    let header = ui.horizontal(|ui| {
        ui.set_min_height(26.0);
        let g = &app.session.graph;
        // One supertag across the items: "# tag"; mixed: "Title" (spec 03 §10).
        let first_tags: std::collections::BTreeSet<NodeId> =
            groups.iter().flat_map(|gr| gr.items.iter()).filter_map(|i| g.get(g.resolve(*i)).and_then(|n| n.tags.first().copied())).collect();
        let name_label = if g.tag_def(node).is_some() {
            format!("# {}", g.name(node))
        } else if first_tags.len() == 1 {
            first_tags.iter().next().map(|t| format!("# {}", g.name(*t))).unwrap_or_default()
        } else {
            "Title".to_string()
        };
        let (r, resp) = ui.allocate_exact_size(vec2(name_w, 24.0), Sense::click());
        ui.painter().text(r.left_center() + vec2(4.0, 0.0), Align2::LEFT_CENTER, name_label, FontId::proportional(13.5), tag_color.unwrap_or(t.dim));
        if let Some(c) = tag_color {
            ui.painter().line_segment([r.left_bottom() + vec2(0.0, 2.0), r.right_bottom() + vec2(0.0, 2.0)], Stroke::new(2.0, c));
        }
        if resp.clicked() {
            let desc = sort.first().is_some_and(|s| s.by == SortBy::Name && !s.desc);
            app.acts.push(Act::Run("view.sort".into(), json!({"node": node.0, "by": "name", "desc": desc})));
        }
        resize_handle(app, ui, t, node, NodeId(0), r, "title");
        for (ci, f) in cols.iter().enumerate() {
            let col_w = col_ws.get(ci).copied().unwrap_or(160.0);
            let (r, resp) = ui.allocate_exact_size(vec2(col_w, 24.0), Sense::click());
            resize_handle(app, ui, t, node, *f, r, &f.0.to_string());
            let g = &app.session.graph;
            let mut label = g.name(*f).to_string();
            if let Some(s) = sort.first().filter(|s| s.by == SortBy::Field(*f)) {
                label.push_str(if s.desc { "  v" } else { "  ^" });
            }
            ui.painter().with_clip_rect(r).text(r.left_center() + vec2(4.0, 0.0), Align2::LEFT_CENTER, label, FontId::proportional(13.5), t.dim);
            if resp.clicked() {
                let desc = sort.first().is_some_and(|s| s.by == SortBy::Field(*f) && !s.desc);
                app.acts.push(Act::Run("view.sort".into(), json!({"node": node.0, "by": f.0.to_string(), "desc": desc})));
            }
            resp.context_menu(|ui| {
                if ui.button("Open field").clicked() {
                    app.acts.push(Act::Run("nav.open".into(), json!({"node": f.0})));
                    ui.close();
                }
                if ui.button("Remove column").clicked() {
                    app.acts.push(Act::Run("view.removeColumn".into(), json!({"node": node.0, "field": f.0})));
                    ui.close();
                }
            });
        }
        if ui
            .add(egui::Label::new(RichText::new("+ Add field").size(13.0).color(t.dim)).sense(Sense::click()))
            .on_hover_text("Add a field column")
            .clicked()
        {
            app.acts.push(Act::Pick(PickMode::Column, node, ui.min_rect().right_bottom()));
        }
    });
    let hr = header.response.rect;
    let line_end = hr.left() + name_w + fixed;
    ui.painter().line_segment([hr.left_bottom(), pos2(line_end, hr.bottom())], Stroke::new(1.0, t.line));
    for gr in groups {
        group_header(ui, t, &gr.label, gr.items.len());
        for item in gr.items.iter().take(1000) {
            let row = ui.horizontal(|ui| {
                ui.set_min_height(30.0);
                cell(ui, name_w, |ui| item_line(app, ui, t, *item, false));
                let target = app.session.graph.resolve(*item);
                for (ci, f) in cols.iter().enumerate() {
                    cell(ui, col_ws.get(ci).copied().unwrap_or(160.0), |ui| {
                        let g = &app.session.graph;
                        let Some(def) = g.field_def(*f).cloned() else { return };
                        let values: Vec<_> = g.get(target).map(|n| n.field_values(*f).into_iter().cloned().collect()).unwrap_or_default();
                        let edit_ix = app.field_edit.as_ref().filter(|e| e.node == target && e.field == *f).map(|e| e.index);
                        field_values(app, ui, t, target, *f, &def, &values, edit_ix);
                    });
                }
            });
            let r = row.response.rect;
            ui.painter().line_segment([r.left_bottom(), pos2(line_end, r.bottom())], Stroke::new(1.0, t.line));
        }
    }
    ui.add_space(4.0);
    add_button(app, ui, t, node, None);
}

/// A drag handle on the right edge of a table header cell.
fn resize_handle(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, node: NodeId, col: NodeId, cell: Rect, key: &str) {
    let edge = Rect::from_center_size(cell.right_center(), vec2(8.0, cell.height()));
    let resp = ui.interact(edge, egui::Id::new((app.panel_ix, "col-resize", node.0, col.0)), Sense::drag());
    if resp.hovered() || resp.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        ui.painter().line_segment([edge.center_top(), edge.center_bottom()], Stroke::new(2.0, t.accent));
    }
    if resp.dragged() {
        let w = (cell.width() + resp.drag_delta().x).clamp(60.0, 1200.0);
        app.col_drag = Some((node, col, w));
    }
    if resp.drag_stopped()
        && let Some((_, _, w)) = app.col_drag.take()
    {
        app.acts.push(Act::Run(
            "view.setColumnWidth".into(),
            json!({"node": node.0, "field": if col == NodeId(0) { json!("title") } else { json!(key) }, "width": w}),
        ));
    }
}

/// A fixed-width table cell: content is clipped, never widens the row.
/// A table cell `w` wide: at least one row high, taller when its text wraps.
fn cell(ui: &mut Ui, w: f32, add: impl FnOnce(&mut Ui)) {
    let top = ui.cursor().min;
    let rect = Rect::from_min_size(top, vec2(w, 30.0));
    let mut child =
        ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(vec2(4.0, 0.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
    let clip = Rect::from_min_max(pos2(rect.left() + 2.0, ui.clip_rect().top()), pos2(rect.right() - 2.0, ui.clip_rect().bottom()));
    child.set_clip_rect(clip.intersect(ui.clip_rect()));
    add(&mut child);
    let h = child.min_rect().bottom().max(rect.bottom()) - top.y;
    ui.allocate_rect(Rect::from_min_size(top, vec2(w, h)), Sense::hover());
}

/// A card: header (bullet + tag chips), body (checkbox + name), footer (display fields).
fn card(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, node: NodeId, item: NodeId, width: f32, draggable: bool) {
    let target = app.session.graph.resolve(item);
    let frame =
        egui::Frame::new().fill(t.field_bg).stroke(Stroke::new(1.0, t.line)).corner_radius(6.0).inner_margin(egui::Margin::symmetric(10, 8)).show(
            ui,
            |ui| {
                ui.set_width(width);
                ui.vertical(|ui| {
                    let g = &app.session.graph;
                    let tags = g.get(target).map(|n| n.tags.clone()).unwrap_or_default();
                    if !tags.is_empty() {
                        ui.horizontal(|ui| {
                            let color = tags.first().and_then(|x| g.tag_def(*x)).map(|d| theme::tag_color(&d.color, app.dark).0).unwrap_or(t.dim);
                            let (r, _) = ui.allocate_exact_size(vec2(12.0, 20.0), Sense::hover());
                            ui.painter().circle_filled(r.center(), 3.2, color);
                            for tag in tags {
                                if chip(ui, g, tag, app.dark).clicked() {
                                    app.acts.push(Act::Run("nav.open".into(), json!({"node": tag.0})));
                                }
                            }
                        });
                        let r = ui.available_rect_before_wrap();
                        ui.painter()
                            .line_segment([pos2(r.left() - 10.0, r.top() + 3.0), pos2(r.right() + 10.0, r.top() + 3.0)], Stroke::new(1.0, t.line));
                        ui.add_space(6.0);
                    }
                    ui.horizontal_wrapped(|ui| {
                        if let Some(done) = app.session.graph.checkbox(target)
                            && checkbox(ui, t, done, 14.0).clicked()
                        {
                            app.acts.push(Act::Run("node.toggleDone".into(), json!({"node": target.0})));
                        }
                        let g = &app.session.graph;
                        let done = g.checkbox(target) == Some(true);
                        let mut link_acts = Vec::new();
                        let clicked = rich_label(ui, &mut link_acts, g, t, g.name(target), if done { t.dim } else { t.text }, false).clicked();
                        crate::outline::title_suffix(ui, g, t, target, theme::FONT);
                        if !link_acts.is_empty() {
                            app.acts.extend(link_acts);
                        } else if clicked {
                            app.acts.push(Act::Run("nav.open".into(), json!({"node": target.0})));
                        }
                    });
                    let line = app.session.graph.display_line(node, item);
                    if !line.is_empty() {
                        ui.add_space(4.0);
                        ui.label(RichText::new(line).size(12.5).color(t.dim));
                    }
                });
            },
        );
    if draggable {
        let resp = ui.interact(frame.response.rect, egui::Id::new((app.panel_ix, "card", node.0, item.0)), Sense::drag());
        if resp.drag_started() {
            resp.dnd_set_drag_payload(CardDrag(target));
        }
        if resp.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        }
    }
}

/// Cards grouped into columns (board). Dropping a card on a column sets its value (spec 04 §5).
fn board(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, node: NodeId, groups: &[Group]) {
    let group = app.session.graph.view_def(node).group;
    let field_name = match group {
        Some(GroupBy::Field(f)) => app.session.graph.name(f).to_string(),
        _ => String::new(),
    };
    // A field board always ends with a "<Field> not set" column (DIFFS P2 #11).
    let mut groups = groups.to_vec();
    let hidden = app.session.graph.view_def(node).hidden_groups.iter().any(|h| h == NO_VALUE);
    if !field_name.is_empty() && !hidden && !groups.iter().any(|g| g.label == NO_VALUE) {
        groups.push(Group { label: NO_VALUE.to_string(), items: Vec::new() });
    }
    // Columns share the width (220 to 290 points each); more scroll sideways.
    let col_w = ((ui.available_width() - 16.0 * groups.len() as f32) / groups.len().max(1) as f32).clamp(220.0, 290.0);
    egui::ScrollArea::horizontal().id_salt(("board", node.0)).show(ui, |ui| {
        ui.horizontal_top(|ui| {
            for g in &groups {
                let col = ui.vertical(|ui| {
                    ui.set_width(col_w);
                    let label = if g.label == NO_VALUE && !field_name.is_empty() { format!("{field_name} not set") } else { g.label.clone() };
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(label).strong().size(14.0));
                        let preset = group.filter(|_| g.label != NO_VALUE).map(|gb| (gb, g.label.clone()));
                        add_button(app, ui, t, node, preset);
                        if ui
                            .add(egui::Label::new(RichText::new("Hide").size(12.0).color(t.faint)).sense(Sense::click()))
                            .on_hover_text("Hide column")
                            .clicked()
                        {
                            app.acts.push(Act::Run("view.toggleGroupHidden".into(), json!({"node": node.0, "group": g.label})));
                        }
                    });
                    ui.add_space(4.0);
                    for i in g.items.iter().take(300) {
                        card(app, ui, t, node, *i, col_w - 22.0, true);
                        ui.add_space(8.0);
                    }
                    ui.allocate_space(vec2(col_w, 40.0));
                });
                let zone = ui.interact(col.response.rect, egui::Id::new((app.panel_ix, "board-col", node.0, g.label.as_str())), Sense::hover());
                if zone.dnd_hover_payload::<CardDrag>().is_some() {
                    ui.painter().rect_stroke(col.response.rect.expand(4.0), 8.0, Stroke::new(1.5, t.accent), StrokeKind::Outside);
                }
                if let Some(p) = zone.dnd_release_payload::<CardDrag>() {
                    let item = p.0;
                    let value = if g.label == NO_VALUE { String::new() } else { g.label.clone() };
                    match group {
                        Some(GroupBy::Field(f)) => app.acts.push(Act::Run("field.set".into(), json!({"node": item.0, "field": f.0, "value": value}))),
                        Some(GroupBy::Done) => app.acts.push(Act::Run("node.setDone".into(), json!({"node": item.0, "done": value == "Done"}))),
                        _ => {}
                    }
                }
                ui.add_space(24.0);
            }
        });
    });
}

/// Each item is a lane: the item at the top, its children below (spec 04 §6).
fn lanes(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, node: NodeId, groups: &[Group]) {
    let items: Vec<NodeId> = groups.iter().flat_map(|g| g.items.iter().copied()).collect();
    egui::ScrollArea::horizontal().id_salt(("lanes", node.0)).show(ui, |ui| {
        ui.horizontal_top(|ui| {
            for item in items.iter().take(100) {
                ui.vertical(|ui| {
                    ui.set_width(250.0);
                    ui.horizontal_wrapped(|ui| item_line(app, ui, t, *item, true));
                    ui.add_space(4.0);
                    let kids: Vec<NodeId> = app.session.graph.children(*item).to_vec();
                    for k in kids.iter().take(200) {
                        ui.horizontal_wrapped(|ui| {
                            ui.add_space(12.0);
                            item_line(app, ui, t, *k, false);
                        });
                    }
                });
                ui.add_space(18.0);
            }
        });
    });
}

/// List, side menu and tabs: pick an item, see it beside or below (spec 04 §7-9).
fn navigation(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, node: NodeId, kind: ViewKind, groups: &[Group]) {
    let items: Vec<NodeId> = groups.iter().flat_map(|g| g.items.iter().copied()).collect();
    let selected = app.session.nav_selected.get(&node).copied().filter(|s| items.contains(s)).or_else(|| items.first().copied());
    let mut pick = None;
    let mut entry = |ui: &mut Ui, app: &TanaApp, item: NodeId, compact: bool| {
        let g = &app.session.graph;
        let active = Some(item) == selected;
        let name = g.text(item);
        let label = RichText::new(if name.is_empty() { "Untitled".to_string() } else { name })
            .size(if compact { 13.5 } else { 14.5 })
            .color(if active { t.text } else { t.dim });
        let r = ui.add(egui::Button::new(label).fill(if active { t.hover } else { Color32::TRANSPARENT }).corner_radius(5.0));
        if r.clicked() {
            pick = Some(item);
        }
        if !compact {
            let line = g.display_line(node, item);
            if !line.is_empty() {
                ui.label(RichText::new(line).size(12.0).color(t.faint));
            }
        }
    };
    match kind {
        ViewKind::Tabs => {
            ui.horizontal_wrapped(|ui| {
                for i in &items {
                    entry(ui, app, *i, true);
                }
            });
            ui.separator();
            if let Some(s) = selected {
                detail(app, ui, t, s);
            }
        }
        _ => {
            let list_w = if kind == ViewKind::List { 280.0 } else { 180.0 };
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(list_w);
                    for i in &items {
                        entry(ui, app, *i, kind == ViewKind::SideMenu);
                    }
                    add_button(app, ui, t, node, None);
                });
                ui.separator();
                ui.vertical(|ui| {
                    if let Some(s) = selected {
                        detail(app, ui, t, s);
                    }
                });
            });
        }
    }
    if let Some(p) = pick {
        app.session.nav_selected.insert(node, p);
    }
}

/// The selected item of a navigation view: title, tags, fields and children.
fn detail(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, item: NodeId) {
    let target = app.session.graph.resolve(item);
    ui.horizontal(|ui| {
        let g = &app.session.graph;
        let r = ui.add(egui::Label::new(crate::outline::rich_job(g, t, g.name(target), t.text, false, 22.0)).sense(Sense::click()));
        if r.on_hover_text("Open").clicked() {
            app.acts.push(Act::Run("nav.open".into(), json!({"node": target.0})));
        }
    });
    ui.horizontal(|ui| {
        let g = &app.session.graph;
        for tag in g.get(target).map(|n| n.tags.clone()).unwrap_or_default() {
            chip(ui, g, tag, app.dark);
        }
    });
    crate::outline::fields(app, ui, t, target, 0.0);
    let kids: Vec<NodeId> = app.session.graph.children(target).to_vec();
    for k in kids.iter().take(300) {
        ui.horizontal(|ui| item_line(app, ui, t, *k, true));
    }
}

fn calendar(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, node: NodeId, groups: &[Group]) {
    let today = app.session.today;
    let mode = app.session.calendar_mode;
    let week_mode = mode == 1;
    let day_mode = mode == 2;
    let (y, m) = app.session.calendar_month.unwrap_or((today.year, today.month));
    let Some(first) = Date::new(y, m, 1) else { return };
    let field = app.session.graph.calendar_field(node);
    let anchor = app.calendar_week_anchor.unwrap_or(today);
    let week_start = anchor.add_days(-(anchor.weekday() as i64));
    ui.horizontal(|ui| {
        let title = if day_mode {
            format!("{}, {}", anchor.day_title(), anchor.year)
        } else if week_mode {
            let (wy, wk) = week_start.iso_week();
            format!("Week {wk}, {wy}")
        } else {
            format!("{} {y}", MONTHS.get(m as usize - 1).copied().unwrap_or(""))
        };
        ui.label(RichText::new(title).size(17.0).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            for label in [">", "Today", "<"] {
                if small_button(ui, t, label, false).clicked() {
                    if week_mode || day_mode {
                        let step = if day_mode { 1 } else { 7 };
                        let base = if day_mode { anchor } else { week_start };
                        app.calendar_week_anchor = match label {
                            ">" => Some(base.add_days(step)),
                            "<" => Some(base.add_days(-step)),
                            _ => None,
                        };
                    } else {
                        let p = match label {
                            ">" => json!({"step": 1}),
                            "<" => json!({"step": -1}),
                            _ => json!({}),
                        };
                        app.acts.push(Act::Run("view.month".into(), p));
                    }
                }
            }
            ui.add_space(8.0);
            for (label, name, m) in [("Month", "month", 0u8), ("Week", "week", 1), ("Day", "day", 2)] {
                if small_button(ui, t, label, mode == m).clicked() {
                    app.acts.push(Act::Run("view.calendarMode".into(), json!({"mode": name})));
                }
            }
            if let Some(f) = field {
                ui.label(RichText::new(format!("by {}", app.session.graph.name(f))).size(12.5).color(t.faint));
            }
        });
    });
    ui.add_space(6.0);
    let items: Vec<NodeId> = groups.iter().flat_map(|g| g.items.iter().copied()).collect();
    let dated: Vec<(Date, NodeId)> = items.iter().filter_map(|i| app.session.graph.item_date(*i, field).map(|d| (d, *i))).collect();
    let undated = items.len() - dated.len();
    if day_mode || week_mode {
        // Week and day: an all-day strip over an hour grid (spec 04 §10).
        let days: Vec<Date> = if day_mode { vec![anchor] } else { (0..7).map(|k| week_start.add_days(k)).collect() };
        time_grid(app, ui, t, node, field, &days, &dated);
        undated_link(app, ui, t, node, field, &items, undated);
        return;
    }
    let w = ui.available_width();
    let col_w = (w / 7.0).floor();
    let painter = ui.painter().clone();
    let (hr, _) = ui.allocate_exact_size(vec2(w, 22.0), Sense::hover());
    let start = if week_mode { week_start } else { first.add_days(-(first.weekday() as i64)) };
    for (i, d) in ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"].iter().enumerate() {
        let label = if week_mode {
            let day = start.add_days(i as i64);
            format!("{d} {:02}/{:02}", day.day, day.month)
        } else {
            d.to_string()
        };
        painter.text(pos2(hr.left() + col_w * (i as f32 + 0.5), hr.center().y), Align2::CENTER_CENTER, label, FontId::proportional(12.5), t.dim);
    }
    let weeks = if week_mode { 1 } else { 6 };
    let mut day = start;
    for _ in 0..weeks {
        if !week_mode && day.month != m && day > first {
            break;
        }
        let row_items: Vec<Vec<NodeId>> = (0..7).map(|k| dated.iter().filter(|(d, _)| *d == day.add_days(k)).map(|(_, i)| *i).collect()).collect();
        let cap = if week_mode { 12 } else { 4 };
        let max = row_items.iter().map(Vec::len).max().unwrap_or(0).min(cap);
        let h = if week_mode { 380.0 } else { (30.0 + max as f32 * 24.0 + 6.0).max(52.0) };
        let (rr, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
        for k in 0..7usize {
            let d = day.add_days(k as i64);
            let cell = Rect::from_min_size(pos2(rr.left() + col_w * k as f32, rr.top()), vec2(col_w, rr.height()));
            painter.rect_filled(cell, 0.0, if k >= 5 { t.field_bg } else { t.panel });
            // Drop an item on a day to move it there (sets the calendar's date field).
            let drop = ui.interact(cell, egui::Id::new((app.panel_ix, "cal-day", node.0, d.to_days())), Sense::hover());
            if drop.dnd_hover_payload::<CardDrag>().is_some() {
                painter.rect_filled(cell, 0.0, t.accent.gamma_multiply(0.12));
            }
            if let (Some(p), Some(f)) = (drop.dnd_release_payload::<CardDrag>(), field) {
                app.acts.push(Act::Run("field.set".into(), json!({"node": p.0.0, "field": f.0, "value": d.to_string()})));
            }
            painter.rect_stroke(cell, 0.0, Stroke::new(1.0, t.line), StrokeKind::Inside);
            let in_month = week_mode || d.month == m;
            let num_pos = cell.right_top() + vec2(-8.0, 12.0);
            if d == today {
                painter.circle_filled(num_pos - vec2(4.0, 0.0), 10.0, Color32::from_rgb(226, 92, 64));
                painter.text(num_pos - vec2(4.0, 0.0), Align2::CENTER_CENTER, d.day.to_string(), FontId::proportional(12.5), Color32::WHITE);
            } else {
                painter.text(num_pos, Align2::RIGHT_CENTER, d.day.to_string(), FontId::proportional(12.5), if in_month { t.text } else { t.faint });
            }
            if k == 0 {
                painter.text(
                    cell.left_top() + vec2(6.0, 12.0),
                    Align2::LEFT_CENTER,
                    format!("W{}", d.iso_week().1),
                    FontId::proportional(11.5),
                    t.faint,
                );
            }
            let cell_items = row_items.get(k).map(Vec::as_slice).unwrap_or(&[]);
            for (j, item) in cell_items.iter().take(cap).enumerate() {
                let r = Rect::from_min_size(cell.left_top() + vec2(4.0, 26.0 + j as f32 * 24.0), vec2(col_w - 8.0, 22.0));
                let g = &app.session.graph;
                let target = g.resolve(*item);
                let color = g
                    .get(target)
                    .and_then(|n| n.tags.first())
                    .and_then(|x| g.tag_def(*x))
                    .map(|d| theme::tag_color(&d.color, app.dark))
                    .unwrap_or((t.dim, t.hover));
                painter.rect_filled(r, 3.0, color.1);
                painter.rect_filled(Rect::from_min_size(r.min, vec2(3.0, r.height())), 1.0, color.0);
                let done = g.checkbox(target) == Some(true);
                let name = g.text(target);
                let mut x = r.left() + 8.0;
                if let Some(d) = g.checkbox(target) {
                    let b = Rect::from_center_size(pos2(x + 5.0, r.center().y), vec2(10.0, 10.0));
                    painter.rect_filled(b, 2.0, if d { Color32::from_rgb(76, 160, 96) } else { t.hover });
                    x += 14.0;
                }
                painter.with_clip_rect(r.shrink(2.0)).text(
                    pos2(x, r.center().y),
                    Align2::LEFT_CENTER,
                    name,
                    FontId::proportional(12.5),
                    if done { t.dim } else { t.text },
                );
                let resp = ui.interact(r, egui::Id::new((app.panel_ix, "cal-item", node.0, item.0, d.to_days())), Sense::click_and_drag());
                if resp.clicked() {
                    app.acts.push(Act::Run("nav.open".into(), json!({"node": target.0})));
                }
                if resp.drag_started() {
                    resp.dnd_set_drag_payload(CardDrag(target));
                }
            }
            let extra = cell_items.len().saturating_sub(cap);
            if extra > 0 {
                painter.text(
                    cell.left_bottom() + vec2(6.0, -6.0),
                    Align2::LEFT_BOTTOM,
                    format!("+{extra} more"),
                    FontId::proportional(11.5),
                    t.faint,
                );
            }
        }
        day = day.add_days(7);
    }
    undated_link(app, ui, t, node, field, &items, undated);
}

fn undated_link(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, node: NodeId, field: Option<NodeId>, items: &[NodeId], undated: usize) {
    ui.add_space(8.0);
    if field.is_none() {
        ui.label(RichText::new("Add a date field to the items to place them on the calendar.").size(13.0).color(t.dim));
    } else {
        // Always shown, also at zero (DIFFS P2 #11).
        let open = app.show_undated.contains(&node);
        if ui
            .add(
                egui::Label::new(RichText::new(format!("{undated} nodes with no dates {}", if open { "v" } else { ">" })).size(13.0).color(t.dim))
                    .sense(Sense::click()),
            )
            .clicked()
        {
            if open {
                app.show_undated.remove(&node);
            } else {
                app.show_undated.insert(node);
            }
        }
        if open {
            let undated_items: Vec<NodeId> = items.iter().copied().filter(|i| app.session.graph.item_date(*i, field).is_none()).collect();
            for i in undated_items {
                ui.horizontal(|ui| item_line(app, ui, t, i, true));
            }
        }
    }
}

// ---- calendar time grid -------------------------------------------------------------------------

const HOUR_H: f32 = 40.0;
const GUTTER: f32 = 50.0;

/// A "New event" dragged out on the time grid (spec 04 §10).
#[derive(Debug, Clone)]
pub struct NewEvent {
    pub view: NodeId,
    pub day: Date,
    pub start: u16,
    pub end: u16,
    /// Still dragging (false: the popover asks for a name).
    pub dragging: bool,
    pub name: String,
    focus_once: bool,
}

/// Minutes, snapped to a quarter hour.
fn snap(min: f32) -> u16 {
    ((min / 15.0).round() * 15.0).clamp(0.0, 1440.0) as u16
}

fn range_text(day: Date, a: u16, b: u16) -> String {
    format!("{day} {}-{}", tanacraft_core::fmt_hm(a), tanacraft_core::fmt_hm(b))
}

/// The colours of an item's chip: (bar, fill).
fn chip_colors(app: &TanaApp, t: &Tokens, target: NodeId) -> (Color32, Color32) {
    let g = &app.session.graph;
    g.get(target).and_then(|n| n.tags.first()).and_then(|x| g.tag_def(*x)).map(|d| theme::tag_color(&d.color, app.dark)).unwrap_or((t.dim, t.hover))
}

fn paint_chip(app: &TanaApp, t: &Tokens, p: &egui::Painter, r: Rect, target: NodeId, sub: Option<String>) {
    let g = &app.session.graph;
    let (bar, fill) = chip_colors(app, t, target);
    p.rect_filled(r, 3.0, fill);
    p.rect_filled(Rect::from_min_size(r.min, vec2(3.0, r.height())), 1.0, bar);
    let clip = p.with_clip_rect(r.shrink(2.0));
    let mut x = r.left() + 8.0;
    if let Some(d) = g.checkbox(target) {
        let b = Rect::from_center_size(pos2(x + 5.0, r.top() + 11.0), vec2(10.0, 10.0));
        clip.rect_filled(b, 2.0, if d { Color32::from_rgb(76, 160, 96) } else { t.hover });
        x += 14.0;
    }
    let done = g.checkbox(target) == Some(true);
    clip.text(pos2(x, r.top() + 11.0), Align2::LEFT_CENTER, g.text(target), FontId::proportional(12.5), if done { t.dim } else { t.text });
    if let Some(sub) = sub
        && r.height() > 30.0
    {
        clip.text(pos2(r.left() + 8.0, r.top() + 27.0), Align2::LEFT_CENTER, sub, FontId::proportional(11.5), t.dim);
    }
}

/// Week / day view: day headers, an all-day strip and a 24-hour grid. Drag an item onto an hour to
/// give it a time, drag a timed event's bottom edge to resize it, drag on empty grid to create an
/// event.
fn time_grid(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, node: NodeId, field: Option<NodeId>, days: &[Date], dated: &[(Date, NodeId)]) {
    const WD: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let today = app.session.today;
    let w = ui.available_width();
    let n = days.len().max(1) as f32;
    let col_w = ((w - GUTTER) / n).floor();
    let col_x = |i: usize, left: f32| left + GUTTER + col_w * i as f32;
    let painter = ui.painter().clone();
    // Day headers.
    let (hr, _) = ui.allocate_exact_size(vec2(w, 24.0), Sense::hover());
    for (i, d) in days.iter().enumerate() {
        let label = format!("{} {:02}/{:02}", WD.get(d.weekday()).copied().unwrap_or(""), d.day, d.month);
        let c = pos2(col_x(i, hr.left()) + col_w / 2.0, hr.center().y);
        if *d == today {
            painter.rect_filled(Rect::from_center_size(c, vec2(78.0, 20.0)), 10.0, Color32::from_rgb(226, 92, 64).gamma_multiply(0.85));
        }
        painter.text(c, Align2::CENTER_CENTER, label, FontId::proportional(12.5), if *d == today { Color32::WHITE } else { t.dim });
    }
    // The all-day strip: items with a date but no time.
    let all_day: Vec<Vec<NodeId>> = days
        .iter()
        .map(|d| dated.iter().filter(|(x, i)| x == d && app.session.graph.item_time(*i, field).is_none()).map(|(_, i)| *i).collect())
        .collect();
    let rows = all_day.iter().map(Vec::len).max().unwrap_or(0).clamp(1, 6);
    let (strip, _) = ui.allocate_exact_size(vec2(w, 8.0 + rows as f32 * 24.0), Sense::hover());
    painter.text(strip.left_top() + vec2(4.0, 12.0), Align2::LEFT_CENTER, "all-day", FontId::proportional(11.0), t.faint);
    for (i, d) in days.iter().enumerate() {
        let cell = Rect::from_min_size(pos2(col_x(i, strip.left()), strip.top()), vec2(col_w, strip.height()));
        painter.rect_stroke(cell, 0.0, Stroke::new(1.0, t.line), StrokeKind::Inside);
        let drop = ui.interact(cell, egui::Id::new((app.panel_ix, "allday", node.0, d.to_days())), Sense::hover());
        if drop.dnd_hover_payload::<CardDrag>().is_some() {
            painter.rect_filled(cell, 0.0, t.accent.gamma_multiply(0.12));
        }
        if let (Some(p), Some(f)) = (drop.dnd_release_payload::<CardDrag>(), field) {
            app.acts.push(Act::Run("field.set".into(), json!({"node": p.0.0, "field": f.0, "value": d.to_string()})));
        }
        for (j, item) in all_day[i].iter().take(6).enumerate() {
            let target = app.session.graph.resolve(*item);
            let r = Rect::from_min_size(cell.left_top() + vec2(3.0, 4.0 + j as f32 * 24.0), vec2(col_w - 6.0, 22.0));
            paint_chip(app, t, &painter, r, target, None);
            let resp = ui.interact(r, egui::Id::new((app.panel_ix, "allday-item", node.0, item.0)), Sense::click_and_drag());
            if resp.clicked() {
                app.acts.push(Act::Run("nav.open".into(), json!({"node": target.0})));
            }
            if resp.drag_started() {
                resp.dnd_set_drag_payload(CardDrag(target));
            }
        }
    }
    // The hour grid, scrolled to 08:00 the first time.
    let mut area = egui::ScrollArea::vertical()
        .id_salt(("cal-grid", app.panel_ix, node.0))
        .max_height(520.0)
        .min_scrolled_height(520.0)
        .auto_shrink([false, true]);
    if app.cal_scrolled.insert(node) {
        area = area.vertical_scroll_offset(8.0 * HOUR_H);
    }
    area.show(ui, |ui| {
        let (grid, resp) = ui.allocate_exact_size(vec2(w, 24.0 * HOUR_H), Sense::click_and_drag());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Time grid"));
        let p = ui.painter().clone();
        for (i, d) in days.iter().enumerate() {
            if d.weekday() >= 5 {
                p.rect_filled(Rect::from_min_size(pos2(col_x(i, grid.left()), grid.top()), vec2(col_w, grid.height())), 0.0, t.field_bg);
            }
            p.line_segment([pos2(col_x(i, grid.left()), grid.top()), pos2(col_x(i, grid.left()), grid.bottom())], Stroke::new(1.0, t.line));
        }
        for h in 0..24 {
            let y = grid.top() + h as f32 * HOUR_H;
            p.line_segment([pos2(grid.left() + GUTTER, y), pos2(grid.right(), y)], Stroke::new(1.0, t.line));
            p.text(pos2(grid.left() + 4.0, y + 2.0), Align2::LEFT_TOP, format!("{h:02}:00"), FontId::proportional(11.0), t.faint);
        }
        let at = |pos: egui::Pos2| -> Option<(usize, u16)> {
            let i = ((pos.x - grid.left() - GUTTER) / col_w).floor();
            (i >= 0.0 && (i as usize) < days.len()).then(|| (i as usize, snap((pos.y - grid.top()) / HOUR_H * 60.0)))
        };
        // Now: a thin red-orange line in today's column.
        if let Some(i) = days.iter().position(|d| *d == today) {
            let y = grid.top() + tanacraft_core::Date::now_minutes() as f32 / 60.0 * HOUR_H;
            let x0 = col_x(i, grid.left());
            let red = Color32::from_rgb(226, 92, 64);
            p.line_segment([pos2(x0, y), pos2(x0 + col_w, y)], Stroke::new(1.5, red));
            p.circle_filled(pos2(x0, y), 3.5, red);
        }
        // Drag on empty grid: select a range, then name the new event.
        if let Some(f) = field {
            if resp.drag_started()
                && let Some((i, m)) = ui.input(|i| i.pointer.press_origin()).and_then(at)
            {
                app.cal_new = Some(NewEvent {
                    view: node,
                    day: days[i],
                    start: m,
                    end: (m + 30).min(1440),
                    dragging: true,
                    name: String::new(),
                    focus_once: true,
                });
            }
            if let Some(ev) = app.cal_new.as_mut().filter(|e| e.view == node && e.dragging) {
                if let Some((_, m)) = ui.ctx().pointer_interact_pos().and_then(at) {
                    ev.end = m.max(ev.start + 15).min(1440);
                }
                if resp.drag_stopped() {
                    ev.dragging = false;
                }
            }
            if let Some(i) = app.cal_new.as_ref().filter(|e| e.view == node).and_then(|e| days.iter().position(|d| *d == e.day)) {
                let ev = app.cal_new.clone().unwrap_or_else(|| NewEvent {
                    view: node,
                    day: today,
                    start: 0,
                    end: 0,
                    dragging: false,
                    name: String::new(),
                    focus_once: false,
                });
                let r = Rect::from_min_max(
                    pos2(col_x(i, grid.left()) + 2.0, grid.top() + ev.start as f32 / 60.0 * HOUR_H),
                    pos2(col_x(i, grid.left()) + col_w - 2.0, grid.top() + ev.end as f32 / 60.0 * HOUR_H),
                );
                p.rect_filled(r, 4.0, t.accent.gamma_multiply(0.25));
                p.rect_stroke(r, 4.0, Stroke::new(1.0, t.accent), StrokeKind::Inside);
                if !ev.dragging {
                    new_event_popover(app, ui, t, f, r);
                }
            }
            // Drop an item on an hour: it gets that time (one hour, or its current length).
            if let Some(pl) = resp.dnd_release_payload::<CardDrag>()
                && let Some((i, m)) = ui.ctx().pointer_interact_pos().and_then(at)
            {
                let len = app.session.graph.item_time(pl.0, Some(f)).map(|(a, b)| b - a).unwrap_or(60);
                let start = m.min(1440 - len.min(1440));
                app.acts.push(Act::Run("field.set".into(), json!({"node": pl.0.0, "field": f.0, "value": range_text(days[i], start, start + len)})));
            }
            if resp.dnd_hover_payload::<CardDrag>().is_some()
                && let Some((i, m)) = ui.ctx().pointer_hover_pos().and_then(at)
            {
                let y = grid.top() + m as f32 / 60.0 * HOUR_H;
                p.line_segment([pos2(col_x(i, grid.left()), y), pos2(col_x(i, grid.left()) + col_w, y)], Stroke::new(2.0, t.accent));
            }
        }
        // Timed events.
        for (i, d) in days.iter().enumerate() {
            for (_, item) in dated.iter().filter(|(x, _)| x == d) {
                let Some((a, b)) = app.session.graph.item_time(*item, field) else { continue };
                let target = app.session.graph.resolve(*item);
                let b_shown = app.cal_resize.filter(|(n, _)| *n == target).map(|(_, e)| e).unwrap_or(b);
                let r = Rect::from_min_max(
                    pos2(col_x(i, grid.left()) + 2.0, grid.top() + a as f32 / 60.0 * HOUR_H + 1.0),
                    pos2(col_x(i, grid.left()) + col_w - 2.0, grid.top() + b_shown as f32 / 60.0 * HOUR_H - 1.0),
                );
                let sub = format!("{} - {}", tanacraft_core::fmt_hm(a), tanacraft_core::fmt_hm(b_shown));
                paint_chip(app, t, &p, r, target, Some(sub));
                let body = Rect::from_min_max(r.min, pos2(r.right(), (r.bottom() - 7.0).max(r.top() + 4.0)));
                let resp = ui.interact(body, egui::Id::new((app.panel_ix, "timed", node.0, item.0)), Sense::click_and_drag());
                if resp.clicked() {
                    app.acts.push(Act::Run("nav.open".into(), json!({"node": target.0})));
                }
                if resp.drag_started() {
                    resp.dnd_set_drag_payload(CardDrag(target));
                }
                // The bottom edge resizes (spec 04 §10).
                let edge = Rect::from_min_max(pos2(r.left(), r.bottom() - 7.0), r.max);
                let er = ui.interact(edge, egui::Id::new((app.panel_ix, "timed-edge", node.0, item.0)), Sense::drag());
                if er.hovered() || er.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                }
                if er.dragged()
                    && let Some(pos) = ui.ctx().pointer_interact_pos()
                {
                    let end = snap((pos.y - grid.top()) / HOUR_H * 60.0).max(a + 15).min(1440);
                    app.cal_resize = Some((target, end));
                }
                if er.drag_stopped()
                    && let (Some((_, end)), Some(f)) = (app.cal_resize.take(), field)
                {
                    app.acts.push(Act::Run("field.set".into(), json!({"node": target.0, "field": f.0, "value": range_text(*d, a, end)})));
                }
            }
        }
    });
}

/// The "New event" popover under a dragged-out range: a name, the range, Enter creates.
fn new_event_popover(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, field: NodeId, under: Rect) {
    let Some(mut ev) = app.cal_new.clone() else { return };
    let mut close = false;
    let mut create = false;
    egui::Area::new(egui::Id::new(("new-event", ev.view.0))).order(egui::Order::Foreground).fixed_pos(under.left_bottom() + vec2(0.0, 6.0)).show(
        ui.ctx(),
        |ui| {
            egui::Frame::popup(ui.style()).fill(t.panel).corner_radius(8.0).inner_margin(10.0).show(ui, |ui| {
                ui.set_width(260.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("New event").strong().size(13.5));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::Label::new(RichText::new("x").color(t.dim)).sense(Sense::click())).clicked() {
                            close = true;
                        }
                    });
                });
                let r = ui.add(egui::TextEdit::singleline(&mut ev.name).hint_text("Name, then Enter").desired_width(240.0));
                if ev.focus_once {
                    r.request_focus();
                    ev.focus_once = false;
                }
                let g = &app.session.graph;
                let shown = tanacraft_core::export::shown_value(g, &tanacraft_core::FieldValue::Timed { date: ev.day, start: ev.start, end: ev.end });
                ui.label(RichText::new(format!("{}: {shown}", g.name(field))).size(12.5).color(t.dim));
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    create = true;
                }
            });
        },
    );
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        close = true;
    }
    if create {
        let name = if ev.name.trim().is_empty() { "New event".to_string() } else { ev.name.clone() };
        let r = app.exec(
            "view.addItem",
            json!({"node": ev.view.0, "name": name, "field": field.0, "value": range_text(ev.day, ev.start, ev.end), "stay": true}),
        );
        if let Err(e) = r {
            app.session.status = e;
        }
        app.cal_new = None;
    } else if close {
        app.cal_new = None;
    } else {
        app.cal_new = Some(ev);
    }
}
