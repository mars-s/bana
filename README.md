<h1 align="center">TanaCraft</h1>

<p align="center">
  <b>A keyboard-first outliner with supertags, fields and daily notes; an open-source, clean-room
  reimplementation of the Tana outliner, rebuilt in pure Rust.</b>
</p>

<p align="center">
  Every bullet is a node. Tag a node with a supertag and it gains that supertag's fields. Reference any
  node from anywhere. Write by hierarchy or by day. Your graph is a local JSON file, and every action
  is a command that people, scripts and AI agents can run.
</p>

<p align="center">
  <img src="docs/images/web/today-dark.png" alt="TanaCraft showing today's page, Sat 10 Oct: the sidebar with Today, Supertags, Recents, Create new, pinned supertags and the workspace; the day page with a meeting node and its Outdoors field, a task with a pink chip, a reference to the Community garden project and an inline link; an Open tasks card on the right." width="100%">
  <br><sub><b>Today.</b> Daily notes, supertag chips, field rows, references and an Open tasks card.</sub>
</p>

## Tour

<table>
<tr>
<td width="50%" valign="top"><img src="docs/images/web/two-panels.png" alt="Community garden with its project fields beside Raised beds in a second panel" width="100%"><p align="center"><sub><b>Panels.</b> Open any node beside the current one; fields from supertags sit in soft boxes.</sub></p></td>
<td width="50%" valign="top"><img src="docs/images/web/calendar.png" alt="Open tasks as a month calendar with tasks on their due dates and today marked" width="100%"><p align="center"><sub><b>Calendar.</b> Items on their dates; month, week and day.</sub></p></td>
</tr>
<tr>
<td width="50%" valign="top"><img src="docs/images/web/query-builder.png" alt="The query builder of Open tasks above its table" width="100%"><p align="center"><sub><b>Search nodes.</b> Live queries with nested AND, OR and NOT groups; results wait for Run once.</sub></p></td>
<td width="50%" valign="top"><img src="docs/images/web/board.png" alt="Open tasks as a board grouped by Status" width="100%"><p align="center"><sub><b>Board.</b> Cards grouped by a field; drag a card to change its value.</sub></p></td>
</tr>
<tr>
<td width="50%" valign="top"><img src="docs/images/web/table.png" alt="Open tasks as a table with Status, Due and Effort columns" width="100%"><p align="center"><sub><b>Table.</b> Cells are typed in place; columns, sort, group and filter from the toolbar.</sub></p></td>
<td width="50%" valign="top"><img src="docs/images/web/supertags.png" alt="The Supertags page: pinned supertags as coloured tiles, then every supertag of the workspace" width="100%"><p align="center"><sub><b>Supertags.</b> Pinned ones as tiles, then every supertag of the workspace.</sub></p></td>
</tr>
<tr>
<td width="50%" valign="top"><img src="docs/images/web/command-line.png" alt="The command line open over the Community garden page" width="100%"><p align="center"><sub><b>Command line (Cmd+K).</b> Steps for the line you are on first, then every command.</sub></p></td>
<td width="50%" valign="top"><img src="docs/images/web/today-light.png" alt="Today's page in the light theme" width="100%"><p align="center"><sub><b>Light theme.</b> Tuned, not inverted: regular text is weight 400 here and 375 in dark.</sub></p></td>
</tr>
</table>

Every screenshot is TanaCraft itself (the web UI, rendered by `cargo xtask shots`). The look follows
the spec team's measured design ([DESIGN.md](DESIGN.md)); icons are our own.

## Why

Tana showed that an outliner gets much more useful when nodes can carry types: tag a bullet `#task`
and it gains a status, a due date and a checkbox; ask a search node for every open task and it stays
live. TanaCraft rebuilds that model as open source:

- **Local and yours.** One JSON file per workspace on your disk, saved atomically. No account, no server.
- **Fast and small.** A Rust engine with a thin web UI (Tauri); it never blocks on the network.
- **Scriptable to the core.** Every action is a command with an id. The keyboard, the command line, the CLI, the MCP server and the JSON API all run the same commands, so an agent can do anything you can.
- **Clean room.** Built from public docs and a black-box behaviour spec; see [Clean room](#clean-room).

## What works today

| Area | Highlights |
|---|---|
| **Outline** | Enter, Shift+Enter, Tab, Shift+Tab, Backspace and Delete as in the spec, wrapped long lines, caret where you click, moves across branches, Escape and Shift+Up/Down selection with a floating toolbar, drag and drop, paste as nodes or references, headings, descriptions edited in place, inline bold/italic/underline/highlight/code that carry on as you type, links, checklists with progress |
| **Supertags** | Inline `#` menu, colours (or none, or custom), extends, base types, template fields and nodes, optional and pinned fields, title expressions, checkbox and done mapping, default child supertag, related content, convert a node to a supertag |
| **Fields** | `>` field rows (Tab to the value, Enter for another value); plain, options, options from a supertag, date (with time ranges), number, URL, email, checkbox; several values per field, option sources and auto-collect, required, hide rules, invalid-value warnings, auto-initialize |
| **Search** | Search nodes with nested AND/OR/NOT groups, field comparisons and field values (parent, relative dates, set, defined), date, type, scope and workspace operators; new results flash; Search and open with fuzzy matching and snippets; a context-aware command line |
| **Views** | Outline, table (full width, rename columns in place, resize), cards, board, lanes (drag between lanes), list (detail in the next panel), side menu, tabs, calendar (month, week and day with a time grid); display, sort, group and filter from an icon toolbar |
| **Daily notes** | Year, week and day nodes, relative titles, Open tasks card, week Schedule, go to date, insert date, dates that link to their day |
| **Shell** | Tabs, resizable panels, sidebar full/mini/hidden with flyouts, pinned items, quick add, capture Inbox, several workspaces side by side (each with its own Library, Schema and Trash), page icons and banners, content width, Help page, settings with sub-pages (date and time, sidebar entries, URL embeds), a native macOS menu bar, trash |
| **Undo** | Every command is one undo step; typing a word is one step; a failed command changes nothing |
| **Files** | One graph file (`~/TanaCraft/graph.tanacraft` by default, a Loro CRDT document so copies edited apart merge; older JSON files are read once and kept as a backup) holding every workspace, atomic autosave, repair of damaged files, Markdown, JSON and OPML export, Markdown and OPML import, HTML plus text on the clipboard |

Status per spec section and screenshot: [docs/parity.md](docs/parity.md) (77 of 95 spec sections
done, 13 partial; 140 of 160 screenshots done). AI features are out of scope for now.

## Data safety

The web UI never owns your notes. Every keystroke goes to the engine at once, in order; a key
pressed while a command is still moving the caret waits and is replayed into the right line; keys
still waiting when the window reloads are typed again after it; the engine autosaves within a
second and saves on quit. Playwright fuzz tests type random keys at full speed (also on a long
page in a small window, scrolled away mid-edit), reload the window at a random point, and
force-kill the engine; not one character may go missing.

The graph file is a [Loro](https://loro.dev) CRDT document: two copies edited apart (two devices,
offline) merge, with concurrent typing in one line, concurrent moves (never a loop), delete versus
edit, and tags and fields added on both sides covered by tests. Undo and redo keep working after a
merge and take back only this copy's edits. History older than a month is compacted on save (at
most once a week, the file before compaction is kept as `<name>.before-compact`), so copies edited
apart for up to a month still merge. Older JSON graph files are read once and kept as a backup.
Sync over the network comes later.

## Speed and robustness

Measured on a generated 50,000-node workspace (`sample::large`), release build, Apple Silicon laptop:

| Action | Time |
|---|---|
| Typing one character (engine) | 0.001 ms |
| Enter, expand or collapse, undo (engine) | under 0.01 ms |
| Substring search / fuzzy Search and open over every node | 15 ms / 27 ms |
| Save while typing (Loro document, changes only) / load the graph file | 30 ms / 0.5 s |
| Compact a month of history (once a week at most, on save) | 0.6 s |
| Web UI: 51,550 rows expanded: jump to 60 % and draw / 15 keys to the engine | about 120 ms / 120 ms (debug engine) |

The web outline draws only the rows near the viewport and asks the engine for that window of rows
only. A row never holds unsaved text (every keystroke is already in the engine), and a key typed
while the line being edited is scrolled away brings it back first. Reproduce:
`cargo test -p tanacraft-commands --test perf --release -- --nocapture` and
`pnpm --dir web exec playwright test perf`.

A fuzz test runs random sequences of every command (params built from each command's own
description) and checks after each step that owners and children agree, nothing is orphaned or in a
loop, every change undoes and redoes exactly, and save/load round-trips; damaged files (loops, double
owners, dangling ids, links to nodes of the wrong kind) load into a sound graph and keep working under
random commands; after every few steps the Loro document written so far must hold exactly the same
graph. The web UI's fuzz tests are described under [Data safety](#data-safety). Longer runs:
`FUZZ_SEEDS=1000 FUZZ_STEPS=400 cargo test -p tanacraft-commands --test fuzz --release` and
`FUZZ_SEEDS=40 pnpm --dir web exec playwright test fuzz`. Which of the old egui UI tests have been
ported to the web UI: [docs/web-port.md](docs/web-port.md).

## Quick start

```sh
pnpm --dir web install && pnpm --dir web build             # the web UI (once, and after UI changes)
cargo run --release -p tanacraft-desktop -- --sample       # the desktop app (Tauri) with the sample graph
cargo run --release -p tanacraft-desktop -- notes.json     # open or create a graph file
cargo run --release -p tanacraft-cli -- serve --sample     # the same UI in a browser: http://127.0.0.1:8787/
cd apps/tanacraft-desktop && ../../web/node_modules/.bin/tauri build --bundles app   # TanaCraft.app
```

Then press **Cmd+K** for every command, **Cmd+S** to search and open, **Cmd+E** for quick add and
**Cmd+/** for the shortcut sheet.

Tests: `cargo xtask ci` (Rust: fmt, clippy, 138 tests) and `cargo xtask web` (typecheck, build,
Playwright: 91 tests, each against its own engine). Screenshots: `cargo xtask shots`.

## For agents: CLI, MCP and the JSON API

Every action is a command with an id (`node.indent`, `tag.add`, `field.addValue`, `search.addGroup`,
`nav.today` ...). The same commands drive the keyboard shortcuts, the command line, the CLI, the MCP
server and the JSON API. `tanacraft-cli commands` lists all 174 with their parameters.

```sh
tanacraft-cli commands                                  # every command, shortcut and params
tanacraft-cli add "Plant garlic" --today --tag task     # prints the new node id
tanacraft-cli capture "Call the plumber"                # into the capture Inbox
tanacraft-cli import notes.md                           # a Markdown outline into the Library
tanacraft-cli run --cmd 'field.set={"node":46,"field":"Due","value":"2026-10-20 09:00-10:00"}'
tanacraft-cli export today                              # Markdown outline
tanacraft-cli export 2 --format json                    # JSON tree of the Library
tanacraft-mcp --file graph.tanacraft                      # MCP server on stdio (headless)
tanacraft-cli serve --port 8787                         # web UI + JSON API (POST /rpc) on 127.0.0.1
```

## Architecture

| Crate | Job |
|---|---|
| `crates/core` | Node graph: nodes, supertags, fields, references, queries, views, daily notes, trash, undo/redo, JSON file, export |
| `crates/commands` | `Session` (graph, focus, panels' navigation) and the command registry (ids, labels, shortcuts, params) |
| `crates/mcp` | MCP server library and the `tanacraft-mcp` binary |
| `crates/server` | `Host` (the Session plus each panel's navigation), JSON view snapshots, a localhost HTTP server |
| `apps/tanacraft-desktop` | The desktop app: Tauri 2 around the `Host` |
| `web/` | The UI (React + TypeScript); runs in the desktop app or a browser |
| `apps/tanacraft-cli` | Command line, including `serve` |
| `xtask` | `cargo xtask ci`, `cargo xtask web`, `cargo xtask shots`, `cargo xtask parity` |
| `legacy/egui` | The old egui front end, reference only and not built (tag `egui-final`) |

The UI holds no data: it runs commands and draws snapshots of the engine.

## Roadmap

Done: the core graph, commands, desktop app, agents (CLI, MCP, control channel), views, the editing
rules of the spec, supertag and field configuration, the calendar time grid, workspaces and the capture
Inbox, and every behaviour difference the spec team reported in its acceptance tests (rounds 3 to 7:
fields saved as typed, on quit and within a second for a force-kill, on rows below the first screen too, no typed key ever dropped, notes outlines on references and in
exports, formatted HTML on the clipboard, menu bar with keys and clipboard items). Next: an open-licence UI font with real bold and,
later, AI features. Details in [ROADMAP.md](ROADMAP.md).

## Clean room

TanaCraft is built from Tana's public documentation and a black-box behaviour spec written by a
separate team. Nobody on this side opens the Tana app, its files or any reverse-engineering tool. See
[AGENTS.md](AGENTS.md). Every asset is original or openly licensed (see [ATTRIBUTION.md](ATTRIBUTION.md)); icons are our own SVGs.

## License

MIT OR Apache-2.0. Not affiliated with Tana Inc.
