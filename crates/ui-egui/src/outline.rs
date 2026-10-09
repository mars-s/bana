//! The outline panel: breadcrumbs, title, tag chips, field rows and bullets.

use egui::text::{CCursor, CCursorRange};
use egui::{Align2, Color32, FontId, Rect, RichText, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::json;
use tanacraft_commands::Session;
use tanacraft_core::{FieldType, FieldValue, Graph, Kind, NodeId, export};

use crate::theme::{self, INDENT, ROW_H, Tokens};
use crate::{Act, PickMode, TanaApp};

const LEFT: f32 = 56.0;
const MAX_W: f32 = 820.0;

/// A page's title as people read it (`Today, Sat, 10 Oct` for today).
pub fn page_title(s: &Session, id: NodeId) -> String {
    let g = &s.graph;
    match g.get(id).map(|n| &n.kind) {
        Some(Kind::Day { date }) if *date == s.today => format!("Today, {}", g.name(id)),
        Some(Kind::Day { date }) if *date == s.today.add_days(-1) => format!("Yesterday, {}", g.name(id)),
        Some(Kind::Day { date }) if *date == s.today.add_days(1) => format!("Tomorrow, {}", g.name(id)),
        Some(Kind::Tag(_)) => format!("#{}", g.name(id)),
        _ if id == g.sys.schema => "Supertags".to_string(),
        _ => g.name(id).to_string(),
    }
}

pub fn panel(app: &mut TanaApp, ui: &mut Ui) {
    let t = app.tokens();
    let width = ui.available_width();
    let side_w = 360.0;
    let wide = width > LEFT + MAX_W.min(600.0) + side_w + 80.0 && !app.session.graph.related_for(app.session.zoom).is_empty();
    let content_w = if wide { (width - LEFT - side_w - 70.0).clamp(200.0, MAX_W) } else { (width - LEFT - 40.0).clamp(200.0, MAX_W) };
    ui.add_space(12.0);
    breadcrumbs(app, ui, &t);
    ui.add_space(18.0);
    ui.horizontal(|ui| {
        ui.add_space(LEFT);
        ui.vertical(|ui| {
            ui.set_width(content_w);
            header(app, ui, &t);
            ui.add_space(10.0);
            let zoom = app.session.zoom;
            if zoom == app.session.graph.sys.trash {
                trash_banner(ui, &t);
            }
            if zoom == app.session.graph.sys.schema {
                tag_overview(app, ui, &t);
                return;
            }
            if zoom == app.session.graph.sys.journal {
                journal_root(app, ui, &t);
            }
            // Related content: beside the page when there is room, else above the content.
            let has_related = !app.session.graph.related_for(zoom).is_empty();
            if has_related && !wide {
                related(app, ui, &t, zoom);
            }
            if app.session.graph.tag_def(zoom).is_some() {
                crate::schema_ui::tag_page(app, ui, &t, zoom);
                references(app, ui, &t, zoom);
                return;
            }
            if app.session.graph.field_def(zoom).is_some() {
                crate::schema_ui::field_page(app, ui, &t, zoom);
                references(app, ui, &t, zoom);
                return;
            }
            let is_tag = false;
            let is_search = app.session.graph.query(zoom).is_some();
            fields(app, ui, &t, zoom, 0.0);
            ui.add_space(4.0);
            if is_search && app.query_open {
                crate::views::query_builder(app, ui, &t, zoom);
            }
            let has_view = is_tag || is_search || app.session.graph.get(zoom).is_some_and(|n| n.view.is_some());
            if has_view {
                crate::views::toolbar(app, ui, &t, zoom);
            }
            if crate::views::uses_layout(app, zoom) {
                crate::views::render(app, ui, &t, zoom);
            } else {
                rows(app, ui, &t);
                let hidden = app.session.graph.view_filtered_out(zoom);
                if hidden > 0 {
                    ui.label(RichText::new(format!("{hidden} items filtered out")).size(12.5).color(t.faint));
                }
            }
            references(app, ui, &t, zoom);
        });
        if wide {
            ui.add_space(24.0);
            ui.vertical(|ui| {
                ui.set_width(side_w);
                ui.add_space(70.0);
                let z = app.session.zoom;
                related(app, ui, &t, z);
            });
        }
    });
    if !app.session.status.is_empty() {
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.add_space(LEFT);
            ui.label(RichText::new(&app.session.status).size(12.5).color(Color32::from_rgb(230, 120, 110)));
        });
    }
    ui.add_space(80.0);
}

/// The Supertags overview (spec 03 §1): a filter box and a grid of supertag tiles, then fields.
fn tag_overview(app: &mut TanaApp, ui: &mut Ui, t: &Tokens) {
    ui.add(egui::TextEdit::singleline(&mut app.tag_filter).hint_text("Filter tags").desired_width(ui.available_width()));
    ui.add_space(10.0);
    let q = app.tag_filter.trim().to_lowercase();
    let g = &app.session.graph;
    let tags: Vec<NodeId> = g.tags().into_iter().filter(|x| g.name(*x).to_lowercase().contains(&q)).collect();
    let w = ui.available_width();
    let tile_w = ((w - 36.0) / 3.0).floor().max(150.0);
    let mut open = None;
    ui.label(RichText::new("Supertags").size(13.0).color(t.faint));
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(12.0, 10.0);
        for tag in &tags {
            let color = g.tag_def(*tag).map(|d| d.color.clone()).unwrap_or_default();
            let (strong, _) = theme::tag_color(&color, app.dark);
            let (r, resp) = ui.allocate_exact_size(vec2(tile_w, 40.0), Sense::click());
            ui.painter().rect_filled(r, 6.0, if resp.hovered() { t.hover } else { t.field_bg });
            crate::icons::hash(ui.painter(), Rect::from_center_size(r.left_center() + vec2(18.0, 0.0), vec2(14.0, 14.0)), strong);
            let count = g.tagged(*tag).len();
            ui.painter().text(r.left_center() + vec2(34.0, 0.0), Align2::LEFT_CENTER, g.name(*tag), FontId::proportional(14.5), t.text);
            ui.painter().text(r.right_center() - vec2(12.0, 0.0), Align2::RIGHT_CENTER, count.to_string(), FontId::proportional(12.0), t.faint);
            if resp.clicked() {
                open = Some(*tag);
            }
        }
    });
    let fields: Vec<NodeId> =
        g.iter().filter(|n| g.field_def(n.id).is_some() && !g.in_trash(n.id) && n.name.to_lowercase().contains(&q)).map(|n| n.id).collect();
    if !fields.is_empty() {
        ui.add_space(16.0);
        ui.label(RichText::new("Fields").size(13.0).color(t.faint));
        for f in fields {
            let ty = g.field_def(f).map(|d| d.field_type).unwrap_or_default();
            let resp = ui
                .horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(16.0, 22.0), Sense::hover());
                    paint_field_icon(ui.painter(), Rect::from_center_size(r.center(), vec2(11.0, 11.0)), ty, t.dim);
                    ui.add(egui::Label::new(RichText::new(g.name(f)).size(14.5)).sense(Sense::click()))
                        | ui.label(RichText::new(type_label(ty)).size(12.0).color(t.faint))
                })
                .inner;
            if resp.clicked() {
                open = Some(f);
            }
        }
    }
    if let Some(o) = open {
        app.acts.push(Act::Run("nav.open".into(), json!({"node": o.0})));
    }
}

/// Our own wording: how the Trash works.
fn trash_banner(ui: &mut Ui, t: &Tokens) {
    let warm = Color32::from_rgb(232, 128, 96);
    egui::Frame::new()
        .fill(warm.gamma_multiply(0.12))
        .stroke(Stroke::new(1.0, warm.gamma_multiply(0.5)))
        .corner_radius(8.0)
        .inner_margin(egui::Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("Deleted nodes wait here").strong().color(warm));
            ui.label(
                RichText::new("Restore a node with the command line (Restore from trash), or empty the trash for good (Empty trash).")
                    .size(13.5)
                    .color(t.dim),
            );
        });
    ui.add_space(10.0);
}

fn breadcrumbs(app: &mut TanaApp, ui: &mut Ui, t: &Tokens) {
    let crumbs = app.session.breadcrumbs();
    ui.horizontal(|ui| {
        ui.add_space(20.0);
        for (i, c) in crumbs.iter().enumerate() {
            if i > 0 {
                ui.label(RichText::new("/").size(13.0).color(t.faint));
            }
            let name = page_title(&app.session, *c);
            let r = ui.add(egui::Label::new(RichText::new(name).size(13.0).color(t.dim)).sense(Sense::click()));
            if r.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if r.clicked() {
                app.acts.push(Act::Run("nav.open".into(), json!({"node": c.0})));
            }
        }
        if crumbs.is_empty() {
            ui.label(RichText::new(" ").size(13.0));
        }
    });
}

fn header(app: &mut TanaApp, ui: &mut Ui, t: &Tokens) {
    let s = &app.session;
    let g = &s.graph;
    let zoom = s.zoom;
    let target = g.resolve(zoom);
    let editable = !g.is_system(zoom) && !matches!(g.get(zoom).map(|n| &n.kind), Some(Kind::Day { .. } | Kind::Week { .. } | Kind::Year { .. }));
    let title_font = FontId::proportional(30.0);
    // Per panel, so two panels never share a title editor (stored for the keyboard handler).
    let title_id = egui::Id::new((app.panel_ix, "page-title"));
    if !app.drawing_other_panel {
        app.title_id = title_id;
    }
    let mut menu_acts = Vec::new();
    ui.horizontal(|ui| {
        if !g.is_system(zoom) {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.menu_button(RichText::new("...").size(16.0).color(t.dim), |ui| node_menu(ui, g, zoom, &mut menu_acts));
                if g.query(zoom).is_some()
                    && ui.add(egui::Button::new(RichText::new("Conditions").size(12.5).color(t.dim)).fill(t.field_bg)).clicked()
                {
                    menu_acts.push(Act::Ui("ui.searchOptions"));
                }
            });
        }
    });
    app.acts.extend(menu_acts);
    let s = &app.session;
    let g = &s.graph;
    ui.horizontal(|ui| {
        if let Some(done) = g.checkbox(target) {
            if checkbox(ui, t, done, 22.0).clicked() {
                app.acts.push(Act::Run("node.toggleDone".into(), json!({"node": target.0})));
            }
            ui.add_space(4.0);
        }
        if editable {
            if app.title_buf.0 != Some(target) || (!ui.ctx().memory(|m| m.has_focus(title_id)) && app.title_buf.1 != g.name(target)) {
                app.title_buf = (Some(target), g.name(target).to_string());
            }
            let out = egui::TextEdit::singleline(&mut app.title_buf.1)
                .id(title_id)
                .font(title_font.clone())
                .text_color(t.text)
                .frame(egui::Frame::NONE)
                .hint_text(RichText::new("Untitled").color(t.faint))
                .desired_width(ui.available_width())
                .show(ui);
            if out.response.changed() {
                app.acts.push(Act::Run("node.setName".into(), json!({"node": target.0, "name": app.title_buf.1})));
            }
        } else {
            ui.label(RichText::new(page_title(s, zoom)).font(title_font).strong().color(t.text));
        }
    });
    let s = &app.session;
    let g = &s.graph;
    let node = g.get(target);
    let tags = node.map(|n| n.tags.clone()).unwrap_or_default();
    if !tags.is_empty() {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            for tag in tags {
                if chip(ui, g, tag, app.dark).clicked() {
                    app.acts.push(Act::Run("nav.open".into(), json!({"node": tag.0})));
                }
            }
        });
    }
    if let Some(d) = node.map(|n| n.description.as_str()).filter(|d| !d.is_empty()) {
        ui.add_space(2.0);
        ui.label(RichText::new(d).size(14.0).color(t.dim));
    }
    let nav: Option<[(&str, &str, serde_json::Value); 2]> = match node.map(|n| &n.kind) {
        Some(Kind::Day { .. }) => Some([("<", "nav.previousDay", json!({})), (">", "nav.nextDay", json!({}))]),
        Some(Kind::Week { .. } | Kind::Year { .. }) => {
            Some([("<", "nav.calendarStep", json!({"step": -1})), (">", "nav.calendarStep", json!({"step": 1}))])
        }
        _ => None,
    };
    if let Some(nav) = nav {
        day_nav(app, ui, t, nav);
    }
}

/// The calendar navigation bar: previous, next, Today, and Go to date (spec 06 §4).
fn day_nav(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, nav: [(&str, &str, serde_json::Value); 2]) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        for (label, id, params) in nav.into_iter().chain(std::iter::once(("Today", "nav.today", json!({})))) {
            let b = egui::Button::new(RichText::new(label).size(13.0).color(t.dim)).fill(t.field_bg).corner_radius(5.0);
            if ui.add(b).clicked() {
                app.acts.push(Act::Run(id.into(), params));
            }
        }
        let b = egui::Button::new(RichText::new("Go to date").size(13.0).color(t.dim)).fill(t.field_bg).corner_radius(5.0);
        let r = ui.add(b);
        if r.clicked() {
            app.acts.push(Act::GoToDate(r.rect.left_bottom() + vec2(0.0, 4.0)));
        }
    });
}

/// Related content cards (e.g. "Open tasks" on day pages): a live list per search.
pub(crate) fn related(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, node: NodeId) {
    for r in app.session.graph.related_for(node) {
        egui::Frame::new().stroke(Stroke::new(1.0, t.line)).corner_radius(8.0).inner_margin(egui::Margin::symmetric(14, 10)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(app.session.graph.name(r)).strong().size(14.0));
            ui.add_space(4.0);
            let items: Vec<NodeId> = app.session.graph.view_groups(r).into_iter().flat_map(|g| g.items).take(50).collect();
            if items.is_empty() {
                ui.label(RichText::new("Nothing here").size(13.0).color(t.faint));
            }
            for i in items {
                ui.horizontal_wrapped(|ui| crate::views::item_line(app, ui, t, i, true));
            }
        });
        ui.add_space(8.0);
    }
}

/// Daily notes root: today's page embedded under the title (spec 06 §5).
fn journal_root(app: &mut TanaApp, ui: &mut Ui, t: &Tokens) {
    let today = app.session.today;
    let day = app.session.graph.find_day(today);
    let r = ui.add(egui::Label::new(RichText::new(format!("Today, {}", today.day_title())).size(20.0).strong()).sense(Sense::click()));
    if r.clicked() {
        app.acts.push(Act::Run("nav.today".into(), json!({})));
    }
    day_nav(app, ui, t, [("<", "nav.previousDay", json!({})), (">", "nav.nextDay", json!({}))]);
    ui.add_space(6.0);
    let kids: Vec<NodeId> = day.map(|d| app.session.graph.children(d).to_vec()).unwrap_or_default();
    for k in kids.iter().take(300) {
        ui.horizontal_wrapped(|ui| crate::views::item_line(app, ui, t, *k, true));
    }
    ui.add_space(16.0);
}

pub(crate) fn type_label(ty: FieldType) -> &'static str {
    match ty {
        FieldType::Plain => "Plain",
        FieldType::Options => "Options",
        FieldType::OptionsFromTag => "Options from supertag",
        FieldType::Date => "Date",
        FieldType::Number => "Number",
        FieldType::Url => "URL",
        FieldType::Email => "Email",
        FieldType::Checkbox => "Checkbox",
    }
}

pub(crate) fn field_frame(ui: &mut Ui, t: &Tokens, add: impl FnOnce(&mut Ui)) {
    egui::Frame::new().fill(t.field_bg).corner_radius(6.0).inner_margin(egui::Margin::symmetric(10, 5)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.set_min_height(22.0);
            add(ui)
        });
    });
    ui.add_space(3.0);
}

pub(crate) fn field_label(ui: &mut Ui, t: &Tokens, name: &str, ty: FieldType) {
    let (rect, _) = ui.allocate_exact_size(vec2(170.0, 22.0), Sense::hover());
    let icon = Rect::from_center_size(rect.left_center() + vec2(7.0, 0.0), vec2(11.0, 11.0));
    paint_field_icon(ui.painter(), icon, ty, t.dim);
    let galley = ui.painter().layout_no_wrap(name.to_string(), FontId::proportional(14.5), t.text);
    ui.painter().with_clip_rect(rect).galley(pos2(rect.left() + 20.0, rect.center().y - galley.size().y / 2.0), galley, t.text);
}

/// A small glyph per field type (drawn, not an image).
fn paint_field_icon(p: &egui::Painter, r: Rect, ty: FieldType, c: Color32) {
    let st = Stroke::new(1.2, c);
    match ty {
        FieldType::Date => {
            p.rect_stroke(r, 2.0, st, StrokeKind::Middle);
            p.line_segment([r.left_top() + vec2(0.0, 3.5), r.right_top() + vec2(0.0, 3.5)], st);
        }
        FieldType::Number => {
            crate::icons::hash(p, r, c);
        }
        FieldType::Checkbox => {
            p.rect_stroke(r, 2.0, st, StrokeKind::Middle);
            p.line_segment([r.left_center() + vec2(2.5, 0.0), r.center_bottom() - vec2(0.0, 3.0)], st);
            p.line_segment([r.center_bottom() - vec2(0.0, 3.0), r.right_top() + vec2(-2.0, 2.5)], st);
        }
        FieldType::Options | FieldType::OptionsFromTag => {
            p.circle_stroke(r.center(), r.width() * 0.45, st);
            p.circle_filled(r.center(), 1.6, c);
        }
        FieldType::Url | FieldType::Email => {
            p.circle_stroke(r.center(), r.width() * 0.45, st);
            p.line_segment([r.left_center(), r.right_center()], st);
        }
        FieldType::Plain => {
            for i in 0..3 {
                let y = r.top() + 2.0 + i as f32 * 3.5;
                p.line_segment([pos2(r.left(), y), pos2(r.right() - if i == 2 { 4.0 } else { 0.0 }, y)], st);
            }
        }
    }
}

/// Field rows of `node` (its own and its supertags'), indented by `indent`.
pub(crate) fn fields(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, node: NodeId, indent: f32) {
    let target = app.session.graph.resolve(node);
    let entries = app.session.graph.get(target).map(|n| n.fields.clone()).unwrap_or_default();
    for e in entries {
        let g = &app.session.graph;
        let Some(def) = g.field_def(e.field).cloned() else { continue };
        let name = g.name(e.field).to_string();
        let value_text = e.value.as_ref().map(|v| export::display_value(g, v));
        let editing = app.field_edit.as_ref().is_some_and(|f| f.node == target && f.field == e.field);
        // Hide-field rules (spec 03 §6).
        let empty = value_text.as_deref().is_none_or(str::is_empty);
        let is_default = def.default.is_some() && e.value == def.default;
        let hidden = match def.hide {
            tanacraft_core::HideMode::Never => false,
            tanacraft_core::HideMode::WhenEmpty => empty,
            tanacraft_core::HideMode::WhenNotEmpty => !empty,
            tanacraft_core::HideMode::WhenDefault => is_default,
            tanacraft_core::HideMode::Always => true,
        };
        if hidden && !editing {
            continue;
        }
        let missing = def.required && empty;
        ui.horizontal(|ui| {
            ui.add_space(indent);
            ui.vertical(|ui| {
                field_frame(ui, t, |ui| {
                    field_label(ui, t, &name, def.field_type);
                    field_value(app, ui, t, target, e.field, &def, e.value.as_ref(), value_text, editing);
                    if missing {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new("Required").size(12.0).color(Color32::from_rgb(232, 128, 96)));
                        });
                    }
                });
            });
        });
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn field_value(
    app: &mut TanaApp,
    ui: &mut Ui,
    t: &Tokens,
    node: NodeId,
    field: NodeId,
    def: &tanacraft_core::FieldDef,
    value: Option<&FieldValue>,
    text: Option<String>,
    editing: bool,
) {
    let set = |v: String| Act::Run("field.set".into(), json!({"node": node.0, "field": field.0, "value": v}));
    match def.field_type {
        FieldType::Checkbox => {
            let on = matches!(value, Some(FieldValue::Checkbox(true)));
            if checkbox(ui, t, on, 15.0).clicked() {
                app.acts.push(set(if on { "no" } else { "yes" }.into()));
            }
        }
        FieldType::Date if !editing => {
            // "Thu, 15 Oct" with a calendar glyph; ">" opens the day (spec 03 §8).
            let date = match value {
                Some(FieldValue::Date(d)) => Some(*d),
                _ => None,
            };
            let label = match date {
                Some(d) if d.year == app.session.today.year => d.day_title(),
                Some(d) => format!("{}, {}", d.day_title(), d.year),
                None => hint(def.field_type).to_string(),
            };
            if date.is_some() {
                let (r, _) = ui.allocate_exact_size(vec2(14.0, 18.0), Sense::hover());
                let b = Rect::from_center_size(r.center(), vec2(11.0, 10.0));
                ui.painter().rect_stroke(b, 2.0, Stroke::new(1.1, t.dim), StrokeKind::Middle);
                ui.painter().line_segment([b.left_top() + vec2(0.0, 3.0), b.right_top() + vec2(0.0, 3.0)], Stroke::new(1.1, t.dim));
            }
            let r =
                ui.add(egui::Label::new(RichText::new(label).size(14.5).color(if date.is_some() { t.text } else { t.faint })).sense(Sense::click()));
            if r.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if r.clicked() {
                app.acts.push(Act::PickDate(node, field, r.rect.left_bottom() + vec2(0.0, 4.0)));
            }
            if let Some(d) = date
                && ui
                    .add(egui::Label::new(RichText::new(">").size(13.0).color(t.faint)).sense(Sense::click()))
                    .on_hover_text("Open the day")
                    .clicked()
            {
                app.acts.push(Act::Run("nav.day".into(), json!({"date": d.to_string()})));
            }
        }
        FieldType::Options if !editing => {
            let current = text.clone().unwrap_or_default();
            let (label, color) = if current.is_empty() { (hint(def.field_type).to_string(), t.faint) } else { (current, t.text) };
            let r = ui.add(egui::Label::new(RichText::new(label).size(14.5).color(color)).sense(Sense::click()));
            if r.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if r.clicked() {
                let at = r.rect.left_bottom() + vec2(0.0, 4.0);
                app.acts.push(if def.field_type == FieldType::Date { Act::PickDate(node, field, at) } else { Act::PickChoice(node, field, at) });
            }
        }
        _ if editing => {
            let Some(fe) = app.field_edit.as_mut() else { return };
            let id = egui::Id::new((app.panel_ix, "field-edit", node.0, field.0));
            let resp = ui.add(
                egui::TextEdit::singleline(&mut fe.buf)
                    .id(id)
                    .frame(egui::Frame::NONE)
                    .desired_width(ui.available_width())
                    .hint_text(hint(def.field_type)),
            );
            if fe.focus_once {
                resp.request_focus();
                fe.focus_once = false;
            } else if resp.lost_focus() {
                let v = fe.buf.clone();
                app.field_edit = None;
                app.acts.push(set(v));
            }
        }
        _ => {
            let (label, color) = match &text {
                Some(s) if !s.is_empty() => {
                    (s.clone(), if matches!(def.field_type, FieldType::Url | FieldType::Email | FieldType::OptionsFromTag) { t.link } else { t.text })
                }
                _ => (hint(def.field_type).to_string(), t.faint),
            };
            let r = ui.add(egui::Label::new(RichText::new(label).size(14.5).color(color)).sense(Sense::click()));
            if r.clicked() {
                app.acts.push(Act::EditField(node, field, text.unwrap_or_default()));
            }
        }
    }
}

fn hint(ty: FieldType) -> &'static str {
    match ty {
        FieldType::Date => "Add date",
        FieldType::Number => "Add number",
        FieldType::Url => "Add link",
        FieldType::Email => "Add email",
        FieldType::OptionsFromTag => "Select node",
        FieldType::Options => "Select option",
        _ => "Empty",
    }
}

/// What an inline reference or link in node text points at.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum LinkTarget {
    Node(NodeId),
    Url(String),
}

/// Node text with its inline marks rendered (bold, italic, underline, highlight, code), inline
/// references shown as the target's name and links in blue. Returns the job and, per section, what
/// clicking it opens.
// ponytail: the default font has no bold weight; bold is drawn brighter and slightly spaced. Ship a
// bold font file (with an ATTRIBUTION row) when that is not enough.
pub(crate) fn rich_job_links(
    g: &Graph,
    t: &Tokens,
    text: &str,
    color: Color32,
    strike: bool,
    size: f32,
) -> (egui::text::LayoutJob, Vec<Option<LinkTarget>>) {
    let mut job = egui::text::LayoutJob::default();
    let mut targets = Vec::new();
    for sp in tanacraft_core::format::spans(text) {
        let mut f = egui::TextFormat { font_id: FontId::proportional(size), color, italics: sp.italic, ..Default::default() };
        if sp.bold {
            f.color = if color == t.text { t.text.linear_multiply(1.15) } else { color };
            f.extra_letter_spacing = 0.35;
        }
        if sp.underline {
            f.underline = Stroke::new(1.0, color);
        }
        if sp.highlight {
            f.background = Color32::from_rgba_unmultiplied(214, 178, 46, 70);
        }
        if sp.code {
            f.font_id = FontId::monospace(size - 1.5);
            f.color = Color32::from_rgb(222, 140, 80);
            f.background = t.hover;
        }
        if strike {
            f.strikethrough = Stroke::new(1.0, color);
        }
        let (shown, target) = match (sp.node, &sp.url) {
            (Some(n), _) => {
                f.color = t.link;
                let name = g.text(NodeId(n));
                (
                    if g.get(NodeId(n)).is_none() {
                        "(missing node)".to_string()
                    } else if name.is_empty() {
                        "Untitled".to_string()
                    } else {
                        name
                    },
                    Some(LinkTarget::Node(NodeId(n))),
                )
            }
            (_, Some(u)) => {
                f.color = t.link;
                f.underline = Stroke::new(1.0, t.link.gamma_multiply(0.6));
                (sp.text.clone(), Some(LinkTarget::Url(u.clone())))
            }
            _ => (sp.text.clone(), None),
        };
        if shown.is_empty() {
            continue;
        }
        job.append(&shown, 0.0, f);
        targets.push(target);
    }
    if job.text.is_empty() {
        job.append(" ", 0.0, egui::TextFormat { font_id: FontId::proportional(size), color, ..Default::default() });
        targets.push(None);
    }
    (job, targets)
}

pub(crate) fn rich_job(g: &Graph, t: &Tokens, text: &str, color: Color32, strike: bool, size: f32) -> egui::text::LayoutJob {
    rich_job_links(g, t, text, color, strike, size).0
}

/// A clickable label with marks, inline references and links. Returns the response and, when the
/// click landed on a reference or link, its target.
pub(crate) fn link_label(ui: &mut Ui, job: egui::text::LayoutJob, targets: &[Option<LinkTarget>]) -> (egui::Response, Option<LinkTarget>) {
    let (pos, galley, resp) = egui::Label::new(job).sense(Sense::click()).layout_in_ui(ui);
    ui.painter().galley(pos, galley.clone(), Color32::WHITE);
    // Keep the text visible to accessibility (and to UI tests).
    let text = galley.text().to_string();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, text.clone()));
    let mut hit = None;
    if let Some(p) = ui.ctx().pointer_hover_pos().filter(|p| resp.rect.contains(*p)) {
        let c = galley.cursor_from_pos(p - pos);
        let byte = galley.job.text.char_indices().nth(c.index.0).map(|(b, _)| b).unwrap_or(galley.job.text.len());
        let sec = galley.job.sections.iter().position(|s| s.byte_range.start.0 <= byte && byte < s.byte_range.end.0);
        hit = sec.and_then(|i| targets.get(i).cloned().flatten());
        if hit.is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }
    let clicked = resp.clicked();
    (resp, if clicked { hit } else { None })
}

/// A clickable label with inline marks rendered; references and links open on click.
pub(crate) fn rich_label(ui: &mut Ui, app_acts: &mut Vec<Act>, g: &Graph, t: &Tokens, text: &str, color: Color32, strike: bool) -> egui::Response {
    let (job, targets) = rich_job_links(g, t, text, color, strike, theme::FONT);
    let (resp, hit) = link_label(ui, job, &targets);
    match hit {
        Some(LinkTarget::Node(n)) => app_acts.push(Act::Run("nav.open".into(), json!({"node": n.0}))),
        Some(LinkTarget::Url(u)) => ui.ctx().open_url(egui::OpenUrl::new_tab(u)),
        None => return resp,
    }
    // The link handled the click; the caller should not also act on it.
    resp.clone().on_hover_text("Open")
}

/// The node menu (right-click a bullet, or "..." on a page).
pub(crate) fn node_menu(ui: &mut Ui, g: &Graph, id: NodeId, acts: &mut Vec<Act>) {
    let n = json!({"node": id.0});
    let mut item = |ui: &mut Ui, label: &str, act: Act| {
        if ui.button(label).clicked() {
            acts.push(act);
            ui.close();
        }
    };
    let run = |cmd: &str| Act::Run(cmd.into(), n.clone());
    item(ui, "Open", run("nav.zoomIn"));
    item(ui, if g.is_expanded(id) { "Collapse" } else { "Expand" }, run("node.toggleExpand"));
    ui.separator();
    let at = ui.min_rect().left_bottom();
    item(ui, "Add supertag", Act::Pick(PickMode::Tag, g.resolve(id), at));
    item(ui, "Add field", Act::Pick(PickMode::Field, g.resolve(id), at));
    item(ui, if g.checkbox(id).is_some() { "Toggle done" } else { "Add checkbox" }, run("node.toggleDone"));
    item(ui, "Add description", Act::Ui("ui.editDescription"));
    item(ui, "Toggle heading", run("node.toggleHeading"));
    let pinned = g.get(g.resolve(id)).is_some_and(|n| n.pinned);
    item(ui, if pinned { "Unpin from sidebar" } else { "Pin to sidebar" }, run("node.togglePin"));
    ui.separator();
    ui.menu_button("View as", |ui| {
        for k in tanacraft_core::ViewKind::ALL {
            if ui.button(k.name()).clicked() {
                acts.push(Act::Run("view.set".into(), json!({"node": id.0, "kind": k.name()})));
                ui.close();
            }
        }
    });
    let mut item = |ui: &mut Ui, label: &str, act: Act| {
        if ui.button(label).clicked() {
            acts.push(act);
            ui.close();
        }
    };
    item(ui, "Duplicate", run("node.duplicate"));
    item(ui, "Move to...", Act::Pick(PickMode::MoveTo, id, at));
    item(ui, "Move up", run("node.moveUp"));
    item(ui, "Move down", run("node.moveDown"));
    item(ui, "Indent", run("node.indent"));
    item(ui, "Outdent", run("node.outdent"));
    ui.separator();
    if g.query(id).is_some() {
        item(ui, "Convert search node to plain list", run("search.toPlainList"));
    }
    if g.in_trash(id) {
        item(ui, "Restore", run("node.restore"));
    } else {
        item(ui, "Delete", run("node.delete"));
    }
}

/// The footer of a page: nodes that point at it (references and node-valued fields).
fn references(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, node: NodeId) {
    let g = &app.session.graph;
    let target = g.resolve(node);
    let refs = g.references_to(target);
    if refs.is_empty() {
        return;
    }
    ui.add_space(24.0);
    ui.label(RichText::new(format!("References  {}", refs.len())).size(13.0).strong().color(t.dim));
    ui.add_space(4.0);
    for r in refs.into_iter().take(200) {
        let g = &app.session.graph;
        // A reference node shows where it sits; a field reference shows the node holding the field.
        let (open, place) = match g.get(r).and_then(|n| n.reference_target()) {
            Some(_) => (g.parent(r).unwrap_or(r), g.parent(r).map(|p| page_title(&app.session, p)).unwrap_or_default()),
            None => (r, g.name(r).to_string()),
        };
        let crumbs: Vec<&str> = g.ancestors(open).iter().skip(1).map(|a| g.name(*a)).collect();
        let resp = ui
            .horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(14.0, 18.0), Sense::hover());
                ui.painter().circle_stroke(r.center(), 4.0, Stroke::new(1.2, t.faint));
                ui.add(egui::Label::new(RichText::new(place).size(14.5).color(t.link)).sense(Sense::click()))
                    | ui.label(RichText::new(crumbs.join(" / ")).size(12.0).color(t.faint))
            })
            .inner;
        if resp.clicked() {
            app.acts.push(Act::Run("nav.open".into(), json!({"node": open.0})));
        }
    }
}

/// A supertag chip: `# name` on a soft fill of the tag colour.
/// A chip whose "#" turns into an "x" on hover: returns (open clicked, remove clicked).
pub(crate) fn tag_chip(ui: &mut Ui, g: &Graph, tag: NodeId, dark: bool) -> (bool, bool) {
    let resp = chip(ui, g, tag, dark);
    let x_zone = Rect::from_min_size(resp.rect.min, vec2(18.0, resp.rect.height()));
    let over_x = ui.ctx().pointer_hover_pos().is_some_and(|p| x_zone.contains(p));
    if resp.hovered() {
        let color = g.tag_def(tag).map(|d| d.color.as_str()).unwrap_or("grey");
        let (strong, soft) = theme::tag_color(color, dark);
        let r = Rect::from_center_size(resp.rect.left_center() + vec2(10.0, 0.0), vec2(12.0, 12.0));
        let p = ui.painter();
        p.rect_filled(r.expand(1.0), 2.0, soft.gamma_multiply(3.0));
        let st = Stroke::new(1.4, strong);
        p.line_segment([r.left_top() + vec2(2.0, 2.0), r.right_bottom() - vec2(2.0, 2.0)], st);
        p.line_segment([r.right_top() + vec2(-2.0, 2.0), r.left_bottom() + vec2(2.0, -2.0)], st);
    }
    let remove = resp.clicked() && over_x;
    if over_x {
        resp.clone().on_hover_text("Remove tag from item");
    }
    (resp.clicked() && !remove, remove)
}

pub fn chip(ui: &mut Ui, g: &Graph, tag: NodeId, dark: bool) -> egui::Response {
    let color = g.tag_def(tag).map(|d| d.color.as_str()).unwrap_or("grey");
    let (strong, soft) = theme::tag_color(color, dark);
    let galley = ui.painter().layout_no_wrap(g.name(tag).to_string(), FontId::proportional(13.0), strong);
    let size = vec2(galley.size().x + 26.0, 20.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    p.rect_filled(rect, 4.0, soft);
    crate::icons::hash(p, Rect::from_center_size(rect.left_center() + vec2(10.0, 0.0), vec2(10.0, 10.0)), strong);
    p.galley(pos2(rect.left() + 19.0, rect.center().y - galley.size().y / 2.0), galley, strong);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

pub(crate) fn checkbox(ui: &mut Ui, t: &Tokens, done: bool, size: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size + 4.0, ROW_H.max(size)), Sense::click());
    let b = Rect::from_center_size(rect.center(), vec2(size, size));
    let p = ui.painter();
    if done {
        // Checked: green fill, white check (spec 02 §9).
        p.rect_filled(b, 3.0, Color32::from_rgb(76, 160, 96));
        let st = Stroke::new(1.8, Color32::WHITE);
        let a = b.left_center() + vec2(size * 0.22, 0.0);
        let m = b.center_bottom() + vec2(-size * 0.08, -size * 0.25);
        let z = b.right_top() + vec2(-size * 0.2, size * 0.25);
        p.line_segment([a, m], st);
        p.line_segment([m, z], st);
    } else {
        p.rect_filled(b, 3.0, t.hover);
        p.rect_stroke(b, 3.0, Stroke::new(1.0, t.line), StrokeKind::Inside);
    }
    resp
}

/// The bullets.
fn rows(app: &mut TanaApp, ui: &mut Ui, t: &Tokens) {
    let rows = app.session.rows();
    let mut bullets: Vec<(usize, egui::Pos2)> = Vec::with_capacity(rows.len());
    let mut bottoms: Vec<f32> = Vec::with_capacity(rows.len());
    // Rows inside the selection (anchor..head) are highlighted, children included.
    let sel_range = app.session.sel.and_then(|(a, h)| {
        let i = rows.iter().position(|r| r.id == a)?;
        let j = rows.iter().position(|r| r.id == h)?;
        Some(i.min(j)..=i.max(j))
    });
    for (ri, row) in rows.iter().enumerate() {
        let selected = sel_range.as_ref().is_some_and(|r| r.contains(&ri));
        let center = row_ui(app, ui, t, row.id, row.depth, selected);
        bullets.push((row.depth, center));
        let expanded = app.session.graph.is_expanded(row.id);
        if expanded {
            fields(app, ui, t, row.id, (row.depth + 1) as f32 * INDENT + 8.0);
        }
        bottoms.push(ui.cursor().top());
    }
    // Guide lines: from an expanded bullet down to its last visible descendant.
    let p = ui.painter();
    for (i, (depth, c)) in bullets.iter().enumerate() {
        let end = rows.iter().enumerate().skip(i + 1).take_while(|(_, r)| r.depth > *depth).last().map(|(j, _)| j);
        let end_y = match end {
            Some(j) => bottoms.get(j).copied(),
            None if app.session.graph.is_expanded(rows[i].id) => bottoms.get(i).copied().filter(|b| *b > c.y + ROW_H),
            None => None,
        };
        if let Some(y) = end_y {
            p.line_segment([pos2(c.x, c.y + 11.0), pos2(c.x, y - 4.0)], Stroke::new(1.0, t.line));
        }
    }
    // Trailing empty bullet: click to add a node at the end.
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    let c = pos2(rect.left() + 10.0, rect.center().y);
    ui.painter().circle_filled(c, 2.5, t.faint.gamma_multiply(0.6));
    if resp.hovered() {
        ui.painter().text(c + vec2(14.0, 0.0), Align2::LEFT_CENTER, "Click to add a node", FontId::proportional(13.5), t.faint);
    }
    if resp.clicked() {
        let zoom = app.session.zoom;
        let r = app.session.run("node.add", &json!({"parent": zoom.0}));
        if let Some(id) = r.ok().and_then(|v| v["id"].as_u64()) {
            app.acts.push(Act::Run("focus.set".into(), json!({"node": id, "caret": 0})));
        }
    }
}

/// One bullet row. Returns the bullet centre.
fn row_ui(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, id: NodeId, depth: usize, selected: bool) -> egui::Pos2 {
    let g = &app.session.graph;
    let target = g.resolve(id);
    let is_ref = target != id;
    let node = g.get(target).cloned();
    let has_kids = !g.children(id).is_empty() || node.as_ref().is_some_and(|n| !n.fields.is_empty());
    let expanded = g.is_expanded(id);
    let tag_color = node
        .as_ref()
        .and_then(|n| n.tags.iter().find(|t| **t != g.sys.day_tag))
        .and_then(|t| g.tag_def(*t))
        .map(|d| theme::tag_color(&d.color, app.dark).0);
    let done = g.checkbox(target);
    let focused = app.session.focus == Some(id);
    let heading = node.as_ref().is_some_and(|n| n.heading);
    let description = node.as_ref().map(|n| n.description.clone()).unwrap_or_default();
    // An unfocused empty line shows no bullet (spec 01 §1).
    let blank = !focused && !is_ref && !has_kids && node.as_ref().is_some_and(|n| n.name.is_empty() && n.tags.is_empty() && n.kind == Kind::Plain);
    let indent = depth as f32 * INDENT;
    let mut center = pos2(0.0, 0.0);
    if selected {
        let r = Rect::from_min_size(ui.cursor().min + vec2(indent - 4.0, 0.0), vec2(ui.available_width() - indent + 4.0, ROW_H));
        ui.painter().rect_filled(r, 4.0, t.accent.gamma_multiply(0.18));
    }
    let row = ui.horizontal(|ui| {
        ui.set_min_height(ROW_H);
        ui.add_space(indent);
        // Chevron + bullet (drag the bullet to move the node).
        let (rect, resp) = ui.allocate_exact_size(vec2(20.0, ROW_H), Sense::click_and_drag());
        center = rect.center();
        if resp.drag_started() {
            resp.dnd_set_drag_payload(id);
        }
        let row_hovered = ui.rect_contains_pointer(Rect::from_min_size(rect.left_top() - vec2(24.0, 0.0), vec2(ui.available_width() + 44.0, ROW_H)));
        if has_kids && (row_hovered || focused) {
            let chev = Rect::from_center_size(center - vec2(18.0, 0.0), vec2(14.0, ROW_H));
            let cresp = ui.interact(chev, egui::Id::new((app.panel_ix, "chev", id.0)), Sense::click());
            let c = chev.center();
            let pts = if expanded {
                vec![c + vec2(-4.0, -2.0), c + vec2(4.0, -2.0), c + vec2(0.0, 3.0)]
            } else {
                vec![c + vec2(-2.0, -4.0), c + vec2(3.0, 0.0), c + vec2(-2.0, 4.0)]
            };
            ui.painter().add(egui::Shape::convex_polygon(pts, if cresp.hovered() { t.text } else { t.dim }, Stroke::NONE));
            if cresp.clicked() {
                app.acts.push(Act::Run("node.toggleExpand".into(), json!({"node": id.0})));
            }
        }
        let bullet_color = tag_color.unwrap_or(t.dim);
        if has_kids && !expanded {
            ui.painter().circle_filled(center, 8.0, t.line);
        }
        if app.session.graph.query(target).is_some() {
            crate::icons::paint(ui.painter(), Rect::from_center_size(center, vec2(12.0, 12.0)), crate::icons::Icon::Search, t.dim);
        } else if is_ref {
            // Dashed ring: twelve short arcs around the dot.
            for i in 0..12 {
                let a0 = i as f32 / 12.0 * std::f32::consts::TAU;
                let a1 = a0 + std::f32::consts::TAU / 24.0;
                let at = |a: f32| center + vec2(a.cos(), a.sin()) * 6.0;
                ui.painter().line_segment([at(a0), at(a1)], Stroke::new(1.2, bullet_color));
            }
            ui.painter().circle_filled(center, 2.2, bullet_color);
        } else if !blank {
            ui.painter().circle_filled(center, 3.2, bullet_color);
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        let mut menu_acts = Vec::new();
        resp.context_menu(|ui| node_menu(ui, &app.session.graph, id, &mut menu_acts));
        app.acts.extend(menu_acts);
        if resp.clicked() {
            app.acts.push(Act::Run("nav.zoomIn".into(), json!({"node": id.0})));
        }
        // Checkbox.
        if let Some(d) = done
            && checkbox(ui, t, d, 15.0).clicked()
        {
            app.acts.push(Act::Run("node.toggleDone".into(), json!({"node": id.0})));
        }
        // Text.
        let name = app.session.graph.name(id).to_string();
        let text_color = if done == Some(true) { t.dim } else { t.text };
        if focused && !is_ref {
            edit_row(app, ui, t, id);
        } else {
            let size = if heading { 20.0 } else { theme::FONT };
            let (mut job, targets) = rich_job_links(&app.session.graph, t, &name, text_color, false, size);
            if is_ref {
                for sec in &mut job.sections {
                    sec.format.underline = Stroke::new(1.0, t.dim);
                }
            }
            let (r, hit) = link_label(ui, job, &targets);
            let link_hit = hit.is_some();
            match hit {
                Some(LinkTarget::Node(n)) => app.acts.push(Act::Run("nav.open".into(), json!({"node": n.0}))),
                Some(LinkTarget::Url(u)) => ui.ctx().open_url(egui::OpenUrl::new_tab(u)),
                None => {}
            }
            if r.clicked() && !link_hit {
                if is_ref {
                    app.acts.push(Act::Run("nav.open".into(), json!({"node": target.0})));
                } else if ui.input(|i| i.modifiers.shift) {
                    let from = app.session.sel.map(|s| s.0).or(app.session.focus).unwrap_or(id);
                    app.acts.push(Act::Run("select.set".into(), json!({"from": from.0, "to": id.0})));
                } else {
                    app.acts.push(Act::Run("select.clear".into(), json!({})));
                    app.acts.push(Act::Run("focus.set".into(), json!({"node": id.0})));
                }
            }
            if focused && is_ref {
                ui.painter().rect_stroke(r.rect.expand(2.0), 3.0, Stroke::new(1.0, t.accent), StrokeKind::Outside);
            }
        }
        // Tag chips (hover the "#" to remove).
        if let Some(n) = &node {
            for tag in n.tags.clone() {
                let (open, remove) = tag_chip(ui, &app.session.graph, tag, app.dark);
                if open {
                    app.acts.push(Act::Run("nav.open".into(), json!({"node": tag.0})));
                }
                if remove {
                    app.acts.push(Act::Run("tag.remove".into(), json!({"node": target.0, "tag": tag.0})));
                }
            }
        }
        // Click on the empty part of the row focuses it.
        let rest = ui.available_rect_before_wrap();
        if rest.width() > 4.0 && (!focused || is_ref) {
            let r = ui.interact(
                Rect::from_min_size(rest.min, vec2(rest.width(), ROW_H)),
                egui::Id::new((app.panel_ix, "row-rest", id.0)),
                Sense::click(),
            );
            if r.clicked() {
                app.acts.push(Act::Run("focus.set".into(), json!({"node": id.0})));
            }
        }
    });
    if !description.is_empty() {
        ui.horizontal(|ui| {
            ui.add_space(indent + 26.0);
            ui.label(RichText::new(description).size(13.0).color(t.dim));
        });
    }
    if depth == 0 {
        let line = app.session.graph.display_line(app.session.zoom, id);
        if !line.is_empty() {
            ui.horizontal(|ui| {
                ui.add_space(indent + 26.0);
                ui.label(RichText::new(line).size(12.5).color(t.dim));
            });
        }
    }
    drop_target(app, ui, t, id, center, row.response.rect);
    center
}

/// Drag and drop: a bullet dropped on a row lands after it; dropped right of the row's bullet
/// (one indent further), it becomes the row's first child.
fn drop_target(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, id: NodeId, bullet: egui::Pos2, rect: Rect) {
    let resp = ui.interact(rect, egui::Id::new((app.panel_ix, "drop", id.0)), Sense::hover());
    let Some(dragged) = resp.dnd_hover_payload::<NodeId>() else { return };
    if *dragged == id {
        return;
    }
    let as_child = ui.ctx().pointer_hover_pos().is_some_and(|p| p.x > bullet.x + INDENT);
    let x = if as_child { bullet.x + INDENT - 8.0 } else { bullet.x - 8.0 };
    ui.painter().line_segment([pos2(x, rect.bottom()), pos2(rect.right(), rect.bottom())], Stroke::new(2.0, t.accent));
    if let Some(dragged) = resp.dnd_release_payload::<NodeId>() {
        let g = &app.session.graph;
        let params = if as_child {
            json!({"node": dragged.0, "parent": g.resolve(id).0, "index": 0})
        } else {
            match (g.parent(id), g.index_in_parent(id)) {
                (Some(p), Some(i)) => json!({"node": dragged.0, "parent": p.0, "index": i + 1}),
                _ => return,
            }
        };
        if as_child {
            app.acts.push(Act::Run("node.expand".into(), json!({"node": id.0})));
        }
        app.acts.push(Act::Run("node.move".into(), params));
    }
}

/// The focused bullet's text field. Typing `#` opens the supertag picker; `>` or `@` on an empty
/// bullet opens the field or reference picker.
fn edit_row(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, id: NodeId) {
    let tid = egui::Id::new((app.panel_ix, "node-edit", id.0));
    if let Some((a, c)) = app.edit.set_cursor.take() {
        let mut st = egui::text_edit::TextEditState::load(ui.ctx(), tid).unwrap_or_default();
        st.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(a), CCursor::new(c))));
        st.store(ui.ctx(), tid);
        ui.ctx().memory_mut(|m| m.request_focus(tid));
    }
    let size = if app.session.graph.get(app.session.graph.resolve(id)).is_some_and(|n| n.heading) { 20.0 } else { theme::FONT };
    let galley_w = ui.painter().layout_no_wrap(app.edit.buf.clone(), FontId::proportional(size), t.text).size().x;
    let out = egui::TextEdit::singleline(&mut app.edit.buf)
        .id(tid)
        .frame(egui::Frame::NONE)
        .font(FontId::proportional(size))
        .hint_text(RichText::new("Type, or # for a supertag, @ for a reference, > for a field").color(t.faint))
        .text_color(t.text)
        .margin(egui::Margin::ZERO)
        .lock_focus(true)
        .desired_width(if galley_w < 1.0 { 420.0 } else { (galley_w + 24.0).max(60.0) })
        .show(ui);
    if let Some(cr) = out.cursor_range {
        app.session.caret = cr.primary.index.0;
        app.session.anchor = cr.secondary.index.0;
        app.edit.selection_empty = cr.primary == cr.secondary;
    }
    if out.response.changed() {
        let buf = app.edit.buf.clone();
        let caret = app.session.caret;
        let anchor = out.response.rect.left_bottom() + vec2(0.0, 4.0);
        let typed = caret.checked_sub(1).and_then(|i| buf.chars().nth(i));
        let trigger = match typed {
            Some('#') => Some(PickMode::Tag),
            Some('>') if buf == ">" => Some(PickMode::Field),
            Some('@') if buf == "@" => Some(PickMode::Reference),
            Some('@') => Some(PickMode::InlineRef),
            Some('/') if buf == "/" => Some(PickMode::Slash),
            _ => None,
        };
        match trigger {
            Some(mode) => {
                // Drop the trigger character; the picker takes over.
                let cleaned: String = buf.chars().enumerate().filter(|(i, _)| *i + 1 != caret).map(|(_, c)| c).collect();
                app.edit.buf = cleaned.clone();
                if cleaned != app.session.graph.name(id) {
                    app.acts.push(Act::Run("node.setName".into(), json!({"node": id.0, "name": cleaned, "caret": caret - 1})));
                }
                let node = if mode == PickMode::Field { app.session.graph.parent(id).unwrap_or(id) } else { id };
                if mode == PickMode::InlineRef {
                    app.acts.push(Act::PickInline(id, caret - 1, anchor));
                } else {
                    app.acts.push(Act::Pick(mode, node, anchor));
                }
                if mode == PickMode::Field {
                    // `>` on an empty bullet becomes a field of the parent: the bullet goes away.
                    app.acts.push(Act::Run("node.backspace".into(), json!({"node": id.0})));
                }
            }
            None => app.acts.push(Act::Run("node.setName".into(), json!({"node": id.0, "name": buf, "caret": caret}))),
        }
    }
    if out.response.lost_focus() && app.picker.is_none() && app.session.focus == Some(id) && !ui.ctx().memory(|m| m.focused().is_some()) {
        // Clicked elsewhere: stop editing (the next click may focus another bullet).
        app.session.focus = None;
        app.edit.node = None;
    }
}
