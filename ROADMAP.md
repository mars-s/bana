# TanaCraft roadmap

TanaCraft aims for the core behaviour of the Tana outliner, rebuilt clean-room in Rust, then goes
further on openness, local-first storage and agent control. Status by spec section is in
[docs/parity.md](docs/parity.md).

## Milestones

| Milestone | Scope | State |
|---|---|---|
| M0 Scaffold | Workspace, licences, docs, xtask | done |
| M1 Core | Node graph, supertags, fields, references, daily notes, trash, undo/redo, JSON file | in progress |
| M2 Commands | Command registry with ids, params, shortcuts | planned |
| M3 Desktop | egui app: sidebar, outliner, breadcrumbs, keyboard editing, tag chips, field rows | planned |
| M4 Screenshots | Offscreen `ui_shot`, README images | planned |
| M5 Agents | CLI (inspect, add, export) and MCP server | planned |
| M6 Parity | `docs/parity.md` maps every spec section to a status | planned |

## Later

Search nodes and live queries, table and board views, inline references inside text, rich text
formatting, multiple workspaces, sync, mobile and web (wasm) builds.
