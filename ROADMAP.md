# TanaCraft roadmap

TanaCraft aims for the core behaviour of the Tana outliner, rebuilt clean-room in Rust, then goes
further on openness, local-first storage and agent control. Status by spec section is in
[docs/parity.md](docs/parity.md).

## Where we are

| Measure | Value |
|---|---|
| Commands (every action; CLI, MCP and the JSON API can run them) | 185 engine commands, plus about 20 UI commands |
| Rust tests | 173: core (incl. merge tests of copies edited apart, undo after merges, a copy offline for 60 days, compaction only up to what paired copies acknowledged), commands (incl. fuzz tests of random command sequences and damaged files, a 50k-node timed test), server, MCP, CLI |
| Web UI tests (Playwright, each with its own engine) | 121, plus 180 use-case scenarios in `web/tests/use-cases/` (the ones still missing are listed as fixme), plus the remote connector's Worker and end-to-end tests (`cargo xtask connector`): editing by keyboard, pickers, field rows and values, the date picker, menus, copy and paste, drag and drop, references, settings and keybindings, tabs and panels, views (table, board, cards, calendar by pointer, query builder, toolbar), Configure supertag, a damaged file in every view; fuzz: random keys never lose a character (also on a long page), reloads and force-kills keep every character, typed keys land only in a visibly focused editor; killing and restarting the engine mid-typing; 51,550 rows |
| UI | Tauri 2 desktop app and the same web UI in a browser (`tanacraft-cli serve`); the egui UI is in `legacy/egui` (reference only); porting status in `docs/web-port.md` |
| Spec items (`docs/parity.md`, sections and screenshots) | 403: 344 done, 27 partial, 8 missing, 10 notes |

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
| M26 Acceptance round 9 | DIFFS N60 to N71: two windows with their own pages and carets and both windows' words kept in one line (P1), typing with nothing focused restores the caret, field drafts become fields on reload, table header opens the field beside, board column add at the top, #, @ and / menus, command line under the line, page and tag menus, account menu, settings, search page, tabs, the N59 list settled with the paired images | done |
| M28 Acceptance round 10 | DIFFS N72 to N76 and the open round 9 items: an offline notice while the engine is out of reach, edits wait first in line and go in when it is back (a restarted engine gets the pages and caret back), click right of a line, keys with nothing focused dropped after a click or reload, Suggested columns, a split in one window carries the other window's caret, the field typed after a line keeps its place, the spec's menu bar, page menu, Supertags header, Workspaces button, table and date cell | done |
| M29 Use cases and MCP | docs/use-cases gaps: MCP on the running app with a per-install token, streamable HTTP MCP at /mcp, a remote connector for claude.ai and ChatGPT (cloudflare/, set up by the user with its wizard), PARENT on day pages and in related content, dates in words, tagging a selection, rolling periods, Add to day, automatic progress bars and rings, unlinked mentions, merging duplicates, Tana Paste, Notion, Logseq and Roam imports, files on nodes, commands on events, a capture shortcut from any app, and the smaller defects (content width, navigation views in place, folding groups, week start in the date picker, Move to other workspaces, numbers out of range kept with a warning) | done |
| M30 Flakes and use cases | Every intermittent web test found with `--repeat-each 5` and a 4x CPU-throttled pass fixed at the root (a stale page read put a line editor's caret back; tests that raced the UI), a capture window of its own from any app (Cmd+Shift+E), command buttons on supertags and fields, Cmd+Shift+M Move to, strikethrough, word counts, regular expressions in searches, searching inside one node, merging duplicate contacts, node links that open from other apps, spreadsheet cells pasted as nodes with fields, aliases on inline references and reading one in place, the whole workspace as a ZIP of Markdown files, locked nodes | done |
| M27 Next | Sync over the network (iroh), publishing a page, AI features (decisions open) | next |

## Later

Sync, workspace members, mobile and web (wasm) builds.

## Known simplifications

- Undo history is per session (merging another copy keeps it; it takes back only local edits).
- History is kept whole until sync exists; compaction then cuts only what every paired copy has acknowledged.
- A further workspace has its own Library, Schema and Trash; Today and Daily notes follow the main workspace.
- One local editor: "edited by" is the computer's user.
- Off macOS, a few Ctrl and Cmd shortcuts from the spec collapse onto the same key (Ctrl+Shift+D).
