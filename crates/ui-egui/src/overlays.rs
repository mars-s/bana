//! Overlays: the command line (commands and nodes), the `#`/`>`/`@` pickers, the shortcuts panel and
//! the shortcut sheet.

use egui::{Align2, FontId, Order, RichText, Sense, Ui, vec2};
use serde_json::json;
use tanacraft_core::{NodeId, export};

use crate::{Act, PickMode, TanaApp, theme};

/// One command-line result.
#[derive(Debug, Clone)]
pub enum Item {
    Command {
        id: String,
        label: String,
        shortcut: String,
    },
    /// A context step for the focused node; non-command steps open a picker next.
    Step {
        label: String,
        act: Act,
    },
}

/// Where a second-step picker opens.
const STEP_AT: egui::Pos2 = egui::pos2(380.0, 150.0);

/// Context steps for `node` (spec 05 §3): add or remove tags, set its fields, move it, view as.
fn context_steps(app: &TanaApp, node: NodeId) -> Vec<Item> {
    let g = &app.session.graph;
    let target = g.resolve(node);
    let step = |label: String, act: Act| Item::Step { label, act };
    let mut out = vec![step("Add tag".into(), Act::Pick(PickMode::Tag, target, STEP_AT))];
    if let Some(n) = g.get(target) {
        for t in &n.tags {
            out.push(step(format!("Remove tag #{}", g.name(*t)), Act::Run("tag.remove".into(), json!({"node": target.0, "tag": t.0}))));
        }
        for e in &n.fields {
            let Some(def) = g.field_def(e.field) else { continue };
            let act = match def.field_type {
                tanacraft_core::FieldType::Options => Act::PickChoice(target, e.field, 0, STEP_AT),
                tanacraft_core::FieldType::Date => Act::PickDate(target, e.field, 0, STEP_AT),
                _ => Act::EditField(target, e.field, 0, String::new()),
            };
            out.push(step(format!("Set {}", g.name(e.field)), act));
        }
    }
    out.push(step("Move to".into(), Act::Pick(PickMode::MoveTo, node, STEP_AT)));
    out.push(step("Move and leave reference to".into(), Act::Pick(PickMode::MoveLeaveRef, node, STEP_AT)));
    for k in tanacraft_core::ViewKind::ALL {
        out.push(step(format!("View as {}", k.name()), Act::Run("view.set".into(), json!({"node": target.0, "kind": k.name()}))));
    }
    // Command nodes run on the focused node (spec 01 §10).
    for c in g.command_nodes() {
        out.push(step(format!("Run {}", g.name(c)), Act::Run("commandNode.run".into(), json!({"command": c.0, "node": target.0}))));
    }
    out
}

/// Results for a command-line query: steps for the focused node first, then commands (spec 05 §3).
pub fn palette_items(app: &TanaApp, query: &str) -> Vec<Item> {
    let q = query.trim();
    let words: Vec<String> = q.split_whitespace().map(str::to_lowercase).collect();
    let hit = |label: &str, id: &str| {
        let hay = format!("{label} {id}").to_lowercase();
        words.iter().all(|w| hay.contains(w.as_str()))
    };
    let mut out: Vec<Item> = Vec::new();
    if let Some(node) = app.session.focus.or(app.session.sel.map(|s| s.1)) {
        out.extend(context_steps(app, node).into_iter().filter(|i| matches!(i, Item::Step { label, .. } if hit(label, ""))));
    }
    for (id, label, shortcut) in crate::UI_COMMANDS {
        if hit(label, id) {
            out.push(Item::Command { id: id.to_string(), label: label.to_string(), shortcut: shortcut.to_string() });
        }
    }
    // With a bullet focused or selected, its node commands lead.
    let mut cmds = app.session.registry.search(q);
    if app.session.focus.is_some() || app.session.sel.is_some() {
        cmds.sort_by_key(|c| !matches!(c.location, "Node" | "Supertag" | "Field" | "Format"));
    }
    for c in cmds {
        out.push(Item::Command { id: c.id.to_string(), label: c.label.to_string(), shortcut: c.shortcut.to_string() });
    }
    out.truncate(40);
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
        Item::Step { act, .. } => {
            app.acts.push(act);
            app.flush();
        }
    }
}

pub fn palette_key(app: &mut TanaApp, k: &str) -> bool {
    let Some(p) = app.palette.as_mut() else { return false };
    match k {
        "Escape" | "Mod+K" => app.palette = None,
        "Up" => {
            p.selected = p.selected.saturating_sub(1);
            p.scroll_to = true;
        }
        "Down" => {
            p.selected += 1;
            p.scroll_to = true;
        }
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
                        .event_filter(crate::KEEP_KEYS)
                        .id(egui::Id::new("palette-query"))
                        .font(FontId::proportional(17.0))
                        .hint_text("Enter command...")
                        .frame(egui::Frame::NONE)
                        .desired_width(w),
                );
                // An open overlay keeps the keyboard: a stray click must not leave typing with nowhere to go.
                if p.focus_once || !r.has_focus() {
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
                egui::ScrollArea::vertical().max_height(440.0).show(ui, |ui| {
                    for (i, item) in items.iter().enumerate() {
                        let (label, right) = match item {
                            Item::Command { label, shortcut, .. } => (label.clone(), pretty_shortcut(shortcut, app.mac)),
                            // Steps that open a picker next show ">" on the right.
                            Item::Step { label, act } => (label.clone(), if matches!(act, Act::Run(..)) { String::new() } else { ">".to_string() }),
                        };
                        let (rect, resp) = ui.allocate_exact_size(vec2(w, 28.0), Sense::click());
                        if i == p.selected || resp.hovered() {
                            ui.painter().rect_filled(rect, 5.0, t.hover);
                        }
                        let painter = ui.painter();
                        painter.text(rect.left_center() + vec2(12.0, 0.0), Align2::LEFT_CENTER, label, FontId::proportional(14.5), t.text);
                        painter.text(rect.right_center() - vec2(10.0, 0.0), Align2::RIGHT_CENTER, right, FontId::proportional(12.5), t.faint);
                        if i == p.selected && p.scroll_to {
                            resp.scroll_to_me(None);
                        }
                        if resp.clicked() {
                            chosen = Some(item.clone());
                        }
                    }
                });
            });
        });
    p.scroll_to = false;
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
            .menu_tags(p.node)
            .into_iter()
            .filter(|t| g.name(*t).to_lowercase().contains(&q))
            .map(|t| Choice::Existing(t, format!("#{}", g.name(t))))
            .collect(),
        PickMode::Field | PickMode::Column | PickMode::TemplateField | PickMode::OptionalField => g
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
            // Options from a supertag: every node with that tag (spec 03 §8).
            let source =
                p.field.and_then(|f| g.field_def(f)).filter(|d| d.field_type == tanacraft_core::FieldType::OptionsFromTag).and_then(|d| d.source_tag);
            let is_user = p.field.and_then(|f| g.field_def(f)).is_some_and(|d| d.field_type == tanacraft_core::FieldType::User);
            match (source, p.field) {
                (_, Some(_)) if is_user => g
                    .user_names()
                    .into_iter()
                    .filter(|n| !current.contains(n) && n.to_lowercase().contains(&q))
                    .map(|n| Choice::Existing(NodeId(0), n))
                    .collect(),
                (Some(tag), _) => g
                    .tagged(tag)
                    .into_iter()
                    .filter(|n| !g.in_trash(*n) && !current.contains(&g.text(*n)) && g.text(*n).to_lowercase().contains(&q))
                    .map(|n| Choice::Existing(n, g.text(n)))
                    .collect(),
                (None, Some(f)) => g
                    .field_options(f)
                    .into_iter()
                    .filter(|o| !current.contains(o) && o.to_lowercase().contains(&q))
                    .map(|o| Choice::Existing(NodeId(0), o))
                    .collect(),
                _ => Vec::new(),
            }
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
        PickMode::InlineRef | PickMode::MoveTo | PickMode::MoveLeaveRef | PickMode::OptionSource if q.is_empty() => app
            .session
            .recents
            .iter()
            .copied()
            .filter(|r| *r != p.node && g.get(*r).is_some())
            .take(8)
            .map(|n| Choice::Existing(n, export::node_line(g, n)))
            .collect(),
        PickMode::InlineRef | PickMode::OptionSource => {
            g.fuzzy(&q, 10).into_iter().filter(|n| *n != p.node).map(|n| Choice::Existing(n, export::node_line(g, n))).collect()
        }
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
        (PickMode::Tag, choice) => {
            // Inline `#name`: the typed text leaves the line (and trailing spaces with it).
            if let Some(h) = p.caret {
                let name: Vec<char> = app.session.graph.name(node).chars().collect();
                let end = (h + 1 + p.query.chars().count()).min(name.len());
                if name.get(h) == Some(&'#') {
                    let kept: String = name[..h].iter().collect::<String>().trim_end().to_string();
                    let rest: String = name[end..].iter().collect();
                    let caret = kept.chars().count();
                    let _ = app.exec("node.setName", json!({"node": node.0, "name": format!("{kept}{rest}"), "caret": caret}));
                }
            }
            match choice {
                Choice::Existing(t, _) => app.exec("tag.add", json!({"node": node.0, "tag": t.0})),
                Choice::Create(name) => app.exec("tag.add", json!({"node": node.0, "tag": name, "color": next_color(app)})),
            }
        }
        (PickMode::Field, c) => {
            let field = match c {
                Choice::Existing(f, _) => json!(f.0),
                Choice::Create(name) => json!(name),
            };
            // The row takes the place of the line `>` was typed on: after the sibling above it.
            let g = &app.session.graph;
            let after = p.line.and_then(|l| g.index_in_parent(l)).and_then(|i| i.checked_sub(1)).and_then(|i| g.children(node).get(i).copied());
            let mut params = json!({"node": node.0, "field": field});
            if let Some(a) = after {
                params["after"] = json!(a.0);
            }
            app.exec("field.add", params).inspect(|_| app.session.graph.set_expanded(node, true))
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
                    app.open_picker(PickMode::Field, parent, at);
                    Ok(json!({}))
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
        (PickMode::MoveTo | PickMode::MoveLeaveRef | PickMode::OptionSource, Choice::Create(_)) => Ok(json!({})),
        (PickMode::OptionSource, Choice::Existing(src, _)) => app.exec("field.addSource", json!({"field": node.0, "node": src.0})),
        (PickMode::Extend, Choice::Existing(base, _)) => app.exec("tag.extend", json!({"tag": node.0, "base": base.0})),
        (PickMode::TemplateField, Choice::Existing(f, _)) => app.exec("tag.addField", json!({"tag": node.0, "field": f.0})),
        (PickMode::TemplateField, Choice::Create(name)) => app.exec("tag.addField", json!({"tag": node.0, "field": name})),
        (PickMode::OptionalField, Choice::Existing(f, _)) => app.exec("tag.setOptionalField", json!({"tag": node.0, "field": f.0, "on": true})),
        (PickMode::OptionalField, Choice::Create(name)) => app.exec("tag.setOptionalField", json!({"tag": node.0, "field": name, "on": true})),
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
/// The source supertag of the field an options picker sets, if it takes options from one.
fn tag_source(app: &TanaApp, p: &crate::Picker) -> Option<NodeId> {
    let d = app.session.graph.field_def(p.field?)?;
    (d.field_type == tanacraft_core::FieldType::OptionsFromTag).then_some(d.source_tag).flatten()
}

fn next_color(app: &TanaApp) -> &'static str {
    let n = app.session.graph.tags().len();
    tanacraft_core::TAG_COLORS.get(2 + n % 12).copied().unwrap_or("grey")
}

/// The field name typed after `>` becomes a field row in place of the line `>` was typed on, and
/// the picker closes. Returns the owner and the field.
pub(crate) fn keep_field_name(app: &mut TanaApp) -> Option<(NodeId, NodeId)> {
    let p = app.picker.as_ref()?;
    let (q, node, line) = (p.query.trim().to_string(), p.node, p.line);
    let exact = picker_items(app).into_iter().find(|c| matches!(c, Choice::Existing(_, n) if n.eq_ignore_ascii_case(&q)));
    let picked = match &exact {
        Some(Choice::Existing(f, _)) => Some(*f),
        _ => None,
    };
    apply_choice(app, exact.unwrap_or(Choice::Create(q)));
    app.picker = None;
    if let Some(l) = line {
        drop(app.exec("node.delete", json!({"node": l.0})));
    }
    let f = picked.or_else(|| app.session.graph.get(node).and_then(|n| n.fields.last()).map(|e| e.field))?;
    Some((node, f))
}

pub fn picker_key(app: &mut TanaApp, k: &str) -> bool {
    let Some(p) = app.picker.as_mut() else { return false };
    match k {
        // A field name typed after `>` is kept: Escape leaves the name and selects the new field
        // row, so Tab goes on to its value (DIFFS N15, N28).
        "Escape" if p.mode == PickMode::Field && p.line.is_some() && !p.query.trim().is_empty() => {
            app.field_sel = keep_field_name(app);
            if app.field_sel.is_some() {
                (app.session.focus, app.session.sel) = (None, None);
            }
        }
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
                    let line = app.picker.as_ref().and_then(|p| p.line);
                    apply_choice(app, c);
                    // After `>`: Tab goes on to the value, Enter starts a new node below (spec 02 §3).
                    if mode == PickMode::Field {
                        let f = picked.or_else(|| app.session.graph.get(node).and_then(|n| n.fields.last()).map(|e| e.field));
                        match (k, f, line) {
                            ("Tab", Some(f), l) => {
                                if let Some(l) = l {
                                    // The line became the field row.
                                    drop(app.exec("node.delete", json!({"node": l.0})));
                                }
                                app.acts.push(Act::EditField(node, f, 0, String::new()));
                            }
                            // The empty line stays, just below the new field row.
                            ("Enter", _, Some(l)) => drop(app.exec("focus.set", json!({"node": l.0}))),
                            ("Enter", _, None) => match app.exec("node.add", json!({"parent": node.0, "index": 0, "name": ""})) {
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
        PickMode::OptionalField => "Add an optional field",
        PickMode::InlineRef => "Reference a node inline",
        PickMode::MoveTo => "Move to",
        PickMode::MoveLeaveRef => "Move and leave reference to",
        PickMode::OptionSource => "Source of options",
    };
    // Above the line when there is no room below it (spec 02 §11).
    let est = 26.0 * items.len().max(1) as f32 + if p.mode == PickMode::Tag && p.caret.is_some() { 20.0 } else { 60.0 };
    let at = if p.anchor.y + est > ctx.content_rect().bottom() { egui::pos2(p.anchor.x, p.anchor.y - est - 30.0) } else { p.anchor };
    // A `>` field row sits on the line itself.
    let at = if p.mode == PickMode::Field && p.line.is_some() { at - vec2(16.0, 30.0) } else { at };
    let area = egui::Area::new(egui::Id::new("picker")).order(Order::Foreground).fixed_pos(at).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).fill(t.panel).corner_radius(8.0).inner_margin(8.0).show(ui, |ui| {
            ui.set_width(300.0);
            // The `#` menu filters by what is typed in the line itself (spec 02 §11); other menus
            // have their own search box.
            let inline = p.mode == PickMode::Tag && p.caret.is_some();
            // `>` on a line: the line itself reads as a field row being named, name on the left and
            // an empty value on the right, with matching fields under it (spec 02 §11).
            let field_row = p.mode == PickMode::Field && p.line.is_some();
            if field_row {
                let mut r = None;
                egui::Frame::new().fill(t.field_bg).corner_radius(6.0).inner_margin(egui::Margin::symmetric(10, 5)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        r = Some(
                            ui.add(
                                egui::TextEdit::singleline(&mut p.query)
                                    .event_filter(crate::KEEP_KEYS)
                                    .id(egui::Id::new("picker-query"))
                                    .frame(egui::Frame::NONE)
                                    .hint_text("Field name")
                                    .desired_width(130.0),
                            ),
                        );
                        ui.add_space(12.0);
                        ui.label(RichText::new("Empty").size(14.0).color(t.faint));
                    });
                });
                if let Some(r) = r {
                    // An open overlay keeps the keyboard: a stray click must not leave typing with nowhere to go.
                    if p.focus_once || !r.has_focus() {
                        r.request_focus();
                        p.focus_once = false;
                    }
                    if r.changed() {
                        p.selected = 0;
                    }
                }
            } else if !inline {
                ui.label(RichText::new(title).size(12.0).color(t.faint));
                let r = ui.add(
                    egui::TextEdit::singleline(&mut p.query)
                        .event_filter(crate::KEEP_KEYS)
                        .id(egui::Id::new("picker-query"))
                        .hint_text("Type to search")
                        .desired_width(300.0),
                );
                // An open overlay keeps the keyboard: a stray click must not leave typing with nowhere to go.
                if p.focus_once || !r.has_focus() {
                    r.request_focus();
                    p.focus_once = false;
                }
                if r.changed() {
                    p.selected = 0;
                }
            } else if items.is_empty() {
                ui.label(RichText::new("Type a supertag name").size(12.5).color(t.faint));
            }
            // The tag menu shows "Create and apply" on top (spec 02 §11); Enter still picks the
            // highlighted match, Cmd+Enter creates.
            let mut order: Vec<usize> = (0..items.len()).collect();
            if p.mode == PickMode::Tag && matches!(items.last(), Some(Choice::Create(_))) {
                order.rotate_right(1);
            }
            for i in order {
                let c = &items[i];
                let (label, color) = match c {
                    Choice::Existing(id, name) => {
                        let color = app.session.graph.tag_def(*id).map(|d| theme::tag_color(&d.color, app.dark).0).unwrap_or(t.text);
                        (name.clone(), color)
                    }
                    Choice::Create(name) if p.mode == PickMode::Tag => (format!("Create and apply \"#{name}\""), t.dim),
                    Choice::Create(name) if p.mode == PickMode::Choice && tag_source(app, &p).is_some() => {
                        (format!("Tag as #{}: {name}", tag_source(app, &p).map(|t| app.session.graph.name(t).to_string()).unwrap_or_default()), t.dim)
                    }
                    Choice::Create(name) => (format!("Create \"{name}\""), t.dim),
                };
                let create = matches!(c, Choice::Create(_));
                let (rect, resp) = ui.allocate_exact_size(vec2(300.0, 26.0), Sense::click());
                if i == p.selected || resp.hovered() {
                    ui.painter().rect_filled(rect, 4.0, t.hover);
                }
                let lr = ui.painter().text(rect.left_center() + vec2(8.0, 0.0), Align2::LEFT_CENTER, label, FontId::proportional(14.0), color);
                // A supertag's description shows after its name, in grey.
                if let Choice::Existing(id, _) = c
                    && app.session.graph.tag_def(*id).is_some()
                {
                    let g = &app.session.graph;
                    let mut d = g.get(*id).map(|n| n.description.clone()).unwrap_or_default();
                    // With several workspaces, which one the supertag belongs to.
                    if g.workspaces().len() > 1 {
                        let ws = g.name(g.workspace_of(*id)).to_string();
                        d = if d.is_empty() { ws } else { format!("{ws} · {d}") };
                    }
                    if !d.is_empty() {
                        let room = (rect.right() - lr.right() - 16.0).max(0.0);
                        let short: String = d.chars().take((room / 6.5) as usize).collect();
                        ui.painter().text(lr.right_center() + vec2(8.0, 0.0), Align2::LEFT_CENTER, short, FontId::proportional(12.0), t.faint);
                    }
                }
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
    } else if ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|at| !area.response.rect.contains(at))) {
        // A click elsewhere closes the menu like Escape (a typed `>` field name is kept).
        picker_key(app, "Escape");
    }
}

// ---- shortcuts panel and description ------------------------------------------------------------

pub fn help_panel(app: &mut TanaApp, ui: &mut Ui) {
    let t = app.tokens();
    ui.set_max_width(ui.available_width());
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
                    // The sheet shows the first of alternative keys (the Help page lists them all).
                    let first = sc.split(" / ").next().unwrap_or(&sc);
                    ui.add(egui::Label::new(RichText::new(pretty_shortcut(first, app.mac)).size(13.0).color(t.dim)).truncate());
                });
                ui.add(egui::Label::new(RichText::new(label).size(13.5)).truncate());
            });
        }
    });
}

// ---- date picker and quick add ------------------------------------------------------------------

/// The picked day plus the typed time range, as a field value text.
fn date_with_time(dp: &crate::DatePick) -> String {
    if let Some(end) = dp.end.as_ref().filter(|e| !e.trim().is_empty()) {
        return format!("{}/{}", dp.buf.trim(), end.trim());
    }
    let (a, b) = (dp.time.0.trim(), dp.time.1.trim());
    match (a.is_empty(), b.is_empty()) {
        (true, _) => dp.buf.trim().to_string(),
        (false, true) => format!("{} {a}", dp.buf.trim()),
        (false, false) => format!("{} {a}-{b}", dp.buf.trim()),
    }
}

pub fn date_picker(app: &mut TanaApp, ctx: &egui::Context) {
    let Some(mut dp) = app.date_pick.take() else { return };
    let t = app.tokens();
    let today = app.session.today;
    let mut pick: Option<String> = None;
    let mut close = false;
    let area = egui::Area::new(egui::Id::new("date-picker")).order(Order::Foreground).fixed_pos(dp.anchor).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).fill(t.panel).corner_radius(8.0).inner_margin(10.0).show(ui, |ui| {
            ui.set_width(286.0);
            // Date fields: pick a day, optionally a time range, then OK (spec 03 §8). Go to date
            // and Insert date take the day at once.
            let is_field = dp.field.is_some();
            let r = ui.add(egui::TextEdit::singleline(&mut dp.buf).hint_text("YYYY-MM-DD, today, tomorrow").desired_width(286.0));
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                pick = Some(if is_field { date_with_time(&dp) } else { dp.buf.clone() });
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
                ui.allocate_exact_size(vec2(26.0, 18.0), Sense::hover());
                for d in ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"] {
                    let (r, _) = ui.allocate_exact_size(vec2(cell, 18.0), Sense::hover());
                    ui.painter().text(r.center(), Align2::CENTER_CENTER, d, FontId::proportional(11.5), t.faint);
                }
            });
            for w in 0..6 {
                ui.horizontal(|ui| {
                    // ISO week number at the start of each row.
                    let (wr, _) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::hover());
                    ui.painter().text(
                        wr.center(),
                        Align2::CENTER_CENTER,
                        format!("W{}", start.add_days(w * 7).iso_week().1),
                        FontId::proportional(10.5),
                        t.faint,
                    );
                    for k in 0..7 {
                        let d = start.add_days(w * 7 + k);
                        let (r, resp) = ui.allocate_exact_size(vec2(cell, 26.0), Sense::click());
                        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, d.to_string()));
                        let ds = d.to_string();
                        let selected = dp.buf == ds || dp.end.as_deref() == Some(ds.as_str());
                        // Days inside a span are shaded (ISO dates compare as text).
                        let inside = dp.end.as_deref().is_some_and(|e| dp.buf.as_str() < ds.as_str() && ds.as_str() < e);
                        if selected {
                            ui.painter().rect_filled(r.shrink(2.0), 5.0, t.accent);
                        } else if inside {
                            ui.painter().rect_filled(r.shrink(2.0), 5.0, t.accent.gamma_multiply(0.3));
                        } else if d == today {
                            ui.painter().circle_filled(r.center(), 11.0, egui::Color32::from_rgb(226, 92, 64));
                        } else if resp.hovered() {
                            ui.painter().rect_filled(r.shrink(2.0), 5.0, t.hover);
                        }
                        let color = if selected || d == today {
                            egui::Color32::WHITE
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
                            if is_field && dp.picking_end {
                                if ds.as_str() > dp.buf.as_str() {
                                    dp.end = Some(ds.clone());
                                    dp.picking_end = false;
                                }
                            } else if is_field {
                                dp.buf = d.to_string();
                            } else {
                                pick = Some(d.to_string());
                            }
                        }
                    }
                });
            }
            if is_field {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Time").size(12.5).color(t.dim));
                    ui.add(egui::TextEdit::singleline(&mut dp.time.0).hint_text("-- : --").desired_width(52.0));
                    ui.label(RichText::new("to").size(12.5).color(t.dim));
                    ui.add(egui::TextEdit::singleline(&mut dp.time.1).hint_text("end").desired_width(52.0));
                });
                // A span of days: "Add end", then click the last day (spec 03 §8).
                ui.horizontal(|ui| match dp.end.clone() {
                    Some(e) => {
                        ui.label(RichText::new(format!("Ends {e}")).size(12.5).color(t.dim));
                        if ui.small_button("x").on_hover_text("Remove the end").clicked() {
                            dp.end = None;
                        }
                    }
                    None if dp.picking_end => {
                        ui.label(RichText::new("Click the last day").size(12.5).color(t.dim));
                    }
                    None => {
                        if ui.button("Add end").clicked() && !dp.buf.trim().is_empty() {
                            dp.picking_end = true;
                        }
                    }
                });
            }
            ui.horizontal(|ui| {
                if ui.button("Today").clicked() {
                    if is_field {
                        dp.buf = today.to_string();
                    } else {
                        pick = Some(today.to_string());
                    }
                }
                if ui.button("Clear").clicked() {
                    pick = Some(String::new());
                }
                if is_field && ui.button("OK").clicked() {
                    pick = Some(date_with_time(&dp));
                }
            });
        });
    });
    let outside = dp.drawn && ctx.input(|i| i.pointer.any_click() && i.pointer.interact_pos().is_some_and(|p| !area.response.rect.contains(p)));
    dp.drawn = true;
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) || outside {
        close = true;
    }
    match pick {
        Some(v) => {
            let r = match dp.field {
                Some(f) => app.exec("field.set", json!({"node": dp.node.0, "field": f.0, "index": dp.index, "value": v})),
                None if dp.insert.is_some() => {
                    let (node, caret) = dp.insert.unwrap_or((dp.node, 0));
                    app.exec("edit.insertDate", json!({"node": node.0, "caret": caret, "date": v}))
                }
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
                    // The box takes the keyboard from anything else (a page title...) while open.
                    let count = lines.len();
                    let in_box = ui.ctx().memory(|m| m.focused()).is_some_and(|f| (0..count).any(|j| egui::Id::new(("quick-add-input", j)) == f));
                    for (i, line) in lines.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(12.0, 22.0), Sense::hover());
                            ui.painter().circle_filled(r.center(), 3.0, t.dim);
                            let resp = ui.add(
                                egui::TextEdit::singleline(line)
                                    .event_filter(crate::KEEP_KEYS)
                                    .id(egui::Id::new(("quick-add-input", i)))
                                    .frame(egui::Frame::NONE)
                                    .hint_text("Type a node, #tag at the end")
                                    .desired_width(w - 20.0),
                            );
                            if i == last && (enter || !in_box) {
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
        save_quick_add(app, &lines);
    } else if !close {
        app.quick_add = Some(lines);
    }
}

/// Adds the quick add lines to today's page; a trailing `#word` tags the node.
pub(crate) fn save_quick_add(app: &mut TanaApp, lines: &[String]) {
    let mut added = 0;
    for text in lines.iter().map(|l| l.trim()).filter(|l| !l.is_empty()) {
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
                let r = ui.add(
                    egui::TextEdit::singleline(&mut p.query)
                        .event_filter(crate::KEEP_KEYS)
                        .id(egui::Id::new("search-open-query"))
                        .hint_text("Search...")
                        .desired_width(w),
                );
                // An open overlay keeps the keyboard: a stray click must not leave typing with nowhere to go.
                if p.focus_once || !r.has_focus() {
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
                    // A content snippet for text matches: the description or a child line with the
                    // words, at most two lines (spec 05 §4).
                    let snippet = export::snippet(g, *id, &p.query);
                    let (rect, resp) = ui.allocate_exact_size(vec2(w, if snippet.is_some() { 76.0 } else { 40.0 }), Sense::click());
                    if i == p.selected || resp.hovered() {
                        ui.painter().rect_filled(rect, 5.0, if i == p.selected { t.accent.gamma_multiply(0.25) } else { t.hover });
                    }
                    let painter = ui.painter();
                    painter.text(rect.left_top() + vec2(10.0, 4.0), Align2::LEFT_TOP, crumbs.join(" / "), FontId::proportional(11.5), t.faint);
                    painter.text(rect.left_top() + vec2(10.0, 19.0), Align2::LEFT_TOP, export::node_line(g, *id), FontId::proportional(14.5), t.text);
                    if let Some(sn) = snippet {
                        let mut job = egui::text::LayoutJob::simple(sn, FontId::proportional(12.5), t.dim, w - 20.0);
                        job.wrap.max_rows = 2;
                        let galley = ui.fonts_mut(|f| f.layout_job(job));
                        ui.painter().galley(rect.left_top() + vec2(10.0, 40.0), galley, t.dim);
                    }
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

/// "Used elsewhere" question before deleting a node that other places point at (DIFFS N20).
pub fn confirm_delete(app: &mut TanaApp, ctx: &egui::Context) {
    let Some((_, name, used)) = app.confirm_delete.clone() else { return };
    let t = app.tokens();
    let screen = ctx.content_rect();
    let w = 420.0_f32.min(screen.width() - 32.0);
    let mut answer = None;
    egui::Area::new(egui::Id::new("confirm-delete"))
        .order(Order::Foreground)
        .fixed_pos(egui::pos2(screen.center().x - w / 2.0, screen.top() + 140.0))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).fill(t.panel).corner_radius(10.0).inner_margin(14.0).show(ui, |ui| {
                ui.set_width(w);
                let places = if used == 1 { "1 other place".to_string() } else { format!("{used} other places") };
                let name = if name.trim().is_empty() { "This node".to_string() } else { format!("\"{name}\"") };
                ui.label(RichText::new(format!("{name} is referenced in {places}. Delete it?")).size(14.5));
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        answer = Some(None);
                    }
                    if ui.button("Show references").clicked() {
                        answer = Some(Some(false));
                    }
                    if ui.button(RichText::new("Delete anyway").color(egui::Color32::from_rgb(220, 70, 60))).clicked() {
                        answer = Some(Some(true));
                    }
                });
                ui.label(RichText::new("Enter deletes, Escape cancels").size(11.5).color(t.faint));
            });
        });
    if let Some(a) = answer {
        app.answer_delete(a);
    }
}
