---
name: tanacraft-ui
description: Use when changing how any TanaCraft screen looks or behaves in the web UI (web/, Tauri app): layout, spacing, colours, type, icons, hover or focus states, keyboard handling, or matching a screen to the spec.
---

# TanaCraft UI (web)

The goal is Tana's look and feel (layout, spacing, colour, type scale, states, motion) built from our own code and assets. The source of truth is the dirty side's visual spec, never the Tana app.

The UI is `web/` (Vite + React + TypeScript, plain CSS). It runs inside the Tauri app (`apps/tanacraft-desktop`) or in a browser against `tanacraft-cli serve`. The old egui UI in `legacy/egui` is reference only: its tests are the porting checklist.

## Sources (clean side only)

- `../tanacraft-spec/10-visual-design.md`, `11-visual-states.md`, `12-screens-visual.md`, `DESIGN.md`, `13-design-principles.md`, `14-motion.md`, `15-asset-inventory.md` (when they exist).
- `../tanacraft-spec/screens/` (1512x949 window shots) and `screens/visual/`.
- `../tanacraft-spec/DIFFS.md` for open differences.
- Never open the Tana app or inspect it. See `../WALL.md`.

## Rules

1. **Tokens, not literals.** Every colour, size, radius, spacing, weight and duration is a CSS custom property in `web/src/tokens.css` (dark in `:root`, light in `:root[data-theme='light']`). A raw hex or px value for a design decision in `app.css` or a component is a bug. Add a token.
2. **Both themes.** Check dark and light shots (`?theme=light` in the URL forces light).
3. **Icons** are our own SVGs in `web/src/icons.tsx` (16x16 grid, stroke 1.35, `currentColor`). Pictures follow the `tanacraft-assets` skill.
4. **States.** Every clickable thing has rest, hover, pressed, focused and disabled looks; every row hover and selected.
5. **The UI never owns data.** Every edit is an engine command (`engine.ts` queue). See "Data safety" below before touching editors.

## The screenshot loop

1. Read the spec section and open the spec screenshot.
2. Change tokens or layout.
3. `cargo build -p tanacraft-cli && pnpm --dir web build && pnpm --dir web shots` (or `cargo xtask shots`): PNGs land in `docs/images/web/` at 2x (3024x1898). Add a test to `web/tests/shots.spec.ts` for a new screen.
4. Read the PNG next to the spec screenshot (spec shots are 1x, 1512x949: halve our pixel numbers). Compare alignment, spacing, row height, text size and weight, colours, icon size, empty space. Sample spec colours with a script (darkest/lightest pixel in a box) rather than guessing.
5. Repeat until it matches. Then `cargo xtask ci`, `cargo xtask web`, commit, one screen per commit.

## Data safety (DIFFS N15, N16, N26, N27, N32)

- `engine.ts`: one ordered queue for every request. A rename waiting behind a rename of the same line is merged (never the one in flight). On unload the waiting requests go out as one `sendBeacon` batch, then `save`.
- `controller.ts`: commands that move the caret run through `exec`, which raises a barrier: keys pressed meanwhile are buffered and replayed (in order) once the new snapshot is drawn. Keys still buffered when the page unloads go to `sessionStorage` and are replayed after reload.
- Before a command, `exec` tells the engine where the caret is (`focus.set`) if it moved without a command.
- Line editors (`LineEditor` in `Outline.tsx`) send `node.setName` on every input and adopt the engine's text only when none of their own edits are pending.
- The outline is virtualized; a row never holds unsaved text. A key pressed while the focused line is scrolled away scrolls it back (`focusScroll`) and is replayed into it.
- Playwright fuzz tests (`web/tests/fuzz.spec.ts`) check that no typed character is lost, also across reloads and force-kills. Run them after any editor change; `FUZZ_SEEDS=40` for a long run.

## Web and Tauri gotchas

- `.panel .title` is either an `h1` (fixed titles) or a `textarea.title-input`; Playwright's `toHaveValue` fails at once on the `h1`, so use `.title-input`.
- Playwright types far faster than people: anything async between a key and the next must keep order (queue + barrier), or keys land in the wrong line.
- Tauri: `cargo run -p tanacraft-desktop` embeds `web/dist` (the `custom-protocol` feature is on by default); `pnpm --dir web tauri dev` uses the Vite dev server instead. A fresh checkout without `web/dist` gets a placeholder page from `build.rs`.
- The HTTP server refuses other origins and Host headers (DNS rebinding); `sendBeacon` uses `text/plain` so it stays a simple request.
- Each Playwright test starts its own `tanacraft-cli serve` (fixtures.ts) with a fixed today (2026-10-10); build the CLI first.
- `useStore(selector)` must return state that already exists (a primitive, or an object found in the state). A selector that builds a new object every call loops forever (React error #185).
- Keys must wait while earlier keys wait: `inBarrier()` is true while the buffer is non-empty, not only while a command runs, or a fast key overtakes buffered ones.
- A page read that a newer read overtook is dropped (`refreshSeq`); drawing it would mount an editor on a line that no longer has the caret, and keys would land there. Callers of `refresh()` still wait until a read at least as new is drawn.
- After a command that moves the caret, keys wait until the new line's editor registers (`holdKeysForEditor`), because React registers editors in an effect after the paint.
- Editors unregister only themselves (`unregisterEditor(h)`): a line editor unmounting after a field editor mounted must not drop the field editor.
- Pickers opened from buttons (not from `#`/`@` typed in a line) get their own search box (`at < 0`).
- Playwright `testIgnore`/`testMatch` regexes match the whole path: anchor them (`[/\\](shots|measure)\.spec\.ts$`), or `zz.spec.ts` also hides `fuzz.spec.ts`.
- The `app` fixture fails a test on any error thrown in the page.
- Measure layout with `pnpm exec playwright test --project=measure` (prints element boxes in 1x points) and compare with `../tanacraft-spec/12-screens-visual.md`; sample spec colours with a darkest/lightest-pixel script.
- Inter Variable from `@fontsource-variable/inter/opsz.css` (optical size axis): body widths match the spec at the automatic size; titles use `--opsz-title` (20) to match the spec's title widths.
- Keys only reach an editor the person can see: a caret or selection in rows the panel does not show (a table, board or calendar view, a configure panel) takes no keys (`outlineVisible`), and typing with the keyboard on a button goes nowhere (N35). The fuzz test "typed keys land only in a visibly focused editor" checks it.
- Commands that move the caret inside the same line (format marks, joins) hand their text and caret to the editor at once (`adopt`); do not make every refresh re-place the caret, it races with typing (seen as picker flakes under load).
- Run the pickers and editing specs with `--repeat-each=4 --workers=4` after touching the key path: some races only show under load.
- A field name typed after `>` is sent as `fieldDraft`; saves write it in as a field so a force-kill keeps it (N26). Renames and field values carry `rev`; the engine drops older ones.
- `PAGE_STACKS=1` makes the fixture report page errors with their stack (find the source in `dist/assets/*.js` at the line and column).
- Every tool and menu on a page is a pointer target too: menus fit the window (`NodeMenu` measures itself), popovers that hold submenus need `overflow: visible`.
