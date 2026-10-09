# TanaCraft parity with the behaviour spec

Maps every `##` section of the clean-room spec (`../tanacraft-spec/*.md`) and every screenshot to its
status here. `cargo xtask parity` fails when a spec section or screenshot is missing from this file.

Status: **done** (implemented and tested), **partial** (works, gaps listed), **missing** (not started),
**n/a** (notes in the spec, not behaviour).

## Summary

| | done | partial | missing | n/a |
|---|---|---|---|---|
| Spec sections (95 rows, spec files 00 to 09 and INDEX) | 74 | 15 | 4 | 2 |
| Screenshots (160) | 139 | 15 | 5 | 1 |
| Beyond the spec | 4 | | | |

## `00-overview.md`

| Section | Status | Notes |
|---|---|---|
| 1. The big picture | done | Single window: top bar, sidebar, panel area; every line is a node; any node opens as a page. |
| 2. Top bar (full window width, ~44 px tall) | done | Sidebar button, back/forward, tab strip with +, then Help (a page listing every command with shortcut and id), keyboard shortcuts toggle and an Account menu (Workspaces, Settings, theme, shortcuts, Help). Credits left out (no billing). |
| 3. Sidebar (left, ~225 px wide, slightly lighter than the panel background) | partial | Today, Supertags, Recents (each can be hidden in Settings), Create new with a square Search button, Pinned (expandable), Workspaces (one header, or every workspace with chevron and items), Manage workspaces. Missing: AI chats (out of scope). |
| 4. Panel area | partial | Breadcrumbs (collapse long chains, content width menu), title with icon, Icon and Banner buttons on hover (banner as a colour band), tag chips, ... menu, content column, related-content side card, References. Missing: AI button, dock button. |
| 5. Visual language | partial | Dark and light themes, tag-tinted chips, guide lines, halos, selection highlight, new search results flash green. Own palette; default egui font (no Inter, bold drawn brighter). |
| 6. Main areas / pages (what each sidebar entry opens) | partial | Today, Supertags overview, Recents, Search and open, Create new, pinned, workspace items, Inbox, Help, Settings. No AI chats. |
| 7. Native menu bar | done | macOS menu bar (muda): app menu (About, Settings, Hide, Quit with save), Edit (Undo, Redo, Select all, Find, Command line, Quick add), View (actual size, zoom in/out, sidebar, theme, content width, full screen), History (Back, Forward, Today, Recents), Options, Window (minimize, tabs, panels), Help. Every item runs a command (`MENU_BAR`, tested); only Quit takes its key from the menu so typing and the clipboard stay with the window. No updater, spellchecker toggle, local API or meeting items. |
| 8. Global overlays | done | Command line (Cmd+K), Search and open (Cmd+S, Cmd+T), Quick add (Cmd+E), context menus, shortcut sheet. |

## `01-data-model.md`

| Section | Status | Notes |
|---|---|---|
| 1. Node | partial | Name with inline marks, description, ordered children, owner, tags, fields (several values), checkbox, heading, icon and banner, view settings, created/edited/done times, expansion, bullet states. Missing: edited-by (one local editor), per-place expansion. |
| 2. Children and the trailing line | done | Ghost line after every child list; click to add. |
| 3. References and inline references | partial | Reference nodes (dashed ring, live), inline references (@ mid-sentence, blue, clickable), References section incl. date backlinks, reference counter setting, Clone reference (Shift+Cmd+C), Remove reference, Hard delete including references, Move and leave reference, notes for this context on a reference (Alt+Cmd+Down, `node.contextNote`), warn on the first edit of a referenced node (setting). Missing: fields for this context. Tested. |
| 4. Supertags (tag definitions) | partial | Colour (none, hues, custom #rrggbb), base type, extends, template fields and child nodes, optional fields, pinned fields, title expression, checkbox, done mapping, default child tag, view tabs, related content, usage. Building block: promote for base types (first in the # menu), hide from searches and menus (`tag.setBuildingBlock`). Missing: AI settings (out of scope). |
| 5. Fields | done | Nine types (plain, options, options from supertag, date with optional time range, number, URL, e-mail, checkbox, user), required, hide modes, auto-initialize, number range, option sources, auto-collect, used in; several values per field (`field.addValue`, `field.removeValue`, `field.set` with `index`; + in the row adds one; search matches any value). Tested. |
| 6. Search nodes | done | Live, editable results, magnifier bullet, views, convert to plain list (`search.toPlainList`). |
| 7. Workspaces | done | All workspaces live in one graph, side by side: the main one is the root (Library, Schema, Daily notes, Inbox made on first use, Trash); each further one (`workspace.create`) has its own Library, Schema and Trash: new supertags join the Schema of the workspace you are in, deleting goes to that workspace's trash, the Supertags overview groups tags by workspace, `WORKSPACE:` query condition. Daily notes and Today follow the main workspace (spec 07 §2). No members (local app). Tested. |
| 8. Calendar nodes (daily notes) | done | Year, ISO week (#week, newest first), day (#day); created lazily, also by date values; relative titles. |
| 9. Trash | done | Delete to trash, restore, banner, empty trash. |
| 10. Other node kinds observed | missing | Chat, command nodes and settings pages not built. |
| 11. Undo | done | Every command is one undo step; typing coalesced; failed commands roll back. |

## `02-outliner-editing.md`

| Section | Status | Notes |
|---|---|---|
| 1. Editing model | done | One line edits at a time, placeholder hint, title Enter makes the first child. Long text wraps (shown and while editing) with the first line level with the bullet; no line breaks inside a node; Home/End go to the node's start/end. Tested. |
| 2. Enter, Shift+Enter | done | Split, first child when expanded, empty line above at position 0, Shift+Enter inserts below. Tested. |
| 3. Tab / Shift+Tab | done | Indent under previous sibling (expands it), outdent after parent, stops at the page. Field rows: after `>`, Tab goes to the value and Enter starts a node below; Enter in a value adds another value; Shift+Tab moves a value out to a node (`field.valueToNode`). Tested. |
| 4. Backspace / Delete | done | Merge into previous sibling, no merge with children, empty line deletes, Delete joins next, deletes selections. Tested. |
| 5. Arrow keys | done | Up/Down between lines, Up into the title, Left at start / Right at end jump lines, Shift+Up/Down selection. |
| 6. Moving nodes | done | Move up/down crossing branches, drag and drop (between rows, as first child, whole selections), Move to..., Move and leave reference to..., Move to top/bottom, Set move target and Move to move target. Tested. |
| 7. Collapse, expand, zoom | done | Cmd+Up/Down, Ctrl+Cmd+Up/Down, chevrons, halos, zoom in/out with caret restore, hover card with edit and create times. |
| 8. Node selection mode | done | Escape selects, Shift+Up/Down and Shift+click extend, floating toolbar (outdent, indent, tag, move, heading), Enter/Backspace/Tab/move/tag on selections, Cmd+C copies. |
| 9. Checkboxes and "done" | done | Cmd+Enter cycle, green checked box, dimmed text, supertag checkboxes, done mapping. |
| 10. Inline formatting | done | Bold, italic, underline, highlight, code marks with Cmd+B/I/U and Ctrl+H; backticks. With a collapsed caret the mark carries on for typed text. While editing, marks show faint and the text between them is styled. Links render blue with a small arrow before them. Bold is drawn brighter (one font weight). Tested. |
| 11. Trigger characters | partial | `#` keeps the text in the line and the menu filters by it (Create and apply on top, tag descriptions, Cmd+Enter creates, Escape leaves the text, opens above the line near the bottom); @ (empty line: reference node; mid-sentence: inline reference; Create), > (field row, Tab to value), / and ! menus. Missing: Space for AI. |
| 12. Copy and paste | done | Cmd+C copies selected nodes as a nested HTML list plus a Markdown list (`export.clipboard`; the desktop app writes both flavours); pasting them inserts references, into an empty line in its place; duplicates are skipped with a notice; URLs show as links and a URL-only node gets a link bullet; multi-line and Markdown paste; Paste as code (`edit.pasteAsCode`). Tested. |
| 13. Description | done | Ctrl+I opens a grey one-line editor under the name; Down from the name enters it, Up or Enter returns; on rows and page titles. Tested. |
| 14. Undo / redo | done | Cmd+Z, Shift+Cmd+Z. |
| 15. Duplicate | done | Shift+Cmd+D, caret stays in the original. |
| 16. Context menu on a node | done | Open, expand, add tag/field/checkbox/description, heading, progress bar, pin, view as, duplicate, move to (also leaving a reference, top, bottom, move target), copy node link (pastes as an inline reference), move, indent, convert search, delete/restore, edit log (`node.editLog`). Publish is left out with sharing (no server). |

## `03-supertags-fields.md`

| Section | Status | Notes |
|---|---|---|
| 1. Supertags overview page (sidebar "Supertags") | done | Filter box, pinned supertags as large cards in the tag colour, then every supertag as a tile under the workspace name. Browse templates left out (no template gallery). |
| 2. Tag page | done | Schema breadcrumb, coloured # badge and checkbox square, header glow, view tabs (Everything tagged, own views, + Add view), + Create new, help button, Configure, table by default. |
| 3. Creating tags | done | # then Enter or Cmd+Enter creates and applies with an auto colour; Convert to supertag (`node.convertToSupertag`: children become template content, the node moves to the Schema and a reference stays). Tested. |
| 4. Applying and removing tags | done | # menu, node menu, command; chip x removes. |
| 5. Configure supertag panel | partial | Opens in its own panel to the right (Done closes it). Colour row (no colour, hues, custom), collapsible cards: Building blocks (base type, extend), Content template (default content with pinned toggles, optional fields, show checkbox, map checked/unchecked), Advanced options (title expression, default child supertag, related content add/remove, shortcuts), used count, delete. Building block promote-for chips and hide checkbox; Add description on the page. Missing: AI and Commands, Voice chat (AI, out of scope). |
| 6. Field definition panel | partial | Type, pre-determined options as a small outline (dashed rings, Empty line to add), sources of options (nodes whose children become options), auto-collect toggle, source tag, min/max, auto-initialize, required, hide, used in. Add description on the page. Fields always live in Schema here, so they are discoverable from the start (spec-gap: no local fields). Missing: AI rows (out of scope). |
| 7. How a tagged node renders | done | Collapsed chips and coloured bullet; expanded field rows before children; zoomed page. |
| 8. Field value pickers | done | Options picker (other options, auto-collected values, create), options-from-supertag picker listing tagged nodes (Tag as #X to create), date picker with ISO week numbers, today circle, selected day, time range and OK (day click alone no longer commits; the opening click no longer closes it), number, checkbox, URL/email as plain text. User fields pick from known names. Date ranges across days: Add end, then the last day (`YYYY-MM-DD/YYYY-MM-DD`); spans show on every day they cover in calendars and match date conditions. Tested. |
| 9. Required, hidden, pinned and invalid fields | done | Empty required fields show an amber ?; invalid URL and e-mail values are kept and get the same ? (Not a valid value); hide-when-empty folds to a + Field button in outlines but stays on the node's page; narrow panels stack field rows; pinned fields come first in view menus. Tested. |
| 10. Template behaviour and inheritance | done | Template fields and nodes are copied, later template additions reach existing tagged nodes, extended supertags pass on fields and nodes and match the parent's searches, mixed views title the first column Title. Title expressions (`${name}` then text and `${Field}`) add a grey computed part after the name on rows, page titles, cards and tables, an empty field shows its name underlined; only the tag's own nodes, not extending tags. Tested. |
| 11. Adding fields | partial | > on a line, template New field / Insert existing, table + Add field adds to the supertag template. AI suggested fields missing. |

## `04-views.md`

| Section | Status | Notes |
|---|---|---|
| 1. Where view controls live | done | View toolbar of square icon buttons: magnifier (quick filter), view as, display, group, sort, filter by, date fields (calendar); active ones get a blue outline and the tooltip summarises them (Displaying:, Grouped by:, Sorted by:); filter chips; options menu. |
| 2. Outline (default) | done | Bullets with display-field line. |
| 3. Table | done | Full panel width; tag-named first column (Title when mixed) with tag-colour underline; field columns with type icon (opens the field) and name (click renames in place); a supertag's template fields are columns even before anything is tagged; resizable columns; + Add field; in-place cells that grow with wrapped text; header context menu: change type, configure, filter, Calculate (sum, average, median, min, max, counts; shown under the rows), sort, move column left/right, hide, hide others, delete column and values. Tested. |
| 4. Cards | done | Grid of cards: tag header, checkbox and name, display fields footer, + Add. |
| 5. Board (cards grouped) | done | Columns per value including empty options and <Field> not set; dragging a card sets the value; + Add per column. |
| 6. Lanes | done | Each item a lane with its children. |
| 7. List (navigation view) | done | A narrow list (name, display fields, + Add); clicking an item opens it in the panel to the right (reused on the next click), the list keeps the focus. Tested. |
| 8. Side menu | done | Compact menu and selected item content; + Add menu item adds an empty item, selects it and puts the caret in its heading. Tested. |
| 9. Tabs | done | Items as tabs, selected item below; + Add tab works like + Add menu item. Tested. |
| 10. Calendar | done | Month grid with week numbers, today circle, chips, drag to another day. Week and Day views: day headers, all-day strip, hour grid (scrolled to 08:00, or the day length you set), now line; timed dates (`YYYY-MM-DD HH:MM-HH:MM`) as events with their range, overlapping ones side by side; drop an item on an hour to time it; drag an event's bottom edge to resize; drag on empty grid opens a New event popover. Date fields and Day length menus; N nodes with no dates. Tested. |
| 11. Display menu | done | Field checkboxes and system fields (created, edited, done time, owner, tags, references). |
| 12. Group by | done | Fields, tags, done, owner, created and edited day; every option gets a group; Not defined; group headers collapse. |
| 13. Sort by | done | Several rules, direction, remove, add, Reapply sort, Reset; empty values first; Sort children commands (`node.sortChildren`). |
| 14. Filter by | done | Checked state, tags, field values; chips with x; N items filtered out. |
| 15. View toolbar visibility | done | view.toggleToolbar. |

## `05-search.md`

| Section | Status | Notes |
|---|---|---|
| 1. Search nodes | done | Slash > Search node > supertag makes "List of X"; magnifier bullet; inline results; + adds a node matching the query to the Library and opens it; new results flash green for a moment. Tested. |
| 2. Query builder | done | Query builder panel: one box per AND / OR / NOT group with its label strip (click to switch AND/OR), condition rows (tag, field, text, operators) with negate and remove, nested groups (+ AND / + OR / + NOT in every group, `search.addGroup` with a path), an Empty line per group, menus for Tag fields (Set / Not set), System fields, Tag types (`TYPE:task`) and Search operators, the query as text, Run once and Done. Results stay live while editing; no workspace scope (one workspace per file). Tested. |
| 3. Command line (Cmd+K) | done | Enter command...: steps for the focused node first (Add tag >, Remove tag, Set <field> >, Move to >, Move and leave reference to >, View as), then commands with shortcuts; steps marked > open a second picker. No node search. Tested. |
| 4. Search and open (Cmd+S, sidebar magnifier) | done | Recently viewed, fuzzy search with breadcrumbs and two-line content snippets (description, long text or a child line with the words), new-tab variant (Cmd+T). Tested. |
| 5. Quick add (Cmd+E) | done | Modal with yellow bar, several lines, Cmd+E saves to the end of today, Escape cancels. |

## `06-daily-notes.md`

| Section | Status | Notes |
|---|---|---|
| 1. Hierarchy | done | Daily notes, year, ISO week (#week, newest first), day (#day); lazy creation. |
| 2. Day titles | done | Today, Yesterday, Tomorrow prefixes; year omitted for the current year. |
| 3. Day page layout (`01-today-shell-dark.png`) | partial | Nav bar, Open tasks card (related content of #day), references. Calendar sync missing. |
| 4. Navigation | done | Previous/next day, Today, Go to date picker marking days that have notes. |
| 5. Daily notes root (`60-daily-notes-root.png`) | done | Today's notes embedded under the title with the nav bar. |
| 6. Week page (`79-week-node.png`) | done | Previous/next week, day nodes, and a Schedule card: the week's time grid with every dated node (by the date field most of them use); New event on it goes on that day. Tested. |
| 7. Year page (`80-year-node.png`) | done | Previous/next year, weeks newest first. |
| 8. Adding to today | done | Typing, quick add, Create new (untitled page on today with suggested tags and Create search node), tag-page Create new. |
| 9. Dates elsewhere | done | Date values (also timed) link to day nodes and show in their References; Insert date... (picker), Insert current date and time, Insert current time put a day link (and time) at the caret. Tested. |

## `07-sidebar-navigation.md`

| Section | Status | Notes |
|---|---|---|
| 1. Sidebar modes | done | Cmd+\ cycles full, mini (icon rail), hidden. |
| 2. Sidebar contents and behaviour | done | Fixed entries, Create new, Search, Pinned with expandable children; one workspace shows its name as the header, several are listed under Workspaces with chevrons and their items; Manage workspaces (create, rename, open). Tested (DIFFS N6). |
| 3. Recents and AI chats | partial | Recents page: list beside the selected node. No AI chats. |
| 4. Tabs | done | Tab strip, Cmd+T (search and open in a new tab), Cmd+W, Ctrl+Tab, Ctrl+Shift+Tab, Cmd+1..9; each tab keeps its panels and history. |
| 5. Panels | done | Cmd+M opens a panel to the right, x closes, own history and breadcrumb, Ctrl+Cmd+M single panel; Configure supertag and list-view details open in the panel to the right. |
| 6. Navigation history | done | Back/forward per panel; zoom out restores the caret. |
| 7. Breadcrumbs | done | Workspace avatar, clickable ancestors, long chains collapse to first / ... / last two. |
| 8. Special pages | done | Library, Schema, Inbox (reached by command, no sidebar row; explains captures), Trash, Settings with a left nav: Preferences (theme, sidebar and top bar entries, reference counter, autosave), Date and time (12/24 hour, week starts Monday or Sunday; saved with the graph, `settings.*`), Journaling, Move targets, Keybindings. No API keys or progress stats (local app). Tested. |

## `08-ai-and-extras.md`

| Section | Status | Notes |
|---|---|---|
| AI | missing | Out of scope so far. |
| Voice / meetings | missing |  |
| Publishing and sharing | missing |  |
| Import / export | done | Markdown, JSON and OPML export (commands, CLI `--format md|json|opml`); Markdown import (nesting, headings, checkboxes, #tags) and OPML import from other outliners (text, notes as descriptions, the same checkbox and #tag rules; `import.opml`, `tanacraft-cli import FILE.opml`); HTML plus text on the clipboard; Markdown bullet paste. Tested. |
| Developer / power features | done | CLI, MCP server and JSON control channel cover scripting; custom keybindings from `keybindings.json` (shortcut: command id, checked before the defaults); Show edit log; Help page listing every command. |
| Misc | partial | Progress bar, content width, page icons and banners. URL embeds missing. |

## `09-shortcuts.md`

| Section | Status | Notes |
|---|---|---|
| Navigating | done | All bindings, including Shift+Cmd+? for the sheet and Ctrl+Shift+PageUp/PageDown to move tabs. Tested. |
| Editing | done | Undo, redo, move, mark done, insert node, duplicate, delete, description, > @ #, create new. |
| Formatting | done | Cmd+B, Cmd+I, Cmd+U, Ctrl+H, backticks. |
| Other keys observed (not in the sheet) | partial | Enter, Backspace, Delete, Up/Down, Left/Right line jumps, Shift+Up/Down, Escape, selection keys, /, Cmd+S, Cmd+E, paste, Cmd+A, Shift+click, Tab in field names, Shift+Cmd+C clone reference. Alt+Cmd+Down (context notes). Missing: Space AI (out of scope). |

## `INDEX.md`

| Section | Status | Notes |
|---|---|---|
| Open gaps (still not observed or not verified) | n/a | The spec team's open questions. |
| Test data left in the demo workspace | n/a | Spec notes. |

## Screenshots

| Screenshot | Status | Notes |
|---|---|---|
| `01-today-shell-dark.png` | partial | Shell: sidebar, tabs, breadcrumbs, day page with Open tasks card. No AI button. |
| `02-shortcuts-panel-1.png` | done | Shortcut sheet bindings. |
| `03-shortcuts-panel-2.png` | done | Editing and formatting shortcuts, Shift+Cmd+? opens the sheet. |
| `10-supertags-overview.png` | done | Pinned supertags as coloured cards, then the workspace's tiles. |
| `11-tag-page-task.png` | partial | Tag page with view tabs; board default view differs. |
| `12-tag-task-calendar.png` | done | Task calendar view. |
| `13-tag-task-list.png` | done | Tag list view. |
| `14-node-menu.jpg` | partial | Node menu: open, open in new panel, view as, toolbar, duplicate, moves, copy link, tag/field/checkbox/description, heading, progress, pin, edit log, more commands, convert to supertag, delete / hard delete / remove reference. No AI, publish or appearance. |
| `15-configure-supertag-task.png` | partial | Configure panel to the right with colour row and collapsible cards; no AI or voice sections. |
| `16-supertag-content-template.png` | done | Content template: field rows with type icon, default value, pinned checkbox and remove; template nodes; New field / Insert existing field. Tested. |
| `17-supertag-config-lower.png` | done | Optional fields, show checkbox, map checkbox to field with checked and unchecked values. Tested. |
| `18-supertag-advanced.png` | done | Advanced options: title expression, default child supertag, related content, building block (promote for, hide), shortcuts. Tested. |
| `19-field-config-options.png` | done | Field page: type, pre-determined options outline, sources of options, auto-collect, auto-initialize, required, hide field, used in, Delete, Add description. AI rows left out. |
| `20-field-type-dropdown.png` | done | Field types including User. |
| `21-tagged-node-zoomed.png` | done | Tagged node zoomed. |
| `22-options-picker.png` | done | Options picker lists the other options. |
| `23-date-picker.png` | done | Week numbers, today circle, selected day, time and end inputs, OK. |
| `24-hash-tag-menu.png` | done | # menu. |
| `25-node-tagged-task-inline.png` | done | Tagged node inline. |
| `26-tagged-node-expanded-fields.png` | done | Expanded fields. |
| `27-at-reference-menu.png` | done | @ reference menu. |
| `28-inline-reference.png` | done | Inline references. |
| `29-multi-select.jpg` | done | Multi-select. |
| `30-text-selection-no-toolbar.png` | n/a | Spec: no toolbar appears on text selection. |
| `31-inline-formatting.png` | done | Inline formatting. |
| `32-field-create-menu.png` | done | > field create. |
| `33-field-name-autocomplete.png` | done | Field name autocomplete. |
| `34-slash-menu.png` | done | Slash menu. |
| `35-search-node-new.png` | done | Create search node. |
| `36-search-node-result.png` | done | Search node results. |
| `37-query-builder.png` | done | Query builder with group boxes, condition rows, Empty lines and the menus. |
| `38-view-display-menu.png` | done | Display menu with fields and system fields. |
| `39-list-with-display-field.png` | done | Display field line. |
| `40-group-by-menu.jpg` | done | Group by: none, tags, done, owner, created, edited and every field; hidden groups can be shown again. |
| `41-sort-menu.jpg` | done | Sort menu. |
| `42-filter-menu.jpg` | done | Filter menu. |
| `43-search-options-popover.png` | done | Conditions opens a Search card with the query as a sentence and Open query editor. |
| `44-query-editor.png` | done | Query editor with nested groups, Run once and Done. |
| `45-search-operators.png` | partial | Search operators: TODO, DONE, DONE/CREATED/EDITED last N days, HAS TAG/FIELD, IS TAG/FIELD/SEARCH/CALENDAR, IN LIBRARY, ON DAY, FOR DATE; no user, media or chat operators. |
| `46-search-node-options-menu.png` | partial | Search node options menu subset. |
| `47-view-as-submenu.jpg` | done | View as submenu. |
| `48-table-view.png` | done | Table view. |
| `49-table-column-icon-field-config.png` | done | The column header icon opens the field definition; the name renames in place. |
| `50-cards-view.png` | done | Cards view. |
| `51-lanes-view.png` | done | Lanes view. |
| `52-calendar-view.png` | done | Calendar month view. |
| `53-calendar-week-view.png` | done | Week view with all-day strip, hour grid and now line. |
| `54-side-menu-view.png` | done | Side menu view. |
| `55-list-nav-view.png` | done | List with the selected item in the panel to the right. |
| `56-command-line.png` | done | Command line. |
| `57-search-panel.png` | done | Search and open. |
| `58-search-typed.png` | done | Fuzzy results. |
| `59-quick-add.png` | done | Quick add. |
| `60-daily-notes-root.png` | done | Daily notes root. |
| `61-daily-notes-prev-day.png` | done | Previous day. |
| `62-recents.png` | done | Recents page. |
| `63-ai-chats.png` | missing | AI chats. |
| `64-create-new.png` | done | Create new: untitled page with suggestions. |
| `65-settings-menu.png` | partial | Account menu: Workspaces, Settings, theme, shortcuts, Help. No account, labs, API tokens or log out (local app). |
| `66-settings-page.png` | done | Settings: left nav of sub-pages, preference rows with label, help and checkbox. Tested. |
| `67-today-light-mode.png` | done | Light mode. |
| `68-reference-node.png` | done | Reference node. |
| `69-bullet-context-menu.png` | partial | Bullet context menu (see 14); no AI, publish or appearance. |
| `70-trash.png` | done | Trash. |
| `71-sidebar-hidden-two-tabs.png` | done | Sidebar hidden and tabs. |
| `72-two-panels.png` | done | Two panels. |
| `73-new-tag-page-empty.png` | done | New tag page with table. |
| `74-add-field-column.png` | partial | Add field column; no AI suggestions. |
| `75-number-field-config.png` | done | Number range. |
| `76-table-number-cell.png` | done | Table number cell. |
| `77-add-view-menu.png` | done | Add view menu. |
| `78-description-and-heading.png` | done | Heading and a description edited in place under the name. |
| `79-week-node.png` | done | Week page with day nodes and the Schedule card. |
| `80-year-node.png` | done | Year page. |
| `81-space-ai-prompt.png` | missing | AI prompt. |
| `82-sidebar-pinned-expanded.png` | done | Expandable pinned rows. |
| `83-help-menu.png` | partial | Help opens a page listing every command; no docs links. |
| `84-credits-popover.png` | missing | Credits. |
| `85-library.png` | done | Library. |
| `86-schema-node.png` | done | Schema. |
| `87-tag-page-create-new.png` | partial | Tag page Create new (no suggestions). |
| `88-board-after-drag.png` | done | Board drag. |
| `89-sort-add-menu.png` | done | Add sort. |
| `90-filter-checked-submenu.png` | done | Filter checked state. |
| `91-filtered-not-done.png` | done | Filtered not done. |
| `92-grouped-outline.png` | done | Grouped outline. |
| `93-query-with-field-condition.png` | done | Field conditions show the field's type icon and name, then the value. |
| `94-go-to-date-picker.png` | done | Go to date marks days with notes. |
| `95-future-day-with-dated-task.png` | done | Future day created by a date. |
| `96-tabs-view.png` | done | Tabs view. |
| `97-move-down-into-next-branch.jpg` | done | Move down into the next branch. |
| `98-shift-click-range-select.jpg` | done | Shift+click range select. |
| `99-multi-drag.jpg` | done | Dragging several nodes. |
| `100-drag-into-empty-child-list.jpg` | done | An expanded node with no children shows a ghost line; dropping on it nests the node (click adds a first child). Tested. |
| `101-drag-drop-line-depth.jpg` | done | Upper half drops above, lower half below, at the row's depth; the drop line starts at that depth; horizontal offset does not nest. |
| `102-cmd-a-select-all.jpg` | done | Cmd+A: text, then all nodes. |
| `103-paste-duplicate-skipped-toast.png` | done | Pasting nodes already in the list skips them with a notice at the top right. |
| `104-paste-node-as-reference.png` | done | Pasting copied nodes inserts references. |
| `105-pasted-url.png` | done | URLs in text render blue with an arrow before them. |
| `106-pasted-url-empty-line.png` | done | A URL-only node gets the link bullet and blue text. |
| `107-system-field-owner-node.jpg` | done | Owner node system field. |
| `108-field-row-tab-enter.jpg` | done | Tab from the field name to the value; Enter adds a value line; Shift+Tab moves a value out. Tested. |
| `109-user-field-picker.png` | partial | User field picker lists you and every name used in user fields (one local user, no workspace members). |
| `110-typed-field-values.png` | done | Typed field values validated. |
| `111-options-from-supertag-picker.png` | done | Options from supertag: dropdown of tagged nodes; a new name is tagged on commit. |
| `112-new-instance-value.png` | done | New instance value. |
| `113-instance-value-auto-tagged.jpg` | done | Instance value auto-tagged. |
| `114-template-with-child-node.png` | done | Template child node copied. |
| `115-required-field-and-template-child.png` | done | Required field and template child. |
| `116-hidden-empty-field-outline.png` | done | Hidden empty field. |
| `117-template-change-propagates.png` | done | Template additions reach nodes tagged earlier. |
| `118-extend-supertag.png` | done | Extend supertag. |
| `119-tag-page-extends.jpg` | done | Tag page shows extends. |
| `120-extended-tag-instance.png` | done | Extended tag instance gets inherited fields. |
| `121-parent-tag-includes-extended.png` | done | Searches for the parent tag include extended tags. |
| `122-query-or-group.png` | done | OR group. |
| `123-query-or-filled.png` | done | OR group filled. |
| `124-query-not-group.png` | done | NOT group. |
| `125-calendar-drag.jpg` | done | Calendar drag. |
| `126-calendar-week-timed-drop.jpg` | done | Dropping an item on an hour gives it a time range. |
| `127-lanes-lab.png` | done | Lanes. |
| `128-lanes-drag.jpg` | done | Drag a node by its grip into another lane: it moves under that lane's item. Tested. |
| `129-content-width-menu.png` | done | Content width menu (Auto, Medium, Full) at the right of the breadcrumb; Cycle content widths command. |
| `130-panel-resized.png` | done | Drag the gap between two panels to resize them; narrow panels stack field rows. Tested. |
| `131-two-tabs.png` | done | Two tabs. |
| `132-mini-sidebar-pinned-flyout.jpg` | done | Mini rail Pinned button opens a flyout of pinned nodes. Tested. |
| `133-mini-sidebar-workspace-flyout.jpg` | done | Mini rail Workspaces button: workspace items and Manage workspaces. |
| `134-manage-workspaces.png` | partial | Manage workspaces menu: open, create, rename. No sharing banner or members (local app). |
| `135-create-workspace.png` | done | Create workspace by name; it is added to the sidebar beside the others. |
| `136-new-workspace.png` | done | A new workspace has its own Library, Schema and Trash; Today and Daily notes follow the main workspace. Tested. |
| `137-workspace-invite-step.jpg` | missing | Workspace invite. |
| `138-sidebar-two-workspaces.png` | done | Workspaces section lists every workspace with avatar, name, chevron and its items. Tested. |
| `139-workspace-home.png` | partial | Workspace home: large round avatar with initials and the name; no member count (local app). |
| `140-workspace-settings-other-ws.jpg` | partial | Date and time settings exist, saved with the graph for all its workspaces (not per workspace). |
| `141-settings-api-keys.png` | missing | API keys settings. |
| `142-invalid-value-warning.jpg` | done | Invalid URL / e-mail keeps the text and shows a yellow ? at the row's end. |
| `143-board-column-add.png` | done | Board column + Add. |
| `144-board-hidden-column.png` | done | Hidden board column. |
| `145-group-menu-hidden-groups.png` | done | Group menu lists hidden groups. |
| `146-field-name-autocomplete.jpg` | done | Field name autocomplete. |
| `147-auto-collected-option.png` | done | Auto-collected option. |
| `148-calendar-day-view.png` | done | Day view: one column time grid. |
| `149-multi-select-indent.png` | done | Indent a multi-selection. |
| `150-bang-heading.jpg` | done | ! on an empty line makes a heading. |
| `151-heading-with-child.jpg` | done | Heading with children. |
| `152-checklist-inserted.jpg` | done | Checklist from the slash menu. |
| `153-checklist-progress.png` | done | Checklist progress bar. |
| `154-table-column-resize.jpg` | done | Table column resize (drag a header edge; saved with the view). |
| `155-calendar-drag-create.png` | done | Drag on the grid, New event popover with the range. Tested. |
| `156-calendar-event-resize.jpg` | done | Drag the bottom edge to change the end time. Tested. |
| `157-title-expr-default-child-config.png` | done | Advanced options: title expression input and default child supertag picker. |
| `158-title-expression-table.png` | done | Grey computed title part in tables and lists; empty fields show their name underlined. |
| `159-default-child-supertag.png` | done | New direct children of a tagged node get the default child supertag. |
| `160-side-menu-add-item.jpg` | done | + Add menu item: new empty item selected, caret in its heading, typing names it. Tested. |
| `161-tabs-add-tab.jpg` | done | + Add tab: new last tab, active, caret in its heading. Tested. |
| `162-search-node-opened-no-builder.jpg` | done | An opened search node shows toolbar and results; the builder opens from Open query editor. Tested (DIFFS N10). |
| `163-table-header-menu.jpg` | done | Header context menu in five groups (type, configure, filter; Calculate; sort; move column; hide, hide others, delete column and values). Tested. |
| `164-hide-when-not-empty.jpg` | done | A filled "hide when not empty" field folds to a grey + Field label; clicking opens the row. Tested. |
| `165-hide-always.jpg` | done | "Always hide" folds the same way regardless of the value. Tested. |
| `166-multi-value-field.jpg` | done | Several values stack, one bulleted line each; Enter in a value adds another (DIFFS N1, N8). Tested. |

## Beyond the spec

| Area | Status | Notes |
|---|---|---|
| Local file | done | One JSON graph file, atomic saves, autosave, repair of dangling links on load. |
| CLI | done | `tanacraft-cli` inspect, view, add, export (Markdown, JSON), search, tags, run, sample, mcp. |
| MCP | done | `tanacraft-mcp`: headless on a file, or `--connect` to a running app. |
| Control channel | done | `tanacraft --control PORT`: JSON lines on 127.0.0.1. |
