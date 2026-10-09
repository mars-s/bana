//! Search pages and views: the query builder, the view toolbar (view as, group, sort, filter) and
//! the table, cards, board and calendar layouts.

use egui::{Align2, Color32, FontId, Rect, RichText, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::json;
use tanacraft_core::query::ClauseKind;
use tanacraft_core::view::{Group, NO_VALUE};
use tanacraft_core::{Date, GroupBy, NodeId, SortBy, ViewKind};

use crate::outline::{checkbox, chip, field_value, rich_label};
use crate::theme::{self, Tokens};
use crate::{Act, PickMode, TanaApp};

const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

fn small_button(ui: &mut Ui, t: &Tokens, label: &str, active: bool) -> egui::Response {
    let b = egui::Button::new(RichText::new(label).size(13.0).color(if active { t.text } else { t.dim }))
        .fill(if active { t.hover } else { t.field_bg })
        .corner_radius(5.0);
    ui.add(b)
}

// ---- query builder ------------------------------------------------------------------------------

/// The search conditions of a search node: chips (click NOT to negate, x to remove), AND/OR, add.
pub fn query_builder(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, node: NodeId) {
    let Some(q) = app.session.graph.query(node).cloned() else { return };
    egui::Frame::new().fill(t.field_bg).stroke(Stroke::new(1.0, t.line)).corner_radius(8.0).inner_margin(egui::Margin::symmetric(14, 10)).show(
        ui,
        |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new("Search conditions").strong().size(14.0));
                ui.label(RichText::new("#tag, words, DONE, TODO, Field:value, Field>value, CREATED:7").size(12.0).color(t.faint));
            });
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                let mode = if q.any { "Any of" } else { "All of" };
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
                        _ => {
                            let one = tanacraft_core::Query {
                                any: false,
                                clauses: vec![tanacraft_core::query::Clause { not: false, kind: c.kind.clone() }],
                            };
                            (one.describe(g), t.text)
                        }
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
                        .id(egui::Id::new("clause-input"))
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
                for (label, clause) in [("+ Not done", "TODO"), ("+ Done", "DONE")] {
                    if small_button(ui, t, label, false).clicked() {
                        app.acts.push(Act::Run("search.addClause".into(), json!({"node": node.0, "clause": clause})));
                    }
                }
            });
            ui.add_space(4.0);
            let n = app.session.graph.view_items(node).len();
            ui.label(RichText::new(format!("({})  {n} results", q.describe(&app.session.graph))).size(12.5).color(t.faint));
        },
    );
    ui.add_space(8.0);
}

// ---- toolbar ------------------------------------------------------------------------------------

/// View as, group, sort and filter for `node`.
pub fn toolbar(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, node: NodeId) {
    let g = &app.session.graph;
    let def = g.view_def(node);
    let fields = g.view_columns(node);
    let field_names: Vec<(NodeId, String)> = fields.iter().map(|f| (*f, g.name(*f).to_string())).collect();
    let mut acts = Vec::new();
    ui.horizontal(|ui| {
        for k in ViewKind::ALL {
            let label = match k {
                ViewKind::List => "List",
                ViewKind::Table => "Table",
                ViewKind::Cards => "Cards",
                ViewKind::Board => "Board",
                ViewKind::Calendar => "Calendar",
            };
            if small_button(ui, t, label, def.kind == k).clicked() {
                acts.push(Act::Run("view.set".into(), json!({"node": node.0, "kind": k.name()})));
            }
        }
        ui.add_space(10.0);
        let group_label = match def.group {
            None => "Group".to_string(),
            Some(GroupBy::Tags) => "Group: tags".into(),
            Some(GroupBy::Done) => "Group: done".into(),
            Some(GroupBy::Field(f)) => format!("Group: {}", g.name(f)),
        };
        ui.menu_button(RichText::new(group_label).size(13.0).color(t.dim), |ui| {
            ui.label(RichText::new("Group by").size(12.0).color(t.faint));
            for (label, by) in [("None", "none".to_string()), ("Supertags", "tags".into()), ("Done", "done".into())] {
                if ui.button(label).clicked() {
                    acts.push(Act::Run("view.group".into(), json!({"node": node.0, "by": by})));
                }
            }
            for (f, name) in &field_names {
                if ui.button(name).clicked() {
                    acts.push(Act::Run("view.group".into(), json!({"node": node.0, "by": f.0.to_string()})));
                }
            }
        });
        let sort_label = match def.sort {
            None => "Sort".to_string(),
            Some(s) => {
                let by = match s.by {
                    SortBy::Name => "name".to_string(),
                    SortBy::Created => "created".into(),
                    SortBy::Done => "done".into(),
                    SortBy::Field(f) => g.name(f).to_string(),
                };
                format!("Sort: {by} {}", if s.desc { "(desc)" } else { "" })
            }
        };
        ui.menu_button(RichText::new(sort_label).size(13.0).color(t.dim), |ui| {
            ui.label(RichText::new("Sort by").size(12.0).color(t.faint));
            let desc = def.sort.is_some_and(|s| s.desc);
            for (label, by) in [("None", "none".to_string()), ("Name", "name".into()), ("Created", "created".into()), ("Done", "done".into())] {
                if ui.button(label).clicked() {
                    acts.push(Act::Run("view.sort".into(), json!({"node": node.0, "by": by, "desc": desc})));
                }
            }
            for (f, name) in &field_names {
                if ui.button(name).clicked() {
                    acts.push(Act::Run("view.sort".into(), json!({"node": node.0, "by": f.0.to_string(), "desc": desc})));
                }
            }
            ui.separator();
            if let Some(s) = def.sort
                && ui.button(if desc { "Ascending" } else { "Descending" }).clicked()
            {
                let by = match s.by {
                    SortBy::Name => "name".to_string(),
                    SortBy::Created => "created".into(),
                    SortBy::Done => "done".into(),
                    SortBy::Field(f) => f.0.to_string(),
                };
                acts.push(Act::Run("view.sort".into(), json!({"node": node.0, "by": by, "desc": !desc})));
            }
        });
        let filter_text = if def.filter.is_empty() { String::new() } else { def.filter.describe(g) };
        let editing = app.filter_buf.as_ref().is_some_and(|(n, _)| *n == node);
        if editing {
            if let Some((_, buf)) = app.filter_buf.as_mut() {
                let r = ui.add(
                    egui::TextEdit::singleline(buf).id(egui::Id::new("filter-input")).hint_text("Filter, e.g. Status:Done").desired_width(220.0),
                );
                r.request_focus();
                if r.lost_focus() {
                    let q = buf.clone();
                    app.filter_buf = None;
                    acts.push(Act::Run("view.filter".into(), json!({"node": node.0, "query": q})));
                }
            }
        } else {
            let label = if filter_text.is_empty() { "Filter".to_string() } else { format!("Filter: {filter_text}") };
            if small_button(ui, t, &label, !filter_text.is_empty()).clicked() {
                app.filter_buf = Some((node, filter_text));
            }
        }
    });
    app.acts.extend(acts);
    ui.add_space(4.0);
    let r = ui.available_rect_before_wrap();
    ui.painter().line_segment([pos2(r.left(), r.top()), pos2(r.right(), r.top())], Stroke::new(1.0, t.line));
    ui.add_space(8.0);
}

/// Draws the items of `node` in its view (list views of search pages use the outline instead).
pub fn render(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, node: NodeId) {
    let kind = app.session.graph.view_def(node).kind;
    let groups = app.session.graph.view_groups(node);
    match kind {
        ViewKind::List => list(app, ui, t, &groups),
        ViewKind::Table => table(app, ui, t, node, &groups),
        ViewKind::Cards => {
            for g in &groups {
                group_header(ui, t, g);
                ui.horizontal_wrapped(|ui| {
                    for i in &g.items {
                        card(app, ui, t, node, *i, 250.0);
                    }
                });
            }
            add_button(app, ui, t, node);
        }
        ViewKind::Board => board(app, ui, t, node, &groups),
        ViewKind::Calendar => calendar(app, ui, t, node, &groups),
    }
}

fn group_header(ui: &mut Ui, t: &Tokens, g: &Group) {
    if g.label.is_empty() {
        return;
    }
    ui.add_space(6.0);
    ui.label(RichText::new(format!("{}  {}", g.label, g.items.len())).size(13.0).strong().color(t.dim));
    ui.add_space(2.0);
}

/// Adds an item: a child of the node (search and supertag pages add to today, with the supertag).
fn add_button(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, node: NodeId) {
    ui.add_space(4.0);
    if ui.add(egui::Label::new(RichText::new("+ Add").size(13.5).color(t.dim)).sense(Sense::click())).clicked() {
        let g = &app.session.graph;
        let tag = g.tag_def(node).map(|_| node).or_else(|| {
            g.query(node)
                .and_then(|q| q.clauses.iter().find_map(|c| if let ClauseKind::Tagged { tag } = c.kind { (!c.not).then_some(tag) } else { None }))
        });
        if g.query(node).is_some() || g.tag_def(node).is_some() {
            let mut p = json!({"name": "New item"});
            if let Some(tag) = tag {
                p["tag"] = json!(g.name(tag));
            }
            app.acts.push(Act::Run("node.quickAdd".into(), p));
        } else {
            app.acts.push(Act::Run("node.add".into(), json!({"parent": node.0, "name": "New item"})));
        }
    }
}

/// One item as a line: bullet in the tag colour, checkbox, text, tag chips. Click opens it.
fn item_line(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, item: NodeId, with_chips: bool) {
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
    if rich_label(ui, t, g.name(target), if done { t.dim } else { t.text }, done).clicked() {
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

fn list(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, groups: &[Group]) {
    for g in groups {
        group_header(ui, t, g);
        for i in g.items.iter().take(1000) {
            ui.horizontal(|ui| {
                ui.set_min_height(26.0);
                item_line(app, ui, t, *i, true);
                let g = &app.session.graph;
                let crumbs: Vec<&str> = g.ancestors(*i).iter().skip(1).map(|a| g.name(*a)).collect();
                ui.label(RichText::new(crumbs.join(" / ")).size(12.0).color(t.faint));
            });
        }
    }
}

fn table(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, node: NodeId, groups: &[Group]) {
    let cols = app.session.graph.view_columns(node);
    let total = ui.available_width();
    let col_w = 160.0_f32;
    let name_w = (total - col_w * cols.len() as f32 - 70.0).max(240.0);
    let sort = app.session.graph.view_def(node).sort;
    // Header.
    let header = ui.horizontal(|ui| {
        ui.set_min_height(26.0);
        let name_label = if app.session.graph.tag_def(node).is_some() { format!("#{}", app.session.graph.name(node)) } else { "Name".to_string() };
        let (r, resp) = ui.allocate_exact_size(vec2(name_w, 24.0), Sense::click());
        ui.painter().text(r.left_center() + vec2(4.0, 0.0), Align2::LEFT_CENTER, name_label, FontId::proportional(13.5), t.dim);
        if resp.clicked() {
            let desc = sort.is_some_and(|s| s.by == SortBy::Name && !s.desc);
            app.acts.push(Act::Run("view.sort".into(), json!({"node": node.0, "by": "name", "desc": desc})));
        }
        for f in &cols {
            let (r, resp) = ui.allocate_exact_size(vec2(col_w, 24.0), Sense::click());
            let g = &app.session.graph;
            let mut label = g.name(*f).to_string();
            if let Some(s) = sort.filter(|s| s.by == SortBy::Field(*f)) {
                label.push_str(if s.desc { "  v" } else { "  ^" });
            }
            ui.painter().with_clip_rect(r).text(r.left_center() + vec2(4.0, 0.0), Align2::LEFT_CENTER, label, FontId::proportional(13.5), t.dim);
            if resp.clicked() {
                let desc = sort.is_some_and(|s| s.by == SortBy::Field(*f) && !s.desc);
                app.acts.push(Act::Run("view.sort".into(), json!({"node": node.0, "by": f.0.to_string(), "desc": desc})));
            }
            resp.context_menu(|ui| {
                if ui.button("Remove column").clicked() {
                    app.acts.push(Act::Run("view.removeColumn".into(), json!({"node": node.0, "field": f.0})));
                    ui.close();
                }
            });
        }
        if ui
            .add(egui::Label::new(RichText::new("+ Add").size(13.0).color(t.dim)).sense(Sense::click()))
            .on_hover_text("Add a field column")
            .clicked()
        {
            app.acts.push(Act::Pick(PickMode::Column, node, ui.min_rect().right_bottom()));
        }
    });
    let hr = header.response.rect;
    ui.painter().line_segment(
        [hr.left_bottom(), pos2(hr.left() + name_w + col_w * cols.len() as f32, hr.bottom())],
        Stroke::new(1.0, t.accent.gamma_multiply(0.7)),
    );
    for gr in groups {
        group_header(ui, t, gr);
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
            ui.painter().line_segment([r.left_bottom(), pos2(r.left() + name_w + col_w * cols.len() as f32, r.bottom())], Stroke::new(1.0, t.line));
        }
    }
    add_button(app, ui, t, node);
}

/// A fixed-width table cell: content is clipped, never widens the row.
fn cell(ui: &mut Ui, w: f32, add: impl FnOnce(&mut Ui)) {
    let (rect, _) = ui.allocate_exact_size(vec2(w, 30.0), Sense::hover());
    let mut child =
        ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(vec2(4.0, 0.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
    child.set_clip_rect(rect.shrink2(vec2(2.0, 0.0)).intersect(ui.clip_rect()));
    add(&mut child);
}

fn card(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, node: NodeId, item: NodeId, width: f32) {
    let target = app.session.graph.resolve(item);
    let cols = app.session.graph.view_columns(node);
    egui::Frame::new().fill(t.panel).stroke(Stroke::new(1.0, t.line)).corner_radius(6.0).inner_margin(egui::Margin::symmetric(10, 8)).show(
        ui,
        |ui| {
            ui.set_width(width);
            ui.vertical(|ui| {
                let g = &app.session.graph;
                let tags = g.get(target).map(|n| n.tags.clone()).unwrap_or_default();
                if !tags.is_empty() {
                    ui.horizontal(|ui| {
                        for tag in tags {
                            if chip(ui, g, tag, app.dark).clicked() {
                                app.acts.push(Act::Run("nav.open".into(), json!({"node": tag.0})));
                            }
                        }
                    });
                    ui.add_space(2.0);
                }
                ui.horizontal_wrapped(|ui| {
                    if let Some(done) = app.session.graph.checkbox(target)
                        && checkbox(ui, t, done, 14.0).clicked()
                    {
                        app.acts.push(Act::Run("node.toggleDone".into(), json!({"node": target.0})));
                    }
                    let g = &app.session.graph;
                    let done = g.checkbox(target) == Some(true);
                    if rich_label(ui, t, g.name(target), if done { t.dim } else { t.text }, done).clicked() {
                        app.acts.push(Act::Run("nav.open".into(), json!({"node": target.0})));
                    }
                });
                let g = &app.session.graph;
                let values: Vec<String> = cols
                    .iter()
                    .filter_map(|f| g.get(target).and_then(|n| n.field_value(*f)).map(|v| tanacraft_core::export::display_value(g, v)))
                    .filter(|s| !s.is_empty())
                    .collect();
                if !values.is_empty() {
                    ui.add_space(4.0);
                    ui.label(RichText::new(values.join("  ·  ")).size(12.5).color(t.dim));
                }
            });
        },
    );
}

fn board(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, node: NodeId, groups: &[Group]) {
    egui::ScrollArea::horizontal().id_salt(("board", node.0)).show(ui, |ui| {
        ui.horizontal_top(|ui| {
            for g in groups {
                ui.vertical(|ui| {
                    ui.set_width(250.0);
                    let label = if g.label.is_empty() { NO_VALUE } else { g.label.as_str() };
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(label).strong().size(13.5));
                        ui.label(RichText::new(g.items.len().to_string()).size(12.5).color(t.faint));
                    });
                    ui.add_space(4.0);
                    for i in g.items.iter().take(300) {
                        card(app, ui, t, node, *i, 228.0);
                        ui.add_space(6.0);
                    }
                });
                ui.add_space(10.0);
            }
        });
    });
    add_button(app, ui, t, node);
}

fn calendar(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, node: NodeId, groups: &[Group]) {
    let today = app.session.today;
    let (y, m) = app.session.calendar_month.unwrap_or((today.year, today.month));
    let Some(first) = Date::new(y, m, 1) else { return };
    let field = app.session.graph.calendar_field(node);
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{} {y}", MONTHS.get(m as usize - 1).copied().unwrap_or(""))).size(17.0).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            for (label, p) in [(">", json!({"step": 1})), ("Today", json!({})), ("<", json!({"step": -1}))] {
                if small_button(ui, t, label, false).clicked() {
                    app.acts.push(Act::Run("view.month".into(), p));
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
    // Weekday header.
    let (hr, _) = ui.allocate_exact_size(vec2(w, 22.0), Sense::hover());
    for (i, d) in ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"].iter().enumerate() {
        painter.text(pos2(hr.left() + col_w * (i as f32 + 0.5), hr.center().y), Align2::CENTER_CENTER, *d, FontId::proportional(12.5), t.dim);
    }
    let start = first.add_days(-(first.weekday() as i64));
    let mut day = start;
    for _week in 0..6 {
        if day.month != m && day > first {
            break;
        }
        let row_items: Vec<Vec<NodeId>> = (0..7).map(|k| dated.iter().filter(|(d, _)| *d == day.add_days(k)).map(|(_, i)| *i).collect()).collect();
        let max = row_items.iter().map(Vec::len).max().unwrap_or(0).min(4);
        let h = 30.0 + max as f32 * 22.0 + 6.0;
        let (rr, _) = ui.allocate_exact_size(vec2(w, h.max(52.0)), Sense::hover());
        for k in 0..7usize {
            let d = day.add_days(k as i64);
            let cell = Rect::from_min_size(pos2(rr.left() + col_w * k as f32, rr.top()), vec2(col_w, rr.height()));
            let weekend = k >= 5;
            painter.rect_filled(cell, 0.0, if weekend { t.field_bg } else { t.panel });
            painter.rect_stroke(cell, 0.0, Stroke::new(1.0, t.line), StrokeKind::Inside);
            let in_month = d.month == m;
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
            for (j, item) in row_items.get(k).map(Vec::as_slice).unwrap_or(&[]).iter().take(4).enumerate() {
                let r = Rect::from_min_size(cell.left_top() + vec2(4.0, 26.0 + j as f32 * 22.0), vec2(col_w - 8.0, 20.0));
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
                let name = tanacraft_core::format::plain(g.name(target));
                painter.with_clip_rect(r.shrink(2.0)).text(
                    r.left_center() + vec2(8.0, 0.0),
                    Align2::LEFT_CENTER,
                    name,
                    FontId::proportional(12.5),
                    if done { t.dim } else { t.text },
                );
                let resp = ui.interact(r, egui::Id::new(("cal-item", node.0, item.0, d.to_days())), Sense::click());
                if resp.clicked() {
                    app.acts.push(Act::Run("nav.open".into(), json!({"node": target.0})));
                }
            }
            let extra = row_items.get(k).map(Vec::len).unwrap_or(0).saturating_sub(4);
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
        ui.label(RichText::new(format!("{undated} nodes with no dates")).size(13.0).color(t.dim));
    }
}
