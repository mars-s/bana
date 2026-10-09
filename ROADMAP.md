# TanaCraft roadmap

TanaCraft aims for the core behaviour of the Tana outliner, rebuilt clean-room in Rust, then goes
further on openness, local-first storage and agent control. Status by spec section is in
[docs/parity.md](docs/parity.md).

## Where we are

| Measure | Value |
|---|---|
| Commands (every action; CLI, MCP and control channel can run them) | 174 engine commands, plus 31 window commands |
| Tests | 189 (core 49, commands 58 incl. fuzz tests of random command sequences and of damaged files, and a 50k-node timed test, UI 80 driving the real UI with keys, pointer drags, clicks and panels plus UI fuzz tests (random keys, text, clicks; damaged files in every view; the keyboard is never lost while typing) and a 50k-node frame-time test, MCP 1, CLI 1) |
| Spec sections (`docs/parity.md`) | 95 mapped: 77 done, 13 partial, 3 missing, 2 notes |
| Spec screenshots | 160 mapped: 140 done, 14 partial, 5 missing, 1 not behaviour |

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
| M18 Next | An open-licence UI font with real bold, AI features (out of scope so far) | next |

## Later

Sync, workspace members, mobile and web (wasm) builds.

## Known simplifications

- Bold is drawn brighter rather than with a bold font (egui's default font has one weight).
- A further workspace has its own Library, Schema and Trash; Today and Daily notes follow the main workspace.
- One local editor: "edited by" is the computer's user.
- Off macOS, a few Ctrl and Cmd shortcuts from the spec collapse onto the same key (Ctrl+Shift+D).
