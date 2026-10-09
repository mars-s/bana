# TanaCraft parity with the behaviour spec

Maps every item in the clean-room spec (`../tanacraft-spec/`) to its status here. The spec currently
holds screenshots only (no prose sections yet), so each screenshot is one row. When prose files land,
each `##` section gets a row too. `cargo xtask parity` lists spec items this file does not mention yet.

Status: **done** (behaviour implemented and tested), **partial** (a first version; gaps listed),
**missing** (not started). Command ids are in parentheses.

## Summary

Of the 69 spec screenshots (`cargo xtask parity` recounts):

| Status | Spec items |
|---|---|
| done | 9 |
| partial | 22 |
| missing | 38 |

## Shell and navigation

| Spec item | Status | Notes |
|---|---|---|
| `01-today-shell-dark.png` | partial | Sidebar (Today, Supertags, Search, Create new, supertag list, workspace, recents), top bar, breadcrumbs, day title "Today, ..." with day chip and prev/next/Today buttons (`nav.today`, `nav.previousDay`, `nav.nextDay`). Missing: right-hand "related content" panel, tabs, pinned items, calendar picker. |
| `02-shortcuts-panel-1.png` | partial | Shortcuts panel (`ui.shortcuts`, Mod+/). Bindings follow this screenshot: Ctrl+Shift+D today, Cmd+Down/Up expand/collapse, Ctrl+Cmd+Down/Up expand/collapse all, Cmd+. / Cmd+, zoom, Tab / Shift+Tab, Cmd+K command line, Cmd+\ sidebar. Missing: tabs and panels shortcuts (Cmd+T, Cmd+W, Cmd+1..9, Cmd+M). |
| `03-shortcuts-panel-2.png` | partial | Undo/redo, move up/down (Cmd+Shift+Up/Down), mark done (Cmd+Enter), duplicate (Cmd+Shift+D), delete (Cmd+Shift+Backspace), description (Ctrl+I), `>` field, `@` reference, `#` tag, create new (Cmd+N). Missing: insert node (Shift+Enter), formatting (bold, italic, underline, highlight, code). |
| `56-command-line.png` | partial | Command line (Cmd+K) searches nodes and all commands. Missing: grouping, recent commands, per-command shortcut recording. |
| `57-search-panel.png` | partial | Search runs through the command line (`graph.search`). Missing: dedicated search panel. |
| `58-search-typed.png` | partial | Substring match on node text, outside the trash. Missing: ranking, breadcrumb context in results. |
| `59-quick-add.png` | missing | Quick add popup. `node.createNew` adds to today instead. |
| `60-daily-notes-root.png` | partial | Daily notes root with year and week nodes; days sorted by date (ISO weeks). Missing: the root page showing today's content inline. |
| `61-daily-notes-prev-day.png` | done | Previous/next day buttons and commands (`nav.previousDay`, `nav.nextDay`). |
| `62-recents.png` | partial | Sidebar recents list (session only). Missing: persisted recents page. |
| `63-ai-chats.png` | missing | Out of scope for now. |
| `64-create-new.png` | partial | Create new button and Cmd+N (`node.createNew`) add an empty node on today's page. spec-gap: exact destination and menu. |
| `65-settings-menu.png` | missing | |
| `66-settings-page.png` | missing | |
| `67-today-light-mode.png` | done | Light and dark themes (`ui.toggleTheme`). Our own palette. |
| `71-sidebar-hidden-two-tabs.png` | partial | Sidebar toggle done (Cmd+\). Tabs missing. |
| `72-two-panels.png` | missing | Side-by-side panels. |

## Outline editing

| Spec item | Status | Notes |
|---|---|---|
| `14-node-menu.jpg` | missing | Node "..." menu. The actions exist as commands (duplicate, delete, move, reference). |
| `29-multi-select.jpg` | missing | Multi-node selection. |
| `30-text-selection-toolbar.png` | missing | Rich text toolbar. |
| `31-inline-formatting.png` | missing | Bold, italic, highlight, code inside node text. Names are plain text today. |
| `34-slash-menu.png` | missing | Slash menu. |
| `69-bullet-context-menu.png` | missing | Bullet context menu. Bullet click zooms in (`nav.zoomIn`). |
| `70-trash.png` | partial | Delete moves to Trash with its old place remembered; `node.restore`, `trash.empty`. The Trash page has an explanatory banner (our wording). Missing: references to trashed nodes shown with a trash mark. |
| (keyboard editing, no screenshot) | done | Enter splits at the caret (end of an expanded parent adds a first child; start of text adds a node above), Tab indent, Shift+Tab outdent (stops at the zoomed node), Backspace at start merges into the node above or removes an empty node, Up/Down move focus, one undo step per word typed. Tested in `crates/commands/src/tests.rs` and through real key events in `crates/ui-egui/tests/typing.rs`. |
| (expand, collapse, zoom, breadcrumbs) | done | Chevrons, halo bullets on collapsed parents, guide lines, zoom in/out, back/forward, clickable breadcrumbs. |

## Supertags and fields

| Spec item | Status | Notes |
|---|---|---|
| `10-supertags-overview.png` | partial | Supertags page lists definitions; the sidebar lists every supertag with its colour. |
| `11-tag-page-task.png` | partial | Supertag page: what it extends, template fields (inherited marked), tagged nodes with checkbox and location. Missing: views (table, list, calendar), filters. |
| `12-tag-task-calendar.png` | missing | Calendar view of tagged nodes. |
| `13-tag-task-list.png` | partial | List of tagged nodes (see above). |
| `73-new-tag-page-empty.png` | partial | A new supertag (from the `#` picker or `tag.create`) opens to a page with empty template fields and tagged nodes lists. Missing: the empty-state guidance. |
| `15-configure-supertag-task.png` | partial | Config by command: colour (`tag.setColor`, 12 colours plus grey), extends (`tag.extend`, cycle-safe), template fields (`tag.addField`), checkbox (`tag.setCheckbox`). Missing: config panel UI. |
| `16-supertag-content-template.png` | partial | Template fields are copied onto tagged nodes with default values. Missing: template child nodes (non-field content). |
| `17-supertag-config-lower.png` | missing | |
| `18-supertag-advanced.png` | missing | |
| `19-field-config-options.png` | partial | Options list (`field.setOptions`), auto-collect of new option values, default value. Missing: hide-field rules, required, config UI. |
| `75-number-field-config.png` | partial | Number fields accept finite numbers only. Missing: min/max limits and their config UI. |
| `20-field-type-dropdown.png` | done | Field types: plain, options, options from supertag, date, number, URL, email, checkbox, each validated on input. User type not implemented. |
| `21-tagged-node-zoomed.png` | done | Zoomed tagged node: checkbox before the title, tag chip, field rows (name column with type glyph, value column, placeholders such as "Add date"), then children. |
| `22-options-picker.png` | partial | Options are picked from a dropdown. Missing: search inside the picker, creating options inline. |
| `23-date-picker.png` | missing | Dates are typed (YYYY-MM-DD, today, tomorrow). No calendar picker. |
| `24-hash-tag-menu.png` | done | Typing `#` opens the supertag picker; Enter applies or creates (`tag.add`). |
| `25-node-tagged-task-inline.png` | done | Tag chips after the text, bullet in the tag colour, checkbox for checkbox supertags. |
| `26-tagged-node-expanded-fields.png` | done | Expanded tagged node shows field rows above its children. |
| `32-field-create-menu.png` | done | `>` on an empty bullet opens the field picker for the parent node; picks or creates a field (`field.add`). |
| `33-field-name-autocomplete.png` | partial | Field picker filters existing fields by name. |

## References

| Spec item | Status | Notes |
|---|---|---|
| `27-at-reference-menu.png` | done | `@` on an empty bullet opens a node search; choosing turns the bullet into a reference (`node.reference` with `replace`). |
| `28-inline-reference.png` | missing | References inside text. Only whole-node references today. |
| `68-reference-node.png` | partial | Reference bullets get a dashed ring around a dot; the target's text and children show and edit in place; cycles are cut. |

## Search nodes and views

| Spec item | Status | Notes |
|---|---|---|
| `35-search-node-new.png` | missing | Live search nodes. |
| `36-search-node-result.png` | missing | |
| `37-query-builder.png` | missing | |
| `38-view-display-menu.png` | missing | |
| `39-list-with-display-field.png` | missing | |
| `40-group-by-menu.jpg` | missing | |
| `41-sort-menu.jpg` | missing | |
| `42-filter-menu.jpg` | missing | |
| `43-search-options-popover.png` | missing | |
| `44-query-editor.png` | missing | |
| `45-search-operators.png` | missing | |
| `46-search-node-options-menu.png` | missing | |
| `47-view-as-submenu.jpg` | missing | |
| `48-table-view.png` | missing | |
| `49-table-column-menu.png` | missing | |
| `74-add-field-column.png` | missing | Adding a field as a table column (needs table view). |
| `50-cards-view.png` | missing | |
| `51-lanes-view.png` | missing | |
| `52-calendar-view.png` | missing | |
| `53-calendar-week-view.png` | missing | |
| `54-side-menu-view.png` | missing | |
| `55-list-nav-view.png` | missing | |

## Beyond the spec

| Area | Status | Notes |
|---|---|---|
| Local file | done | One JSON graph file, atomic saves, autosave, repair of dangling links on load. |
| Undo/redo | done | Every command is one step (typing coalesced); a failed command changes nothing. |
| CLI | done | `tanacraft-cli` inspect, view, add, export (Markdown, JSON), search, tags, run, sample, mcp. |
| MCP | done | `tanacraft-mcp` (or `tanacraft-cli mcp`): headless on a file, or `--connect` to a running app. |
| Control channel | done | `tanacraft --control PORT`: JSON lines on 127.0.0.1. |
