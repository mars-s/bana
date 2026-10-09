# Legacy egui front end (reference only)

This folder holds TanaCraft's first front end, written with egui. It is **reference only and is
not built**: the workspace excludes `legacy/`, and nothing here is compiled, linted or tested by
`cargo xtask ci`. The last working version is the git tag `egui-final`
(`git checkout egui-final && cargo run -p tanacraft -- --sample`).

The desktop app is now the Tauri app in `apps/tanacraft-desktop`, with its web UI in `web/`.

## Why it is kept

The behaviours and tests here are the porting checklist for the Tauri UI:

- `ui-egui/tests/typing.rs`: keyboard and editing behaviour (spec 02 and DIFFS), one test per rule.
- `ui-egui/tests/ui_fuzz.rs`: the fuzz invariants (a typed key is never lost, quitting or a
  force-kill at a random point keeps every character, long pages in a small window).
- `ui-egui/tests/perf.rs`: the 50k-node frame-time test.
- `ui-egui/src/keys.rs`, `outline.rs`, `overlays.rs`, `views.rs`, `schema_ui.rs`: what each key,
  picker and view does.
- `shots.jsonl`: the screens the README showed.

When a behaviour is ported to the web UI with a Playwright test, it counts as done here. Do not
fix bugs in this folder; fix them in the web UI.

| Folder | Was |
|---|---|
| `ui-egui/` | `crates/ui-egui` (`tanacraft-ui-egui`) |
| `tanacraft-app/` | `apps/tanacraft` (the eframe desktop app, `--control PORT` JSON channel, macOS menu bar) |
