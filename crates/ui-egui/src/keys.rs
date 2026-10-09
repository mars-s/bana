//! Keyboard: turns key presses into shortcut strings (`Mod+Shift+Up`) and dispatches them to
//! commands before any text field sees them.

use egui::{Event, Key, Modifiers};
use serde_json::json;

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
    if app.quick_add.is_some() || app.date_pick.is_some() {
        return false; // the overlay reads Enter and Escape itself
    }
    if app.field_edit.is_some() || app.desc_edit.is_some() {
        if k == "Escape" {
            app.field_edit = None;
            app.desc_edit = None;
            return true;
        }
        return false;
    }
    for (id, _, sc) in crate::UI_COMMANDS {
        if matches(sc, &k, app.mac) {
            app.exec(id, json!({})).ok();
            return true;
        }
    }
    let editing = app.session.focus.is_some();
    let title_focused = ctx.memory(|m| m.focused()) == Some(egui::Id::new("page-title"));
    if title_focused {
        if k == "Enter" || k == "Down" {
            app.exec("node.enter", json!({"node": app.session.zoom.0})).ok();
            ctx.memory_mut(|m| m.surrender_focus(egui::Id::new("page-title")));
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
            if let Err(e) = app.exec("node.delete", json!({})) {
                app.session.status = e;
            }
            app.session.sel = None;
            return true;
        }
        "Up" | "Down" if selecting => {
            let head = app.session.sel.map(|s| s.1);
            app.session.sel = None;
            if let Some(h) = head {
                app.exec("focus.set", json!({"node": h.0})).ok();
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
        // Up from the first bullet goes into the page title (spec 02 §5).
        "Up" if editing && app.session.rows().first().map(|r| r.id) == app.session.focus => {
            app.session.focus = None;
            ctx.memory_mut(|m| m.request_focus(egui::Id::new("page-title")));
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
