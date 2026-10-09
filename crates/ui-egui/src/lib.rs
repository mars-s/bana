//! TanaCraft's egui front end. Thin: it reads the [`Session`] and runs commands by id.
//!
//! Layout: a top bar (sidebar toggle, back/forward, the open page), a sidebar (Today, Supertags,
//! Home, Daily notes, Trash, the supertag list, recents), the outline panel (breadcrumbs, title, tag
//! chips, field rows, bullets) and overlays (command line, `#`/`>`/`@` pickers, shortcuts panel).
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod icons;
mod keys;
mod outline;
mod overlays;
mod schema_ui;
mod tabs;
pub mod theme;

pub use tabs::Tab;
mod views;

use egui::{Color32, CornerRadius, Frame, Margin, RichText, Sense, Ui, vec2};
use serde_json::{Value, json};
use tanacraft_commands::Session;
use tanacraft_core::{NodeId, PRODUCT_NAME};

use crate::icons::Icon;
use crate::theme::Tokens;

/// UI-only commands (no graph change), listed with the engine's in the command line and help.
pub const UI_COMMANDS: &[(&str, &str, &str)] = &[
    ("ui.commandLine", "Command line", "Mod+K"),
    ("ui.toggleSidebar", "Toggle sidebar (full, mini, hidden)", "Mod+\\"),
    ("ui.newTab", "New tab", "Mod+T"),
    ("ui.closeTab", "Close tab", "Mod+W"),
    ("ui.nextTab", "Next tab", "Ctrl+Tab"),
    ("ui.prevTab", "Previous tab", "Ctrl+Shift+Tab"),
    ("ui.openInPanel", "Open in new panel", "Mod+M"),
    ("ui.singlePanel", "Single panel", "Ctrl+Mod+M"),
    ("ui.shortcuts", "Keyboard shortcuts", "Mod+/"),
    ("ui.toggleTheme", "Switch light or dark theme", ""),
    ("ui.editDescription", "Add description", "Ctrl+I"),
    ("ui.quickAdd", "Quick add", "Mod+E"),
    ("ui.searchOpen", "Search and open", "Mod+S"),
    ("ui.configureSupertag", "Configure supertag", ""),
    ("ui.searchOptions", "Show or hide search conditions", ""),
];

/// The `#`, `>` and `@` pickers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PickMode {
    Tag,
    Field,
    Reference,
    /// A supertag condition for a search node.
    SearchTag,
    /// A field column for a view.
    Column,
    /// A value of an options field (`Picker::field`).
    Choice,
    /// The `/` insert menu on an empty line.
    Slash,
    /// Pick the supertag of a new search node ("List of #tag").
    SearchCreate,
    /// A supertag that `node` (a supertag) extends.
    Extend,
    /// A field for the template of `node` (a supertag).
    TemplateField,
}

#[derive(Debug, Clone)]
pub struct Picker {
    pub mode: PickMode,
    /// Tag: the node to tag. Field: the node that gets the field. Reference: the empty node to replace.
    pub node: NodeId,
    pub query: String,
    pub selected: usize,
    pub anchor: egui::Pos2,
    /// The field, for `Choice`.
    pub field: Option<NodeId>,
    focus_once: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Palette {
    pub query: String,
    pub selected: usize,
    focus_once: bool,
}

/// The bullet being typed into.
#[derive(Debug, Default)]
struct EditState {
    node: Option<NodeId>,
    buf: String,
    /// Put the caret here on the next frame (and take keyboard focus).
    /// (anchor, caret) in chars.
    set_cursor: Option<(usize, usize)>,
    selection_empty: bool,
}

#[derive(Debug, Clone)]
struct FieldEdit {
    node: NodeId,
    field: NodeId,
    buf: String,
    focus_once: bool,
}

/// Deferred UI actions (collected while drawing, applied after).
#[derive(Debug, Clone)]
pub(crate) enum Act {
    Run(String, Value),
    Ui(&'static str),
    Pick(PickMode, NodeId, egui::Pos2),
    EditField(NodeId, NodeId, String),
    PickChoice(NodeId, NodeId, egui::Pos2),
    PickDate(NodeId, NodeId, egui::Pos2),
    ClosePanel(usize),
}

/// The date picker of a date field.
#[derive(Debug, Clone)]
pub struct DatePick {
    pub node: NodeId,
    pub field: NodeId,
    pub month: (i32, u8),
    pub anchor: egui::Pos2,
    pub buf: String,
}

/// A request from the JSON control channel (`tanacraft --control PORT`), answered on `reply`.
pub struct ControlRequest {
    pub method: String,
    pub params: Value,
    pub reply: std::sync::mpsc::Sender<Value>,
}

pub struct TanaApp {
    pub session: Session,
    /// Control-channel requests, handled once per frame.
    pub control: Option<std::sync::mpsc::Receiver<ControlRequest>>,
    pub dark: bool,
    /// 0 = full, 1 = mini (icons), 2 = hidden. Mod+\\ cycles (spec 07 §1).
    pub sidebar: u8,
    pub tabs: Vec<Tab>,
    /// The active tab.
    pub tab: usize,
    /// "Search and open" opens its pick in a new tab.
    search_new_tab: bool,
    /// The active panel's title editor (for the keyboard handler).
    pub(crate) title_id: egui::Id,
    /// True while a non-active panel is drawn.
    pub(crate) drawing_other_panel: bool,
    /// Index of the panel being drawn (keeps widget ids apart between panels).
    pub(crate) panel_ix: usize,
    pub help: bool,
    pub palette: Option<Palette>,
    pub picker: Option<Picker>,
    pub date_pick: Option<DatePick>,
    /// Quick add lines (a small outline of new nodes for today).
    pub quick_add: Option<Vec<String>>,
    /// The "Search and open" dialog.
    pub search_open: Option<Palette>,
    pub mac: bool,
    /// Save to `session.path` shortly after changes.
    pub autosave: bool,
    edit: EditState,
    field_edit: Option<FieldEdit>,
    title_buf: (Option<NodeId>, String),
    desc_edit: Option<(NodeId, String)>,
    /// Search pages: the condition editor is open.
    pub query_open: bool,
    clause_buf: String,
    tag_filter: String,
    /// Supertag pages with the configure section open, and their active view tab.
    config_open: std::collections::HashSet<NodeId>,
    tag_tab: std::collections::HashMap<NodeId, NodeId>,
    template_buf: String,
    option_buf: String,
    range_buf: Option<(String, String)>,
    /// Quick text filter per view node (toolbar magnifier).
    quick_filter: std::collections::HashMap<NodeId, String>,
    quick_filter_open: Option<NodeId>,
    quick_filter_focus: bool,
    /// Week shown by a calendar in week mode (None: this week).
    calendar_week_anchor: Option<tanacraft_core::Date>,
    /// Calendars whose "nodes with no dates" list is open.
    show_undated: std::collections::HashSet<NodeId>,
    dirty_since: Option<f64>,
    acts: Vec<Act>,
}

impl TanaApp {
    pub fn new(session: Session) -> TanaApp {
        let first = Tab { panels: vec![session.nav_state()], active: 0 };
        let mut app = TanaApp {
            session,
            control: None,
            dark: true,
            sidebar: 0,
            tabs: Vec::new(),
            tab: 0,
            search_new_tab: false,
            title_id: egui::Id::new("page-title"),
            drawing_other_panel: false,
            panel_ix: 0,
            help: false,
            palette: None,
            picker: None,
            date_pick: None,
            quick_add: None,
            search_open: None,
            mac: cfg!(target_os = "macos"),
            autosave: false,
            edit: EditState::default(),
            field_edit: None,
            title_buf: (None, String::new()),
            desc_edit: None,
            query_open: true,
            clause_buf: String::new(),
            tag_filter: String::new(),
            config_open: Default::default(),
            tag_tab: Default::default(),
            template_buf: String::new(),
            option_buf: String::new(),
            range_buf: None,
            quick_filter: Default::default(),
            quick_filter_open: None,
            quick_filter_focus: false,
            calendar_week_anchor: None,
            show_undated: Default::default(),
            dirty_since: None,
            acts: Vec::new(),
        };
        app.tabs.push(first);
        app
    }

    /// Forget the text field state (another panel or tab is now active).
    pub(crate) fn edit_reset(&mut self) {
        self.edit.node = None;
    }

    pub fn tokens(&self) -> Tokens {
        if self.dark { Tokens::dark() } else { Tokens::light() }
    }

    /// Runs a command or a `ui.*` command by id (used by the command line, scripts and tests).
    pub fn exec(&mut self, id: &str, params: Value) -> Result<Value, String> {
        if id.starts_with("ui.") {
            self.ui_command(id);
            return Ok(json!({}));
        }
        let before = (self.session.focus, self.session.caret);
        let r = self.session.run(id, &params).map_err(|e| e.to_string());
        if (self.session.focus, self.session.caret) != before || id != "node.setName" {
            self.edit.node = None; // reload text and caret on the next frame
        }
        r
    }

    fn ui_command(&mut self, id: &str) {
        match id {
            "ui.commandLine" => self.palette = Some(Palette { focus_once: true, ..Palette::default() }),
            "ui.toggleSidebar" => self.sidebar = (self.sidebar + 1) % 3,
            "ui.newTab" => {
                self.search_new_tab = true;
                self.search_open = Some(Palette { focus_once: true, ..Palette::default() });
            }
            "ui.closeTab" => self.close_tab(self.tab),
            "ui.nextTab" => self.switch_tab((self.tab + 1) % self.tabs.len().max(1)),
            "ui.prevTab" => self.switch_tab((self.tab + self.tabs.len().max(1) - 1) % self.tabs.len().max(1)),
            "ui.openInPanel" => {
                let n = self.session.focus.unwrap_or(self.session.zoom);
                self.open_in_panel(self.session.graph.resolve(n));
            }
            "ui.singlePanel" => self.single_panel(),
            "ui.shortcuts" => self.help = !self.help,
            "ui.toggleTheme" => self.dark = !self.dark,
            "ui.quickAdd" => self.quick_add = Some(vec![String::new()]),
            "ui.searchOpen" => self.search_open = Some(Palette { focus_once: true, ..Palette::default() }),
            "ui.searchOptions" => self.query_open = !self.query_open,
            "ui.configureSupertag" => {
                let z = self.session.zoom;
                if self.session.graph.tag_def(z).is_some() && !self.config_open.remove(&z) {
                    self.config_open.insert(z);
                }
            }
            "ui.editDescription" => {
                if let Some(f) = self.session.focus.or(Some(self.session.zoom)) {
                    let t = self.session.graph.resolve(f);
                    let text = self.session.graph.get(t).map(|n| n.description.clone()).unwrap_or_default();
                    self.desc_edit = Some((t, text));
                }
            }
            _ => {}
        }
    }

    /// Opens a picker (`#` tag, `>` field, `@` reference) for `node` at `anchor`.
    pub fn open_picker(&mut self, mode: PickMode, node: NodeId, anchor: egui::Pos2) {
        self.picker = Some(Picker { mode, node, query: String::new(), selected: 0, anchor, field: None, focus_once: true });
    }

    /// Per-frame logic before drawing: styles, keyboard shortcuts, edit-buffer sync, autosave.
    pub fn logic(&mut self, ctx: &egui::Context) {
        self.apply_style(ctx);
        self.serve_control();
        self.sync_edit();
        keys::handle(self, ctx);
        self.sync_edit();
        self.autosave_tick(ctx);
    }

    /// Control channel: `{"method": "command", "params": {"command": id, "params": {...}}}` runs a
    /// command; `commands` lists them; `view` describes what the panel shows.
    fn serve_control(&mut self) {
        let reqs: Vec<ControlRequest> = self.control.as_ref().map(|rx| rx.try_iter().collect()).unwrap_or_default();
        for r in reqs {
            let reply = match r.method.as_str() {
                "command" => {
                    let id = r.params.get("command").and_then(Value::as_str).unwrap_or_default().to_string();
                    let params = r.params.get("params").cloned().filter(|v| !v.is_null()).unwrap_or(json!({}));
                    match self.exec(&id, params) {
                        Ok(v) => json!({"result": v}),
                        Err(e) => json!({"error": e}),
                    }
                }
                "commands" => json!({"result": self.session.registry.describe()}),
                "view" => match self.session.run("graph.view", &json!({})) {
                    Ok(v) => json!({"result": v}),
                    Err(e) => json!({"error": e.to_string()}),
                },
                m => json!({"error": format!("unknown method `{m}`")}),
            };
            let _ = r.reply.send(reply);
        }
    }

    fn sync_edit(&mut self) {
        let focus = self.session.focus;
        if self.edit.node != focus {
            self.edit.node = focus;
            self.edit.buf = focus.map(|f| self.session.graph.name(f).to_string()).unwrap_or_default();
            self.edit.set_cursor = focus.map(|_| (self.session.anchor, self.session.caret));
        } else if let Some(f) = focus {
            let name = self.session.graph.name(f);
            if self.edit.buf != name {
                self.edit.buf = name.to_string();
                self.edit.set_cursor = Some((self.session.anchor, self.session.caret));
            }
        }
    }

    fn autosave_tick(&mut self, ctx: &egui::Context) {
        if !self.autosave || self.session.path.is_none() || !self.session.graph.is_dirty() {
            self.dirty_since = None;
            return;
        }
        let now = ctx.input(|i| i.time);
        let since = *self.dirty_since.get_or_insert(now);
        if now - since > 1.0 {
            if let Err(e) = self.session.run("file.save", &json!({})) {
                self.session.status = e.to_string();
            }
            self.dirty_since = None;
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(1100));
        }
    }

    fn apply_style(&self, ctx: &egui::Context) {
        let t = self.tokens();
        let mut v = if self.dark { egui::Visuals::dark() } else { egui::Visuals::light() };
        v.panel_fill = t.bg;
        v.window_fill = t.panel;
        v.extreme_bg_color = t.field_bg;
        v.override_text_color = Some(t.text);
        v.selection.bg_fill = t.accent.gamma_multiply(0.45);
        v.hyperlink_color = t.link;
        v.widgets.noninteractive.bg_stroke.color = t.line;
        v.window_stroke.color = t.line;
        ctx.set_visuals(v);
        ctx.global_style_mut(|s| {
            s.spacing.item_spacing = vec2(6.0, 2.0);
            s.spacing.button_padding = vec2(8.0, 4.0);
            for (style, size) in [(egui::TextStyle::Body, theme::FONT), (egui::TextStyle::Button, 14.0), (egui::TextStyle::Small, 12.0)] {
                if let Some(f) = s.text_styles.get_mut(&style) {
                    f.size = size;
                }
            }
        });
    }

    /// Draws the whole window.
    pub fn ui(&mut self, ui: &mut Ui) {
        let t = self.tokens();
        let screen = ui.ctx().content_rect();
        ui.ctx().layer_painter(egui::LayerId::background()).rect_filled(screen.expand(16.0), 0.0, t.bg);
        egui::Panel::top("top")
            .exact_size(40.0)
            .frame(Frame::new().fill(t.sidebar).inner_margin(Margin::symmetric(10, 6)))
            .show(ui, |ui| self.top_bar(ui));
        if self.sidebar == 1 {
            egui::Panel::left("sidebar-mini")
                .exact_size(48.0)
                .resizable(false)
                .show_separator_line(false)
                .frame(Frame::new().fill(t.sidebar).inner_margin(Margin::symmetric(8, 6)))
                .show(ui, |ui| self.mini_sidebar(ui));
        }
        if self.sidebar == 0 {
            egui::Panel::left("sidebar")
                .exact_size(236.0)
                .resizable(false)
                .show_separator_line(false)
                .frame(Frame::new().fill(t.sidebar).inner_margin(Margin { left: 10, right: 10, top: 4, bottom: 10 }))
                .show(ui, |ui| self.sidebar_ui(ui));
        }
        if self.help {
            egui::Panel::right("help")
                .exact_size(300.0)
                .resizable(false)
                .show_separator_line(false)
                .frame(Frame::new().fill(t.sidebar).inner_margin(Margin::symmetric(14, 10)))
                .show(ui, |ui| overlays::help_panel(self, ui));
        }
        egui::CentralPanel::default()
            .frame(Frame::new().fill(t.bg).outer_margin(Margin { left: 0, right: 8, top: 0, bottom: 8 }))
            .show(ui, |ui| self.panels_ui(ui));
        let ctx = ui.ctx().clone();
        overlays::palette(self, &ctx);
        overlays::picker(self, &ctx);
        overlays::description(self, &ctx);
        overlays::date_picker(self, &ctx);
        overlays::quick_add(self, &ctx);
        overlays::search_open(self, &ctx);
        self.flush();
    }

    pub(crate) fn flush(&mut self) {
        for a in std::mem::take(&mut self.acts) {
            match a {
                Act::Run(id, params) => {
                    let _ = self.exec(&id, params);
                }
                Act::Ui(id) => self.ui_command(id),
                Act::Pick(mode, node, at) => self.open_picker(mode, node, at),
                Act::ClosePanel(i) => self.close_panel(i),
                Act::EditField(node, field, buf) => self.field_edit = Some(FieldEdit { node, field, buf, focus_once: true }),
                Act::PickChoice(node, field, at) => {
                    self.open_picker(PickMode::Choice, node, at);
                    if let Some(p) = self.picker.as_mut() {
                        p.field = Some(field);
                    }
                }
                Act::PickDate(node, field, anchor) => {
                    let current = match self.session.graph.get(node).and_then(|n| n.field_value(field)) {
                        Some(tanacraft_core::FieldValue::Date(d)) => Some(*d),
                        _ => None,
                    };
                    let d = current.unwrap_or(self.session.today);
                    self.date_pick =
                        Some(DatePick { node, field, month: (d.year, d.month), anchor, buf: current.map(|d| d.to_string()).unwrap_or_default() });
                }
            }
        }
    }

    fn top_bar(&mut self, ui: &mut Ui) {
        let t = self.tokens();
        ui.horizontal_centered(|ui| {
            if self.mac {
                ui.add_space(64.0); // traffic lights
            }
            if icon_button(ui, Icon::Sidebar, t.dim, "Toggle sidebar").clicked() {
                self.acts.push(Act::Ui("ui.toggleSidebar"));
            }
            if icon_button(ui, Icon::Back, if self.session.can_go_back() { t.dim } else { t.faint }, "Back").clicked() {
                self.acts.push(Act::Run("nav.back".into(), json!({})));
            }
            if icon_button(ui, Icon::Forward, if self.session.can_go_forward() { t.dim } else { t.faint }, "Forward").clicked() {
                self.acts.push(Act::Run("nav.forward".into(), json!({})));
            }
            ui.add_space(6.0);
            self.tabs_ui(ui);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icon_button(ui, Icon::Keyboard, if self.help { t.text } else { t.dim }, "Keyboard shortcuts").clicked() {
                    self.acts.push(Act::Ui("ui.shortcuts"));
                }
                let dirty = if self.session.graph.is_dirty() && self.session.path.is_some() { " (unsaved)" } else { "" };
                ui.label(RichText::new(format!("{PRODUCT_NAME}{dirty}")).size(12.0).color(t.faint));
            });
        });
    }

    fn mini_sidebar(&mut self, ui: &mut Ui) {
        let t = self.tokens();
        for (icon, tip, act) in [
            (Icon::Today, "Today", Act::Run("nav.today".into(), json!({}))),
            (Icon::Tag, "Supertags", Act::Run("nav.supertags".into(), json!({}))),
            (Icon::Plus, "Create new", Act::Run("node.createNew".into(), json!({}))),
            (Icon::Search, "Search", Act::Ui("ui.searchOpen")),
            (Icon::Home, "Library", Act::Run("nav.home".into(), json!({}))),
            (Icon::Calendar, "Daily notes", Act::Run("nav.dailyNotes".into(), json!({}))),
            (Icon::Trash, "Trash", Act::Run("nav.trash".into(), json!({}))),
        ] {
            if icon_button(ui, icon, t.dim, tip).clicked() {
                self.acts.push(act);
            }
            ui.add_space(4.0);
        }
    }

    fn sidebar_ui(&mut self, ui: &mut Ui) {
        let t = self.tokens();
        let g = &self.session.graph;
        let zoom = self.session.zoom;
        let today_open = g.get(zoom).is_some_and(|n| n.kind == tanacraft_core::Kind::Day { date: self.session.today });
        ui.add_space(4.0);
        let mut acts = Vec::new();
        if side_item(ui, &t, Icon::Today, "Today", today_open).clicked() {
            acts.push(Act::Run("nav.today".into(), json!({})));
        }
        if side_item(ui, &t, Icon::Tag, "Supertags", zoom == g.sys.schema).clicked() {
            acts.push(Act::Run("nav.supertags".into(), json!({})));
        }
        if side_item(ui, &t, Icon::Search, "Search", self.search_open.is_some()).clicked() {
            acts.push(Act::Ui("ui.searchOpen"));
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let r = Frame::new().fill(t.hover).corner_radius(CornerRadius::same(6)).inner_margin(Margin::symmetric(8, 5)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                    icons::paint(ui.painter(), rect, Icon::Plus, t.text);
                    ui.label(RichText::new("Create new").size(14.0));
                });
            });
            if r.response.interact(Sense::click()).clicked() {
                acts.push(Act::Run("node.createNew".into(), json!({})));
            }
        });
        ui.add_space(14.0);
        section(ui, &t, "Pinned");
        for p in g.pinned() {
            let name = outline::page_title(&self.session, p);
            let resp = if let Some(def) = g.tag_def(p) {
                let (strong, _) = theme::tag_color(&def.color, self.dark);
                side_row(ui, &t, |pt, r| icons::hash(pt, r, strong), g.name(p), zoom == p)
            } else {
                side_row(
                    ui,
                    &t,
                    |pt, r| {
                        pt.circle_filled(r.center(), 3.0, t.dim);
                    },
                    &name,
                    zoom == p,
                )
            };
            if resp.clicked() {
                acts.push(Act::Run("nav.open".into(), json!({"node": p.0})));
            }
        }
        ui.add_space(14.0);
        section(ui, &t, "Workspace");
        for (icon, label, id) in
            [(Icon::Home, "Library", g.sys.library), (Icon::Calendar, "Daily notes", g.sys.journal), (Icon::Trash, "Trash", g.sys.trash)]
        {
            if side_item(ui, &t, icon, label, zoom == id).clicked() {
                acts.push(Act::Run("nav.open".into(), json!({"node": id.0})));
            }
        }
        let recents: Vec<NodeId> = self.session.recents.iter().copied().filter(|r| g.get(*r).is_some() && !g.is_system(*r)).take(6).collect();
        if !recents.is_empty() {
            ui.add_space(14.0);
            section(ui, &t, "Recents");
            for r in recents {
                let name = outline::page_title(&self.session, r);
                if side_item(ui, &t, Icon::Recent, &name, zoom == r).clicked() {
                    acts.push(Act::Run("nav.open".into(), json!({"node": r.0})));
                }
            }
        }
        self.acts.extend(acts);
    }
}

fn icon_button(ui: &mut Ui, icon: Icon, color: Color32, tip: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 5.0, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    icons::paint(ui.painter(), rect.shrink(6.0), icon, color);
    resp.on_hover_text(tip)
}

fn section(ui: &mut Ui, t: &Tokens, label: &str) {
    ui.add_space(2.0);
    ui.label(RichText::new(label).size(13.0).color(t.faint));
    ui.add_space(2.0);
}

fn side_item(ui: &mut Ui, t: &Tokens, icon: Icon, label: &str, active: bool) -> egui::Response {
    let color = if active { t.text } else { t.dim };
    side_row(ui, t, |p, r| icons::paint(p, r, icon, color), label, active)
}

fn side_row(ui: &mut Ui, t: &Tokens, paint_icon: impl FnOnce(&egui::Painter, egui::Rect), label: &str, active: bool) -> egui::Response {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 28.0), Sense::click());
    if active || resp.hovered() {
        ui.painter().rect_filled(rect, 6.0, t.hover);
    }
    let icon_rect = egui::Rect::from_center_size(rect.left_center() + vec2(16.0, 0.0), vec2(15.0, 15.0));
    paint_icon(ui.painter(), icon_rect);
    let galley =
        ui.painter().layout_no_wrap(label.to_string(), egui::FontId::proportional(14.5), if active { t.text } else { t.text.gamma_multiply(0.9) });
    let clip = rect.shrink2(vec2(4.0, 0.0));
    ui.painter().with_clip_rect(clip).galley(egui::pos2(rect.left() + 32.0, rect.center().y - galley.size().y / 2.0), galley, t.text);
    resp
}
