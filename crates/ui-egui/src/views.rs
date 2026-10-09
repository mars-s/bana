//! Search pages and views (spec 04, 05): the condition editor, the view toolbar (quick filter,
//! view as, display, group, sort, filter chips) and the table, cards / board, lanes, navigation
//! (list, side menu, tabs) and calendar layouts.

use egui::{Align2, Color32, FontId, Rect, RichText, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::json;
use tanacraft_core::query::{Clause, ClauseKind, Query};
use tanacraft_core::view::{Group, NO_VALUE};
use tanacraft_core::{Date, FieldType, GroupBy, NodeId, SortBy, ViewKind};

use crate::outline::{checkbox, chip, field_value, rich_label};
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

/// The search conditions of a search node: chips (click NOT to negate, x to remove), AND/OR, add.
pub fn query_builder(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, node: NodeId) {
    let Some(q) = app.session.graph.query(node).cloned() else { return };
    egui::Frame::new().fill(t.field_bg).stroke(Stroke::new(1.0, t.line)).corner_radius(8.0).inner_margin(egui::Margin::symmetric(14, 10)).show(
        ui,
        |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new("Search conditions").strong().size(14.0));
                ui.label(RichText::new("#tag, words, DONE, TODO, Field:value, Field>value, CREATED:7, ( ... OR ... )").size(12.0).color(t.faint));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Label::new(RichText::new("x").color(t.dim)).sense(Sense::click())).on_hover_text("Close").clicked() {
                        app.acts.push(Act::Ui("ui.searchOptions"));
                    }
                });
            });
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                let mode = if q.any { "Any of (OR)" } else { "All of (AND)" };
                if small_button(ui, t, mode, true).on_hover_text("Switch between AND and OR").clicked() {
                    app.acts.push(Act::Run("search.setMode".into(), json!({"node": node.0, "any": !q.any})));
                }
                for (i, c) in q.clauses.iter().enumerate() {
                    let g = &app.session.graph;
                    let (label, color) = match &c.kind {
                        ClauseKind::Tagged { tag } => {
                            let col = g.tag_def(*tag).map(|d| theme::tag_color(&d.color, app.dark).0).unwrap_or(t.text);
                            (format!("#{}", g.name(*tag)), col)
                        }
                        _ => (clause_text(g, c), t.text),
                    };
                    egui::Frame::new().fill(t.panel).corner_radius(5.0).inner_margin(egui::Margin::symmetric(6, 2)).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let not = ui.add(
                                egui::Label::new(RichText::new("NOT").size(11.5).color(if c.not {
                                    Color32::from_rgb(232, 128, 96)
                                } else {
                                    t.faint
                                }))
                                .sense(Sense::click()),
                            );
                            if not.on_hover_text("Negate").clicked() {
                                app.acts.push(Act::Run("search.toggleNot".into(), json!({"node": node.0, "index": i})));
                            }
                            ui.label(RichText::new(label).size(13.5).color(color));
                            if ui
                                .add(egui::Label::new(RichText::new("x").size(12.0).color(t.faint)).sense(Sense::click()))
                                .on_hover_text("Remove")
                                .clicked()
                            {
                                app.acts.push(Act::Run("search.removeClause".into(), json!({"node": node.0, "index": i})));
                            }
                        });
                    });
                }
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let r = ui.add(
                    egui::TextEdit::singleline(&mut app.clause_buf)
                        .id(egui::Id::new((app.panel_ix, "clause-input")))
                        .hint_text("Add a condition, then Enter")
                        .desired_width(260.0),
                );
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !app.clause_buf.trim().is_empty() {
                    let clause = std::mem::take(&mut app.clause_buf);
                    app.acts.push(Act::Run("search.addClause".into(), json!({"node": node.0, "clause": clause})));
                }
                if small_button(ui, t, "+ Supertag", false).clicked() {
                    app.acts.push(Act::Pick(PickMode::SearchTag, node, ui.min_rect().left_bottom()));
                }
                ui.menu_button(RichText::new("Operators").size(13.0).color(t.dim), |ui| {
                    for (label, clause) in [
                        ("TODO (not done)", "TODO"),
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
            ui.add_space(4.0);
            let n = app.session.graph.view_items(node).len();
            ui.label(RichText::new(format!("({})  {n} results", q.describe(&app.session.graph))).size(12.5).color(t.faint));
        },
    );
    ui.add_space(8.0);
}

// ---- toolbar ------------------------------------------------------------------------------------

fn by_key(by: SortBy) -> String {
    match by {
        SortBy::Name => "name".into(),
        SortBy::Created => "created".into(),
        SortBy::Done => "done".into(),
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
    let field_names: Vec<(NodeId, String, FieldType)> =
        fields.iter().map(|f| (*f, g.name(*f).to_string(), g.field_def(*f).map(|d| d.field_type).unwrap_or_default())).collect();
    let sort_label = |by: SortBy| match by {
        SortBy::Name => "Name".to_string(),
        SortBy::Created => "Created time".into(),
        SortBy::Done => "Done".into(),
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
            RichText::new(if def.columns.is_empty() { "Display".to_string() } else { format!("Display ({})", def.columns.len()) })
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
                Some(GroupBy::Field(f)) => format!("Group: {}", g.name(f)),
            };
            ui.menu_button(RichText::new(group_label).size(13.0).color(t.dim), |ui| {
                ui.label(RichText::new("Group by").size(12.0).color(t.faint));
                for (label, by) in [("None", "none".to_string()), ("Tags", "tags".into()), ("Done", "done".into())] {
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
                for (label, by) in [("Name", "name".to_string()), ("Created time", "created".into()), ("Done", "done".into())] {
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
                let opts = g.field_def(*f).map(|d| d.options.clone()).unwrap_or_default();
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
        group_header(ui, t, &g.label, g.items.len());
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
    let col_w = 160.0_f32;
    let name_w = (total - col_w * cols.len() as f32 - 70.0).max(240.0);
    let sort = app.session.graph.view_def(node).sort;
    let tag_color = app.session.graph.tag_def(node).map(|d| theme::tag_color(&d.color, app.dark).0);
    let header = ui.horizontal(|ui| {
        ui.set_min_height(26.0);
        let name_label = if app.session.graph.tag_def(node).is_some() { format!("# {}", app.session.graph.name(node)) } else { "Name".to_string() };
        let (r, resp) = ui.allocate_exact_size(vec2(name_w, 24.0), Sense::click());
        ui.painter().text(r.left_center() + vec2(4.0, 0.0), Align2::LEFT_CENTER, name_label, FontId::proportional(13.5), tag_color.unwrap_or(t.dim));
        if let Some(c) = tag_color {
            ui.painter().line_segment([r.left_bottom() + vec2(0.0, 2.0), r.right_bottom() + vec2(0.0, 2.0)], Stroke::new(2.0, c));
        }
        if resp.clicked() {
            let desc = sort.first().is_some_and(|s| s.by == SortBy::Name && !s.desc);
            app.acts.push(Act::Run("view.sort".into(), json!({"node": node.0, "by": "name", "desc": desc})));
        }
        for f in &cols {
            let (r, resp) = ui.allocate_exact_size(vec2(col_w, 24.0), Sense::click());
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
    let line_end = hr.left() + name_w + col_w * cols.len() as f32;
    ui.painter().line_segment([hr.left_bottom(), pos2(line_end, hr.bottom())], Stroke::new(1.0, t.line));
    for gr in groups {
        group_header(ui, t, &gr.label, gr.items.len());
        for item in gr.items.iter().take(1000) {
            let row = ui.horizontal(|ui| {
                ui.set_min_height(30.0);
                cell(ui, name_w, |ui| item_line(app, ui, t, *item, false));
                let target = app.session.graph.resolve(*item);
                for f in &cols {
                    cell(ui, col_w, |ui| {
                        let g = &app.session.graph;
                        let Some(def) = g.field_def(*f).cloned() else { return };
                        let value = g.get(target).and_then(|n| n.field_value(*f)).cloned();
                        let text = value.as_ref().map(|v| tanacraft_core::export::display_value(g, v));
                        let editing = app.field_edit.as_ref().is_some_and(|e| e.node == target && e.field == *f);
                        field_value(app, ui, t, target, *f, &def, value.as_ref(), text, editing);
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

/// A fixed-width table cell: content is clipped, never widens the row.
fn cell(ui: &mut Ui, w: f32, add: impl FnOnce(&mut Ui)) {
    let (rect, _) = ui.allocate_exact_size(vec2(w, 30.0), Sense::hover());
    let mut child =
        ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(vec2(4.0, 0.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
    child.set_clip_rect(rect.shrink2(vec2(2.0, 0.0)).intersect(ui.clip_rect()));
    add(&mut child);
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
    egui::ScrollArea::horizontal().id_salt(("board", node.0)).show(ui, |ui| {
        ui.horizontal_top(|ui| {
            for g in groups {
                let col = ui.vertical(|ui| {
                    ui.set_width(290.0);
                    let label = if g.label == NO_VALUE && !field_name.is_empty() { format!("{field_name} not set") } else { g.label.clone() };
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(label).strong().size(14.0));
                        ui.label(RichText::new(g.items.len().to_string()).size(12.5).color(t.faint));
                        let preset = group.filter(|_| g.label != NO_VALUE).map(|gb| (gb, g.label.clone()));
                        add_button(app, ui, t, node, preset);
                    });
                    ui.add_space(4.0);
                    for i in g.items.iter().take(300) {
                        card(app, ui, t, node, *i, 268.0, true);
                        ui.add_space(8.0);
                    }
                    ui.allocate_space(vec2(290.0, 40.0));
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
    let week_mode = app.session.calendar_week;
    let (y, m) = app.session.calendar_month.unwrap_or((today.year, today.month));
    let Some(first) = Date::new(y, m, 1) else { return };
    let field = app.session.graph.calendar_field(node);
    let anchor = app.calendar_week_anchor.unwrap_or(today);
    let week_start = anchor.add_days(-(anchor.weekday() as i64));
    ui.horizontal(|ui| {
        let title = if week_mode {
            let (wy, wk) = week_start.iso_week();
            format!("Week {wk}, {wy}")
        } else {
            format!("{} {y}", MONTHS.get(m as usize - 1).copied().unwrap_or(""))
        };
        ui.label(RichText::new(title).size(17.0).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            for label in [">", "Today", "<"] {
                if small_button(ui, t, label, false).clicked() {
                    if week_mode {
                        app.calendar_week_anchor = match label {
                            ">" => Some(week_start.add_days(7)),
                            "<" => Some(week_start.add_days(-7)),
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
            for (label, mode) in [("Month", "month"), ("Week", "week")] {
                if small_button(ui, t, label, (mode == "week") == week_mode).clicked() {
                    app.acts.push(Act::Run("view.calendarMode".into(), json!({"mode": mode})));
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
                let resp = ui.interact(r, egui::Id::new((app.panel_ix, "cal-item", node.0, item.0, d.to_days())), Sense::click());
                if resp.clicked() {
                    app.acts.push(Act::Run("nav.open".into(), json!({"node": target.0})));
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
    ui.add_space(8.0);
    if field.is_none() {
        ui.label(RichText::new("Add a date field to the items to place them on the calendar.").size(13.0).color(t.dim));
    } else if undated > 0 {
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
