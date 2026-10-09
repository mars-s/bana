# TanaCraft roadmap

TanaCraft aims for the core behaviour of the Tana outliner, rebuilt clean-room in Rust, then goes
further on openness, local-first storage and agent control. Status by spec section is in
[docs/parity.md](docs/parity.md).

## Milestones

| Milestone | Scope | State |
|---|---|---|
| M0 Scaffold | Workspace, licences, docs, xtask | done |
| M1 Core | Node graph, supertags, fields, references, daily notes, trash, undo/redo, JSON file | done |
| M2 Commands | Command registry with ids, params, shortcuts (64 commands) | done |
| M3 Desktop | egui app: sidebar, outliner, breadcrumbs, keyboard editing, tag chips, field rows, pickers, command line, control channel | done (first version) |
| M4 Screenshots | Offscreen `ui_shot`, README images | done |
| M5 Agents | CLI (inspect, view, add, export, run) and MCP server | done |
| M6 Parity | `docs/parity.md` maps every spec item; `cargo xtask parity` flags new ones | done |
| M7 Views | Search nodes, table, list, cards, calendar views, filters, sort, group | next |
| M8 Rich text | Inline formatting, inline references, links, multi-select, Shift+Enter | next |
| M9 Panels | Tabs, side-by-side panels, node and bullet menus, supertag config panel, date picker | next |

## Later

Multiple workspaces, sync, settings, mobile and web (wasm) builds.

## Known simplifications

- One value per field (Tana allows several).
- The caret lands at the end when you click a bullet's text.
- Off macOS, a few Ctrl and Cmd shortcuts from the spec collapse onto the same key (Ctrl+Shift+D).
