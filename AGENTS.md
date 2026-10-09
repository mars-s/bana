# TanaCraft: instructions for agents

TanaCraft (working name; the product name lives in one constant, `tanacraft_core::PRODUCT_NAME`) is a
clean-room, open-source, Rust-native outliner that targets the behaviour of the Tana outliner: nodes,
supertags with fields, references, daily notes, and a keyboard-first outline editor. It follows the
conventions of the storytold "craft" apps (wordcraft, deckcraft): every action is a command with an id,
and the same commands drive the UI, keyboard shortcuts, the command palette, a CLI and an MCP server.

## The clean-room wall (non-negotiable)

This repo is the **clean side** of a two-team clean room. The rules are in `../WALL.md`.

- Allowed sources: the behaviour specs in `../tanacraft-spec/` (written by the other team), Tana's
  **public** website and docs (tana.inc, outliner.tana.inc/learn), and the public storytold repos.
- **Never** open, run, screenshot or inspect the Tana app, its app bundle, its Application Support
  data, its network traffic, or any reverse-engineering tool (rea, Hopper, disassemblers, decompilers,
  computer-use on Tana). Not even "just to check one thing".
- Never copy Tana source, CSS, icons, images, fonts, internal identifiers or UI strings beyond short
  generic labels ("Today", "Search", "Supertags").
- If the spec is silent, use the public docs; if those are silent, pick a sensible outliner behaviour
  and mark it `ponytail:`/`spec-gap:` in code or in `docs/parity.md` so the spec team can fill it in.
- Every asset is original (drawn in code) or openly licensed, with a row in `ATTRIBUTION.md`.

## Never crash

People trust an outliner with their notes. No `unwrap()`, `expect()`, `panic!`, `todo!` in non-test
code (crate roots carry `#![deny(clippy::unwrap_used, ...)]`). Errors are `Result`. Input from files,
the CLI and MCP is hostile: validate ids, clamp indexes, bound recursion.

## Architecture

| Crate | Job |
|---|---|
| `crates/core` (`tanacraft-core`) | Node graph model, supertags, fields, references, daily notes, trash, undo/redo, JSON persistence, Markdown/JSON export |
| `crates/commands` (`tanacraft-commands`) | `Session` (graph + focus/zoom/selection), `CommandSpec` registry: ids, labels, shortcuts, params, `run` |
| `crates/mcp` (`tanacraft-mcp`) | MCP server (JSON-RPC 2.0 over stdio) exposing the commands; `tanacraft-mcp` binary |
| `crates/ui-egui` (`tanacraft-ui-egui`) | egui front end; `examples/ui_shot.rs` renders offscreen screenshots |
| `apps/tanacraft` | Desktop app (eframe) |
| `apps/tanacraft-cli` | CLI: inspect, add, export, run commands, `mcp` |
| `xtask` | `cargo xtask ci`, `cargo xtask shots` |

Nothing below `ui-egui` depends on egui. **Everything is a command**: new behaviour = a `CommandSpec`
in `crates/commands/src/cmds.rs` + a test. The UI only calls `Session::run`.

## Quality gates

Before every commit: `cargo xtask ci` (fmt check, clippy -D warnings, tests). Commit after each piece
that builds. No em dashes in any text (code comments, docs, commit messages).

## Looking at the app

- `cargo run -p tanacraft -- --sample` opens the desktop app with the sample graph.
- `cargo xtask shots` (or `cargo run -p tanacraft-ui-egui --example ui_shot -- script.jsonl`) renders
  the window offscreen to PNGs in `docs/images/`. For UI work, **look at the PNG** before committing.
- `cargo run -p tanacraft-cli -- --help`; MCP: `cargo run -p tanacraft-mcp -- --file graph.json`.
- Parallel agents: use a separate `CARGO_TARGET_DIR`; keep builds to a few jobs (`.cargo/config.toml`).
