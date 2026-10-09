//! Overlays: the command line (commands and nodes), the `#`/`>`/`@` pickers, the shortcuts panel and
//! the description editor.

use egui::{Align2, FontId, Order, RichText, Sense, Ui, vec2};
use serde_json::json;
use tanacraft_core::{NodeId, export};

use crate::{PickMode, TanaApp, theme};

const MAX_ITEMS: usize = 12;

/// One command-line result.
#[derive(Debug, Clone)]
pub enum Item {
    Command { id: String, label: String, shortcut: String },
    Node { id: NodeId, line: String },
}

/// Results for a command-line query: matching nodes first, then commands.
pub fn palette_items(app: &TanaApp, query: &str) -> Vec<Item> {
    let mut out = Vec::new();
    let q = query.trim();
    if !q.is_empty() {
        for id in app.session.graph.search(q).into_iter().take(6) {
            out.push(Item::Node { id, line: export::node_line(&app.session.graph, id) });
        }
    }
    let words: Vec<String> = q.split_whitespace().map(str::to_lowercase).collect();
    for (id, label, shortcut) in crate::UI_COMMANDS {
        let hay = format!("{label} {id}").to_lowercase();
        if words.iter().all(|w| hay.contains(w.as_str())) {
            out.push(Item::Command { id: id.to_string(), label: label.to_string(), shortcut: shortcut.to_string() });
        }
    }
    for c in app.session.registry.search(q) {
        out.push(Item::Command { id: c.id.to_string(), label: c.label.to_string(), shortcut: c.shortcut.to_string() });
    }
    out.truncate(MAX_ITEMS + 6);
    out
}

fn run_item(app: &mut TanaApp, item: Item) {
    app.palette = None;
    match item {
        Item::Command { id, .. } => {
            if let Err(e) = app.exec(&id, json!({})) {
                app.session.status = e;
            }
        }
        Item::Node { id, .. } => {
            let _ = app.exec("nav.open", json!({"node": id.0}));
        }
    }
}

pub fn palette_key(app: &mut TanaApp, k: &str) -> bool {
    let Some(p) = app.palette.as_mut() else { return false };
    match k {
        "Escape" | "Mod+K" => app.palette = None,
        "Up" => p.selected = p.selected.saturating_sub(1),
        "Down" => p.selected += 1,
        "Enter" => {
            let items = palette_items(app, &app.palette.as_ref().map(|p| p.query.clone()).unwrap_or_default());
            let sel = app.palette.as_ref().map(|p| p.selected).unwrap_or(0);
            if let Some(item) = items.get(sel.min(items.len().saturating_sub(1))).cloned() {
                run_item(app, item);
            }
        }
        _ => return false,
    }
    true
}

pub fn palette(app: &mut TanaApp, ctx: &egui::Context) {
    let Some(mut p) = app.palette.take() else { return };
    let t = app.tokens();
    let screen = ctx.content_rect();
    let w = 560.0_f32.min(screen.width() - 32.0);
    let items = palette_items(app, &p.query);
    p.selected = p.selected.min(items.len().saturating_sub(1));
    let mut chosen = None;
    let mut close = false;
    let area = egui::Area::new(egui::Id::new("palette"))
        .order(Order::Foreground)
        .fixed_pos(egui::pos2(screen.center().x - w / 2.0, screen.top() + 90.0))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).fill(t.panel).corner_radius(10.0).inner_margin(10.0).show(ui, |ui| {
                ui.set_width(w);
                let r = ui.add(
                    egui::TextEdit::singleline(&mut p.query)
                        .id(egui::Id::new("palette-query"))
                        .font(FontId::proportional(17.0))
                        .hint_text("Search nodes or run a command")
                        .frame(egui::Frame::NONE)
                        .desired_width(w),
                );
                if p.focus_once {
                    r.request_focus();
                    p.focus_once = false;
                }
                if r.changed() {
                    p.selected = 0;
                }
                ui.add_space(6.0);
                ui.separator();
                if items.is_empty() {
                    ui.label(RichText::new("No matches").color(t.faint));
                }
                for (i, item) in items.iter().enumerate() {
                    let (label, right, is_node) = match item {
                        Item::Command { label, shortcut, .. } => (label.clone(), pretty_shortcut(shortcut, app.mac), false),
                        Item::Node { line, .. } => (line.clone(), "Open".to_string(), true),
                    };
                    let (rect, resp) = ui.allocate_exact_size(vec2(w, 28.0), Sense::click());
                    if i == p.selected || resp.hovered() {
                        ui.painter().rect_filled(rect, 5.0, t.hover);
                    }
                    let painter = ui.painter();
                    let lead = if is_node { "\u{2022}" } else { ">" };
                    painter.text(rect.left_center() + vec2(10.0, 0.0), Align2::LEFT_CENTER, lead, FontId::proportional(14.0), t.faint);
                    painter.text(rect.left_center() + vec2(28.0, 0.0), Align2::LEFT_CENTER, label, FontId::proportional(14.5), t.text);
                    painter.text(rect.right_center() - vec2(10.0, 0.0), Align2::RIGHT_CENTER, right, FontId::proportional(12.5), t.faint);
                    if resp.clicked() {
                        chosen = Some(item.clone());
                    }
                }
            });
        });
    let outside = ctx.input(|i| i.pointer.any_click() && i.pointer.interact_pos().is_some_and(|p| !area.response.rect.contains(p)));
    if outside && chosen.is_none() {
        close = true;
    }
    if !close {
        app.palette = Some(p);
    }
    if let Some(item) = chosen {
        run_item(app, item);
    }
}

/// `Mod+Shift+Up` as people read it on this platform.
pub fn pretty_shortcut(s: &str, mac: bool) -> String {
    if s.is_empty() {
        return String::new();
    }
    s.split(" / ")
        .map(|k| {
            let k = if mac {
                k.replace("Ctrl+", "Ctrl ").replace("Mod+", "Cmd ").replace("Shift+", "Shift ").replace("Alt+", "Opt ")
            } else {
                k.replace("Ctrl+Mod+", "Ctrl ").replace("Mod+", "Ctrl ").replace("Ctrl+", "Ctrl ").replace("Shift+", "Shift ").replace("Alt+", "Alt ")
            };
            k.replace("Backspace", "Bksp")
        })
        .collect::<Vec<_>>()
        .join(" / ")
}

// ---- pickers ------------------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Choice {
    Existing(NodeId, String),
    Create(String),
}

pub fn picker_items(app: &TanaApp) -> Vec<Choice> {
    let Some(p) = &app.picker else { return Vec::new() };
    let g = &app.session.graph;
    let q = p.query.trim().trim_start_matches('#').to_lowercase();
    let mut out: Vec<Choice> = match p.mode {
        PickMode::Tag | PickMode::SearchTag => g
            .tags()
            .into_iter()
            .filter(|t| *t != g.sys.day_tag && g.name(*t).to_lowercase().contains(&q))
            .map(|t| Choice::Existing(t, format!("#{}", g.name(t))))
            .collect(),
        PickMode::Field | PickMode::Column => g
            .iter()
            .filter(|n| g.field_def(n.id).is_some() && !g.in_trash(n.id) && n.name.to_lowercase().contains(&q))
            .map(|n| Choice::Existing(n.id, n.name.clone()))
            .collect(),
        PickMode::Choice => p
            .field
            .and_then(|f| g.field_def(f))
            .map(|d| d.options.iter().filter(|o| o.to_lowercase().contains(&q)).map(|o| Choice::Existing(NodeId(0), o.clone())).collect())
            .unwrap_or_default(),
        PickMode::Reference if q.is_empty() => Vec::new(),
        PickMode::Reference => {
            g.search(&q).into_iter().filter(|n| *n != p.node).take(10).map(|n| Choice::Existing(n, export::node_line(g, n))).collect()
        }
    };
    out.truncate(10);
    let exact = out.iter().any(|c| matches!(c, Choice::Existing(_, name) if name.trim_start_matches('#').eq_ignore_ascii_case(&q)));
    if !q.is_empty() && !exact && !matches!(p.mode, PickMode::Reference | PickMode::SearchTag) {
        out.push(Choice::Create(p.query.trim().trim_start_matches('#').to_string()));
    }
    out
}

fn apply_choice(app: &mut TanaApp, choice: Choice) {
    let Some(p) = app.picker.take() else { return };
    let node = p.node;
    let r = match (p.mode, choice) {
        (PickMode::Tag, Choice::Existing(t, _)) => app.exec("tag.add", json!({"node": node.0, "tag": t.0})),
        (PickMode::Tag, Choice::Create(name)) => app.exec("tag.add", json!({"node": node.0, "tag": name, "color": next_color(app)})),
        (PickMode::Field, Choice::Existing(f, _)) => {
            app.exec("field.add", json!({"node": node.0, "field": f.0})).inspect(|_| app.session.graph.set_expanded(node, true))
        }
        (PickMode::Field, Choice::Create(name)) => {
            app.exec("field.add", json!({"node": node.0, "field": name})).inspect(|_| app.session.graph.set_expanded(node, true))
        }
        (PickMode::Reference, Choice::Existing(target, _)) => app.exec("node.reference", json!({"target": target.0, "replace": node.0})),
        (PickMode::Reference, Choice::Create(_)) | (PickMode::SearchTag, Choice::Create(_)) => Ok(json!({})),
        (PickMode::SearchTag, Choice::Existing(t, _)) => {
            app.exec("search.addClause", json!({"node": node.0, "clause": format!("#\"{}\"", app.session.graph.name(t))}))
        }
        (PickMode::Column, Choice::Existing(f, _)) => app.exec("view.addColumn", json!({"node": node.0, "field": f.0})),
        (PickMode::Column, Choice::Create(name)) => app.exec("view.addColumn", json!({"node": node.0, "field": name})),
        (PickMode::Choice, Choice::Existing(_, v) | Choice::Create(v)) => match p.field {
            Some(f) => app.exec("field.set", json!({"node": node.0, "field": f.0, "value": v})),
            None => Ok(json!({})),
        },
    };
    if let Err(e) = r {
        app.session.status = e;
    }
}

/// New supertags cycle through the palette so they are told apart at a glance.
fn next_color(app: &TanaApp) -> &'static str {
    let n = app.session.graph.tags().len();
    tanacraft_core::TAG_COLORS.get(1 + n % 12).copied().unwrap_or("grey")
}

pub fn picker_key(app: &mut TanaApp, k: &str) -> bool {
    let Some(p) = app.picker.as_mut() else { return false };
    match k {
        "Escape" => {
            app.picker = None;
        }
        "Up" => p.selected = p.selected.saturating_sub(1),
        "Down" => p.selected += 1,
        "Enter" | "Tab" => {
            let items = picker_items(app);
            let sel = app.picker.as_ref().map(|p| p.selected).unwrap_or(0);
            match items.get(sel.min(items.len().saturating_sub(1))).cloned() {
                Some(c) => apply_choice(app, c),
                None => app.picker = None,
            }
        }
        _ => return false,
    }
    true
}

pub fn picker(app: &mut TanaApp, ctx: &egui::Context) {
    let Some(mut p) = app.picker.take() else { return };
    let t = app.tokens();
    app.picker = Some(p.clone());
    let items = picker_items(app);
    p.selected = p.selected.min(items.len().saturating_sub(1));
    let mut chosen = None;
    let title = match p.mode {
        PickMode::Tag => "Add supertag",
        PickMode::Field => "Add field",
        PickMode::Reference => "Reference a node",
        PickMode::SearchTag => "Find nodes with supertag",
        PickMode::Column => "Add a field column",
        PickMode::Choice => "Choose an option",
    };
    egui::Area::new(egui::Id::new("picker")).order(Order::Foreground).fixed_pos(p.anchor).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).fill(t.panel).corner_radius(8.0).inner_margin(8.0).show(ui, |ui| {
            ui.set_width(300.0);
            ui.label(RichText::new(title).size(12.0).color(t.faint));
            let r =
                ui.add(egui::TextEdit::singleline(&mut p.query).id(egui::Id::new("picker-query")).hint_text("Type to search").desired_width(300.0));
            if p.focus_once {
                r.request_focus();
                p.focus_once = false;
            }
            if r.changed() {
                p.selected = 0;
            }
            for (i, c) in items.iter().enumerate() {
                let (label, color) = match c {
                    Choice::Existing(id, name) => {
                        let color = app.session.graph.tag_def(*id).map(|d| theme::tag_color(&d.color, app.dark).0).unwrap_or(t.text);
                        (name.clone(), color)
                    }
                    Choice::Create(name) => (format!("Create \"{name}\""), t.dim),
                };
                let (rect, resp) = ui.allocate_exact_size(vec2(300.0, 26.0), Sense::click());
                if i == p.selected || resp.hovered() {
                    ui.painter().rect_filled(rect, 4.0, t.hover);
                }
                ui.painter().text(rect.left_center() + vec2(8.0, 0.0), Align2::LEFT_CENTER, label, FontId::proportional(14.0), color);
                if resp.clicked() {
                    chosen = Some(c.clone());
                }
            }
        });
    });
    app.picker = Some(p);
    if let Some(c) = chosen {
        apply_choice(app, c);
    }
}

// ---- shortcuts panel and description ------------------------------------------------------------

pub fn help_panel(app: &mut TanaApp, ui: &mut Ui) {
    let t = app.tokens();
    ui.horizontal(|ui| {
        ui.label(RichText::new("Keyboard shortcuts").size(15.0).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(egui::Label::new(RichText::new("x").color(t.dim)).sense(Sense::click())).clicked() {
                app.help = false;
            }
        });
    });
    ui.add_space(8.0);
    egui::ScrollArea::vertical().show(ui, |ui| {
        let mut rows: Vec<(String, String, String)> =
            crate::UI_COMMANDS.iter().filter(|c| !c.2.is_empty()).map(|(_, l, s)| ("General".to_string(), l.to_string(), s.to_string())).collect();
        rows.extend(
            app.session
                .registry
                .all()
                .iter()
                .filter(|c| !c.shortcut.is_empty())
                .map(|c| (c.location.to_string(), c.label.to_string(), c.shortcut.to_string())),
        );
        let mut last = String::new();
        for (loc, label, sc) in rows {
            if loc != last {
                ui.add_space(8.0);
                ui.label(RichText::new(&loc).size(13.0).color(t.faint));
                last = loc;
            }
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(vec2(110.0, 22.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.set_min_width(110.0);
                    ui.label(RichText::new(pretty_shortcut(&sc, app.mac)).size(13.0).color(t.dim));
                });
                ui.label(RichText::new(label).size(13.5));
            });
        }
    });
}

pub fn description(app: &mut TanaApp, ctx: &egui::Context) {
    let Some((node, mut text)) = app.desc_edit.take() else { return };
    let t = app.tokens();
    let mut done = false;
    let mut cancel = false;
    egui::Window::new("Description").collapsible(false).resizable(false).anchor(Align2::CENTER_TOP, vec2(0.0, 120.0)).show(ctx, |ui| {
        ui.label(RichText::new(app.session.graph.name(node)).color(t.dim));
        ui.add(egui::TextEdit::multiline(&mut text).desired_width(420.0).desired_rows(3));
        ui.horizontal(|ui| {
            done = ui.button("Save").clicked();
            cancel = ui.button("Cancel").clicked();
        });
    });
    if done {
        let _ = app.exec("node.setDescription", json!({"node": node.0, "text": text}));
    } else if !cancel {
        app.desc_edit = Some((node, text));
    }
}

// ---- date picker and quick add ------------------------------------------------------------------

pub fn date_picker(app: &mut TanaApp, ctx: &egui::Context) {
    let Some(mut dp) = app.date_pick.take() else { return };
    let t = app.tokens();
    let today = app.session.today;
    let mut pick: Option<String> = None;
    let mut close = false;
    let area = egui::Area::new(egui::Id::new("date-picker")).order(Order::Foreground).fixed_pos(dp.anchor).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).fill(t.panel).corner_radius(8.0).inner_margin(10.0).show(ui, |ui| {
            ui.set_width(252.0);
            let r = ui.add(egui::TextEdit::singleline(&mut dp.buf).hint_text("YYYY-MM-DD, today, tomorrow").desired_width(252.0));
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                pick = Some(dp.buf.clone());
            }
            ui.horizontal(|ui| {
                let (y, m) = dp.month;
                if ui.small_button("<").clicked() {
                    dp.month = if m == 1 { (y - 1, 12) } else { (y, m - 1) };
                }
                ui.label(RichText::new(format!("{} {y}", MONTH_NAMES.get(m as usize - 1).copied().unwrap_or(""))).strong());
                if ui.small_button(">").clicked() {
                    dp.month = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
                }
            });
            let Some(first) = tanacraft_core::Date::new(dp.month.0, dp.month.1, 1) else { return };
            let start = first.add_days(-(first.weekday() as i64));
            let cell = 36.0;
            ui.horizontal(|ui| {
                for d in ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"] {
                    let (r, _) = ui.allocate_exact_size(vec2(cell, 18.0), Sense::hover());
                    ui.painter().text(r.center(), Align2::CENTER_CENTER, d, FontId::proportional(11.5), t.faint);
                }
            });
            for w in 0..6 {
                ui.horizontal(|ui| {
                    for k in 0..7 {
                        let d = start.add_days(w * 7 + k);
                        let (r, resp) = ui.allocate_exact_size(vec2(cell, 26.0), Sense::click());
                        let selected = dp.buf == d.to_string();
                        if selected {
                            ui.painter().rect_filled(r.shrink(2.0), 5.0, t.accent);
                        } else if resp.hovered() {
                            ui.painter().rect_filled(r.shrink(2.0), 5.0, t.hover);
                        }
                        let color = if selected {
                            egui::Color32::WHITE
                        } else if d == today {
                            egui::Color32::from_rgb(226, 92, 64)
                        } else if d.month == dp.month.1 {
                            t.text
                        } else {
                            t.faint
                        };
                        ui.painter().text(r.center(), Align2::CENTER_CENTER, d.day.to_string(), FontId::proportional(13.0), color);
                        if resp.clicked() {
                            pick = Some(d.to_string());
                        }
                    }
                });
            }
            ui.horizontal(|ui| {
                if ui.button("Today").clicked() {
                    pick = Some(today.to_string());
                }
                if ui.button("Clear").clicked() {
                    pick = Some(String::new());
                }
            });
        });
    });
    let outside = ctx.input(|i| i.pointer.any_click() && i.pointer.interact_pos().is_some_and(|p| !area.response.rect.contains(p)));
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) || outside {
        close = true;
    }
    match pick {
        Some(v) => {
            if let Err(e) = app.exec("field.set", json!({"node": dp.node.0, "field": dp.field.0, "value": v})) {
                app.session.status = e;
            }
        }
        None if !close => app.date_pick = Some(dp),
        None => {}
    }
}

const MONTH_NAMES: [&str; 12] =
    ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

/// Quick add (Mod+E): type a node, Enter puts it on today's page without leaving this one.
pub fn quick_add(app: &mut TanaApp, ctx: &egui::Context) {
    let Some(mut text) = app.quick_add.take() else { return };
    let t = app.tokens();
    let screen = ctx.content_rect();
    let w = 520.0_f32.min(screen.width() - 32.0);
    let mut submit = false;
    let mut close = false;
    egui::Area::new(egui::Id::new("quick-add-dim")).order(Order::Middle).fixed_pos(screen.min).show(ctx, |ui| {
        let (r, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
        ui.painter().rect_filled(r, 0.0, egui::Color32::from_black_alpha(140));
        close |= resp.clicked();
    });
    egui::Area::new(egui::Id::new("quick-add"))
        .order(Order::Foreground)
        .fixed_pos(egui::pos2(screen.center().x - w / 2.0, screen.center().y - 90.0))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).fill(t.panel).corner_radius(10.0).inner_margin(egui::Margin::symmetric(22, 16)).show(ui, |ui| {
                ui.set_width(w);
                let (bar, _) = ui.allocate_exact_size(vec2(w, 3.0), Sense::hover());
                ui.painter().rect_filled(bar, 1.5, t.accent);
                ui.add_space(8.0);
                ui.label(RichText::new("Quick add").size(18.0).strong());
                ui.separator();
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(12.0, 22.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 3.0, t.dim);
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut text)
                            .id(egui::Id::new("quick-add-input"))
                            .frame(egui::Frame::NONE)
                            .hint_text("Type a node, #tag at the end")
                            .desired_width(w - 20.0),
                    );
                    resp.request_focus();
                    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        submit = true;
                    }
                });
                ui.separator();
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("Goes to today's page. Enter adds, Escape closes.").size(12.5).color(t.dim));
                });
            });
        });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        close = true;
    }
    if submit && !text.trim().is_empty() {
        // A trailing `#word` tags the node.
        let (name, tag) = match text.trim().rsplit_once(" #") {
            Some((n, t)) if !t.contains(' ') && !t.is_empty() => (n.to_string(), Some(t.to_string())),
            _ => (text.trim().to_string(), None),
        };
        let mut p = json!({"name": name});
        if let Some(tag) = tag {
            p["tag"] = json!(tag);
        }
        match app.exec("node.quickAdd", p) {
            Ok(_) => app.session.status = "Added to today".into(),
            Err(e) => app.session.status = e,
        }
    } else if !close && !submit {
        app.quick_add = Some(text);
    }
}
