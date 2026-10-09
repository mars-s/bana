# TanaCraft roadmap

TanaCraft aims for the core behaviour of the Tana outliner, rebuilt clean-room in Rust, then goes
further on openness, local-first storage and agent control. Status by spec section is in
[docs/parity.md](docs/parity.md).

## Where we are

| Measure | Value |
|---|---|
| Commands (every action; CLI, MCP and control channel can run them) | 124 |
| Tests | 73 (core 33, commands 26, UI 12 including real key, drag and drop, paste and panel tests, MCP 1, CLI 1) |
| Spec sections (`docs/parity.md`) | 95 mapped: 43 done, 44 partial, 6 missing, 2 notes |
| Spec screenshots | 153 mapped: 88 done, 37 partial, 27 missing, 1 not behaviour |

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
| M12 Next | Calendar time grid (timed dates, drag to create and resize), several values per field, pinned fields, workspaces and Inbox, AI features, native menus | next |

## Later

Multiple workspaces, sync, settings, mobile and web (wasm) builds.

## Known simplifications

- One value per field (Tana allows several).
- Bold is drawn brighter rather than with a bold font (egui's default font has one weight).
- Search results are live; the spec's "Run once" / "Done" editing step is not modelled.
- The caret lands at the end when you click a bullet's text.
- Off macOS, a few Ctrl and Cmd shortcuts from the spec collapse onto the same key (Ctrl+Shift+D).
