# TanaCraft parity with the behaviour spec

Maps every `##` section of the clean-room spec (`../tanacraft-spec/*.md`) and every screenshot to its
status here. `cargo xtask parity` fails when a spec section or screenshot is missing from this file.

Status: **done** (implemented and tested), **partial** (works, gaps listed), **missing** (not started),
**n/a** (notes in the spec, not behaviour).

## Summary

| | done | partial | missing | n/a |
|---|---|---|---|---|
| Spec sections (99) | 59 | 33 | 5 | 2 |
| Screenshots (153) | 99 | 32 | 21 | 1 |

## `00-overview.md`

| Section | Status | Notes |
|---|---|---|
| 1. The big picture | done | Single window: top bar, sidebar, panel area; every line is a node; any node opens as a page. |
| 2. Top bar (full window width, ~44 px tall) | done | Sidebar button, back/forward, tab strip with +, then Help (a page listing every command with shortcut and id), keyboard shortcuts toggle and an Account menu (Workspaces, Settings, theme, shortcuts, Help). Credits left out (no billing). |
| 3. Sidebar (left, ~225 px wide, slightly lighter than the panel background) | partial | Today, Supertags, Recents, Create new with a square Search button, Pinned (expandable), workspace avatar and name (menu: switch, new, rename), Library, Daily notes, Inbox, Trash. Missing: AI chats. |
| 4. Panel area | partial | Breadcrumbs, title, tag chips, ... menu, content column, related-content side card, References. Missing: icon/banner, breadcrumb collapse, AI button, dock button. |
| 5. Visual language | partial | Dark and light themes, tag-tinted chips, guide lines, halos, selection highlight. Own palette; default egui font (no Inter); no flash on new live results. |
| 6. Main areas / pages (what each sidebar entry opens) | partial | Today, Supertags overview, Search and open, Create new, pinned, workspace items. Recents and AI chats pages missing. |
| 7. Native menu bar | missing | Not built (egui window has no native menus). |
| 8. Global overlays | done | Command line (Cmd+K), Search and open (Cmd+S, Cmd+T), Quick add (Cmd+E), context menus, shortcut sheet. |

## `01-data-model.md`

| Section | Status | Notes |
|---|---|---|
| 1. Node | partial | Name with inline marks, description, ordered children, owner, tags, fields, checkbox, heading, view settings, created/edited/done times, expansion, bullet states. Missing: icon, banner, edited-by, per-place expansion. |
| 2. Children and the trailing line | done | Ghost line after every child list; click to add. |
| 3. References and inline references | partial | Reference nodes (dashed ring, live), inline references (@ mid-sentence, blue, clickable), References section incl. date backlinks, reference counter setting, Clone reference (Shift+Cmd+C), Remove reference, Hard delete including references, Move and leave reference. Missing: per-context notes and fields (Alt+Cmd+Down), warn when editing referenced nodes. |
| 4. Supertags (tag definitions) | partial | Colour (none, hues, custom #rrggbb), base type, extends, template fields and child nodes, optional fields, pinned fields, title expression, checkbox, done mapping, default child tag, view tabs, related content, usage. Missing: building-block promotion, AI settings. |
| 5. Fields | partial | Eight types (no user type), required, hide modes, auto-initialize, number range, used in. Several values per field (`field.addValue`, `field.removeValue`, `field.set` with `index`; + in the row adds one; search matches any value). Missing: user type. |
| 6. Search nodes | done | Live, editable results, magnifier bullet, views, convert to plain list (`search.toPlainList`). |
| 7. Workspaces | partial | Each workspace is its own graph file in one folder: list, create, switch (saves first), rename (`workspace.*`); sidebar avatar and name open the workspace menu. Library, Schema, Daily notes, Inbox (made on first use; `node.capture`, `tanacraft-cli capture`), Trash. Missing: tags grouped by several workspaces in one overview, cross-workspace search. |
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
| 12. Copy and paste | partial | Cmd+C copies selected nodes as a Markdown list; pasting them inserts references; duplicates are skipped with a notice; URLs show as links and a URL-only node gets a link bullet; multi-line and Markdown paste. Missing: rich HTML on the clipboard, paste as code. |
| 13. Description | done | Ctrl+I opens a grey one-line editor under the name; Down from the name enters it, Up or Enter returns; on rows and page titles. Tested. |
| 14. Undo / redo | done | Cmd+Z, Shift+Cmd+Z. |
| 15. Duplicate | done | Shift+Cmd+D, caret stays in the original. |
| 16. Context menu on a node | partial | Open, expand, add tag/field/checkbox/description, heading, progress bar, pin, view as, duplicate, move to (also leaving a reference, top, bottom, move target), copy node link (pastes as an inline reference), move, indent, convert search, delete/restore. Missing: publish, edit log. |

## `03-supertags-fields.md`

| Section | Status | Notes |
|---|---|---|
| 1. Supertags overview page (sidebar "Supertags") | done | Filter box, pinned supertags as large cards in the tag colour, then every supertag as a tile under the workspace name. Browse templates left out (no template gallery). |
| 2. Tag page | partial | View tabs (Everything tagged, own views, + Add view), + Create new, table by default, header glow. Missing: help button. |
| 3. Creating tags | done | # then Enter or Cmd+Enter creates and applies with an auto colour; Convert to supertag (`node.convertToSupertag`: children become template content, the node moves to the Schema and a reference stays). Tested. |
| 4. Applying and removing tags | done | # menu, node menu, command; chip x removes. |
| 5. Configure supertag panel | partial | Opens in its own panel to the right (Done closes it). Colour row (no colour, hues, custom), collapsible cards: Building blocks (base type, extend), Content template (default content with pinned toggles, optional fields, show checkbox, map checked/unchecked), Advanced options (title expression, default child supertag, related content add/remove, shortcuts), used count, delete. Missing: AI and Commands, Voice chat, building-block promotion, tag description. |
| 6. Field definition panel | partial | Type, pre-determined options as a small outline (dashed rings, Empty line to add), sources of options (nodes whose children become options), auto-collect toggle, source tag, min/max, auto-initialize, required, hide, used in. Missing: make discoverable, AI rows, field description. |
| 7. How a tagged node renders | done | Collapsed chips and coloured bullet; expanded field rows before children; zoomed page. |
| 8. Field value pickers | partial | Options picker (other options, auto-collected values, create), options-from-supertag picker listing tagged nodes (Tag as #X to create), date picker with ISO week numbers, today circle, selected day, time range and OK (day click alone no longer commits; the opening click no longer closes it), number, checkbox, URL/email as plain text. Missing: user type, date ranges across days. Tested. |
| 9. Required, hidden, pinned and invalid fields | done | Empty required fields show an amber ?; invalid URL and e-mail values are kept and get the same ? (Not a valid value); hide-when-empty folds to a + Field button in outlines but stays on the node's page; narrow panels stack field rows; pinned fields come first in view menus. Tested. |
| 10. Template behaviour and inheritance | done | Template fields and nodes are copied, later template additions reach existing tagged nodes, extended supertags pass on fields and nodes and match the parent's searches, mixed views title the first column Title. Title expressions (`${name}` then text and `${Field}`) add a grey computed part after the name on rows, page titles, cards and tables, an empty field shows its name underlined; only the tag's own nodes, not extending tags. Tested. |
| 11. Adding fields | partial | > on a line, template New field / Insert existing, table + Add field adds to the supertag template. AI suggested fields missing. |

## `04-views.md`

| Section | Status | Notes |
|---|---|---|
| 1. Where view controls live | partial | Toolbar: quick filter, view as, display, group, sort, filter, active filter chips; options menu. Text buttons rather than icons. |
| 2. Outline (default) | done | Bullets with display-field line. |
| 3. Table | partial | Tag-named first column (Title when mixed) with tag-colour underline, field columns, resizable columns, sort by header, + Add field, in-place cells. Header rename in place missing. |
| 4. Cards | done | Grid of cards: tag header, checkbox and name, display fields footer, + Add. |
| 5. Board (cards grouped) | done | Columns per value including empty options and <Field> not set; dragging a card sets the value; + Add per column. |
| 6. Lanes | done | Each item a lane with its children. |
| 7. List (navigation view) | partial | Item list with the selected item beside it (same panel, not a second panel). |
| 8. Side menu | done | Compact menu and selected item content. |
| 9. Tabs | done | Items as tabs, selected item below. |
| 10. Calendar | partial | Month grid with week numbers, today circle, chips, drag to another day. Week and Day views: day headers, all-day strip, 24-hour grid (scrolled to 08:00), now line; timed dates (`YYYY-MM-DD HH:MM-HH:MM`) as events with their range; drop an item on an hour to time it (keeps its length, else one hour); drag an event's bottom edge to resize; drag on empty grid opens a New event popover (Enter creates, Escape cancels). Date field menu, N nodes with no dates. Missing: day length setting, overlapping events side by side. |
| 11. Display menu | done | Field checkboxes and system fields (created, edited, done time, owner, tags, references). |
| 12. Group by | done | Fields, tags, done, owner, created and edited day; every option gets a group; Not defined; group headers collapse. |
| 13. Sort by | partial | Several rules, direction, remove, add, reset; empty values first. Reapply sort and sort-children commands missing. |
| 14. Filter by | done | Checked state, tags, field values; chips with x; N items filtered out. |
| 15. View toolbar visibility | done | view.toggleToolbar. |

## `05-search.md`

| Section | Status | Notes |
|---|---|---|
| 1. Search nodes | partial | Slash > Search node > supertag makes "List of X"; magnifier bullet; inline results; + adds a node matching the query to the Library and opens it. New-result flash missing. |
| 2. Query builder | partial | Query builder panel: one box per AND / OR / NOT group with its label strip (click to switch AND/OR), condition rows (tag, field, text, operators) with negate and remove, nested groups (+ AND / + OR / + NOT in every group, `search.addGroup` with a path), an Empty line per group to type a condition, Search operators menu, the query as text, Run once and Done. Results stay live while editing. Missing: Tag fields / System fields / Field values / Tag types menus, workspace scope. |
| 3. Command line (Cmd+K) | done | Enter command...: steps for the focused node first (Add tag >, Remove tag, Set <field> >, Move to >, Move and leave reference to >, View as), then commands with shortcuts; steps marked > open a second picker. No node search. Tested. |
| 4. Search and open (Cmd+S, sidebar magnifier) | partial | Recently viewed, fuzzy search with breadcrumbs, new-tab variant (Cmd+T). Content snippets missing. |
| 5. Quick add (Cmd+E) | done | Modal with yellow bar, several lines, Cmd+E saves to the end of today, Escape cancels. |

## `06-daily-notes.md`

| Section | Status | Notes |
|---|---|---|
| 1. Hierarchy | done | Daily notes, year, ISO week (#week, newest first), day (#day); lazy creation. |
| 2. Day titles | done | Today, Yesterday, Tomorrow prefixes; year omitted for the current year. |
| 3. Day page layout (`01-today-shell-dark.png`) | partial | Nav bar, Open tasks card (related content of #day), references. Calendar sync missing. |
| 4. Navigation | done | Previous/next day, Today, Go to date picker marking days that have notes. |
| 5. Daily notes root (`60-daily-notes-root.png`) | done | Today's notes embedded under the title with the nav bar. |
| 6. Week page (`79-week-node.png`) | partial | Previous/next week, day nodes. Schedule card missing. |
| 7. Year page (`80-year-node.png`) | done | Previous/next year, weeks newest first. |
| 8. Adding to today | done | Typing, quick add, Create new (untitled page on today with suggested tags and Create search node), tag-page Create new. |
| 9. Dates elsewhere | done | Date values (also timed) link to day nodes and show in their References; Insert date... (picker), Insert current date and time, Insert current time put a day link (and time) at the caret. Tested. |

## `07-sidebar-navigation.md`

| Section | Status | Notes |
|---|---|---|
| 1. Sidebar modes | done | Cmd+\ cycles full, mini (icon rail), hidden. |
| 2. Sidebar contents and behaviour | done | Fixed entries, Create new, Search, Pinned with expandable children, workspace items. |
| 3. Recents and AI chats | partial | Recents page: list beside the selected node. No AI chats. |
| 4. Tabs | done | Tab strip, Cmd+T (search and open in a new tab), Cmd+W, Ctrl+Tab, Ctrl+Shift+Tab, Cmd+1..9; each tab keeps its panels and history. |
| 5. Panels | partial | Cmd+M opens a panel to the right, x closes, own history and breadcrumb, Ctrl+Cmd+M single panel. Configure and list views stay in the same panel. |
| 6. Navigation history | done | Back/forward per panel; zoom out restores the caret. |
| 7. Breadcrumbs | done | Workspace avatar, clickable ancestors, long chains collapse to first / ... / last two. |
| 8. Special pages | done | Library, Schema, Inbox (Open capture inbox; explains where captures come from), Trash, Settings (theme, sidebar, autosave). |

## `08-ai-and-extras.md`

| Section | Status | Notes |
|---|---|---|
| AI | missing | Out of scope so far. |
| Voice / meetings | missing |  |
| Publishing and sharing | missing |  |
| Import / export | partial | Markdown and JSON export (commands, CLI); Markdown bullet paste. No import of other formats. |
| Developer / power features | partial | CLI, MCP server and JSON control channel cover scripting. Customize keybindings, edit log missing. |
| Misc | partial | Progress bar. Content width, icons/banners, URL embeds missing. |

## `09-shortcuts.md`

| Section | Status | Notes |
|---|---|---|
| Navigating | done | All bindings, including Shift+Cmd+? for the sheet and Ctrl+Shift+PageUp/PageDown to move tabs. Tested. |
| Editing | done | Undo, redo, move, mark done, insert node, duplicate, delete, description, > @ #, create new. |
| Formatting | done | Cmd+B, Cmd+I, Cmd+U, Ctrl+H, backticks. |
| Other keys observed (not in the sheet) | partial | Enter, Backspace, Delete, Up/Down, Left/Right line jumps, Shift+Up/Down, Escape, selection keys, /, Cmd+S, Cmd+E, paste, Cmd+A, Shift+click, Tab in field names, Shift+Cmd+C clone reference. Missing: Space AI, Alt+Cmd+Down (per-context notes). |

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
| `03-shortcuts-panel-2.png` | partial | Editing and formatting shortcuts; Shift+Cmd+? differs. |
| `10-supertags-overview.png` | partial | Supertags overview grid; no pinned cards. |
| `11-tag-page-task.png` | partial | Tag page with view tabs; board default view differs. |
| `12-tag-task-calendar.png` | done | Task calendar view. |
| `13-tag-task-list.png` | done | Tag list view. |
| `14-node-menu.jpg` | partial | Node menu subset. |
| `15-configure-supertag-task.png` | partial | Configure panel to the right with colour row and collapsible cards; no AI or voice sections. |
| `16-supertag-content-template.png` | partial | Content template with fields and nodes. |
| `17-supertag-config-lower.png` | partial | Checkbox and done mapping. |
| `18-supertag-advanced.png` | partial | Default child supertag only. |
| `19-field-config-options.png` | partial | Field page options. |
| `20-field-type-dropdown.png` | partial | Field types without user type. |
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
| `37-query-builder.png` | partial | Query builder (conditions panel). |
| `38-view-display-menu.png` | partial | Display menu without system fields. |
| `39-list-with-display-field.png` | done | Display field line. |
| `40-group-by-menu.jpg` | partial | Group by without system fields. |
| `41-sort-menu.jpg` | done | Sort menu. |
| `42-filter-menu.jpg` | done | Filter menu. |
| `43-search-options-popover.png` | partial | Search options summary in the conditions panel. |
| `44-query-editor.png` | partial | Query editor. |
| `45-search-operators.png` | partial | Search operators subset. |
| `46-search-node-options-menu.png` | partial | Search node options menu subset. |
| `47-view-as-submenu.jpg` | done | View as submenu. |
| `48-table-view.png` | done | Table view. |
| `49-table-column-icon-field-config.png` | partial | Column menu: open field, remove. |
| `50-cards-view.png` | done | Cards view. |
| `51-lanes-view.png` | done | Lanes view. |
| `52-calendar-view.png` | done | Calendar month view. |
| `53-calendar-week-view.png` | done | Week view with all-day strip, hour grid and now line. |
| `54-side-menu-view.png` | done | Side menu view. |
| `55-list-nav-view.png` | partial | List view in one panel. |
| `56-command-line.png` | done | Command line. |
| `57-search-panel.png` | done | Search and open. |
| `58-search-typed.png` | done | Fuzzy results. |
| `59-quick-add.png` | done | Quick add. |
| `60-daily-notes-root.png` | done | Daily notes root. |
| `61-daily-notes-prev-day.png` | done | Previous day. |
| `62-recents.png` | done | Recents page. |
| `63-ai-chats.png` | missing | AI chats. |
| `64-create-new.png` | done | Create new: untitled page with suggestions. |
| `65-settings-menu.png` | missing | Account menu. |
| `66-settings-page.png` | partial | Settings page with our preferences. |
| `67-today-light-mode.png` | done | Light mode. |
| `68-reference-node.png` | done | Reference node. |
| `69-bullet-context-menu.png` | partial | Bullet context menu subset. |
| `70-trash.png` | done | Trash. |
| `71-sidebar-hidden-two-tabs.png` | done | Sidebar hidden and tabs. |
| `72-two-panels.png` | done | Two panels. |
| `73-new-tag-page-empty.png` | done | New tag page with table. |
| `74-add-field-column.png` | partial | Add field column; no AI suggestions. |
| `75-number-field-config.png` | done | Number range. |
| `76-table-number-cell.png` | done | Table number cell. |
| `77-add-view-menu.png` | done | Add view menu. |
| `78-description-and-heading.png` | partial | Heading and description (dialog). |
| `79-week-node.png` | partial | Week page without Schedule. |
| `80-year-node.png` | done | Year page. |
| `81-space-ai-prompt.png` | missing | AI prompt. |
| `82-sidebar-pinned-expanded.png` | done | Expandable pinned rows. |
| `83-help-menu.png` | missing | Help menu. |
| `84-credits-popover.png` | missing | Credits. |
| `85-library.png` | done | Library. |
| `86-schema-node.png` | done | Schema. |
| `87-tag-page-create-new.png` | partial | Tag page Create new (no suggestions). |
| `88-board-after-drag.png` | done | Board drag. |
| `89-sort-add-menu.png` | done | Add sort. |
| `90-filter-checked-submenu.png` | done | Filter checked state. |
| `91-filtered-not-done.png` | done | Filtered not done. |
| `92-grouped-outline.png` | done | Grouped outline. |
| `93-query-with-field-condition.png` | partial | Field condition (text form). |
| `94-go-to-date-picker.png` | done | Go to date marks days with notes. |
| `95-future-day-with-dated-task.png` | done | Future day created by a date. |
| `96-tabs-view.png` | done | Tabs view. |
| `97-move-down-into-next-branch.jpg` | done | Move down into the next branch. |
| `98-shift-click-range-select.jpg` | done | Shift+click range select. |
| `99-multi-drag.jpg` | done | Dragging several nodes. |
| `100-drag-into-empty-child-list.jpg` | partial | Drop as first child. |
| `101-drag-drop-line-depth.jpg` | partial | Drop line depth (child by pointer offset). |
| `102-cmd-a-select-all.jpg` | done | Cmd+A: text, then all nodes. |
| `103-paste-duplicate-skipped-toast.png` | missing | Paste duplicate toast. |
| `104-paste-node-as-reference.png` | done | Pasting copied nodes inserts references. |
| `105-pasted-url.png` | partial | URLs in text render as links; no paste special-casing. |
| `106-pasted-url-empty-line.png` | partial | URLs in text render as links. |
| `107-system-field-owner-node.jpg` | done | Owner node system field. |
| `108-field-row-tab-enter.jpg` | missing | Tab and Enter in field rows. |
| `109-user-field-picker.png` | missing | User field picker. |
| `110-typed-field-values.png` | done | Typed field values validated. |
| `111-options-from-supertag-picker.png` | done | Options from supertag: dropdown of tagged nodes; a new name is tagged on commit. |
| `112-new-instance-value.png` | done | New instance value. |
| `113-instance-value-auto-tagged.jpg` | done | Instance value auto-tagged. |
| `114-template-with-child-node.png` | done | Template child node copied. |
| `115-required-field-and-template-child.png` | done | Required field and template child. |
| `116-hidden-empty-field-outline.png` | done | Hidden empty field. |
| `117-template-change-propagates.png` | partial | New template fields reach tagged nodes; template nodes do not. |
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
| `128-lanes-drag.jpg` | missing | Lanes drag. |
| `129-content-width-menu.png` | missing | Content width. |
| `130-panel-resized.png` | missing | Panel resize. |
| `131-two-tabs.png` | done | Two tabs. |
| `132-mini-sidebar-pinned-flyout.jpg` | missing | Mini sidebar flyout for pinned items. |
| `133-mini-sidebar-workspace-flyout.jpg` | missing | Mini sidebar workspace flyout. |
| `134-manage-workspaces.png` | missing | Manage workspaces. |
| `135-create-workspace.png` | missing | Create workspace. |
| `136-new-workspace.png` | missing | New workspace. |
| `137-workspace-invite-step.jpg` | missing | Workspace invite. |
| `138-sidebar-two-workspaces.png` | missing | Two workspaces in the sidebar. |
| `139-workspace-home.png` | missing | Workspace home. |
| `140-workspace-settings-other-ws.jpg` | missing | Workspace settings. |
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

## Beyond the spec

| Area | Status | Notes |
|---|---|---|
| Local file | done | One JSON graph file, atomic saves, autosave, repair of dangling links on load. |
| CLI | done | `tanacraft-cli` inspect, view, add, export (Markdown, JSON), search, tags, run, sample, mcp. |
| MCP | done | `tanacraft-mcp`: headless on a file, or `--connect` to a running app. |
| Control channel | done | `tanacraft --control PORT`: JSON lines on 127.0.0.1. |
