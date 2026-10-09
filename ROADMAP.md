# TanaCraft roadmap

TanaCraft aims for the core behaviour of the Tana outliner, rebuilt clean-room in Rust, then goes
further on openness, local-first storage and agent control. Status by spec section is in
[docs/parity.md](docs/parity.md).

## Where we are

| Measure | Value |
|---|---|
| Commands (every action; CLI, MCP and control channel can run them) | 158 engine commands, plus 22 window commands |
| Tests | 133 (core 40, commands 41, UI 50 driving the real UI with keys, pointer drags, clicks and panels, MCP 1, CLI 1) |
| Spec sections (`docs/parity.md`) | 99 mapped: 72 done, 20 partial, 5 missing, 2 notes |
| Spec screenshots | 153 mapped: 124 done, 21 partial, 7 missing, 1 not behaviour |

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
| M15 Next | Native menus, AI features, per-context notes on references, rich HTML clipboard, import from other apps, workspace members | next |

## Later

Multiple workspaces, sync, settings, mobile and web (wasm) builds.

## Known simplifications

- Bold is drawn brighter rather than with a bold font (egui's default font has one weight).
- Search results stay live while the query builder is open; Run once and Done are there but results do not wait for them.
- Off macOS, a few Ctrl and Cmd shortcuts from the spec collapse onto the same key (Ctrl+Shift+D).
