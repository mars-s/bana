# TanaCraft roadmap

TanaCraft aims for the core behaviour of the Tana outliner, rebuilt clean-room in Rust, then goes
further on openness, local-first storage and agent control. Status by spec section is in
[docs/parity.md](docs/parity.md).

## Where we are

| Measure | Value |
|---|---|
| Commands (every action; CLI, MCP and the JSON API can run them) | 177 engine commands, plus about 20 UI commands |
| Rust tests | 145: core (incl. merge tests of copies edited apart, undo after merges, a copy offline for 60 days, compaction only up to what paired copies acknowledged), commands (incl. fuzz tests of random command sequences and damaged files, a 50k-node timed test), server, MCP, CLI |
| Web UI tests (Playwright, each with its own engine) | 108: editing by keyboard, pickers, field rows and values, the date picker, menus, copy and paste, drag and drop, references, settings and keybindings, tabs and panels, views (table, board, cards, calendar by pointer, query builder, toolbar), Configure supertag, a damaged file in every view; fuzz: random keys never lose a character (also on a long page), reloads and force-kills keep every character, typed keys land only in a visibly focused editor; 51,550 rows |
| UI | Tauri 2 desktop app and the same web UI in a browser (`tanacraft-cli serve`); the egui UI is in `legacy/egui` (reference only); porting status in `docs/web-port.md` |
| Spec sections (`docs/parity.md`) | 171 mapped (files 00 to 15, DESIGN.md): 147 done, 16 partial, 3 missing, 5 notes |
| Spec screenshots | 214 mapped (incl. `screens/visual/`): 186 done, 18 partial, 5 missing, 5 not behaviour |

## Milestones

| Milestone | Scope | State |
|---|---|---|
| M0 Scaffold | Workspace, licences, docs, xtask | done |
| M1 Core | Node graph, supertags, fields, references, daily notes, trash, undo/redo, JSON file | done |
| M2 Commands | Command registry with ids, params, shortcuts | done |
| M3 Desktop | egui app: sidebar, outliner, breadcrumbs, keyboard editing, tag chips, field rows, pickers | done |
| M4 Screenshots | Offscreen `ui_shot`, README images | done |
| M5 Agents | CLI and MCP server, control channel | done |
| M6 Parity | `docs/parity.md` maps every spec section and screenshot; `cargo xtask parity` checks | done |
| M7 Views | Search nodes with a query language (nested groups, operators), outline/table/cards/board/lanes/list/side menu/tabs/calendar, sort/group/filter/display | done (first versions) |
| M8 Editing per spec 02 | Enter/Backspace/Delete rules, moves across branches, selection mode, multi-select, drag and drop, paste, inline formatting, headings, descriptions | done (first versions) |
| M9 Shell | Tabs, panels, sidebar modes, Search and open, quick add, slash menu, related content, go to date | done (first versions) |
| M10 Schema | Supertag page with view tabs and configure section, field definition page, done mapping, templates, auto-initialize, hide and required fields | done (first versions) |
| M11 Spec round two | Inline references and links, Recents and Settings pages, Move to, system fields, calendar drag and day view, selection toolbar, copy/paste as references, checklists, column resize, template propagation | done (first versions) |
| M12 Acceptance round 3 | Every P1, P2 and P3 item of the spec team's DIFFS.md (except AI): caret formatting, `>` field rows, Escape selection, copying selections, quick add focus, nested query groups, configure supertag panel, supertags overview, board and calendar details, context-aware command line, checklist caret, tag page header, shell, link styling, field options outline, inline `#` menu | done |
| M13 Fields and calendar | Several values per field, pinned and optional fields, title expressions, option sources, invalid-value warnings, date picker with time; calendar time grid with timed dates, drop to time, resize, drag to create | done |
| M14 Workspaces and shell | Workspaces as graph files (switch, create, rename), capture Inbox, Help page, account menu, content width, panel resize, mini sidebar flyouts, list view in a second panel, page icons and banners, custom keybindings, Markdown import, user fields | done |
| M15 Acceptance round 4 | DIFFS N1 to N14: field values, `>` field rows in place, selection keys, Cmd+. title, page edge moves, workspaces side by side, search builder closed on open, fuller selection toolbar; table header menu with calculations, + Add menu item / tab, hidden-field labels | done |
| M16 Robustness, speed, menus | Fuzz test of random command sequences (fixed: untracked day creation clobbered by undo, references picking up child tags, day creation recursion, file repair of loops); 50k-node outline draws only visible rows; native macOS menu bar running commands; HTML plus text on the clipboard; Settings sub-pages (date and time, sidebar entries) | done |
| M17 Acceptance round 5 | DIFFS N13 and N15 to N25: field values and names saved as typed, keyboard never lost (fuzz-checked after every key), Shift+Tab out of a value below the field, checkbox carried by Enter, no empty format marks, delete question for used nodes, formatted HTML clipboard, notes outlines on references, settings defaults and system theme, folded workspace rows, menu bar keys and clipboard items, query editor by keyboard; OPML keeps fields, imports make `[[Name]]` links | done |
| M18 Acceptance round 6 | DIFFS N26 to N31 and N23: every open editor is written into the graph on quit, typing on a selected (or invisibly selected) line goes into it, Escape in a new field name selects the field row, a tagged task's checkbox stays with the tag, dialog and notes wording, checkbox field Space, User settings sections; exports keep a reference's notes for this place | done |
| M19 Acceptance round 7 | DIFFS N32 to N34 and the N26 remainder: rows holding an editor are always laid out and scroll into view (field values below the first screen were lost), open editors autosave within a second so a force-kill keeps a `>` field name, Enter on a selected field row adds a line below and letters extend its name, typing after a cleared selection goes in at the start; Markdown export with descriptions and field rows in place; new event and workspace names kept on blur | done |
| M20 Web UI | Tauri 2 app and a web UI (React, TypeScript, plain CSS tokens) on the same engine; browser transport (`tanacraft-cli serve`); virtualized outline; ordered command queue with key buffering; pickers, field values, views, query builder, menus, Supertags, Settings; Inter Variable | done (first version) |
| M21 Loro store | The graph file is a Loro CRDT document (movable tree, text, maps); incremental writes; merge of copies edited apart; one-time import of JSON files | done |
| M22 Design pass | Tokens and screens from the spec team's DESIGN.md and 10 to 15: shell, outline, day page, views, Supertags, motion, icon inventory | done (first version) |
| M24 Acceptance round 8 (web UI) | DIFFS N35 to N59: keys only reach visibly focused editors (P1), the rewrite's regressions, view tabs in place, node and page menus, drag and drop per spec, tables, toolbar, board, calendar by pointer, sidebar and panel resizing, chips, settings panel, system theme, field names kept on a force-kill, visual list | done (see the git log) |
| M23 Ports and assets | Remaining egui behaviours (selected field rows, Configure in a second panel, table header rename and menu, mini rail flyouts, tab moves, custom keybindings, reference counter, edit warning, link previews, context notes and fields, calendar drag, date picker with time and end, damaged files in every view); icon inventory; undo across merges; history compaction (only what paired copies acknowledged) | done |
| M25 Data safety and boards | Write-ahead log of every edit and waiting key (reloads under load lose nothing), history kept for paired copies, board cards reordered in a column with the others sliding aside (`view.moveItem`) | done |
| M26 Next | Sync over the network (iroh), AI features | next |

## Later

Sync, workspace members, mobile and web (wasm) builds.

## Known simplifications

- Undo history is per session (merging another copy keeps it; it takes back only local edits).
- History is kept whole until sync exists; compaction then cuts only what every paired copy has acknowledged.
- A further workspace has its own Library, Schema and Trash; Today and Daily notes follow the main workspace.
- One local editor: "edited by" is the computer's user.
- Off macOS, a few Ctrl and Cmd shortcuts from the spec collapse onto the same key (Ctrl+Shift+D).
