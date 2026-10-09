//! Tabs and panels (spec 07 §4-5). The session always holds the active panel's navigation state;
//! the others are kept here and swapped in while they are drawn.

use egui::{CornerRadius, Frame, Margin, RichText, Sense, Ui, vec2};
use tanacraft_commands::NavState;
use tanacraft_core::NodeId;

use crate::{TanaApp, outline};

#[derive(Debug, Clone)]
pub struct Tab {
    pub panels: Vec<NavState>,
    pub active: usize,
}

impl TanaApp {
    /// Saves the live session state into the active panel slot.
    pub(crate) fn store_active(&mut self) {
        let state = self.session.nav_state();
        if let Some(tab) = self.tabs.get_mut(self.tab)
            && let Some(p) = tab.panels.get_mut(tab.active)
        {
            *p = state;
        }
    }

    fn load_active(&mut self) {
        let state = self.tabs.get(self.tab).and_then(|t| t.panels.get(t.active)).cloned();
        if let Some(s) = state {
            self.session.set_nav_state(s);
        }
        self.edit_reset();
    }

    pub fn switch_tab(&mut self, i: usize) {
        if i >= self.tabs.len() || i == self.tab {
            return;
        }
        self.store_active();
        self.tab = i;
        self.load_active();
    }

    pub fn new_tab(&mut self, node: NodeId) {
        self.store_active();
        let nav = self.session.new_nav(node);
        self.tabs.push(Tab { panels: vec![nav], active: 0 });
        self.tab = self.tabs.len() - 1;
        self.load_active();
        let _ = self.session.open(node);
        self.session.recents.retain(|r| *r != node);
        self.session.recents.insert(0, node);
    }

    pub fn close_tab(&mut self, i: usize) {
        if self.tabs.len() <= 1 || i >= self.tabs.len() {
            return;
        }
        self.store_active();
        self.tabs.remove(i);
        if self.tab >= self.tabs.len() || self.tab > i || self.tab == i {
            self.tab = self.tab.saturating_sub(usize::from(self.tab >= i && self.tab > 0)).min(self.tabs.len() - 1);
        }
        self.load_active();
    }

    /// Opens `node` in a new panel to the right of the active one (Cmd+M).
    pub fn open_in_panel(&mut self, node: NodeId) {
        self.store_active();
        let nav = self.session.new_nav(node);
        if let Some(tab) = self.tabs.get_mut(self.tab) {
            let at = (tab.active + 1).min(tab.panels.len());
            tab.panels.insert(at, nav);
            tab.active = at;
        }
        self.load_active();
    }

    /// Shows `node` in the panel to the right of the active one (made when missing), keeping the
    /// active panel (a list view's detail, spec 04 §7).
    pub fn show_beside(&mut self, node: NodeId) {
        self.store_active();
        let nav = self.session.new_nav(node);
        if let Some(tab) = self.tabs.get_mut(self.tab) {
            let at = tab.active + 1;
            if at < tab.panels.len() {
                tab.panels[at] = nav;
            } else {
                tab.panels.push(nav);
            }
        }
        self.load_active();
    }

    pub fn close_panel(&mut self, i: usize) {
        self.store_active();
        if let Some(tab) = self.tabs.get_mut(self.tab)
            && tab.panels.len() > 1
            && i < tab.panels.len()
        {
            tab.panels.remove(i);
            if tab.active >= i && tab.active > 0 {
                tab.active -= 1;
            }
            tab.active = tab.active.min(tab.panels.len() - 1);
        }
        self.load_active();
    }

    /// Back to one panel (keeps the active one).
    pub fn single_panel(&mut self) {
        self.store_active();
        if let Some(tab) = self.tabs.get_mut(self.tab)
            && let Some(p) = tab.panels.get(tab.active).cloned()
        {
            tab.panels = vec![p];
            tab.active = 0;
        }
        self.load_active();
    }

    pub fn panel_count(&self) -> usize {
        self.tabs.get(self.tab).map(|t| t.panels.len()).unwrap_or(1)
    }

    /// Draws every panel of the active tab side by side. A panel becomes active when it is used.
    pub(crate) fn panels_ui(&mut self, ui: &mut Ui) {
        let t = self.tokens();
        let n = self.panel_count().max(1);
        let total = ui.available_rect_before_wrap();
        let gap = 8.0;
        let w = (total.width() - gap * (n as f32 - 1.0)) / n as f32;
        for i in 0..n {
            let rect = egui::Rect::from_min_size(total.min + vec2(i as f32 * (w + gap), 0.0), vec2(w, total.height()));
            let active = self.tabs.get(self.tab).map(|t| t.active).unwrap_or(0);
            let is_active = i == active;
            if !is_active {
                self.store_active();
                let mut state = self.tabs.get(self.tab).and_then(|t| t.panels.get(i)).cloned();
                if let Some(s) = state.as_mut() {
                    s.focus = None; // only the active panel edits text
                    self.session.set_nav_state(s.clone());
                }
            }
            self.drawing_other_panel = !is_active;
            self.panel_ix = i;
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).id_salt(("panel", i)));
            let pressed = ui.input(|inp| inp.pointer.primary_pressed() && inp.pointer.interact_pos().is_some_and(|p| rect.contains(p)));
            Frame::new().fill(t.panel).corner_radius(CornerRadius::same(10)).show(&mut child, |ui| {
                ui.set_min_size(rect.size());
                egui::ScrollArea::vertical().id_salt(("panel-scroll", i)).auto_shrink([false, false]).show(ui, |ui| {
                    if n > 1 {
                        ui.horizontal(|ui| {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.add_space(10.0);
                                if ui
                                    .add(egui::Label::new(RichText::new("x").color(t.dim)).sense(Sense::click()))
                                    .on_hover_text("Close panel")
                                    .clicked()
                                {
                                    self.acts.push(crate::Act::ClosePanel(i));
                                }
                            });
                        });
                    }
                    outline::panel(self, ui);
                });
            });
            self.drawing_other_panel = false;
            let used = pressed || !self.acts.is_empty();
            if is_active {
                self.flush();
            } else if used {
                // Activate this panel: its state is live in the session now.
                if let Some(tab) = self.tabs.get_mut(self.tab) {
                    tab.active = i;
                }
                self.edit_reset();
                self.flush();
            } else {
                // Put the panel's state back and restore the active one.
                let state = self.session.nav_state();
                if let Some(tab) = self.tabs.get_mut(self.tab)
                    && let Some(p) = tab.panels.get_mut(i)
                {
                    *p = state;
                }
                self.load_active();
            }
        }
        ui.allocate_rect(total, Sense::hover());
    }

    /// The tab strip in the top bar.
    pub(crate) fn tabs_ui(&mut self, ui: &mut Ui) {
        let t = self.tokens();
        self.store_active();
        let titles: Vec<String> = self
            .tabs
            .iter()
            .map(|tab| tab.panels.iter().map(|p| outline::page_title(&self.session, p.zoom)).collect::<Vec<_>>().join(" | "))
            .collect();
        let mut switch = None;
        let mut close = None;
        for (i, title) in titles.iter().enumerate() {
            let on = i == self.tab;
            let resp = Frame::new()
                .fill(if on { t.panel } else { t.sidebar })
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::symmetric(10, 4))
                .show(ui, |ui| {
                    ui.set_width(150.0);
                    ui.horizontal(|ui| {
                        ui.add(egui::Label::new(RichText::new(title).size(13.0).color(if on { t.text } else { t.dim })).truncate());
                        if titles.len() > 1 && ui.add(egui::Label::new(RichText::new("x").size(11.0).color(t.faint)).sense(Sense::click())).clicked()
                        {
                            close = Some(i);
                        }
                    });
                })
                .response
                .interact(Sense::click());
            if resp.clicked() {
                switch = Some(i);
            }
        }
        if ui.add(egui::Label::new(RichText::new("+").size(16.0).color(t.dim)).sense(Sense::click())).on_hover_text("Open in a new tab").clicked() {
            self.acts.push(crate::Act::Ui("ui.newTab"));
        }
        if let Some(c) = close {
            self.close_tab(c);
        } else if let Some(s) = switch {
            self.switch_tab(s);
        }
    }
}
