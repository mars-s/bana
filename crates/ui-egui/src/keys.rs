//! Keyboard: turns key presses into shortcut strings (`Mod+Shift+Up`) and dispatches them to
//! commands before any text field sees them.

use egui::{Event, Key, Modifiers};
use serde_json::json;
use tanacraft_core::NodeId;

use crate::TanaApp;

/// `Mod` is Cmd on macOS and Ctrl elsewhere; `Ctrl` is the Control key on macOS.
pub fn shortcut(key: Key, m: Modifiers, mac: bool) -> String {
    let mut s = String::new();
    if mac && m.ctrl {
        s.push_str("Ctrl+");
    }
    if m.command || (mac && m.mac_cmd) {
        s.push_str("Mod+");
    }
    if m.alt {
        s.push_str("Alt+");
    }
    if m.shift {
        s.push_str("Shift+");
    }
    s.push_str(key_name(key));
    s
}

fn key_name(key: Key) -> &'static str {
    match key {
        Key::ArrowUp => "Up",
        Key::ArrowDown => "Down",
        Key::ArrowLeft => "Left",
        Key::ArrowRight => "Right",
        Key::Period => ".",
        Key::Comma => ",",
        Key::OpenBracket => "[",
        Key::CloseBracket => "]",
        Key::Backslash => "\\",
        Key::Slash => "/",
        other => other.name(),
    }
}

/// Whether a registry shortcut (`A / B`) matches the pressed one. Off macOS, `Ctrl` means `Mod`.
pub fn matches(spec: &str, pressed: &str, mac: bool) -> bool {
    spec.split(" / ").filter(|k| !k.is_empty()).any(|k| {
        let k = if mac { k.to_string() } else { k.replace("Ctrl+Mod+", "Mod+").replace("Ctrl+", "Mod+") };
        k.eq_ignore_ascii_case(pressed)
    })
}

/// Handles this frame's key presses; the ones used are removed so text fields never see them.
pub fn handle(app: &mut TanaApp, ctx: &egui::Context) {
    let events = ctx.input(|i| i.events.clone());
    let mut used = Vec::new();
    for (i, ev) in events.iter().enumerate() {
        // Cmd+C on selected nodes copies them (plain text and HTML); pasting that text back
        // inserts references to the same nodes.
        if matches!(ev, Event::Copy) && app.session.sel.is_some() && (app.session.focus.is_none() || app.edit.selection_empty) {
            let ids = app.session.selected();
            let text = tanacraft_core::export::to_plain(&app.session.graph, &ids);
            // Formatted HTML too, where the platform clipboard takes one (spec 02 §12, DIFFS N21).
            let html = tanacraft_core::export::to_html(&app.session.graph, &ids);
            match app.rich_clipboard.as_mut() {
                Some(put) => put(&html, &text),
                None => ctx.copy_text(text.clone()),
            }
            app.node_clip = Some((text, ids));
            used.push(i);
            continue;
        }
        if let Event::Paste(text) = ev
            && let Some((clip, ids)) = app.node_clip.clone()
            && clip.trim() == text.trim()
            && app.picker.is_none()
            && app.palette.is_none()
        {
            let anchor = app.session.focus.or(app.session.sel.map(|s| s.1));
            if let Some(a) = anchor {
                let parent = app.session.graph.parent(a);
                let mut index = app.session.graph.index_in_parent(a).map(|i| i + 1);
                // Nodes already in this list are skipped, with a notice (spec 02 §12).
                let present: Vec<NodeId> =
                    parent.map(|p| app.session.graph.children(p).iter().map(|c| app.session.graph.resolve(*c)).collect()).unwrap_or_default();
                let skipped = ids.iter().filter(|n| present.contains(n)).count();
                if skipped > 0 {
                    app.toast(format!("{skipped} node(s) were already here and were skipped"));
                }
                // Pasting into an empty line: the first reference takes that line's place (DIFFS N12).
                let g = &app.session.graph;
                let mut replace =
                    Some(a).filter(|a| app.session.focus == Some(*a) && g.get(*a).is_some_and(|x| x.name.is_empty() && x.children.is_empty()));
                for n in ids.into_iter().filter(|n| !present.contains(n)) {
                    if let Some(p) = parent {
                        let mut params = json!({"target": n.0, "parent": p.0, "index": index});
                        if let Some(r) = replace.take() {
                            params["replace"] = json!(r.0);
                        }
                        let r = app.exec("node.reference", params);
                        index = index.map(|i| i + 1);
                        if let Err(e) = r {
                            app.session.status = e;
                        }
                    }
                }
            }
            used.push(i);
            continue;
        }
        // Multi-line paste into a bullet becomes nodes (spec 02 §12).
        if let Event::Paste(text) = ev
            && app.session.focus.is_some()
            && app.picker.is_none()
            && app.palette.is_none()
            && text.contains('\n')
        {
            if let Err(e) = app.exec("node.paste", json!({"text": text})) {
                app.session.status = e;
            }
            used.push(i);
            continue;
        }
        if let Event::Key { key, pressed: true, modifiers, .. } = ev
            && on_key(app, ctx, shortcut(*key, *modifiers, app.mac))
        {
            used.push(i);
        }
    }
    if !used.is_empty() {
        ctx.input_mut(|inp| {
            let mut idx = 0;
            inp.events.retain(|_| {
                let keep = !used.contains(&idx);
                idx += 1;
                keep
            });
        });
    }
}

fn on_key(app: &mut TanaApp, ctx: &egui::Context, k: String) -> bool {
    // Overlays first.
    if app.palette.is_some() {
        return crate::overlays::palette_key(app, &k);
    }
    if app.picker.is_some() {
        return crate::overlays::picker_key(app, &k);
    }
    if app.search_open.is_some() {
        return crate::overlays::search_open_key(app, &k);
    }
    // The "used elsewhere" question: Enter deletes anyway, Escape cancels.
    if app.confirm_delete.is_some() {
        match k.as_str() {
            "Enter" => app.answer_delete(Some(true)),
            "Escape" => app.answer_delete(None),
            _ => {}
        }
        return true;
    }
    if app.quick_add.is_some() || app.date_pick.is_some() {
        return false; // the overlay reads Enter and Escape itself
    }
    if let Some((node, text)) = app.desc_edit.clone() {
        // Escape and Up leave the description (keeping it); Up goes back to the name.
        if k == "Escape" || k == "Up" {
            app.desc_edit = None;
            let is_ref = app.session.graph.get(node).is_some_and(|n| n.reference_target().is_some());
            let cmd = if is_ref { "node.contextNote" } else { "node.setDescription" };
            app.exec(cmd, json!({"node": node.0, "text": text})).ok();
            if k == "Up" {
                app.exec("focus.set", json!({"node": node.0, "caret": usize::MAX})).ok();
            }
            return true;
        }
        return false;
    }
    if let Some(fe) = app.field_edit.clone() {
        match k.as_str() {
            // Escape, Up and Down keep the typed value (DIFFS N15).
            "Escape" | "Up" | "Down" => app.leave_field(&k),
            // Enter keeps the value and opens another value line in the same field (spec 02 §3).
            "Enter" if !fe.buf.trim().is_empty() => {
                app.commit_field_edit();
                {
                    let owner = app.session.graph.resolve(fe.node);
                    let index = app.session.graph.get(owner).map_or(fe.index + 1, |n| n.field_values(fe.field).len());
                    app.start_field_edit(crate::FieldEdit { index, buf: String::new(), focus_once: true, ..fe });
                }
            }
            // Shift+Tab moves the value out of the field: it becomes a line under the field row,
            // and an empty value line becomes an empty line there (DIFFS N16, N17). Enter on an
            // empty value line does the same.
            "Shift+Tab" | "Enter" => {
                app.field_edit = None;
                let p = json!({"node": fe.node.0, "field": fe.field.0, "index": fe.index, "text": fe.buf});
                if let Err(e) = app.exec("field.valueToNode", p) {
                    app.session.status = e;
                }
            }
            _ => return false,
        }
        return true;
    }
    // Mod+1..8 go to tab N, Mod+9 to the last tab (spec 09).
    if let Some(d) = k.strip_prefix("Mod+").and_then(|d| d.parse::<usize>().ok()).filter(|d| (1..=9).contains(d)) {
        let i = if d == 9 { app.tabs.len().saturating_sub(1) } else { d - 1 };
        app.switch_tab(i);
        return true;
    }
    // Custom keybindings first (Customize keybindings, spec 09).
    if let Some(cmd) = app.keymap.iter().find(|(sc, _)| matches(sc, &k, app.mac)).map(|(_, c)| c.clone()) {
        if let Err(e) = app.exec(&cmd, json!({})) {
            app.session.status = e;
        }
        return true;
    }
    for (id, _, sc) in crate::UI_COMMANDS {
        if matches(sc, &k, app.mac) {
            app.exec(id, json!({})).ok();
            return true;
        }
    }
    let editing = app.session.focus.is_some();
    // (egui itself drops focus on Escape before keys reach here, hence "last frame".)
    let title_focused = ctx.memory(|m| m.focused() == Some(app.title_id) || k == "Escape" && m.had_focus_last_frame(app.title_id));
    if title_focused {
        if k == "Enter" || k == "Shift+Enter" || k == "Down" {
            app.exec("node.enter", json!({"node": app.session.zoom.0})).ok();
            ctx.memory_mut(|m| m.surrender_focus(app.title_id));
            return true;
        }
        // Escape leaves the title for the first line (selected), or stays when there is none.
        if k == "Escape" {
            if let Some(r) = app.session.rows().first().filter(|_| app.outline_shown()) {
                let first = r.id;
                ctx.memory_mut(|m| m.surrender_focus(app.title_id));
                app.exec("select.set", json!({"from": first.0})).ok();
            }
            return true;
        }
        if !k.contains("Mod+") {
            return false;
        }
    }
    let selecting = app.session.sel.is_some();
    match k.as_str() {
        "Escape" if selecting => {
            app.session.sel = None;
            return true;
        }
        // With nodes selected: Enter adds a sibling below, Backspace/Delete delete (spec 02 §8).
        "Enter" if selecting => {
            let head = app.session.sel.map(|s| s.1);
            app.session.sel = None;
            if let Some(h) = head {
                app.exec("node.insert", json!({"node": h.0})).ok();
            }
            return true;
        }
        "Backspace" | "Delete" if selecting => {
            // The line that takes the deleted lines' place is selected next, so the keyboard
            // still has somewhere to go. A failed delete keeps the selection.
            let rows = app.session.rows();
            let first = app.session.sel.and_then(|(a, h)| rows.iter().position(|r| r.id == a || r.id == h));
            match app.exec("node.delete", json!({})) {
                Ok(v) if v.get("confirm").is_some() => {}
                Ok(_) => {
                    app.session.sel = None;
                    let rows = app.session.rows();
                    if let Some(r) = first.and_then(|i| rows.get(i).or(rows.last())) {
                        app.exec("select.set", json!({"from": r.id.0})).ok();
                    }
                }
                Err(e) => app.session.status = e,
            }
            return true;
        }
        // Up / Down leave selection mode with the caret in the neighbouring line (spec 02 §8).
        "Up" | "Down" if selecting => {
            let rows = app.session.rows();
            let picked: Vec<usize> = app
                .session
                .sel
                .map(|(a, h)| rows.iter().enumerate().filter(|(_, r)| r.id == a || r.id == h).map(|(i, _)| i).collect())
                .unwrap_or_default();
            app.session.sel = None;
            let up = k == "Up";
            let at = if up { picked.iter().min().and_then(|i| i.checked_sub(1)) } else { picked.iter().max().map(|i| i + 1) };
            let fallback = picked.first().copied();
            if let Some(r) = at.and_then(|i| rows.get(i)).or_else(|| fallback.and_then(|i| rows.get(i))) {
                app.exec("focus.set", json!({"node": r.id.0, "caret": if up { usize::MAX } else { 0 }})).ok();
            }
            return true;
        }
        // Alt+Cmd+Down on a reference: notes for this context, under the line (spec 01 §3).
        "Mod+Alt+Down" if app.session.focus.is_some_and(|f| app.session.graph.get(f).is_some_and(|n| n.reference_target().is_some())) => {
            if let Some(f) = app.session.focus {
                let note = app.session.graph.get(f).map(|n| n.description.clone()).unwrap_or_default();
                app.desc_edit = Some((f, note));
                app.desc_focus = true;
                app.session.focus = None;
            }
            return true;
        }
        // Cmd+. zooms in and puts the caret in the new page's title (spec 02 §7).
        "Mod+." if editing => {
            match app.exec("nav.zoomIn", json!({})) {
                // Day, week and year titles are fixed: the caret goes to the first line instead.
                Ok(_) if !crate::outline::title_editable(&app.session.graph, app.session.zoom) => {
                    let zoom = app.session.zoom;
                    app.keyboard_home(zoom);
                }
                Ok(_) => app.focus_title = true,
                Err(e) => app.session.status = e,
            }
            return true;
        }
        // Escape leaves text editing and selects the node; Escape again clears.
        "Escape" if editing => {
            if let Some(f) = app.session.focus {
                app.exec("select.set", json!({"from": f.0})).ok();
            }
            return true;
        }
        "Delete" if editing => {
            let len = app.session.focus.map(|f| app.session.graph.name(f).chars().count()).unwrap_or(0);
            if app.session.caret >= len && app.edit.selection_empty {
                app.exec("node.joinNext", json!({})).ok();
                return true;
            }
            return false;
        }
        // Left at the start / Right at the end jump to the neighbouring line (spec 02 §5).
        "Left" | "Right" if editing && app.edit.selection_empty => {
            let Some(f) = app.session.focus else { return false };
            let len = app.session.graph.name(f).chars().count();
            let left = k == "Left";
            if (left && app.session.caret > 0) || (!left && app.session.caret < len) {
                return false;
            }
            let rows = app.session.rows();
            let at = rows.iter().position(|r| r.id == f);
            let next = at.and_then(|i| if left { i.checked_sub(1) } else { Some(i + 1) }).and_then(|i| rows.get(i)).map(|r| r.id);
            if let Some(n) = next {
                app.exec("focus.set", json!({"node": n.0, "caret": if left { usize::MAX } else { 0 }})).ok();
                return true;
            }
            return false;
        }
        // Home / End: start / end of the whole node, also when its text wraps (spec 02 §5).
        "Home" | "End" | "Shift+Home" | "Shift+End" if editing => {
            let to = if k.ends_with("Home") { 0 } else { app.edit.buf.chars().count() };
            let anchor = if k.starts_with("Shift+") { app.session.anchor } else { to };
            app.edit.set_cursor = Some((anchor, to));
            app.session.caret = to;
            return true;
        }
        // Cmd+A selects the text first, then (all text already selected) every bullet.
        "Mod+A" if editing => {
            let len = app.session.focus.map(|f| app.session.graph.name(f).chars().count()).unwrap_or(0);
            let all = app.session.anchor.min(app.session.caret) == 0 && app.session.anchor.max(app.session.caret) == len;
            if !all {
                return false;
            }
            app.exec("select.all", json!({})).ok();
            return true;
        }
        // Up from the first bullet goes into the page title (spec 02 §5).
        // (Pages whose title is not editable, like days, keep the caret where it is.)
        "Up" if editing
            && app.session.rows().first().map(|r| r.id) == app.session.focus
            && crate::outline::title_editable(&app.session.graph, app.session.zoom) =>
        {
            app.session.focus = None;
            app.focus_title = true;
            return true;
        }
        "Enter" if editing => {
            app.exec("node.enter", json!({"caret": app.session.caret})).ok();
            return true;
        }
        "Backspace" if editing => {
            if app.session.caret == 0 && app.edit.selection_empty {
                app.exec("node.backspace", json!({})).ok();
                return true;
            }
            return false;
        }
        // Down from a name with a description enters the description (spec 02 §13).
        "Down"
            if editing
                && app
                    .session
                    .focus
                    .is_some_and(|f| app.session.graph.get(app.session.graph.resolve(f)).is_some_and(|n| !n.description.is_empty())) =>
        {
            app.exec("ui.editDescription", json!({})).ok();
            return true;
        }
        "Up" | "Down" => {
            let id = if k == "Up" { "focus.up" } else { "focus.down" };
            app.exec(id, json!({})).ok();
            return true;
        }
        // Text keys are typed into the bullet; `#`, `>` and `@` open pickers from the text.
        _ => {}
    }
    let plain_text_key = !k.contains('+') && k.chars().count() == 1;
    if plain_text_key {
        return false;
    }
    let registry = app.session.registry.clone();
    let hit =
        registry.all().iter().find(|c| !c.shortcut.is_empty() && matches(c.shortcut, &k, app.mac) && !is_text_trigger(c.shortcut)).map(|c| c.id);
    match hit {
        Some(id) => {
            // Editing keys need a focused bullet; leave Tab alone otherwise (egui focus traversal).
            let needs_focus =
                matches!(id, "node.indent" | "node.outdent" | "node.enter" | "node.backspace" | "node.insert") || id.starts_with("format.");
            if needs_focus && !editing && !selecting {
                return false;
            }
            if let Err(e) = app.exec(id, json!({})) {
                app.session.status = e;
            }
            true
        }
        None => false,
    }
}

/// `#`, `>` and `@` are typed characters, handled from the text (see `outline`).
fn is_text_trigger(s: &str) -> bool {
    matches!(s, "#" | ">" | "@")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_strings() {
        let cmd_shift = Modifiers { command: true, mac_cmd: true, shift: true, ..Modifiers::NONE };
        assert_eq!(shortcut(Key::ArrowUp, cmd_shift, true), "Mod+Shift+Up");
        let ctrl_shift = Modifiers { ctrl: true, shift: true, ..Modifiers::NONE };
        assert_eq!(shortcut(Key::D, ctrl_shift, true), "Ctrl+Shift+D");
        assert!(matches("Ctrl+Shift+D", "Ctrl+Shift+D", true));
        // Off macOS Ctrl is reported as Mod.
        let ctrl_shift_pc = Modifiers { ctrl: true, command: true, shift: true, ..Modifiers::NONE };
        assert_eq!(shortcut(Key::D, ctrl_shift_pc, false), "Mod+Shift+D");
        assert!(matches("Ctrl+Shift+D", "Mod+Shift+D", false));
        assert!(matches("Ctrl+Mod+Down", "Mod+Down", false));
        assert!(!matches("Mod+Z", "Mod+Shift+Z", true));
    }
}
