//! Overlays: the command line (commands and nodes), the `#`/`>`/`@` pickers, the shortcuts panel and
//! the shortcut sheet.

use egui::{Align2, FontId, Order, RichText, Sense, Ui, vec2};
use serde_json::json;
use tanacraft_core::{NodeId, export};

use crate::{Act, PickMode, TanaApp, theme};

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
    // Context first: with a bullet focused or selected, its node commands lead (spec 05 §3).
    let mut cmds = app.session.registry.search(q);
    if app.session.focus.is_some() || app.session.sel.is_some() {
        cmds.sort_by_key(|c| !matches!(c.location, "Node" | "Supertag" | "Field" | "Format"));
    }
    for c in cmds {
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
        PickMode::Extend => g
            .tags()
            .into_iter()
            .filter(|t| *t != p.node && *t != g.sys.day_tag && g.name(*t).to_lowercase().contains(&q))
            .map(|t| Choice::Existing(t, format!("#{}", g.name(t))))
            .collect(),
        PickMode::Tag | PickMode::SearchTag => g
            .tags()
            .into_iter()
            .filter(|t| *t != g.sys.day_tag && g.name(*t).to_lowercase().contains(&q))
            .map(|t| Choice::Existing(t, format!("#{}", g.name(t))))
            .collect(),
        PickMode::Field | PickMode::Column | PickMode::TemplateField => g
            .iter()
            .filter(|n| g.field_def(n.id).is_some() && !g.in_trash(n.id) && n.name.to_lowercase().contains(&q))
            .map(|n| Choice::Existing(n.id, n.name.clone()))
            .collect(),
        PickMode::Choice => {
            // The other options (the current value is left out, spec 03 §8).
            let current: Vec<String> = p
                .field
                .and_then(|f| Some(g.get(p.node)?.field_values(f).into_iter().map(|v| export::display_value(g, v)).collect()))
                .unwrap_or_default();
            p.field
                .and_then(|f| g.field_def(f))
                .map(|d| {
                    d.options
                        .iter()
                        .filter(|o| !current.contains(o) && o.to_lowercase().contains(&q))
                        .map(|o| Choice::Existing(NodeId(0), o.clone()))
                        .collect()
                })
                .unwrap_or_default()
        }
        PickMode::Slash => ["Search node", "Field", "Reference", "Heading", "Checkbox", "Checklist", "Command line"]
            .into_iter()
            .filter(|l| l.to_lowercase().contains(&q))
            .map(|l| Choice::Existing(NodeId(0), l.to_string()))
            .collect(),
        PickMode::SearchCreate => g
            .tags()
            .into_iter()
            .filter(|t| *t != g.sys.day_tag && g.name(*t).to_lowercase().contains(&q))
            .map(|t| Choice::Existing(t, format!("#{}", g.name(t))))
            .collect(),
        PickMode::InlineRef | PickMode::MoveTo | PickMode::MoveLeaveRef if q.is_empty() => app
            .session
            .recents
            .iter()
            .copied()
            .filter(|r| *r != p.node && g.get(*r).is_some())
            .take(8)
            .map(|n| Choice::Existing(n, export::node_line(g, n)))
            .collect(),
        PickMode::InlineRef => g.fuzzy(&q, 10).into_iter().filter(|n| *n != p.node).map(|n| Choice::Existing(n, export::node_line(g, n))).collect(),
        PickMode::MoveTo | PickMode::MoveLeaveRef => {
            let mut v: Vec<Choice> = [g.sys.library, g.sys.journal]
                .into_iter()
                .filter(|n| g.name(*n).to_lowercase().contains(&q))
                .map(|n| Choice::Existing(n, g.name(n).to_string()))
                .collect();
            v.extend(
                g.fuzzy(&q, 10).into_iter().filter(|n| !g.is_ancestor_or_self(p.node, *n)).map(|n| Choice::Existing(n, export::node_line(g, n))),
            );
            v
        }
        PickMode::Reference if q.is_empty() => Vec::new(),
        PickMode::Reference => {
            g.search(&q).into_iter().filter(|n| *n != p.node).take(10).map(|n| Choice::Existing(n, export::node_line(g, n))).collect()
        }
    };
    out.truncate(10);
    let exact = out.iter().any(|c| matches!(c, Choice::Existing(_, name) if name.trim_start_matches('#').eq_ignore_ascii_case(&q)));
    if !q.is_empty()
        && !exact
        && !matches!(
            p.mode,
            PickMode::SearchTag | PickMode::Slash | PickMode::SearchCreate | PickMode::Extend | PickMode::MoveTo | PickMode::MoveLeaveRef
        )
    {
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
        // "Create": a new node in the Library, referenced here (spec 02 §11).
        (PickMode::Reference, Choice::Create(name)) => app
            .exec("node.add", json!({"parent": app.session.graph.sys.library.0, "name": name}))
            .and_then(|v| app.exec("node.reference", json!({"target": v["id"], "replace": node.0}))),
        (PickMode::SearchTag, Choice::Create(_)) => Ok(json!({})),
        (PickMode::SearchTag, Choice::Existing(t, _)) => {
            app.exec("search.addClause", json!({"node": node.0, "clause": format!("#\"{}\"", app.session.graph.name(t))}))
        }
        (PickMode::Column, Choice::Existing(f, _)) => app.exec("view.addColumn", json!({"node": node.0, "field": f.0})),
        (PickMode::Column, Choice::Create(name)) => app.exec("view.addColumn", json!({"node": node.0, "field": name})),
        (PickMode::Slash, Choice::Existing(_, label)) => {
            let at = p.anchor;
            match label.as_str() {
                "Search node" => {
                    app.open_picker(PickMode::SearchCreate, node, at);
                    Ok(json!({}))
                }
                "Field" => {
                    let parent = app.session.graph.parent(node).unwrap_or(node);
                    let r = app.exec("node.backspace", json!({"node": node.0}));
                    app.open_picker(PickMode::Field, parent, at);
                    r
                }
                "Reference" => {
                    app.open_picker(PickMode::Reference, node, at);
                    Ok(json!({}))
                }
                "Heading" => app.exec("node.toggleHeading", json!({"node": node.0})),
                "Checkbox" => app.exec("node.toggleDone", json!({"node": node.0})),
                "Checklist" => app.exec("node.insertChecklist", json!({"node": node.0})),
                _ => app.exec("ui.commandLine", json!({})),
            }
        }
        (PickMode::SearchCreate, Choice::Existing(t, _)) => {
            let name = app.session.graph.name(t).to_string();
            let mut p = json!({"query": format!("#\"{name}\""), "name": format!("List of {name}")});
            // On a page itself (empty-page action) the search goes inside; on an empty line it replaces it.
            if node == app.session.zoom {
                p["parent"] = json!(node.0);
            } else {
                p["replace"] = json!(node.0);
            }
            app.exec("search.create", p)
        }
        (PickMode::Slash | PickMode::SearchCreate | PickMode::Extend, Choice::Create(_)) => Ok(json!({})),
        (PickMode::InlineRef, choice) => {
            // Insert `[[id:N]]` where the `@` was; "Create" makes a new node in the Library first.
            let target = match choice {
                Choice::Existing(n, _) => Ok(n),
                Choice::Create(name) => app
                    .exec("node.add", json!({"parent": app.session.graph.sys.library.0, "name": name}))
                    .map(|v| NodeId(v["id"].as_u64().unwrap_or(0))),
            };
            match target {
                Ok(n) if app.session.graph.get(n).is_some() => {
                    let name = app.session.graph.name(node).to_string();
                    let at = p.caret.unwrap_or(name.chars().count()).min(name.chars().count());
                    let byte = name.char_indices().nth(at).map(|(b, _)| b).unwrap_or(name.len());
                    let token = tanacraft_core::format::ref_token(n.0);
                    let new = format!("{}{}{}", &name[..byte], token, &name[byte..]);
                    let caret = at + token.chars().count();
                    app.exec("node.setName", json!({"node": node.0, "name": new, "caret": caret}))
                }
                Ok(_) => Ok(json!({})),
                Err(e) => Err(e),
            }
        }
        (PickMode::MoveTo, Choice::Existing(dest, _)) => app.exec("node.move", json!({"node": node.0, "parent": dest.0})),
        (PickMode::MoveLeaveRef, Choice::Existing(dest, _)) => {
            app.exec("node.move", json!({"node": node.0, "parent": dest.0, "leaveReference": true}))
        }
        (PickMode::MoveTo | PickMode::MoveLeaveRef, Choice::Create(_)) => Ok(json!({})),
        (PickMode::Extend, Choice::Existing(base, _)) => app.exec("tag.extend", json!({"tag": node.0, "base": base.0})),
        (PickMode::TemplateField, Choice::Existing(f, _)) => app.exec("tag.addField", json!({"tag": node.0, "field": f.0})),
        (PickMode::TemplateField, Choice::Create(name)) => app.exec("tag.addField", json!({"tag": node.0, "field": name})),
        (PickMode::Choice, Choice::Existing(_, v) | Choice::Create(v)) => match p.field {
            Some(f) => app.exec("field.set", json!({"node": node.0, "field": f.0, "index": p.index, "value": v})),
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
        // Cmd+Enter creates from the typed text instead of picking a match (spec 02 §2, §11).
        "Mod+Enter" => {
            let q = p.query.trim().trim_start_matches('#').to_string();
            if q.is_empty() {
                return true;
            }
            apply_choice(app, Choice::Create(q));
        }
        "Up" => p.selected = p.selected.saturating_sub(1),
        "Down" => p.selected += 1,
        "Enter" | "Tab" => {
            let items = picker_items(app);
            let (sel, mode, node) = app.picker.as_ref().map(|p| (p.selected, p.mode, p.node)).unwrap_or((0, PickMode::Tag, NodeId(0)));
            match items.get(sel.min(items.len().saturating_sub(1))).cloned() {
                Some(c) => {
                    let picked = match &c {
                        Choice::Existing(f, _) => Some(*f),
                        Choice::Create(_) => None,
                    };
                    apply_choice(app, c);
                    // After `>`: Tab goes on to the value, Enter starts a new node below (spec 02 §3).
                    if mode == PickMode::Field {
                        let f = picked.or_else(|| app.session.graph.get(node).and_then(|n| n.fields.last()).map(|e| e.field));
                        match (k, f) {
                            ("Tab", Some(f)) => app.acts.push(Act::EditField(node, f, 0, String::new())),
                            ("Enter", _) => match app.exec("node.add", json!({"parent": node.0, "index": 0, "name": ""})) {
                                Ok(v) => drop(app.exec("focus.set", json!({"node": v["id"]}))),
                                Err(e) => app.session.status = e,
                            },
                            _ => {}
                        }
                    }
                }
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
        PickMode::Slash => "Insert",
        PickMode::SearchCreate => "Create search node: find supertag",
        PickMode::Extend => "Extend supertag",
        PickMode::TemplateField => "Add a field to the template",
        PickMode::InlineRef => "Reference a node inline",
        PickMode::MoveTo => "Move to",
        PickMode::MoveLeaveRef => "Move and leave reference to",
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
                    Choice::Create(name) if p.mode == PickMode::Tag => (format!("Create and apply \"#{name}\""), t.dim),
                    Choice::Create(name) => (format!("Create \"{name}\""), t.dim),
                };
                let create = matches!(c, Choice::Create(_));
                let (rect, resp) = ui.allocate_exact_size(vec2(300.0, 26.0), Sense::click());
                if i == p.selected || resp.hovered() {
                    ui.painter().rect_filled(rect, 4.0, t.hover);
                }
                ui.painter().text(rect.left_center() + vec2(8.0, 0.0), Align2::LEFT_CENTER, label, FontId::proportional(14.0), color);
                if create {
                    let hint = if app.mac { "Cmd+Enter" } else { "Ctrl+Enter" };
                    ui.painter().text(rect.right_center() - vec2(8.0, 0.0), Align2::RIGHT_CENTER, hint, FontId::proportional(11.5), t.faint);
                }
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
                        // Go to date marks days that already have notes (spec 06 §4).
                        if dp.field.is_none() && app.session.graph.find_day(d).is_some_and(|n| !app.session.graph.children(n).is_empty()) {
                            ui.painter().circle_filled(r.center_bottom() - vec2(0.0, 4.0), 2.0, t.accent);
                        }
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
            let r = match dp.field {
                Some(f) => app.exec("field.set", json!({"node": dp.node.0, "field": f.0, "index": dp.index, "value": v})),
                None if !v.trim().is_empty() => app.exec("nav.day", json!({"date": v})),
                None => Ok(json!({})),
            };
            if let Err(e) = r {
                app.session.status = e;
            }
        }
        None if !close => app.date_pick = Some(dp),
        None => {}
    }
}

const MONTH_NAMES: [&str; 12] =
    ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

/// Quick add (Mod+E): a small outline; Enter starts another line, Mod+E saves everything to the
/// end of today's page and closes, Escape cancels (spec 05 §5).
pub fn quick_add(app: &mut TanaApp, ctx: &egui::Context) {
    let Some(mut lines) = app.quick_add.take() else { return };
    let t = app.tokens();
    let screen = ctx.content_rect();
    let w = 560.0_f32.min(screen.width() - 32.0);
    let save = ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::E));
    let enter = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
    let mut close = ctx.input(|i| i.key_pressed(egui::Key::Escape));
    if enter {
        lines.push(String::new());
    }
    egui::Area::new(egui::Id::new("quick-add-dim")).order(Order::Middle).fixed_pos(screen.min).show(ctx, |ui| {
        let (r, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
        ui.painter().rect_filled(r, 0.0, egui::Color32::from_black_alpha(140));
        close |= resp.clicked();
    });
    egui::Area::new(egui::Id::new("quick-add"))
        .order(Order::Foreground)
        .fixed_pos(egui::pos2(screen.center().x - w / 2.0, screen.center().y - 120.0))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style())
                .fill(t.panel)
                .corner_radius(10.0)
                .inner_margin(egui::Margin { left: 22, right: 22, top: 0, bottom: 16 })
                .show(ui, |ui| {
                    ui.set_width(w);
                    let (bar, _) = ui.allocate_exact_size(vec2(w, 3.0), Sense::hover());
                    ui.painter().rect_filled(bar.expand2(vec2(22.0, 0.0)), 1.5, egui::Color32::from_rgb(214, 178, 46));
                    ui.add_space(12.0);
                    ui.label(RichText::new("Quick add").size(18.0).strong());
                    ui.separator();
                    let last = lines.len().saturating_sub(1);
                    for (i, line) in lines.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(12.0, 22.0), Sense::hover());
                            ui.painter().circle_filled(r.center(), 3.0, t.dim);
                            let resp = ui.add(
                                egui::TextEdit::singleline(line)
                                    .id(egui::Id::new(("quick-add-input", i)))
                                    .frame(egui::Frame::NONE)
                                    .hint_text("Type a node, #tag at the end")
                                    .desired_width(w - 20.0),
                            );
                            let nothing_focused = ui.ctx().memory(|m| m.focused().is_none());
                            if i == last && (enter || nothing_focused) {
                                resp.request_focus();
                            }
                        });
                    }
                    ui.separator();
                    ui.vertical_centered(|ui| {
                        let key = if app.mac { "Cmd+E" } else { "Ctrl+E" };
                        ui.label(RichText::new(format!("Goes to the end of today's page. {key} saves, Escape cancels.")).size(12.5).color(t.dim));
                    });
                });
        });
    if save {
        let mut added = 0;
        for text in lines.iter().map(|l| l.trim()).filter(|l| !l.is_empty()) {
            // A trailing `#word` tags the node.
            let (name, tag) = match text.rsplit_once(" #") {
                Some((n, tg)) if !tg.contains(' ') && !tg.is_empty() => (n.to_string(), Some(tg.to_string())),
                _ => (text.to_string(), None),
            };
            let mut p = json!({"name": name});
            if let Some(tag) = tag {
                p["tag"] = json!(tag);
            }
            match app.exec("node.quickAdd", p) {
                Ok(_) => added += 1,
                Err(e) => app.session.status = e,
            }
        }
        if added > 0 {
            app.session.status = format!("Added {added} to today");
        }
    } else if !close {
        app.quick_add = Some(lines);
    }
}

/// "Search and open" (Mod+S): recently viewed, or fuzzy matches with their location.
pub fn search_open_items(app: &TanaApp, query: &str) -> Vec<NodeId> {
    let g = &app.session.graph;
    if query.trim().is_empty() { app.session.recents.iter().copied().filter(|r| g.get(*r).is_some()).take(10).collect() } else { g.fuzzy(query, 12) }
}

/// Opens a "Search and open" pick here, or in a new tab (Mod+T).
fn open_choice(app: &mut TanaApp, id: NodeId) {
    if std::mem::take(&mut app.search_new_tab) {
        app.new_tab(id);
    } else {
        let _ = app.exec("nav.open", json!({"node": id.0}));
    }
}

pub fn search_open_key(app: &mut TanaApp, k: &str) -> bool {
    let Some(p) = app.search_open.as_mut() else { return false };
    match k {
        "Escape" => {
            app.search_open = None;
            app.search_new_tab = false;
        }
        "Up" => p.selected = p.selected.saturating_sub(1),
        "Down" => p.selected += 1,
        "Enter" => {
            let q = p.query.clone();
            let sel = p.selected;
            let items = search_open_items(app, &q);
            app.search_open = None;
            if let Some(id) = items.get(sel.min(items.len().saturating_sub(1))) {
                open_choice(app, *id);
            }
        }
        _ => return false,
    }
    true
}

pub fn search_open(app: &mut TanaApp, ctx: &egui::Context) {
    let Some(mut p) = app.search_open.take() else { return };
    let t = app.tokens();
    let screen = ctx.content_rect();
    let w = 520.0_f32.min(screen.width() - 32.0);
    let items = search_open_items(app, &p.query);
    p.selected = p.selected.min(items.len().saturating_sub(1));
    let mut chosen = None;
    let mut close = false;
    egui::Area::new(egui::Id::new("search-dim")).order(Order::Middle).fixed_pos(screen.min).show(ctx, |ui| {
        let (r, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
        ui.painter().rect_filled(r, 0.0, egui::Color32::from_black_alpha(120));
        close |= resp.clicked();
    });
    egui::Area::new(egui::Id::new("search-open"))
        .order(Order::Foreground)
        .fixed_pos(egui::pos2(screen.center().x - w / 2.0, screen.top() + 110.0))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).fill(t.panel).corner_radius(10.0).inner_margin(12.0).show(ui, |ui| {
                ui.set_width(w);
                ui.label(RichText::new(if app.search_new_tab { "Search and open in a new tab" } else { "Search and open" }).size(15.0).strong());
                let r =
                    ui.add(egui::TextEdit::singleline(&mut p.query).id(egui::Id::new("search-open-query")).hint_text("Search...").desired_width(w));
                if p.focus_once {
                    r.request_focus();
                    p.focus_once = false;
                }
                if r.changed() {
                    p.selected = 0;
                }
                ui.add_space(6.0);
                ui.label(RichText::new(if p.query.trim().is_empty() { "RECENTLY VIEWED" } else { "FUZZY SEARCH" }).size(11.5).color(t.faint));
                let g = &app.session.graph;
                for (i, id) in items.iter().enumerate() {
                    let crumbs: Vec<String> = g.ancestors(*id).iter().skip(1).map(|a| crate::outline::page_title(&app.session, *a)).collect();
                    let (rect, resp) = ui.allocate_exact_size(vec2(w, 40.0), Sense::click());
                    if i == p.selected || resp.hovered() {
                        ui.painter().rect_filled(rect, 5.0, if i == p.selected { t.accent.gamma_multiply(0.25) } else { t.hover });
                    }
                    let painter = ui.painter();
                    painter.text(rect.left_top() + vec2(10.0, 4.0), Align2::LEFT_TOP, crumbs.join(" / "), FontId::proportional(11.5), t.faint);
                    painter.text(rect.left_top() + vec2(10.0, 19.0), Align2::LEFT_TOP, export::node_line(g, *id), FontId::proportional(14.5), t.text);
                    if resp.clicked() {
                        chosen = Some(*id);
                    }
                }
                if items.is_empty() {
                    ui.label(RichText::new(if p.query.trim().is_empty() { "Nothing viewed yet" } else { "No matches" }).color(t.faint));
                }
            });
        });
    if let Some(id) = chosen {
        open_choice(app, id);
    } else if !close {
        app.search_open = Some(p);
    } else {
        app.search_new_tab = false;
    }
}
