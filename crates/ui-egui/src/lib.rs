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
    ("ui.shortcuts", "Keyboard shortcuts", "Mod+/ / Mod+Shift+/ / Mod+Shift+?"),
    ("ui.moveTabLeft", "Move tab left", "Ctrl+Shift+PageUp"),
    ("ui.moveTabRight", "Move tab right", "Ctrl+Shift+PageDown"),
    ("ui.toggleTheme", "Switch light or dark theme", ""),
    ("ui.editDescription", "Add description", "Ctrl+I"),
    ("ui.quickAdd", "Quick add", "Mod+E"),
    ("ui.searchOpen", "Search and open", "Mod+S"),
    ("ui.configureSupertag", "Configure supertag", ""),
    ("ui.recents", "Open recents", ""),
    ("ui.settings", "Open settings", ""),
    ("ui.searchOptions", "Show or hide search conditions", ""),
    ("ui.insertDate", "Insert date...", ""),
    ("ui.help", "Help", ""),
    ("ui.cycleContentWidth", "Cycle content widths", ""),
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
    /// `@` mid-sentence: an inline reference inserted at `Picker::caret`.
    InlineRef,
    /// A source of options for `node` (a field): any node.
    OptionSource,
    /// An optional field for `node` (a supertag).
    OptionalField,
    /// "Move and leave reference to": like `MoveTo`, a reference stays behind.
    MoveLeaveRef,
    /// "Move to": a new owner for `node`.
    MoveTo,
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
    /// Where an inline reference goes (chars), for `InlineRef`.
    pub caret: Option<usize>,
    /// Which of the field's values a `Choice` sets (the count adds one).
    pub index: usize,
    /// Field: the empty line `>` was typed on; it shows as the field row being named, and the row
    /// lands in its place (spec 02 §11).
    pub line: Option<NodeId>,
    focus_once: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Palette {
    pub query: String,
    pub selected: usize,
    focus_once: bool,
    /// Scroll the highlighted row into view (after arrow keys).
    scroll_to: bool,
}

/// The bullet being typed into.
#[derive(Debug, Default)]
struct EditState {
    node: Option<NodeId>,
    buf: String,
    /// Put the caret here on the next frame (and take keyboard focus).
    /// (anchor, caret) in chars.
    set_cursor: Option<(usize, usize)>,
    /// The caret placed last frame: if egui's own arrow-key focus moving took the keyboard away
    /// in that same frame, take it back once (DIFFS N3).
    verify_focus: Option<(usize, usize)>,
    selection_empty: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct FieldEdit {
    node: NodeId,
    field: NodeId,
    /// Which value (the count adds one).
    index: usize,
    buf: String,
    focus_once: bool,
}

/// Deferred UI actions (collected while drawing, applied after).
#[derive(Debug, Clone)]
pub(crate) enum Act {
    Run(String, Value),
    Ui(&'static str),
    Pick(PickMode, NodeId, egui::Pos2),
    /// Edit value number `usize` of a field (the count adds a value).
    EditField(NodeId, NodeId, usize, String),
    PickChoice(NodeId, NodeId, usize, egui::Pos2),
    PickDate(NodeId, NodeId, usize, egui::Pos2),
    /// "Go to date": a month picker that opens the chosen day.
    GoToDate(egui::Pos2),
    ClosePanel(usize),
    OpenInPanel(NodeId),
    /// Show the node's edit log as a notice.
    EditLog(NodeId),
    /// Show a node in the panel to the right (list view detail).
    ShowBeside(NodeId),
    /// `@` typed mid-sentence at this caret.
    PickInline(NodeId, usize, egui::Pos2),
    /// `#` typed at this char index: the tag menu filters by the text after it.
    PickTagInline(NodeId, usize, egui::Pos2),
}

/// The date picker of a date field.
#[derive(Debug, Clone)]
pub struct DatePick {
    pub node: NodeId,
    /// The date field to set; `None` = "Go to date" (open the picked day).
    pub field: Option<NodeId>,
    /// Which of the field's values (the count adds one).
    pub index: usize,
    /// "Insert date": the picked day goes into the focused line as a link.
    pub insert: Option<(NodeId, usize)>,
    pub month: (i32, u8),
    pub anchor: egui::Pos2,
    pub buf: String,
    /// Date fields: optional start and end times ("HH:MM").
    pub time: (String, String),
    /// Drawn at least once (the click that opened it does not close it).
    pub drawn: bool,
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
    /// Give the description editor keyboard focus on the next frame.
    desc_focus: bool,
    /// Search pages: the condition editor is open.
    pub query_open: bool,
    tag_filter: String,
    /// Group headers the person collapsed, per view node.
    closed_groups: std::collections::HashSet<(NodeId, String)>,
    /// The Recents and Settings pages (they are views, not nodes).
    pub recents_page: bool,
    pub settings_page: bool,
    /// The Help page: every command with its shortcut.
    pub help_page: bool,
    recents_selected: Option<NodeId>,
    /// Put the caret in the page title on the next frame.
    pub(crate) focus_title: bool,
    /// A table column being resized: (view, column, width).
    pub(crate) col_drag: Option<(NodeId, NodeId, f32)>,
    /// Last measured height of each outline row (with its fields), so rows outside the viewport
    /// are skipped as blank space of the right size.
    pub(crate) row_heights: std::collections::HashMap<NodeId, f32>,
    /// A short notice in the top-right corner, and when it appeared.
    toast_msg: Option<(String, f64)>,
    /// The last nodes copied with Cmd+C and the text that went to the clipboard.
    node_clip: Option<(String, Vec<NodeId>)>,
    /// Pinned sidebar rows shown expanded.
    pin_open: std::collections::HashSet<NodeId>,
    /// A navigation view's just-added item: (node, heading being typed, focus it once).
    pub(crate) nav_title: Option<(NodeId, String, bool)>,
    /// Workspaces folded in the sidebar (all start open).
    ws_closed: std::collections::HashSet<NodeId>,
    /// Supertag pages with the configure section open, and their active view tab.
    /// Content width: 0 auto, 1 medium, 2 full (spec 07 §5).
    pub content_width: u8,
    /// The first panel's share of the width when two panels are open.
    pub panel_split: f32,
    /// Search results seen last frame, per search node (to spot new ones).
    pub last_results: std::collections::HashMap<NodeId, std::collections::BTreeSet<NodeId>>,
    /// New search results glow until this time (seconds of egui time).
    pub flash_until: std::collections::HashMap<NodeId, f64>,
    /// Custom keybindings: (shortcut like `Mod+Shift+H`, command id), checked before the defaults.
    pub keymap: Vec<(String, String)>,
    /// A table column header being renamed (field, text).
    pub header_edit: Option<(NodeId, String)>,
    /// Settings: show reference counts next to nodes.
    pub ref_counter: bool,
    /// Configure-supertag sections the user folded.
    config_closed: std::collections::HashSet<String>,
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
    /// Calendar time grid: a "New event" being dragged out or named.
    pub cal_new: Option<crate::views::NewEvent>,
    /// Calendar time grid: an event being resized (item, new end in minutes).
    pub cal_resize: Option<(NodeId, u16)>,
    /// Calendar views already scrolled to the start of the day.
    cal_scrolled: std::collections::HashSet<NodeId>,
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
            desc_focus: false,
            query_open: false,
            tag_filter: String::new(),
            closed_groups: Default::default(),
            pin_open: Default::default(),
            ws_closed: Default::default(),
            nav_title: None,
            recents_page: false,
            settings_page: false,
            help_page: false,
            recents_selected: None,
            focus_title: false,
            node_clip: None,
            toast_msg: None,
            col_drag: None,
            row_heights: Default::default(),
            config_closed: Default::default(),
            ref_counter: false,
            header_edit: None,
            keymap: Vec::new(),
            last_results: Default::default(),
            flash_until: Default::default(),
            content_width: 0,
            panel_split: 0.5,
            tag_tab: Default::default(),
            template_buf: String::new(),
            option_buf: String::new(),
            range_buf: None,
            quick_filter: Default::default(),
            quick_filter_open: None,
            quick_filter_focus: false,
            calendar_week_anchor: None,
            show_undated: Default::default(),
            cal_new: None,
            cal_resize: None,
            cal_scrolled: Default::default(),
            dirty_since: None,
            acts: Vec::new(),
        };
        app.tabs.push(first);
        app
    }

    /// Shows a short notice for a few seconds.
    pub fn toast(&mut self, msg: String) {
        self.toast_msg = Some((msg, f64::NAN));
    }

    /// Forget the text field state (another panel or tab is now active).
    /// Starts typing into a field value. One caret at a time: the bullet being edited lets go, or
    /// it would take the keyboard back and the next keys would land in it (DIFFS N1).
    pub(crate) fn start_field_edit(&mut self, fe: FieldEdit) {
        self.session.focus = None;
        self.edit_reset();
        self.field_edit = Some(fe);
    }

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
        if id.starts_with("nav.") || id == "node.createNew" || id == "workspace.switch" {
            // An opened search node shows its results; the builder waits for "Open query editor"
            // (spec 05 §1).
            self.query_open = false;
            self.recents_page = false;
            self.settings_page = false;
            self.help_page = false;
        }
        let r = self.session.run(id, &params).map_err(|e| e.to_string());
        if id == "file.open" && r.is_ok() {
            self.reset_tabs();
        }
        if id == "node.createNew" && r.is_ok() {
            self.focus_title = true;
        }
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
            "ui.insertDate" => {
                // A month picker; the chosen day goes in at the caret of the focused line.
                if let Some(f) = self.session.focus {
                    let d = self.session.today;
                    self.date_pick = Some(DatePick {
                        node: f,
                        field: None,
                        index: 0,
                        insert: Some((f, self.session.caret)),
                        month: (d.year, d.month),
                        anchor: egui::pos2(380.0, 160.0),
                        buf: String::new(),
                        time: Default::default(),
                        drawn: false,
                    });
                }
            }
            "ui.moveTabLeft" | "ui.moveTabRight" => {
                let to = if id == "ui.moveTabLeft" { self.tab.checked_sub(1) } else { Some(self.tab + 1).filter(|i| *i < self.tabs.len()) };
                if let Some(to) = to {
                    self.tabs.swap(self.tab, to);
                    self.tab = to;
                }
            }
            "ui.nextTab" => self.switch_tab((self.tab + 1) % self.tabs.len().max(1)),
            "ui.prevTab" => self.switch_tab((self.tab + self.tabs.len().max(1) - 1) % self.tabs.len().max(1)),
            "ui.openInPanel" => {
                let n = self.session.focus.unwrap_or(self.session.zoom);
                self.open_in_panel(self.session.graph.resolve(n));
            }
            "ui.singlePanel" => self.single_panel(),
            "ui.shortcuts" => self.help = !self.help,
            "ui.toggleTheme" => self.dark = !self.dark,
            "ui.quickAdd" => {
                // The modal takes the keyboard from the line being edited (DIFFS P1 #5).
                self.quick_add = Some(vec![String::new()]);
                self.session.focus = None;
                self.edit.node = None;
            }
            "ui.searchOpen" => self.search_open = Some(Palette { focus_once: true, ..Palette::default() }),
            "ui.searchOptions" => self.query_open = !self.query_open,
            "ui.recents" => {
                self.recents_page = true;
                self.settings_page = false;
                self.help_page = false;
            }
            "ui.settings" => {
                self.settings_page = true;
                self.recents_page = false;
                self.help_page = false;
            }
            "ui.cycleContentWidth" => self.content_width = (self.content_width + 1) % 3,
            "ui.help" => {
                self.help_page = !self.help_page;
                self.settings_page = false;
                self.recents_page = false;
            }
            "ui.configureSupertag" => {
                let z = self.session.zoom;
                // Configuration opens in its own panel to the right; Done (or the command again)
                // closes it (spec 03 §5).
                if self.session.config_view {
                    if self.panel_count() > 1 {
                        let i = self.tabs.get(self.tab).map(|t| t.active).unwrap_or(0);
                        self.close_panel(i);
                    } else {
                        self.session.config_view = false;
                    }
                } else if self.session.graph.tag_def(z).is_some() {
                    self.open_in_panel(z);
                    self.session.config_view = true;
                }
            }
            "ui.editDescription" => {
                if let Some(f) = self.session.focus.or(Some(self.session.zoom)) {
                    let t = self.session.graph.resolve(f);
                    let text = self.session.graph.get(t).map(|n| n.description.clone()).unwrap_or_default();
                    self.desc_edit = Some((t, text));
                    self.desc_focus = true;
                    self.session.focus = None;
                }
            }
            _ => {}
        }
    }

    /// Opens a picker (`#` tag, `>` field, `@` reference) for `node` at `anchor`.
    pub fn open_picker(&mut self, mode: PickMode, node: NodeId, anchor: egui::Pos2) {
        let g = &self.session.graph;
        let line = self.session.focus.filter(|f| {
            mode == PickMode::Field && g.parent(*f) == Some(g.resolve(node)) && g.get(*f).is_some_and(|n| n.name.is_empty() && n.children.is_empty())
        });
        self.picker =
            Some(Picker { mode, node, query: String::new(), selected: 0, anchor, field: None, caret: None, index: 0, line, focus_once: true });
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
        overlays::date_picker(self, &ctx);
        overlays::quick_add(self, &ctx);
        overlays::search_open(self, &ctx);
        self.toast_ui(&ctx);
        self.flush();
    }

    fn toast_ui(&mut self, ctx: &egui::Context) {
        let Some((msg, since)) = self.toast_msg.clone() else { return };
        let now = ctx.input(|i| i.time);
        let since = if since.is_nan() { now } else { since };
        if now - since > 4.0 {
            self.toast_msg = None;
            return;
        }
        self.toast_msg = Some((msg.clone(), since));
        let t = self.tokens();
        let screen = ctx.content_rect();
        egui::Area::new(egui::Id::new("toast"))
            .order(egui::Order::Foreground)
            .fixed_pos(egui::pos2(screen.right() - 380.0, screen.top() + 52.0))
            .show(ctx, |ui| {
                Frame::popup(ui.style()).fill(t.panel).corner_radius(8.0).inner_margin(10.0).show(ui, |ui| {
                    ui.set_width(340.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("!").strong().color(egui::Color32::from_rgb(224, 176, 48)));
                        ui.label(RichText::new(msg).size(13.5));
                    });
                });
            });
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
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
                Act::ShowBeside(n) => self.show_beside(n),
                Act::OpenInPanel(n) => self.open_in_panel(n),
                Act::EditLog(n) => {
                    if let Ok(v) = self.exec("node.editLog", json!({"node": n.0})) {
                        let lines: Vec<String> = v["log"]
                            .as_array()
                            .map(|a| a.iter().map(|e| format!("{} {}", e["event"].as_str().unwrap_or(""), e["at"].as_str().unwrap_or(""))).collect())
                            .unwrap_or_default();
                        self.toast(format!("Edit log: {}", lines.join(", ")));
                    }
                }
                Act::PickTagInline(node, hash, at) => {
                    self.open_picker(PickMode::Tag, node, at);
                    if let Some(p) = self.picker.as_mut() {
                        p.caret = Some(hash);
                        p.focus_once = false;
                    }
                }
                Act::PickInline(node, caret, at) => {
                    self.open_picker(PickMode::InlineRef, node, at);
                    if let Some(p) = self.picker.as_mut() {
                        p.caret = Some(caret);
                    }
                }
                Act::GoToDate(anchor) => {
                    let d = self.session.today;
                    self.date_pick = Some(DatePick {
                        node: self.session.zoom,
                        field: None,
                        index: 0,
                        insert: None,
                        month: (d.year, d.month),
                        anchor,
                        buf: String::new(),
                        time: Default::default(),
                        drawn: false,
                    });
                }
                Act::EditField(node, field, index, buf) => self.start_field_edit(FieldEdit { node, field, index, buf, focus_once: true }),
                Act::PickChoice(node, field, index, at) => {
                    self.open_picker(PickMode::Choice, node, at);
                    if let Some(p) = self.picker.as_mut() {
                        p.field = Some(field);
                        p.index = index;
                    }
                }
                Act::PickDate(node, field, index, anchor) => {
                    let value = self.session.graph.get(node).and_then(|n| n.field_values(field).get(index).copied()).cloned();
                    let current = value.as_ref().and_then(tanacraft_core::FieldValue::date);
                    let time = match value {
                        Some(tanacraft_core::FieldValue::Timed { start, end, .. }) => (tanacraft_core::fmt_hm(start), tanacraft_core::fmt_hm(end)),
                        _ => Default::default(),
                    };
                    let d = current.unwrap_or(self.session.today);
                    self.date_pick = Some(DatePick {
                        node,
                        field: Some(field),
                        index,
                        insert: None,
                        month: (d.year, d.month),
                        anchor,
                        buf: current.map(|d| d.to_string()).unwrap_or_default(),
                        time,
                        drawn: false,
                    });
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
                // Right cluster (spec 00 §2): account menu, keyboard shortcuts, help.
                let account = icon_button(ui, Icon::Person, t.dim, "Account");
                let mut acts = Vec::new();
                egui::Popup::menu(&account).show(|ui| {
                    ui.menu_button("Workspaces", |ui| self.workspace_menu(ui, &mut acts));
                    for (label, act) in [
                        ("Settings", Act::Ui("ui.settings")),
                        ("Switch light or dark theme", Act::Ui("ui.toggleTheme")),
                        ("Keyboard shortcuts", Act::Ui("ui.shortcuts")),
                        ("Help", Act::Ui("ui.help")),
                    ] {
                        if ui.button(label).clicked() {
                            acts.push(act);
                            ui.close();
                        }
                    }
                });
                self.acts.extend(acts);
                if icon_button(ui, Icon::Keyboard, if self.help { t.text } else { t.dim }, "Keyboard shortcuts").clicked() {
                    self.acts.push(Act::Ui("ui.shortcuts"));
                }
                if icon_button(ui, Icon::Help, if self.help_page { t.text } else { t.dim }, "Help").clicked() {
                    self.acts.push(Act::Ui("ui.help"));
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
        // Pinned nodes and Workspaces open flyouts with the lists the full sidebar shows (spec 07 §1).
        let mut acts = Vec::new();
        let pin = icon_button(ui, Icon::Pin, t.dim, "Pinned");
        egui::Popup::menu(&pin).show(|ui| {
            ui.set_min_width(220.0);
            let g = &self.session.graph;
            let pinned = g.pinned();
            if pinned.is_empty() {
                ui.label(RichText::new("Nothing pinned yet").color(t.faint));
            }
            for p in pinned {
                if ui.button(outline::page_title(&self.session, p)).clicked() {
                    acts.push(Act::Run("nav.open".into(), json!({"node": p.0})));
                    ui.close();
                }
            }
        });
        ui.add_space(4.0);
        let ws = icon_button(ui, Icon::Building, t.dim, "Workspaces");
        egui::Popup::menu(&ws).show(|ui| {
            ui.set_min_width(220.0);
            let g = &self.session.graph;
            ui.label(RichText::new(g.name(g.sys.root)).strong());
            for (label, id) in
                [("Library", Some(g.sys.library)), ("Daily notes", Some(g.sys.journal)), ("Inbox", g.sys.inbox), ("Trash", Some(g.sys.trash))]
            {
                if let Some(id) = id
                    && ui.button(label).clicked()
                {
                    acts.push(Act::Run("nav.open".into(), json!({"node": id.0})));
                    ui.close();
                }
            }
            ui.separator();
            ui.menu_button("Manage workspaces", |ui| self.workspace_menu(ui, &mut acts));
        });
        self.acts.extend(acts);
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
        if side_item(ui, &t, Icon::Recent, "Recents", self.recents_page).clicked() {
            acts.push(Act::Ui("ui.recents"));
        }
        ui.add_space(8.0);
        // A wide Create new button and a square Search button (spec 00 §3).
        ui.horizontal(|ui| {
            let r = Frame::new().fill(t.hover).corner_radius(CornerRadius::same(6)).inner_margin(Margin::symmetric(8, 5)).show(ui, |ui| {
                ui.set_width(ui.available_width() - 44.0);
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                    icons::paint(ui.painter(), rect, Icon::Plus, t.text);
                    ui.label(RichText::new("Create new").size(14.0));
                });
            });
            if r.response.interact(Sense::click()).clicked() {
                acts.push(Act::Run("node.createNew".into(), json!({})));
            }
            let (sq, resp) = ui.allocate_exact_size(vec2(r.response.rect.height(), r.response.rect.height()), Sense::click());
            ui.painter().rect_filled(sq, 6.0, if resp.hovered() || self.search_open.is_some() { t.line } else { t.hover });
            icons::paint(ui.painter(), sq.shrink(8.0), Icon::Search, t.text);
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Search"));
            if resp.on_hover_text("Search").clicked() {
                acts.push(Act::Ui("ui.searchOpen"));
            }
        });
        ui.add_space(14.0);
        section(ui, &t, "Pinned");
        let mut toggle = None;
        for p in g.pinned() {
            let name = outline::page_title(&self.session, p);
            let kids: Vec<NodeId> = if g.tag_def(p).is_some() { Vec::new() } else { g.children(p).to_vec() };
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
            // Expandable pinned rows: a chevron in the left gutter shows the children (spec 07 §2).
            let open = self.pin_open.contains(&p);
            if !kids.is_empty() {
                let chev = egui::Rect::from_center_size(resp.rect.left_center() - vec2(4.0, 0.0), vec2(10.0, 20.0));
                let c = ui.interact(chev, egui::Id::new(("pin-chev", p.0)), Sense::click());
                if resp.hovered() || open || c.hovered() {
                    let m = chev.center();
                    let pts = if open {
                        vec![m + vec2(-3.0, -1.5), m + vec2(3.0, -1.5), m + vec2(0.0, 2.5)]
                    } else {
                        vec![m + vec2(-1.5, -3.0), m + vec2(2.5, 0.0), m + vec2(-1.5, 3.0)]
                    };
                    ui.painter().add(egui::Shape::convex_polygon(pts, t.dim, egui::Stroke::NONE));
                }
                if c.clicked() {
                    toggle = Some(p);
                }
            }
            if resp.clicked() {
                acts.push(Act::Run("nav.open".into(), json!({"node": p.0})));
            }
            if open {
                for k in kids.iter().take(30) {
                    let text = g.text(*k);
                    let label = if text.chars().count() > 26 { format!("{}...", text.chars().take(26).collect::<String>()) } else { text };
                    let r = ui.horizontal(|ui| {
                        ui.add_space(26.0);
                        ui.add(egui::Label::new(RichText::new(label).size(13.5).color(t.dim)).sense(Sense::click()))
                    });
                    if r.inner.clicked() {
                        acts.push(Act::Run("nav.open".into(), json!({"node": k.0})));
                    }
                }
            }
        }
        if let Some(p) = toggle
            && !self.pin_open.remove(&p)
        {
            self.pin_open.insert(p);
        }
        ui.add_space(14.0);
        // Workspaces, all open side by side (spec 07 §2): one alone shows its name as the header;
        // several are listed under "Workspaces", each with a chevron that shows its items.
        let all = g.workspaces();
        let several = all.len() > 1;
        if several {
            ui.label(RichText::new("Workspaces").size(12.5).color(t.faint));
        }
        let mut toggle_ws = None;
        for ws in all {
            let g = &self.session.graph;
            let name = g.name(ws).to_string();
            let open = !self.ws_closed.contains(&ws);
            let head = ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                ui.painter().circle_filled(r.center(), 9.0, avatar_color(&name, &t));
                let initial: String = name.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
                ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, initial, egui::FontId::proportional(11.0), egui::Color32::WHITE);
                ui.label(RichText::new(&name).size(13.5).color(t.dim));
            });
            let head = ui.interact(head.response.rect, egui::Id::new(("ws-head", ws.0)), Sense::click());
            if several {
                // Chevron in the left gutter, as on pinned rows.
                let chev = egui::Rect::from_center_size(head.rect.left_center() - vec2(8.0, 0.0), vec2(10.0, 20.0));
                let c = ui.interact(chev, egui::Id::new(("ws-chev", ws.0)), Sense::click());
                let m = chev.center();
                let pts = if open {
                    vec![m + vec2(-3.0, -1.5), m + vec2(3.0, -1.5), m + vec2(0.0, 2.5)]
                } else {
                    vec![m + vec2(-1.5, -3.0), m + vec2(2.5, 0.0), m + vec2(-1.5, 3.0)]
                };
                ui.painter().add(egui::Shape::convex_polygon(pts, t.faint, egui::Stroke::NONE));
                if c.clicked() || head.clicked() {
                    toggle_ws = Some(ws);
                }
            } else {
                egui::Popup::menu(&head).show(|ui| self.workspace_menu(ui, &mut acts));
            }
            if !open && several {
                continue;
            }
            ui.add_space(2.0);
            let g = &self.session.graph;
            let items: Vec<(Icon, &str, NodeId)> = if ws == g.sys.root {
                vec![(Icon::Home, "Library", g.sys.library), (Icon::Calendar, "Daily notes", g.sys.journal), (Icon::Trash, "Trash", g.sys.trash)]
            } else {
                vec![(Icon::Home, "Library", g.workspace_library(ws))]
            };
            for (icon, label, id) in items {
                ui.horizontal(|ui| {
                    if several {
                        ui.add_space(8.0);
                    }
                    if side_item(ui, &t, icon, label, zoom == id).clicked() {
                        acts.push(Act::Run("nav.open".into(), json!({"node": id.0})));
                    }
                });
            }
            ui.add_space(4.0);
        }
        if let Some(ws) = toggle_ws
            && !self.ws_closed.remove(&ws)
        {
            self.ws_closed.insert(ws);
        }
        let manage = side_item(ui, &t, Icon::Building, "Manage workspaces", false);
        egui::Popup::menu(&manage).show(|ui| self.workspace_menu(ui, &mut acts));
        self.acts.extend(acts);
    }
}

/// A workspace avatar colour picked from its name.
fn avatar_color(name: &str, t: &Tokens) -> Color32 {
    let hues = [
        t.accent,
        Color32::from_rgb(64, 160, 120),
        Color32::from_rgb(200, 120, 60),
        Color32::from_rgb(150, 100, 200),
        Color32::from_rgb(200, 80, 110),
    ];
    let h = name.bytes().fold(0usize, |a, b| a.wrapping_mul(31).wrapping_add(b as usize));
    hues[h % hues.len()].gamma_multiply(0.8)
}

impl TanaApp {
    /// Manage workspaces: open one, add one beside the others, rename the main one.
    fn workspace_menu(&mut self, ui: &mut Ui, acts: &mut Vec<Act>) {
        let g = &self.session.graph;
        for ws in g.workspaces() {
            if ui.button(format!("Open {}", g.name(ws))).clicked() {
                acts.push(Act::Run("nav.open".into(), json!({"node": g.workspace_library(ws).0})));
                ui.close();
            }
        }
        ui.separator();
        let id = egui::Id::new("ws-new");
        let mut buf = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_default();
        let r = ui.add(egui::TextEdit::singleline(&mut buf).hint_text("Create workspace: name, Enter").desired_width(220.0));
        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !buf.trim().is_empty() {
            acts.push(Act::Run("workspace.create".into(), json!({"name": buf.trim()})));
            buf.clear();
            ui.close();
        }
        ui.data_mut(|d| d.insert_temp(id, buf));
        let rid = egui::Id::new("ws-rename");
        let mut name = ui.data_mut(|d| d.get_temp::<String>(rid)).unwrap_or_else(|| self.session.graph.name(self.session.graph.sys.root).to_string());
        let r = ui.add(egui::TextEdit::singleline(&mut name).hint_text("Rename the main workspace").desired_width(220.0));
        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !name.trim().is_empty() {
            acts.push(Act::Run("workspace.rename".into(), json!({"name": name.trim()})));
            ui.data_mut(|d| d.remove::<String>(rid));
            ui.close();
        } else {
            ui.data_mut(|d| d.insert_temp(rid, name));
        }
    }

    /// Reads custom keybindings from a JSON object `{"Mod+Shift+H": "node.toggleHeading"}`.
    /// Unknown commands are refused so a typo cannot silently eat a key.
    pub fn set_keybindings(&mut self, json_text: &str) -> Result<usize, String> {
        let v: Value = serde_json::from_str(json_text).map_err(|e| format!("keybindings: {e}"))?;
        let obj = v.as_object().ok_or("keybindings: expected an object of shortcut: command")?;
        let mut map = Vec::new();
        for (key, cmd) in obj {
            let cmd = cmd.as_str().ok_or_else(|| format!("keybindings: `{key}` needs a command id"))?;
            let known = self.session.registry.get(cmd).is_some() || UI_COMMANDS.iter().any(|(id, _, _)| *id == cmd);
            if !known {
                return Err(format!("keybindings: unknown command `{cmd}`"));
            }
            map.push((key.clone(), cmd.to_string()));
        }
        self.keymap = map;
        Ok(self.keymap.len())
    }

    /// After the whole graph changed (another workspace or file): one tab on the current page.
    pub(crate) fn reset_tabs(&mut self) {
        self.tabs = vec![crate::tabs::Tab { panels: vec![self.session.nav_state()], active: 0 }];
        self.tab = 0;
        self.edit_reset();
    }
}

pub(crate) fn icon_button(ui: &mut Ui, icon: Icon, color: Color32, tip: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 5.0, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    icons::paint(ui.painter(), rect.shrink(6.0), icon, color);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tip));
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
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    resp
}
