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
        Some(Kind::Tag(_)) => format!("#{}", g.name(id)),
        _ => g.name(id).to_string(),
    }
}

pub fn panel(app: &mut TanaApp, ui: &mut Ui) {
    let t = app.tokens();
    let width = ui.available_width();
    let content_w = (width - LEFT - 40.0).clamp(200.0, MAX_W);
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
            if app.session.graph.tag_def(zoom).is_some() {
                tag_page(app, ui, &t, zoom);
            } else {
                fields(app, ui, &t, zoom, 0.0);
                ui.add_space(4.0);
                rows(app, ui, &t);
            }
        });
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

/// Our own wording: how the Trash works.
fn trash_banner(ui: &mut Ui, t: &Tokens) {
    let warm = Color32::from_rgb(232, 128, 96);
    egui::Frame::new().fill(warm.gamma_multiply(0.12)).stroke(Stroke::new(1.0, warm.gamma_multiply(0.5))).corner_radius(8.0).inner_margin(egui::Margin::symmetric(14, 10)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new("Deleted nodes wait here").strong().color(warm));
        ui.label(RichText::new("Restore a node with the command line (Restore from trash), or empty the trash for good (Empty trash).").size(13.5).color(t.dim));
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
    ui.horizontal(|ui| {
        if let Some(done) = g.checkbox(target) {
            if checkbox(ui, t, done, 22.0).clicked() {
                app.acts.push(Act::Run("node.toggleDone".into(), json!({"node": target.0})));
            }
            ui.add_space(4.0);
        }
        if editable {
            if app.title_buf.0 != Some(target)
                || (!ui.ctx().memory(|m| m.has_focus(egui::Id::new("page-title"))) && app.title_buf.1 != g.name(target))
            {
                app.title_buf = (Some(target), g.name(target).to_string());
            }
            let out = egui::TextEdit::singleline(&mut app.title_buf.1)
                .id(egui::Id::new("page-title"))
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
    if let Some(Kind::Day { .. }) = node.map(|n| &n.kind) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            for (label, id) in [("<", "nav.previousDay"), (">", "nav.nextDay"), ("Today", "nav.today")] {
                let b = egui::Button::new(RichText::new(label).size(13.0).color(t.dim)).fill(t.field_bg).corner_radius(5.0);
                if ui.add(b).clicked() {
                    app.acts.push(Act::Run(id.into(), json!({})));
                }
            }
        });
    }
}

/// A supertag's page: what it extends, its template fields and the nodes that carry it.
fn tag_page(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, tag: NodeId) {
    let g = &app.session.graph;
    let Some(def) = g.tag_def(tag).cloned() else { return };
    ui.horizontal(|ui| {
        ui.label(RichText::new("Supertag").size(13.0).color(t.faint));
        for b in &def.extends {
            ui.label(RichText::new("extends").size(13.0).color(t.faint));
            if chip(ui, g, *b, app.dark).clicked() {
                app.acts.push(Act::Run("nav.open".into(), json!({"node": b.0})));
            }
        }
        if def.checkbox {
            ui.label(RichText::new("shows a checkbox").size(13.0).color(t.faint));
        }
    });
    ui.add_space(10.0);
    ui.label(RichText::new("Template fields").size(13.0).color(t.dim));
    for f in g.template_fields(tag) {
        let inherited = !def.fields.contains(&f);
        let ty = g.field_def(f).map(|d| d.field_type).unwrap_or_default();
        field_frame(ui, t, |ui| {
            field_label(ui, t, g.name(f), ty);
            let mut text = type_label(ty).to_string();
            if inherited {
                text.push_str("  (inherited)");
            }
            ui.label(RichText::new(text).size(14.0).color(t.faint));
        });
    }
    ui.add_space(14.0);
    let tagged = g.tagged(tag);
    ui.label(RichText::new(format!("Tagged nodes ({})", tagged.len())).size(13.0).color(t.dim));
    ui.add_space(4.0);
    let color = theme::tag_color(&def.color, app.dark).0;
    for n in tagged.into_iter().take(500) {
        let resp = ui
            .horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(20.0, ROW_H), Sense::hover());
                ui.painter().circle_filled(rect.center(), 3.5, color);
                if let Some(done) = g.checkbox(n) {
                    checkbox(ui, t, done, 15.0);
                }
                let crumbs: Vec<&str> = g.ancestors(n).iter().skip(1).map(|a| g.name(*a)).collect();
                ui.add(egui::Label::new(RichText::new(g.name(n)).size(theme::FONT).color(t.text)).sense(Sense::click()))
                    | ui.label(RichText::new(crumbs.join(" / ")).size(12.0).color(t.faint))
            })
            .inner;
        if resp.clicked() {
            app.acts.push(Act::Run("nav.open".into(), json!({"node": n.0})));
        }
    }
}

fn type_label(ty: FieldType) -> &'static str {
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

fn field_frame(ui: &mut Ui, t: &Tokens, add: impl FnOnce(&mut Ui)) {
    egui::Frame::new().fill(t.field_bg).corner_radius(6.0).inner_margin(egui::Margin::symmetric(10, 5)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.set_min_height(22.0);
            add(ui)
        });
    });
    ui.add_space(3.0);
}

fn field_label(ui: &mut Ui, t: &Tokens, name: &str, ty: FieldType) {
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
fn fields(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, node: NodeId, indent: f32) {
    let target = app.session.graph.resolve(node);
    let entries = app.session.graph.get(target).map(|n| n.fields.clone()).unwrap_or_default();
    for e in entries {
        let g = &app.session.graph;
        let Some(def) = g.field_def(e.field).cloned() else { continue };
        let name = g.name(e.field).to_string();
        let value_text = e.value.as_ref().map(|v| export::display_value(g, v));
        let editing = app.field_edit.as_ref().is_some_and(|f| f.node == target && f.field == e.field);
        ui.horizontal(|ui| {
            ui.add_space(indent);
            ui.vertical(|ui| {
                field_frame(ui, t, |ui| {
                    field_label(ui, t, &name, def.field_type);
                    field_value(app, ui, t, target, e.field, &def, e.value.as_ref(), value_text, editing);
                });
            });
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn field_value(
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
        FieldType::Options => {
            let current = text.clone().unwrap_or_default();
            let mut picked = None;
            egui::ComboBox::from_id_salt(("opt", node.0, field.0))
                .selected_text(
                    RichText::new(if current.is_empty() { "Select option".to_string() } else { current.clone() }).color(if current.is_empty() {
                        t.faint
                    } else {
                        t.text
                    }),
                )
                .width(220.0)
                .show_ui(ui, |ui| {
                    for o in &def.options {
                        if ui.selectable_label(*o == current, o).clicked() {
                            picked = Some(o.clone());
                        }
                    }
                });
            if let Some(p) = picked {
                app.acts.push(set(p));
            }
        }
        _ if editing => {
            let Some(fe) = app.field_edit.as_mut() else { return };
            let id = egui::Id::new(("field-edit", node.0, field.0));
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
        _ => "Empty",
    }
}

/// A supertag chip: `# name` on a soft fill of the tag colour.
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

fn checkbox(ui: &mut Ui, t: &Tokens, done: bool, size: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size + 4.0, ROW_H.max(size)), Sense::click());
    let b = Rect::from_center_size(rect.center(), vec2(size, size));
    let p = ui.painter();
    if done {
        p.rect_filled(b, 3.0, t.accent);
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
    for row in &rows {
        let center = row_ui(app, ui, t, row.id, row.depth);
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
fn row_ui(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, id: NodeId, depth: usize) -> egui::Pos2 {
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
    let indent = depth as f32 * INDENT;
    let mut center = pos2(0.0, 0.0);
    ui.horizontal(|ui| {
        ui.set_min_height(ROW_H);
        ui.add_space(indent);
        // Chevron + bullet.
        let (rect, resp) = ui.allocate_exact_size(vec2(20.0, ROW_H), Sense::click());
        center = rect.center();
        let row_hovered = ui.rect_contains_pointer(Rect::from_min_size(rect.left_top() - vec2(24.0, 0.0), vec2(ui.available_width() + 44.0, ROW_H)));
        if has_kids && (row_hovered || focused) {
            let chev = Rect::from_center_size(center - vec2(18.0, 0.0), vec2(14.0, ROW_H));
            let cresp = ui.interact(chev, egui::Id::new(("chev", id.0)), Sense::click());
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
        if is_ref {
            // Dashed ring: twelve short arcs around the dot.
            for i in 0..12 {
                let a0 = i as f32 / 12.0 * std::f32::consts::TAU;
                let a1 = a0 + std::f32::consts::TAU / 24.0;
                let at = |a: f32| center + vec2(a.cos(), a.sin()) * 6.0;
                ui.painter().line_segment([at(a0), at(a1)], Stroke::new(1.2, bullet_color));
            }
            ui.painter().circle_filled(center, 2.2, bullet_color);
        } else {
            ui.painter().circle_filled(center, 3.2, bullet_color);
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
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
            let mut rt = RichText::new(if name.is_empty() { " ".to_string() } else { name }).size(theme::FONT).color(text_color);
            if done == Some(true) {
                rt = rt.strikethrough();
            }
            if is_ref {
                rt = rt.underline();
            }
            let r = ui.add(egui::Label::new(rt).sense(Sense::click()));
            if r.clicked() {
                if is_ref {
                    app.acts.push(Act::Run("nav.open".into(), json!({"node": target.0})));
                } else {
                    app.acts.push(Act::Run("focus.set".into(), json!({"node": id.0})));
                }
            }
            if focused && is_ref {
                ui.painter().rect_stroke(r.rect.expand(2.0), 3.0, Stroke::new(1.0, t.accent), StrokeKind::Outside);
            }
        }
        // Tag chips.
        if let Some(n) = &node {
            for tag in n.tags.clone() {
                if chip(ui, &app.session.graph, tag, app.dark).clicked() {
                    app.acts.push(Act::Run("nav.open".into(), json!({"node": tag.0})));
                }
            }
        }
        // Click on the empty part of the row focuses it.
        let rest = ui.available_rect_before_wrap();
        if rest.width() > 4.0 && (!focused || is_ref) {
            let r = ui.interact(Rect::from_min_size(rest.min, vec2(rest.width(), ROW_H)), egui::Id::new(("row-rest", id.0)), Sense::click());
            if r.clicked() {
                app.acts.push(Act::Run("focus.set".into(), json!({"node": id.0})));
            }
        }
    });
    center
}

/// The focused bullet's text field. Typing `#` opens the supertag picker; `>` or `@` on an empty
/// bullet opens the field or reference picker.
fn edit_row(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, id: NodeId) {
    let tid = egui::Id::new(("node-edit", id.0));
    if let Some(c) = app.edit.set_cursor.take() {
        let mut st = egui::text_edit::TextEditState::load(ui.ctx(), tid).unwrap_or_default();
        st.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(c))));
        st.store(ui.ctx(), tid);
        ui.ctx().memory_mut(|m| m.request_focus(tid));
    }
    let galley_w = ui.painter().layout_no_wrap(app.edit.buf.clone(), FontId::proportional(theme::FONT), t.text).size().x;
    let out = egui::TextEdit::singleline(&mut app.edit.buf)
        .id(tid)
        .frame(egui::Frame::NONE)
        .font(FontId::proportional(theme::FONT))
        .text_color(t.text)
        .margin(egui::Margin::ZERO)
        .lock_focus(true)
        .desired_width((galley_w + 24.0).max(60.0))
        .show(ui);
    if let Some(cr) = out.cursor_range {
        app.session.caret = cr.primary.index.0;
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
                app.acts.push(Act::Pick(mode, node, anchor));
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
