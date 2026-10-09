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
        Some(Kind::Tag(_)) => g.name(id).to_string(),
        _ if id == g.sys.schema => "Supertags".to_string(),
        _ => g.name(id).to_string(),
    }
}

pub fn panel(app: &mut TanaApp, ui: &mut Ui) {
    let t = app.tokens();
    if app.recents_page {
        recents_page(app, ui, &t);
        return;
    }
    if app.settings_page {
        settings_page(app, ui, &t);
        return;
    }
    if app.help_page {
        help_page(app, ui, &t);
        return;
    }
    let width = ui.available_width();
    let side_w = 360.0;
    let wide = width > LEFT + MAX_W.min(600.0) + side_w + 80.0 && !app.session.graph.related_for(app.session.zoom).is_empty();
    // Tables, boards and calendars use the full panel width (spec 04 §3).
    let zoom = app.session.zoom;
    let g = &app.session.graph;
    let shown = if g.tag_def(zoom).is_some() { app.tag_tab.get(&zoom).copied().unwrap_or(zoom) } else { zoom };
    let full = !app.session.config_view
        && matches!(g.view_def(shown).kind, tanacraft_core::ViewKind::Table | tanacraft_core::ViewKind::Cards | tanacraft_core::ViewKind::Calendar)
        && (g.query(shown).is_some() || g.get(shown).is_some_and(|n| n.view.is_some()) || g.tag_def(zoom).is_some());
    let max_w = match app.content_width {
        _ if full => f32::INFINITY,
        1 => 1080.0,
        2 => f32::INFINITY,
        _ => MAX_W,
    };
    let content_w = if wide { (width - LEFT - side_w - 70.0).clamp(200.0, max_w) } else { (width - LEFT - 40.0).clamp(200.0, max_w) };
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
            if Some(zoom) == app.session.graph.sys.inbox {
                ui.label(
                    RichText::new("Captures land here: `tanacraft-cli capture`, the MCP `node.capture` tool and the Capture to inbox command. Move them where they belong.")
                        .size(13.0)
                        .color(t.dim),
                );
                ui.add_space(8.0);
            }
            if zoom == app.session.graph.sys.schema {
                tag_overview(app, ui, &t);
                return;
            }
            let empty_page = app.session.graph.get(zoom).is_some_and(|n| {
                n.kind == Kind::Plain && n.name.is_empty() && n.children.is_empty() && n.tags.is_empty() && !app.session.graph.is_system(zoom)
            });
            if empty_page {
                empty_page_suggestions(app, ui, &t, zoom);
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
            crate::views::track_results(app, ui, zoom);
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
                // A week page ends with its Schedule: the week's time grid (spec 06 §6).
                if let Some(Kind::Week { year, week }) = app.session.graph.get(zoom).map(|n| n.kind.clone()) {
                    crate::views::schedule(app, ui, &t, zoom, year, week);
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

/// A new, empty page (spec 06 §8): suggested supertags and "Create search node".
fn empty_page_suggestions(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, page: NodeId) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Suggested tags").size(12.5).color(t.faint));
        let g = &app.session.graph;
        for tag in g.tags().into_iter().filter(|x| *x != g.sys.day_tag && g.name(*x) != "week").take(6) {
            if chip(ui, g, tag, app.dark).clicked() {
                app.acts.push(Act::Run("tag.add".into(), json!({"node": page.0, "tag": tag.0})));
            }
        }
    });
    ui.horizontal(|ui| {
        if ui.add(egui::Button::new(RichText::new("Create search node").size(13.0)).fill(t.field_bg)).clicked() {
            app.acts.push(Act::Pick(PickMode::SearchCreate, page, ui.min_rect().left_bottom()));
        }
    });
    ui.add_space(8.0);
}

/// The Recents page (spec 07 §3): recently opened nodes beside the selected one.
pub(crate) fn recents_page(app: &mut TanaApp, ui: &mut Ui, t: &Tokens) {
    ui.add_space(12.0);
    let recents: Vec<NodeId> = app.session.recents.iter().copied().filter(|r| app.session.graph.get(*r).is_some()).collect();
    let selected = app.recents_selected.filter(|r| recents.contains(r)).or_else(|| recents.first().copied());
    ui.horizontal_top(|ui| {
        ui.add_space(20.0);
        ui.vertical(|ui| {
            ui.set_width(300.0);
            ui.label(RichText::new("Recents").size(22.0).strong());
            ui.label(RichText::new("Today").size(12.5).color(t.faint));
            for r in &recents {
                let on = Some(*r) == selected;
                let name = page_title(&app.session, *r);
                let b = egui::Button::new(RichText::new(if name.is_empty() { "Untitled".to_string() } else { name }).size(14.0).color(if on {
                    t.text
                } else {
                    t.dim
                }))
                .fill(if on { t.hover } else { Color32::TRANSPARENT })
                .corner_radius(5.0);
                if ui.add(b).clicked() {
                    app.recents_selected = Some(*r);
                }
            }
        });
        ui.separator();
        ui.vertical(|ui| {
            if let Some(s) = selected {
                let r = ui.add(egui::Label::new(RichText::new(page_title(&app.session, s)).size(24.0).strong()).sense(Sense::click()));
                if r.on_hover_text("Open").clicked() {
                    app.acts.push(Act::Run("nav.open".into(), json!({"node": s.0})));
                }
                fields(app, ui, t, s, 0.0);
                let kids: Vec<NodeId> = app.session.graph.children(s).to_vec();
                for k in kids.iter().take(200) {
                    ui.horizontal_wrapped(|ui| crate::views::item_line(app, ui, t, *k, true));
                }
            } else {
                ui.label(RichText::new("Nothing opened yet").color(t.faint));
            }
        });
    });
}

/// The Settings page (spec 07 §8): the preferences this app has.
/// Help: what the app is, then every command (engine and UI) by area, with shortcut and params.
pub(crate) fn help_page(app: &mut TanaApp, ui: &mut Ui, t: &Tokens) {
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        ui.add_space(LEFT);
        ui.vertical(|ui| {
            ui.set_width(MAX_W.min(ui.available_width() - 40.0));
            ui.label(RichText::new("Help").size(30.0).strong());
            ui.label(
                RichText::new(format!(
                    "Every action in {} is a command. The same commands run from the keyboard, the command line (Cmd+K), the CLI and the MCP server.",
                    tanacraft_core::PRODUCT_NAME
                ))
                .color(t.dim),
            );
            ui.add_space(10.0);
            let reg = app.session.registry.clone();
            let mut areas: Vec<&str> = reg.all().iter().map(|c| c.location).collect();
            areas.dedup();
            areas.sort_unstable();
            areas.dedup();
            let row = |ui: &mut Ui, label: &str, id: &str, key: &str, params: &str| {
                ui.horizontal(|ui| {
                    let cell = |ui: &mut Ui, w: f32, text: RichText| {
                        ui.allocate_ui_with_layout(vec2(w, 18.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.set_min_width(w);
                            ui.label(text);
                        });
                    };
                    cell(ui, 260.0, RichText::new(label).size(13.5));
                    cell(ui, 150.0, RichText::new(crate::overlays::pretty_shortcut(key, app.mac)).size(12.5).color(t.dim));
                    ui.label(RichText::new(id).size(12.0).monospace().color(t.faint));
                })
                .response
                .on_hover_text(params);
            };
            for area in areas {
                ui.add_space(8.0);
                ui.label(RichText::new(area).strong().size(15.0));
                for c in reg.all().iter().filter(|c| c.location == area) {
                    row(ui, c.label, c.id, c.shortcut, c.params);
                }
            }
            ui.add_space(8.0);
            ui.label(RichText::new("Window").strong().size(15.0));
            for (id, label, key) in crate::UI_COMMANDS {
                row(ui, label, id, key, "");
            }
        });
    });
}

pub(crate) fn settings_page(app: &mut TanaApp, ui: &mut Ui, t: &Tokens) {
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        ui.add_space(LEFT);
        ui.vertical(|ui| {
            ui.set_width(MAX_W.min(ui.available_width() - 40.0));
            ui.label(RichText::new("Settings").size(30.0).strong());
            ui.add_space(10.0);
            let pref = |ui: &mut Ui, label: &str, help: &str, on: &mut bool| {
                egui::Frame::new().fill(t.field_bg).corner_radius(8.0).inner_margin(egui::Margin::symmetric(14, 10)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(RichText::new(label).strong());
                            ui.label(RichText::new(help).size(12.5).color(t.dim));
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| ui.checkbox(on, ""));
                    });
                });
                ui.add_space(6.0);
            };
            let mut dark = app.dark;
            pref(ui, "Dark mode", "Use the dark theme.", &mut dark);
            app.dark = dark;
            let mut full = app.sidebar == 0;
            pref(ui, "Show sidebar", "Show the full sidebar (Cmd+\\ cycles full, mini, hidden).", &mut full);
            if full != (app.sidebar == 0) {
                app.sidebar = if full { 0 } else { 2 };
            }
            let mut counter = app.ref_counter;
            pref(ui, "Reference counter", "Show how many places reference a node, next to it.", &mut counter);
            app.ref_counter = counter;
            let mut autosave = app.autosave;
            pref(ui, "Save automatically", "Write the graph file a second after each change.", &mut autosave);
            app.autosave = autosave;
            let path =
                app.session.path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "Not saved to a file (sample or scratch graph)".into());
            ui.label(RichText::new(format!("Graph file: {path}")).size(12.5).color(t.faint));
        });
    });
}

/// The Supertags overview (spec 03 §1): a filter box and a grid of supertag tiles, then fields.
fn tag_overview(app: &mut TanaApp, ui: &mut Ui, t: &Tokens) {
    ui.add(egui::TextEdit::singleline(&mut app.tag_filter).hint_text("Filter tags").desired_width(ui.available_width()));
    ui.add_space(12.0);
    let q = app.tag_filter.trim().to_lowercase();
    let g = &app.session.graph;
    let tags: Vec<NodeId> = g.tags().into_iter().filter(|x| *x != g.sys.day_tag && g.name(*x).to_lowercase().contains(&q)).collect();
    let pinned: Vec<NodeId> = tags.iter().copied().filter(|x| g.get(*x).is_some_and(|n| n.pinned)).collect();
    let w = ui.available_width();
    let tile_w = ((w - 24.0) / 3.0).floor().max(150.0);
    let mut open = None;
    // Pinned supertags: large cards in the tag colour (spec 03 §1).
    if !pinned.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(12.0, 12.0);
            for tag in &pinned {
                let color = g.tag_def(*tag).map(|d| d.color.clone()).unwrap_or_default();
                let (strong, _) = theme::tag_color(&color, app.dark);
                let (r, resp) = ui.allocate_exact_size(vec2(tile_w, 110.0), Sense::click());
                let fill = if app.dark { strong.gamma_multiply(0.55) } else { strong.gamma_multiply(0.85) };
                ui.painter().rect_filled(r, 10.0, if resp.hovered() { strong.gamma_multiply(0.7) } else { fill });
                ui.painter().text(r.left_top() + vec2(14.0, 14.0), Align2::LEFT_TOP, g.name(*tag), FontId::proportional(17.0), Color32::WHITE);
                let badge = Rect::from_min_size(r.left_bottom() + vec2(14.0, -36.0), vec2(22.0, 22.0));
                ui.painter().rect_filled(badge, 5.0, Color32::from_black_alpha(70));
                crate::icons::hash(ui.painter(), badge.shrink(5.0), Color32::WHITE);
                if resp.clicked() {
                    open = Some(*tag);
                }
            }
        });
        ui.add_space(18.0);
    }
    // Then every supertag of the workspace as a compact tile, under the workspace's name.
    let ws = g.name(g.sys.root);
    ui.label(RichText::new(if ws.is_empty() { "Workspace" } else { ws }).size(15.0).strong().color(t.text));
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(12.0, 10.0);
        for tag in &tags {
            let color = g.tag_def(*tag).map(|d| d.color.clone()).unwrap_or_default();
            let (strong, _) = theme::tag_color(&color, app.dark);
            let (r, resp) = ui.allocate_exact_size(vec2(tile_w, 40.0), Sense::click());
            ui.painter().rect_filled(r, 6.0, if resp.hovered() { t.hover } else { t.field_bg });
            crate::icons::hash(ui.painter(), Rect::from_center_size(r.left_center() + vec2(18.0, 0.0), vec2(14.0, 14.0)), strong);
            ui.painter().text(r.left_center() + vec2(34.0, 0.0), Align2::LEFT_CENTER, g.name(*tag), FontId::proportional(14.5), t.text);
            if resp.clicked() {
                open = Some(*tag);
            }
        }
    });
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
        // Workspace avatar: a small circle; it opens the Library (spec 07 §7).
        let (r, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
        ui.painter().circle_filled(r.center(), 8.0, t.faint);
        ui.painter().line_segment([r.center() - vec2(4.0, 0.0), r.center() + vec2(4.0, 0.0)], Stroke::new(1.5, t.panel));
        if resp.on_hover_text(PRODUCT_NAME_WORKSPACE).clicked() {
            app.acts.push(Act::Run("nav.home".into(), json!({})));
        }
        // Long chains: first, "...", then the last two (spec 00 §4).
        let shown: Vec<Option<NodeId>> = if crumbs.len() > 4 {
            let mut v = vec![crumbs.first().copied(), None];
            v.extend(crumbs.iter().skip(crumbs.len() - 2).copied().map(Some));
            v
        } else {
            crumbs.iter().copied().map(Some).collect()
        };
        for c in shown {
            ui.label(RichText::new("/").size(13.0).color(t.faint));
            let Some(c) = c else {
                let hidden: Vec<String> = crumbs.iter().skip(1).take(crumbs.len().saturating_sub(3)).map(|h| page_title(&app.session, *h)).collect();
                ui.label(RichText::new("...").size(13.0).color(t.dim)).on_hover_text(hidden.join(" / "));
                continue;
            };
            // The schema reads "Schema" on the way to a supertag (its own page is "Supertags").
            let name = if c == app.session.graph.sys.schema { "Schema".to_string() } else { page_title(&app.session, c) };
            let r = ui.add(egui::Label::new(RichText::new(name).size(13.0).color(t.dim)).sense(Sense::click()));
            if r.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if r.clicked() {
                app.acts.push(Act::Run("nav.open".into(), json!({"node": c.0})));
            }
        }
        // Set content width (single panel; with several panels this spot holds the close button).
        if app.panel_count() == 1 {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(14.0);
                let label = ["Auto", "Medium", "Full"].get(app.content_width as usize).copied().unwrap_or("Auto");
                let r = crate::icon_button(ui, crate::icons::Icon::Width, t.faint, &format!("Content width: {label}"));
                egui::Popup::menu(&r).show(|ui| {
                    for (i, l) in ["Auto", "Medium", "Full"].iter().enumerate() {
                        if ui.selectable_label(app.content_width as usize == i, *l).clicked() {
                            app.content_width = i as u8;
                            ui.close();
                        }
                    }
                });
            });
        }
    });
}

const PRODUCT_NAME_WORKSPACE: &str = "Workspace: open the Library";

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
                // Search options: a card with the query as a sentence and Open query editor.
                if let Some(q) = g.query(zoom) {
                    let r = ui.add(egui::Button::new(RichText::new("Conditions").size(12.5).color(t.dim)).fill(t.field_bg));
                    egui::Popup::menu(&r).show(|ui| {
                        ui.set_max_width(300.0);
                        ui.label(RichText::new("Search").strong());
                        ui.label(RichText::new(q.sentence(g)).size(13.0).color(t.dim));
                        if ui.button("Open query editor").clicked() {
                            menu_acts.push(Act::Ui("ui.searchOptions"));
                            ui.close();
                        }
                    });
                }
            });
        }
    });
    app.acts.extend(menu_acts);
    decorations(app, ui, t, target);
    let s = &app.session;
    let g = &s.graph;
    ui.horizontal(|ui| {
        if let Some(icon) = g.get(target).and_then(|n| n.icon.clone()) {
            ui.label(RichText::new(icon).size(30.0));
        }
        // A supertag page: a square "#" badge in the tag colour, then a checkbox square when the tag
        // shows one (spec DIFFS P3 #14).
        if let Some(def) = g.tag_def(target) {
            let (strong, _) = theme::tag_color(&def.color, app.dark);
            let (r, _) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::hover());
            ui.painter().rect_filled(r, 7.0, strong.gamma_multiply(0.85));
            crate::icons::hash(ui.painter(), r.shrink(8.0), Color32::WHITE);
            if def.checkbox {
                checkbox(ui, t, false, 22.0);
            }
            ui.add_space(4.0);
        }
        if let Some(done) = g.checkbox(target) {
            if checkbox(ui, t, done, 22.0).clicked() {
                app.acts.push(Act::Run("node.toggleDone".into(), json!({"node": target.0})));
            }
            ui.add_space(4.0);
        }
        let has_suffix = !app.session.graph.title_suffix(target).is_empty();
        if editable {
            if app.title_buf.0 != Some(target) || (!ui.ctx().memory(|m| m.has_focus(title_id)) && app.title_buf.1 != g.name(target)) {
                app.title_buf = (Some(target), g.name(target).to_string());
            }
            if app.focus_title && !app.drawing_other_panel && app.picker.is_none() {
                ui.ctx().memory_mut(|m| m.request_focus(title_id));
                app.focus_title = false;
            }
            let title_w = if has_suffix {
                ui.painter().layout_no_wrap(app.title_buf.1.clone(), title_font.clone(), t.text).size().x + 12.0
            } else {
                ui.available_width()
            };
            let out = egui::TextEdit::singleline(&mut app.title_buf.1)
                .id(title_id)
                .font(title_font.clone())
                .text_color(t.text)
                .frame(egui::Frame::NONE)
                .hint_text(RichText::new("Untitled").color(t.faint))
                .desired_width(title_w)
                .show(ui);
            if out.response.changed() {
                app.acts.push(Act::Run("node.setName".into(), json!({"node": target.0, "name": app.title_buf.1})));
            }
            if has_suffix {
                title_suffix(ui, &app.session.graph, t, target, 30.0);
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
    let description = node.map(|n| n.description.clone()).unwrap_or_default();
    let nav: Option<[(&str, &str, serde_json::Value); 2]> = match node.map(|n| &n.kind) {
        Some(Kind::Day { .. }) => Some([("<", "nav.previousDay", json!({})), (">", "nav.nextDay", json!({}))]),
        Some(Kind::Week { .. } | Kind::Year { .. }) => {
            Some([("<", "nav.calendarStep", json!({"step": -1})), (">", "nav.calendarStep", json!({"step": 1}))])
        }
        _ => None,
    };
    description_line(app, ui, t, target, &description, 14.0);
    if let Some(nav) = nav {
        day_nav(app, ui, t, nav);
    }
}

/// Page decorations (spec 00 §4): a banner band in its colour, and on hover the small Icon and
/// Banner buttons above the title.
fn decorations(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, page: NodeId) {
    let g = &app.session.graph;
    if g.is_system(page) {
        return;
    }
    let (icon, banner) = g.get(page).map(|n| (n.icon.clone(), n.banner.clone())).unwrap_or_default();
    if let Some(b) = &banner {
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 70.0), Sense::hover());
        ui.painter().rect_filled(r, 8.0, theme::tag_color(b, app.dark).0.gamma_multiply(0.6));
    }
    let area = Rect::from_min_size(ui.cursor().min - vec2(0.0, 6.0), vec2(ui.available_width(), 90.0));
    let show = ui.rect_contains_pointer(area);
    let mut acts = Vec::new();
    ui.horizontal(|ui| {
        ui.set_min_height(18.0);
        if !show {
            return;
        }
        let ib = ui.add(egui::Label::new(RichText::new("Icon").size(12.0).color(t.faint)).sense(Sense::click()));
        egui::Popup::menu(&ib).show(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.set_max_width(220.0);
                for e in [
                    "\u{1F331}",
                    "\u{1F4DA}",
                    "\u{1F4CC}",
                    "\u{2B50}",
                    "\u{1F525}",
                    "\u{1F4A1}",
                    "\u{1F3AF}",
                    "\u{1F4C5}",
                    "\u{2705}",
                    "\u{1F4DD}",
                    "\u{1F680}",
                    "\u{1F333}",
                ] {
                    if ui.button(RichText::new(e).size(18.0)).clicked() {
                        acts.push(Act::Run("node.setIcon".into(), json!({"node": page.0, "icon": e})));
                        ui.close();
                    }
                }
            });
            if icon.is_some() && ui.button("Remove icon").clicked() {
                acts.push(Act::Run("node.setIcon".into(), json!({"node": page.0, "icon": ""})));
                ui.close();
            }
        });
        let bb = ui.add(egui::Label::new(RichText::new("Banner").size(12.0).color(t.faint)).sense(Sense::click()));
        egui::Popup::menu(&bb).show(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.set_max_width(220.0);
                for c in tanacraft_core::TAG_COLORS.iter().skip(2) {
                    let (r, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                    ui.painter().circle_filled(r.center(), 9.0, theme::tag_color(c, app.dark).0);
                    if resp.on_hover_text(*c).clicked() {
                        acts.push(Act::Run("node.setBanner".into(), json!({"node": page.0, "color": c})));
                        ui.close();
                    }
                }
            });
            if banner.is_some() && ui.button("Remove banner").clicked() {
                acts.push(Act::Run("node.setBanner".into(), json!({"node": page.0, "color": ""})));
                ui.close();
            }
        });
    });
    app.acts.extend(acts);
}

/// The calendar navigation bar: previous, next, Today, and Go to date (spec 06 §4).
fn day_nav(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, nav: [(&str, &str, serde_json::Value); 2]) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        // Icons: previous, next, Today, and a calendar that opens Go to date (DIFFS P3 #15).
        let [(_, prev, pp), (_, next, np)] = nav;
        for (icon, tip, id, params) in [(crate::icons::Icon::Back, "Previous", prev, pp), (crate::icons::Icon::Forward, "Next", next, np)] {
            if crate::icon_button(ui, icon, t.dim, tip).clicked() {
                app.acts.push(Act::Run(id.into(), params));
            }
        }
        let b = egui::Button::new(RichText::new("Today").size(13.0).color(t.dim)).fill(t.field_bg).corner_radius(5.0);
        if ui.add(b).clicked() {
            app.acts.push(Act::Run("nav.today".into(), json!({})));
        }
        let r = crate::icon_button(ui, crate::icons::Icon::Calendar, t.dim, "Go to date");
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
                ui.horizontal(|ui| crate::views::item_line(app, ui, t, i, true));
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
        FieldType::User => "User",
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
pub(crate) fn paint_field_icon(p: &egui::Painter, r: Rect, ty: FieldType, c: Color32) {
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
        FieldType::User => {
            p.rect_stroke(r, 2.0, st, StrokeKind::Middle);
            p.circle_stroke(r.center() - vec2(0.0, 1.5), 1.8, st);
            p.line_segment([r.left_bottom() + vec2(2.0, -1.5), r.right_bottom() + vec2(-2.0, -1.5)], st);
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
    fields_at(app, ui, t, node, indent, None);
}

/// The field rows of `node` shown after its child `slot` (`None`: above the children, with the
/// "+ Field" buttons).
pub(crate) fn fields_at(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, node: NodeId, indent: f32, slot: Option<NodeId>) {
    let target = app.session.graph.resolve(node);
    // In an outline an empty "hide when empty" field folds into a "+ Field" button; on the node's
    // own page it stays a row (spec 03 §9).
    let zoomed = target == app.session.graph.resolve(app.session.zoom);
    let kids = app.session.graph.children(target).to_vec();
    let entries: Vec<_> = app
        .session
        .graph
        .get(target)
        .map(|n| n.fields.iter().filter(|e| e.after.filter(|a| kids.contains(a)) == slot).cloned().collect())
        .unwrap_or_default();
    let mut folded: Vec<(NodeId, String)> = Vec::new();
    for e in entries {
        let g = &app.session.graph;
        let Some(def) = g.field_def(e.field).cloned() else { continue };
        let name = g.name(e.field).to_string();
        let value_text = e.value.as_ref().map(|v| export::display_value(g, v));
        let values: Vec<FieldValue> = e.values().cloned().collect();
        let edit_ix = app.field_edit.as_ref().filter(|f| f.node == target && f.field == e.field).map(|f| f.index);
        let editing = edit_ix.is_some();
        // Hide-field rules (spec 03 §6).
        let empty = value_text.as_deref().is_none_or(str::is_empty);
        let is_default = def.default.is_some() && e.value == def.default;
        let hidden = match def.hide {
            tanacraft_core::HideMode::Never => false,
            tanacraft_core::HideMode::WhenEmpty => empty && !zoomed,
            tanacraft_core::HideMode::WhenNotEmpty => !empty,
            tanacraft_core::HideMode::WhenDefault => is_default,
            tanacraft_core::HideMode::Always => true,
        };
        if hidden && !editing {
            if def.hide == tanacraft_core::HideMode::WhenEmpty {
                folded.push((e.field, name));
            }
            continue;
        }
        let missing = def.required && empty;
        let invalid = values.iter().any(|v| !v.is_valid());
        // Narrow panels stack the name above the value (spec 03 §9).
        let narrow = ui.available_width() - indent < 420.0;
        ui.horizontal(|ui| {
            ui.add_space(indent);
            ui.vertical(|ui| {
                let body = |ui: &mut Ui, app: &mut TanaApp| {
                    field_label(ui, t, &name, def.field_type);
                    if narrow {
                        ui.end_row();
                    }
                    field_values(app, ui, t, target, e.field, &def, &values, edit_ix);
                    if missing || invalid {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let (r, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                            let amber = Color32::from_rgb(224, 176, 48);
                            ui.painter().circle_stroke(r.center(), 7.0, Stroke::new(1.4, amber));
                            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, "?", FontId::proportional(11.0), amber);
                            let tip = if invalid { "Not a valid value for this field type" } else { "Required" };
                            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, tip));
                            resp.on_hover_text(tip);
                        });
                    }
                };
                if narrow {
                    egui::Frame::new().fill(t.field_bg).corner_radius(6.0).inner_margin(egui::Margin::symmetric(10, 5)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal_wrapped(|ui| body(ui, app));
                    });
                    ui.add_space(3.0);
                } else {
                    field_frame_mut(ui, t, app, body);
                }
            });
        });
    }
    if slot.is_some() {
        return;
    }
    // Optional fields of the node's supertags are offered the same way until added (spec 03 §5).
    let g = &app.session.graph;
    let have: Vec<NodeId> = g.get(target).map(|n| n.fields.iter().map(|e| e.field).collect()).unwrap_or_default();
    let tags = g.get(target).map(|n| n.tags.clone()).unwrap_or_default();
    let mut optional: Vec<NodeId> = Vec::new();
    for tg in tags.iter().flat_map(|x| g.tag_lineage(*x)) {
        for f in g.tag_def(tg).map(|d| d.optional.clone()).unwrap_or_default() {
            if !have.contains(&f) && !optional.contains(&f) && g.field_def(f).is_some() {
                optional.push(f);
            }
        }
    }
    let optional: Vec<(NodeId, String)> = optional.into_iter().map(|f| (f, g.name(f).to_string())).collect();
    if !folded.is_empty() || !optional.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(indent + 4.0);
            for (f, name) in folded {
                if ui.add(egui::Label::new(RichText::new(format!("+ {name}")).size(12.5).color(t.faint)).sense(Sense::click())).clicked() {
                    app.acts.push(Act::EditField(target, f, 0, String::new()));
                }
            }
            for (f, name) in optional {
                if ui.add(egui::Label::new(RichText::new(format!("+ {name}")).size(12.5).color(t.faint)).sense(Sense::click())).clicked() {
                    app.acts.push(Act::Run("field.add".into(), json!({"node": target.0, "field": f.0})));
                    app.acts.push(Act::EditField(target, f, 0, String::new()));
                }
            }
        });
    }
}

fn field_frame_mut(ui: &mut Ui, t: &Tokens, app: &mut TanaApp, add: impl FnOnce(&mut Ui, &mut TanaApp)) {
    egui::Frame::new().fill(t.field_bg).corner_radius(6.0).inner_margin(egui::Margin::symmetric(10, 5)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.set_min_height(22.0);
            add(ui, app)
        });
    });
    ui.add_space(3.0);
}

/// Every value of a field side by side, each editable, then a faint + that adds another value (a
/// field can hold several, spec 01 §5). `edit_ix` is the value being typed into.
#[allow(clippy::too_many_arguments)]
pub(crate) fn field_values(
    app: &mut TanaApp,
    ui: &mut Ui,
    t: &Tokens,
    node: NodeId,
    field: NodeId,
    def: &tanacraft_core::FieldDef,
    values: &[FieldValue],
    edit_ix: Option<usize>,
) {
    // Several values stack, one bulleted line each (spec 03 §8, `166`); one value is a plain line.
    let add = values.len();
    let adding = edit_ix == Some(add) && !values.is_empty();
    let stacked = values.len() > 1 || adding;
    ui.vertical(|ui| {
        let n = values.len().max(1);
        for i in 0..n + usize::from(adding) {
            ui.horizontal(|ui| {
                if stacked {
                    let (r, _) = ui.allocate_exact_size(vec2(8.0, 18.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 2.2, t.dim);
                }
                let v = values.get(i);
                let text = v.map(|v| export::display_value(&app.session.graph, v));
                field_value(app, ui, t, node, field, i, def, v, text, edit_ix == Some(i));
                // The + shows while the pointer is over the last value.
                if i + 1 == n && !adding && !values.is_empty() && def.field_type != FieldType::Checkbox {
                    let shown = ui.rect_contains_pointer(ui.max_rect().expand2(vec2(0.0, 4.0)));
                    let r = ui
                        .add(
                            egui::Label::new(RichText::new("+").size(14.0).color(if shown { t.faint } else { Color32::TRANSPARENT }))
                                .sense(Sense::click()),
                        )
                        .on_hover_text("Add another value");
                    if r.clicked() {
                        let at = r.rect.left_bottom() + vec2(0.0, 4.0);
                        app.acts.push(match def.field_type {
                            FieldType::Options => Act::PickChoice(node, field, add, at),
                            FieldType::Date => Act::PickDate(node, field, add, at),
                            _ => Act::EditField(node, field, add, String::new()),
                        });
                    }
                }
            });
        }
    });
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn field_value(
    app: &mut TanaApp,
    ui: &mut Ui,
    t: &Tokens,
    node: NodeId,
    field: NodeId,
    index: usize,
    def: &tanacraft_core::FieldDef,
    value: Option<&FieldValue>,
    text: Option<String>,
    editing: bool,
) {
    let set = |v: String| Act::Run("field.set".into(), json!({"node": node.0, "field": field.0, "index": index, "value": v}));
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
                Some(v) => v.date(),
                None => None,
            };
            let time = match value {
                Some(FieldValue::Timed { start, end, .. }) => format!(", {} - {}", tanacraft_core::fmt_hm(*start), tanacraft_core::fmt_hm(*end)),
                _ => String::new(),
            };
            let label = match date {
                Some(d) if d.year == app.session.today.year => format!("{}{time}", d.day_title()),
                Some(d) => format!("{}, {}{time}", d.day_title(), d.year),
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
                app.acts.push(Act::PickDate(node, field, index, r.rect.left_bottom() + vec2(0.0, 4.0)));
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
        FieldType::Options | FieldType::OptionsFromTag | FieldType::User if !editing => {
            let current = text.clone().unwrap_or_default();
            let set_color = if def.field_type == FieldType::OptionsFromTag { t.link } else { t.text };
            let (label, color) = if current.is_empty() { (hint(def.field_type).to_string(), t.faint) } else { (current, set_color) };
            let r = ui.add(egui::Label::new(RichText::new(label).size(14.5).color(color)).sense(Sense::click()));
            if r.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if r.clicked() {
                let at = r.rect.left_bottom() + vec2(0.0, 4.0);
                app.acts.push(if def.field_type == FieldType::Date {
                    Act::PickDate(node, field, index, at)
                } else {
                    Act::PickChoice(node, field, index, at)
                });
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
                Some(s) if !s.is_empty() => (s.clone(), if def.field_type == FieldType::OptionsFromTag { t.link } else { t.text }),
                _ => (hint(def.field_type).to_string(), t.faint),
            };
            let r = ui.add(egui::Label::new(RichText::new(label).size(14.5).color(color)).sense(Sense::click()));
            if r.clicked() {
                app.acts.push(Act::EditField(node, field, index, text.unwrap_or_default()));
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
        FieldType::User => "Add user",
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
    let spans = tanacraft_core::format::spans(text);
    // A link inside text gets a small arrow before it; a link-only node shows it as its bullet.
    let link_only = spans.iter().filter(|s| !s.text.trim().is_empty()).count() == 1;
    for sp in spans {
        let mut f = span_format(t, &sp, color, size);
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
                if !link_only {
                    job.append("\u{2197}\u{2009}", 0.0, f.clone());
                    targets.push(Some(LinkTarget::Url(u.clone())));
                }
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

/// The look of one styled span.
fn span_format(t: &Tokens, sp: &tanacraft_core::format::Span, color: Color32, size: f32) -> egui::TextFormat {
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
    if sp.url.is_some() || sp.node.is_some() {
        f.color = t.link;
    }
    f
}

/// The raw text being edited, styled: marks shown faint, the text between them formatted.
fn edit_job(t: &Tokens, text: &str, size: f32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    for (sp, marker) in tanacraft_core::format::raw_spans(text) {
        let f = if marker {
            egui::TextFormat { font_id: FontId::proportional(size), color: t.faint, ..Default::default() }
        } else {
            span_format(t, &sp, t.text, size)
        };
        job.append(&sp.text, 0.0, f);
    }
    job
}

pub(crate) fn rich_job(g: &Graph, t: &Tokens, text: &str, color: Color32, strike: bool, size: f32) -> egui::text::LayoutJob {
    rich_job_links(g, t, text, color, strike, size).0
}

/// A clickable label with marks, inline references and links. Returns the response and, when the
/// click landed on a reference or link, its target.
pub(crate) fn link_label(
    ui: &mut Ui,
    job: egui::text::LayoutJob,
    targets: &[Option<LinkTarget>],
) -> (egui::Response, Option<LinkTarget>, Option<usize>) {
    // Laid out here rather than by `Label`, which would replace the job's wrap width.
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    let resp = if galley.rows.len() > 1 {
        // Wrapped: the first line sits where a one-line label would, the rest hangs below.
        let h0 = galley.rows[0].rect().height();
        let top = ui.cursor().top() + ((theme::ROW_H - h0) / 2.0).max(0.0);
        let rect = Rect::from_min_size(pos2(ui.cursor().left(), top), galley.size());
        ui.allocate_rect(rect, Sense::click())
    } else {
        ui.allocate_exact_size(galley.size(), Sense::click()).1
    };
    let pos = resp.rect.min;
    ui.painter().galley(pos, galley.clone(), Color32::WHITE);
    // Keep the text visible to accessibility (and to UI tests).
    let text = galley.text().to_string();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, text.clone()));
    let mut hit = None;
    let mut at = None;
    if let Some(p) = ui.ctx().pointer_hover_pos().filter(|p| resp.rect.contains(*p)) {
        let c = galley.cursor_from_pos(p - pos);
        at = Some(c.index.0);
        let byte = galley.job.text.char_indices().nth(c.index.0).map(|(b, _)| b).unwrap_or(galley.job.text.len());
        let sec = galley.job.sections.iter().position(|s| s.byte_range.start.0 <= byte && byte < s.byte_range.end.0);
        hit = sec.and_then(|i| targets.get(i).cloned().flatten());
        if hit.is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }
    let clicked = resp.clicked();
    (resp, if clicked { hit } else { None }, at)
}

/// A clickable label with inline marks rendered; references and links open on click.
pub(crate) fn rich_label(ui: &mut Ui, app_acts: &mut Vec<Act>, g: &Graph, t: &Tokens, text: &str, color: Color32, strike: bool) -> egui::Response {
    let (mut job, targets) = rich_job_links(g, t, text, color, strike, theme::FONT);
    // Wrap at the room left (cards, table cells, lists).
    let room = ui.available_size_before_wrap().x;
    if room.is_finite() && room > 60.0 {
        job.wrap.max_width = room;
    }
    let (resp, hit, _) = link_label(ui, job, &targets);
    match hit {
        Some(LinkTarget::Node(n)) => app_acts.push(Act::Run("nav.open".into(), json!({"node": n.0}))),
        Some(LinkTarget::Url(u)) => ui.ctx().open_url(egui::OpenUrl::new_tab(u)),
        None => return resp,
    }
    // The link handled the click; the caller should not also act on it.
    resp.clone().on_hover_text("Open")
}

/// The whole text is one link.
fn is_link_only(name: &str) -> bool {
    let sp = tanacraft_core::format::spans(name.trim());
    sp.len() == 1 && sp.first().is_some_and(|x| x.url.is_some())
}

/// "3 minutes ago"-style time.
pub(crate) fn ago(secs: u64) -> String {
    let now = tanacraft_core::now_secs();
    let d = now.saturating_sub(secs);
    match d {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} minutes ago", d / 60),
        3600..=86_399 => format!("{} hours ago", d / 3600),
        _ => format!("{} days ago", d / 86_400),
    }
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
    item(ui, "Open in new panel", Act::OpenInPanel(g.resolve(id)));
    item(ui, if g.is_expanded(id) { "Collapse" } else { "Expand" }, run("node.toggleExpand"));
    ui.separator();
    let at = ui.min_rect().left_bottom();
    item(ui, "Add supertag", Act::Pick(PickMode::Tag, g.resolve(id), at));
    item(ui, "Add field", Act::Pick(PickMode::Field, g.resolve(id), at));
    item(ui, if g.checkbox(id).is_some() { "Toggle done" } else { "Add checkbox" }, run("node.toggleDone"));
    item(ui, "Add description", Act::Ui("ui.editDescription"));
    item(ui, "Toggle heading", run("node.toggleHeading"));
    item(ui, "Show progress bar", run("node.toggleProgress"));
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
    item(ui, "Show or hide view toolbar", run("view.toggleToolbar"));
    item(ui, "Show edit log", Act::EditLog(id));
    item(ui, "Move to...", Act::Pick(PickMode::MoveTo, id, at));
    item(ui, "Move and leave reference to...", Act::Pick(PickMode::MoveLeaveRef, id, at));
    item(ui, "Move to top", run("node.moveToTop"));
    item(ui, "Move to bottom", run("node.moveToBottom"));
    item(ui, "Set as move target", run("node.setMoveTarget"));
    item(ui, "Move to move target", run("node.moveToTarget"));
    // The link is an inline reference token: pasting it into a line links back here.
    if ui.button("Copy node link").clicked() {
        ui.ctx().copy_text(tanacraft_core::format::ref_token(g.resolve(id).0));
        ui.close();
    }
    item(ui, "Move up", run("node.moveUp"));
    item(ui, "Move down", run("node.moveDown"));
    item(ui, "Indent", run("node.indent"));
    item(ui, "Outdent", run("node.outdent"));
    ui.separator();
    if g.query(id).is_some() {
        item(ui, "Convert search node to plain list", run("search.toPlainList"));
    }
    item(ui, "More commands", Act::Ui("ui.commandLine"));
    if g.get(id).is_some_and(|n| n.kind == Kind::Plain && n.reference_target().is_none()) && !g.is_system(id) {
        item(ui, "Convert to supertag", run("node.convertToSupertag"));
    }
    if g.in_trash(id) {
        item(ui, "Restore", run("node.restore"));
    } else if g.get(id).is_some_and(|n| n.reference_target().is_some()) {
        // A reference: removing it leaves the original alone (spec 01 §3).
        item(ui, "Remove reference", run("node.delete"));
    } else {
        item(ui, "Delete", run("node.delete"));
        item(ui, "Hard delete including references", run("node.hardDelete"));
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
    // Rows well outside the viewport are blank space of their last measured height (one line
    // until first seen), so a huge expanded outline costs only what is on screen.
    let clip = ui.clip_rect().expand(ROW_H * 4.0);
    let left = ui.cursor().left();
    let mut bullet_dx = 10.0;
    // Rows whose subtree is still open: when it closes, field rows placed after that row show.
    let mut open_rows: Vec<usize> = Vec::new();
    let anchored = |app: &TanaApp, id: NodeId| {
        let g = &app.session.graph;
        g.parent(id).and_then(|p| g.get(p)).is_some_and(|p| p.fields.iter().any(|e| e.after == Some(id)))
    };
    for (ri, row) in rows.iter().enumerate() {
        while let Some(&oi) = open_rows.last() {
            if rows[oi].depth < row.depth {
                break;
            }
            open_rows.pop();
            if anchored(app, rows[oi].id) {
                let r = rows[oi];
                let parent = app.session.graph.parent(r.id).unwrap_or(app.session.zoom);
                fields_at(app, ui, t, parent, r.depth as f32 * INDENT + 8.0, Some(r.id));
            }
        }
        open_rows.push(ri);
        let selected = sel_range.as_ref().is_some_and(|r| r.contains(&ri));
        let top = ui.cursor().top();
        let h = app.row_heights.get(&row.id).copied().unwrap_or(ROW_H);
        let offscreen = top > clip.bottom() || top + h < clip.top();
        if offscreen && !selected && app.session.focus != Some(row.id) {
            bullets.push((row.depth, pos2(left + row.depth as f32 * INDENT + bullet_dx, top + ROW_H / 2.0)));
            ui.add_space(h);
            bottoms.push(ui.cursor().top());
            continue;
        }
        let center = row_ui(app, ui, t, row.id, row.depth, selected);
        bullet_dx = center.x - left - row.depth as f32 * INDENT;
        bullets.push((row.depth, center));
        let expanded = app.session.graph.is_expanded(row.id);
        if expanded {
            fields(app, ui, t, row.id, (row.depth + 1) as f32 * INDENT + 8.0);
            if app.session.row_children(row.id).is_empty() && app.session.graph.query(app.session.graph.resolve(row.id)).is_none() {
                ghost_line(app, ui, t, row.id, row.depth + 1);
            }
        }
        bottoms.push(ui.cursor().top());
        app.row_heights.insert(row.id, ui.cursor().top() - top);
    }
    while let Some(oi) = open_rows.pop() {
        if anchored(app, rows[oi].id) {
            let r = rows[oi];
            let parent = app.session.graph.parent(r.id).unwrap_or(app.session.zoom);
            fields_at(app, ui, t, parent, r.depth as f32 * INDENT + 8.0, Some(r.id));
        }
    }
    // Guide lines: from an expanded bullet down to its last visible descendant (one pass).
    let mut last_desc: Vec<Option<usize>> = vec![None; rows.len()];
    let mut open: Vec<usize> = Vec::new();
    for (j, r) in rows.iter().enumerate() {
        while let Some(&i) = open.last() {
            if rows[i].depth < r.depth {
                break;
            }
            open.pop();
            if j - 1 > i {
                last_desc[i] = Some(j - 1);
            }
        }
        open.push(j);
    }
    for i in open {
        if rows.len() - 1 > i {
            last_desc[i] = Some(rows.len() - 1);
        }
    }
    let p = ui.painter();
    for (i, (_, c)) in bullets.iter().enumerate() {
        let end_y = match last_desc[i] {
            Some(j) => bottoms.get(j).copied(),
            None if app.session.graph.is_expanded(rows[i].id) => bottoms.get(i).copied().filter(|b| *b > c.y + ROW_H),
            None => None,
        };
        if let Some(y) = end_y
            && y >= clip.top()
            && c.y <= clip.bottom()
        {
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
        ui.painter().rect_stroke(r, 4.0, Stroke::new(1.0, t.accent.gamma_multiply(0.7)), StrokeKind::Inside);
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
        } else if node.as_ref().is_some_and(|n| is_link_only(&n.name)) {
            // A node that is just a URL is a link node: the bullet becomes a link glyph (spec 02 §12).
            let st = Stroke::new(1.3, t.link);
            let b = Rect::from_center_size(center, vec2(9.0, 9.0));
            ui.painter().rect_stroke(b, 1.5, st, StrokeKind::Middle);
            ui.painter().line_segment([b.center(), b.right_top() + vec2(2.0, -2.0)], st);
        } else if !blank {
            ui.painter().circle_filled(center, 3.2, bullet_color);
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        // Hover card: when the node was edited and created.
        let resp = match node.as_ref() {
            Some(n) => {
                resp.on_hover_text(format!("{}\nEdited {}, created {}", app.session.graph.text(target), ago(n.edited.max(n.created)), ago(n.created)))
            }
            None => resp,
        };
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
        if let Some(icon) = node.as_ref().and_then(|n| n.icon.clone()) {
            ui.label(RichText::new(icon).size(theme::FONT));
        }
        // Text.
        let name = app.session.graph.name(id).to_string();
        let text_color = if done == Some(true) { t.dim } else { t.text };
        if focused && !is_ref {
            edit_row(app, ui, t, id);
        } else {
            let size = if heading { 20.0 } else { theme::FONT };
            let (mut job, targets) = rich_job_links(&app.session.graph, t, &name, text_color, false, size);
            // Long text wraps, leaving room for the tag chips (spec 02 §1).
            job.wrap.max_width = (ui.available_width() - chips_width(&app.session.graph, id)).max(120.0);
            let (r, hit, at) = link_label(ui, job, &targets);
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
                    // The caret goes where the text was clicked (spec 02 §1).
                    let caret = at.and_then(|c| tanacraft_core::format::shown_to_raw(&name, c)).unwrap_or(usize::MAX);
                    app.acts.push(Act::Run("focus.set".into(), json!({"node": id.0, "caret": caret})));
                }
            }
            if focused && is_ref {
                ui.painter().rect_stroke(r.rect.expand(2.0), 3.0, Stroke::new(1.0, t.accent), StrokeKind::Outside);
            }
        }
        title_suffix(ui, &app.session.graph, t, id, if heading { 20.0 } else { theme::FONT });
        // Reference counter (a setting): how many places point at this node.
        // ponytail: counts by scanning the graph per row; cache per frame if big graphs feel slow.
        if app.ref_counter {
            let n = app.session.graph.references_to(target).len();
            if n > 0 {
                let r = ui.label(RichText::new(n.to_string()).size(11.0).color(t.faint).background_color(t.hover));
                r.on_hover_text(format!("Referenced in {n} place(s)"));
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
    if !description.is_empty() || app.desc_edit.as_ref().is_some_and(|d| d.0 == target) {
        ui.add_space(-6.0);
        ui.horizontal(|ui| {
            ui.add_space(indent + 26.0);
            description_line(app, ui, t, target, &description, 13.0);
        });
    }
    // Progress bar of checkbox children, under the name (spec screenshot 152).
    if node.as_ref().is_some_and(|n| n.progress) {
        let (done_n, total) = app.session.graph.progress(target);
        ui.horizontal(|ui| {
            ui.add_space(indent + 26.0);
            let (r, _) = ui.allocate_exact_size(vec2(100.0, 12.0), Sense::hover());
            let bar = Rect::from_min_size(r.left_center() - vec2(0.0, 2.0), vec2(96.0, 4.0));
            ui.painter().rect_filled(bar, 2.0, t.line);
            if total > 0 {
                let w = bar.width() * done_n as f32 / total as f32;
                ui.painter().rect_filled(Rect::from_min_size(bar.min, vec2(w, 4.0)), 2.0, Color32::from_rgb(76, 160, 96));
            }
            ui.label(RichText::new(format!("{done_n} / {total}")).size(11.5).color(t.faint));
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
    crate::views::flash(app, ui, id, row.response.rect);
    drop_target(app, ui, t, id, center, row.response.rect);
    if selected && app.session.sel == Some((id, id)) {
        selection_toolbar(app, ui, t, id, row.response.rect);
    }
    center
}

/// A node's description under its name: grey text, edited in place (Ctrl+I, or Down from the
/// name; spec 02 §13). Enter or Up goes back to the name; clicking elsewhere keeps the text.
fn description_line(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, node: NodeId, text: &str, size: f32) {
    let editing = app.desc_edit.as_ref().is_some_and(|d| d.0 == node);
    if !editing {
        if !text.is_empty() {
            let r = ui.add(egui::Label::new(RichText::new(text).size(size).color(t.dim)).sense(Sense::click()));
            if r.clicked() {
                app.desc_edit = Some((node, text.to_string()));
                app.desc_focus = true;
            }
        }
        return;
    }
    let id = egui::Id::new((app.panel_ix, "desc-edit", node.0));
    let Some((_, buf)) = app.desc_edit.as_mut() else { return };
    let resp = ui.add(
        egui::TextEdit::singleline(buf)
            .id(id)
            .frame(egui::Frame::NONE)
            .font(FontId::proportional(size))
            .text_color(t.dim)
            .hint_text("Description")
            .margin(egui::Margin::ZERO)
            .desired_width(ui.available_width()),
    );
    if app.desc_focus {
        resp.request_focus();
        app.desc_focus = false;
    } else if resp.lost_focus() {
        let text = buf.clone();
        app.desc_edit = None;
        app.acts.push(Act::Run("node.setDescription".into(), json!({"node": node.0, "text": text})));
        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            app.acts.push(Act::Run("focus.set".into(), json!({"node": node.0, "caret": usize::MAX})));
        }
    }
}

/// The computed part of a title from its supertag's title expression, in grey; a field without a
/// value shows its name, underlined (spec 03 §10).
pub(crate) fn title_suffix(ui: &mut Ui, g: &Graph, t: &Tokens, id: NodeId, size: f32) {
    let parts = g.title_suffix(id);
    if parts.is_empty() {
        return;
    }
    let mut job = egui::text::LayoutJob::default();
    for (text, missing) in parts {
        let f = egui::TextFormat {
            font_id: FontId::proportional(size),
            color: t.faint,
            underline: if missing { Stroke::new(1.0, t.faint) } else { Stroke::NONE },
            ..Default::default()
        };
        job.append(&text, 0.0, f);
    }
    ui.add_space(-ui.spacing().item_spacing.x);
    ui.label(job);
}

/// Rough width of a row's tag chips, so wrapped text leaves room for them.
fn chips_width(g: &Graph, id: NodeId) -> f32 {
    g.get(g.resolve(id)).map(|n| n.tags.iter().map(|t| g.name(*t).chars().count() as f32 * 8.0 + 34.0).sum()).unwrap_or(0.0)
}

/// The faint empty line of an expanded node with no children: click to add its first child, or drop
/// a dragged bullet on it to nest it there (spec 02 §6, `100-drag-into-empty-child-list.jpg`).
fn ghost_line(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, parent: NodeId, depth: usize) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H - 4.0), Sense::click());
    let c = pos2(rect.left() + depth as f32 * INDENT + 10.0, rect.center().y);
    ui.painter().circle_filled(c, 2.5, t.faint.gamma_multiply(0.6));
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, format!("ghost {}", parent.0)));
    let target = app.session.graph.resolve(parent);
    if resp.clicked() {
        let r = app.session.run("node.add", &json!({"parent": target.0}));
        if let Some(id) = r.ok().and_then(|v| v["id"].as_u64()) {
            app.acts.push(Act::Run("focus.set".into(), json!({"node": id, "caret": 0})));
        }
    }
    if resp.dnd_hover_payload::<NodeId>().is_some() {
        ui.painter().line_segment([pos2(c.x - 8.0, rect.center().y), pos2(rect.right(), rect.center().y)], Stroke::new(2.0, t.accent));
    }
    if let Some(dragged) = resp.dnd_release_payload::<NodeId>()
        && *dragged != parent
    {
        let selected = app.session.selected();
        let nodes: Vec<u64> = if selected.contains(&dragged) { selected.iter().map(|n| n.0).collect() } else { vec![dragged.0] };
        app.acts.push(Act::Run("node.moveMany".into(), json!({"nodes": nodes, "parent": target.0, "index": 0})));
    }
}

/// The floating toolbar above one selected node (spec 02 §8).
fn selection_toolbar(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, id: NodeId, row: Rect) {
    let anchor = row.left_top() + vec2(18.0, -34.0);
    let mut acts = Vec::new();
    egui::Area::new(egui::Id::new((app.panel_ix, "sel-toolbar"))).order(egui::Order::Foreground).fixed_pos(anchor).show(ui.ctx(), |ui| {
        egui::Frame::popup(ui.style()).fill(t.panel).corner_radius(6.0).inner_margin(egui::Margin::symmetric(6, 3)).show(ui, |ui| {
            ui.horizontal(|ui| {
                let at = ui.min_rect().left_bottom() + vec2(0.0, 30.0);
                for (label, act, tip) in [
                    ("<", Act::Run("node.outdent".into(), json!({"node": id.0})), "Outdent"),
                    (">", Act::Run("node.indent".into(), json!({"node": id.0})), "Indent"),
                    ("# Tag", Act::Pick(PickMode::Tag, id, at), "Add supertag"),
                    ("Move", Act::Pick(PickMode::MoveTo, id, at), "Move to"),
                    ("H", Act::Run("node.toggleHeading".into(), json!({"node": id.0})), "Toggle heading"),
                ] {
                    if ui.add(egui::Button::new(RichText::new(label).size(12.5)).frame(false)).on_hover_text(tip).clicked() {
                        acts.push(act);
                    }
                }
            });
        });
    });
    app.acts.extend(acts);
}

/// Drag and drop: a bullet dropped on a row lands after it; dropped right of the row's bullet
/// (one indent further), it becomes the row's first child.
fn drop_target(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, id: NodeId, bullet: egui::Pos2, rect: Rect) {
    let resp = ui.interact(rect, egui::Id::new((app.panel_ix, "drop", id.0)), Sense::hover());
    let Some(dragged) = resp.dnd_hover_payload::<NodeId>() else { return };
    if *dragged == id {
        return;
    }
    // Upper half: above this row; lower half: below it; both at this row's depth. The line starts
    // at that depth (spec 02 §6); nesting happens on a ghost line (see `ghost_line`).
    let above = ui.ctx().pointer_hover_pos().is_some_and(|p| p.y < rect.center().y);
    let y = if above { rect.top() } else { rect.bottom() };
    ui.painter().line_segment([pos2(bullet.x - 8.0, y), pos2(rect.right(), y)], Stroke::new(2.0, t.accent));
    if let Some(dragged) = resp.dnd_release_payload::<NodeId>() {
        let g = &app.session.graph;
        // Dragging a selected bullet moves the whole selection.
        let selected = app.session.selected();
        let nodes: Vec<u64> = if selected.contains(&dragged) { selected.iter().map(|n| n.0).collect() } else { vec![dragged.0] };
        let params = match (g.parent(id), g.index_in_parent(id)) {
            (Some(p), Some(i)) => json!({"nodes": nodes, "parent": p.0, "index": if above { i } else { i + 1 }}),
            _ => return,
        };
        app.acts.push(Act::Run("node.moveMany".into(), params));
    }
}

/// The focused bullet's text field. Typing `#` opens the supertag picker; `>` or `@` on an empty
/// bullet opens the field or reference picker.
fn edit_row(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, id: NodeId) {
    let tid = egui::Id::new((app.panel_ix, "node-edit", id.0));
    // An open picker keeps the keyboard; the caret moves once it closes.
    if let Some((a, c)) = app.edit.set_cursor.take_if(|_| app.picker.is_none()) {
        let mut st = egui::text_edit::TextEditState::load(ui.ctx(), tid).unwrap_or_default();
        st.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(a), CCursor::new(c))));
        st.store(ui.ctx(), tid);
        ui.ctx().memory_mut(|m| m.request_focus(tid));
    }
    let size = if app.session.graph.get(app.session.graph.resolve(id)).is_some_and(|n| n.heading) { 20.0 } else { theme::FONT };
    let galley_w = ui.painter().layout_no_wrap(app.edit.buf.clone(), FontId::proportional(size), t.text).size().x;
    // Long text wraps while editing (spec 02 §1); keys never insert line breaks (Enter is a command).
    let room = (ui.available_width() - chips_width(&app.session.graph, id)).max(120.0);
    // Wrapped text: keep the first line level with the bullet (as for labels).
    let w = if galley_w < 1.0 { 420.0f32.min(room) } else { (galley_w + 24.0).clamp(60.0, room) };
    let lines = ui.painter().layout(app.edit.buf.clone(), FontId::proportional(size), t.text, w - 4.0).rows.len();
    let mut child = None;
    if lines > 1 {
        let top = ui.cursor().top() + ((theme::ROW_H - size * 1.3) / 2.0).max(0.0);
        let r = Rect::from_min_size(pos2(ui.cursor().left(), top), vec2(w, 2000.0));
        child = Some(ui.new_child(egui::UiBuilder::new().max_rect(r).layout(egui::Layout::top_down(egui::Align::Min))));
    }
    let edit_ui = child.as_mut().unwrap_or(ui);
    let tt = *t;
    let mut layouter = |ui: &Ui, buf: &dyn egui::TextBuffer, wrap: f32| {
        let mut job = edit_job(&tt, buf.as_str(), size);
        job.wrap.max_width = wrap;
        ui.fonts_mut(|f| f.layout_job(job))
    };
    let hint =
        if app.picker.as_ref().is_some_and(|p| p.line == Some(id)) { "" } else { "Type, or # for a supertag, @ for a reference, > for a field" };
    let out = egui::TextEdit::multiline(&mut app.edit.buf)
        .layouter(&mut layouter)
        .desired_rows(1)
        .id(tid)
        .frame(egui::Frame::NONE)
        .font(FontId::proportional(size))
        .hint_text(RichText::new(hint).color(t.faint))
        .text_color(t.text)
        .margin(egui::Margin::ZERO)
        .lock_focus(true)
        .desired_width(w)
        .show(edit_ui);
    if let Some(c) = child {
        ui.allocate_rect(c.min_rect(), Sense::hover());
    }
    if let Some(cr) = out.cursor_range {
        app.session.caret = cr.primary.index.0;
        app.session.anchor = cr.secondary.index.0;
        app.edit.selection_empty = cr.primary == cr.secondary;
    }
    if out.response.changed() {
        if app.edit.buf.contains('\n') {
            app.edit.buf = app.edit.buf.replace('\n', " ");
        }
        let buf = app.edit.buf.clone();
        let caret = app.session.caret;
        let anchor = out.response.rect.left_bottom() + vec2(0.0, 4.0);
        let typed = caret.checked_sub(1).and_then(|i| buf.chars().nth(i));
        // An open inline `#` menu follows the text typed after the `#`.
        if let Some(p) = app.picker.as_mut()
            && p.mode == PickMode::Tag
            && let Some(h) = p.caret
        {
            let chars: Vec<char> = buf.chars().collect();
            if chars.get(h) == Some(&'#') && caret > h {
                let q: String = chars[h + 1..caret.min(chars.len())].iter().collect();
                if q != p.query {
                    p.query = q;
                    p.selected = 0;
                }
            } else {
                app.picker = None;
            }
        }
        let trigger = match typed {
            Some('#') => {
                // The `#` stays in the line; the menu filters by what follows it.
                app.acts.push(Act::Run("node.setName".into(), json!({"node": id.0, "name": buf, "caret": caret})));
                app.acts.push(Act::PickTagInline(id, caret - 1, anchor));
                return;
            }
            Some('>') if buf == ">" => Some(PickMode::Field),
            Some('@') if buf == "@" => Some(PickMode::Reference),
            Some('@') => Some(PickMode::InlineRef),
            Some('/') if buf == "/" => Some(PickMode::Slash),
            Some('!') if buf == "!" => {
                // `!` on an empty line makes it a heading (spec 02 §10).
                app.acts.push(Act::Run("node.setName".into(), json!({"node": id.0, "name": "", "caret": 0})));
                app.acts.push(Act::Run("node.toggleHeading".into(), json!({"node": id.0})));
                app.edit.buf.clear();
                return;
            }
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
