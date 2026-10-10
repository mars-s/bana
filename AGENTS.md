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
| `crates/server` (`tanacraft-server`) | `Host`: the Session plus one navigation state per panel; view snapshots as JSON; a std-only localhost HTTP server (`POST /rpc`, `GET /events`, static web UI) |
| `apps/tanacraft-desktop` | **The desktop app**: Tauri 2, holds the `Host` behind a mutex, IPC `rpc` command plus `changed` events |
| `web/` | The UI: Vite + React + TypeScript, plain CSS with every design token in `web/src/tokens.css`; runs in the Tauri app or in a browser against `tanacraft-cli serve` |
| `apps/tanacraft-cli` | CLI: inspect, add, export, run commands, `mcp`, `serve` (web UI + JSON API on 127.0.0.1) |
| `xtask` | `cargo xtask ci`, `cargo xtask web` (build + Playwright), `cargo xtask shots` |
| `legacy/egui` | The old egui front end: **reference only, not built**. Its tests are the porting checklist (`legacy/egui/README.md`); last working version is tag `egui-final` |

**Everything is a command**: new behaviour = a `CommandSpec` in `crates/commands/src/cmds.rs` + a
test. The UI only runs commands (through `Host::handle`) and draws snapshots; the JS side never owns
data. Every keystroke reaches the engine at once; the engine autosaves within a second.

## Quality gates

Before every commit: `cargo xtask ci` (fmt check, clippy -D warnings, tests) and, for UI changes,
`cargo xtask web` (typecheck, build, Playwright); for MCP or `cloudflare/` changes,
`cargo xtask connector`. Commit after each piece that builds. No em dashes in any text (code comments, docs, commit messages).

## Looking at the app

- Desktop app: `pnpm --dir web build && cargo run -p tanacraft-desktop -- --sample`.
- Browser: `pnpm --dir web build && cargo run -p tanacraft-cli -- serve --sample` then open
  http://127.0.0.1:8787/ (or `pnpm --dir web dev` on :5173, which proxies to the server).
- `cargo xtask shots` writes Playwright screenshots to `docs/images/web/`. For UI work, **look at the
  PNG** next to the spec screenshot before committing (see `.claude/skills/tanacraft-ui`).
- `cargo run -p tanacraft-cli -- --help`; MCP: `cargo run -p tanacraft-mcp -- --file graph.tanacraft`
  (works on the running app when one has the file open). Remote connector: `docs/mcp.md`; never
  deploy it or create cloud resources yourself, the user runs `cloudflare/wizard.sh`.
- Parallel agents: use a separate `CARGO_TARGET_DIR`; keep builds to a few jobs (`.cargo/config.toml`).
