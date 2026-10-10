---
name: bana-ui
description: Use when changing how any Bana screen looks or behaves in the web UI (web/, Tauri app): layout, spacing, colours, type, icons, hover or focus states, keyboard handling, or matching a screen to the spec.
---

# Bana UI (web)

The goal is Tana's look and feel (layout, spacing, colour, type scale, states, motion) built from our own code and assets. The source of truth is the dirty side's visual spec, never the Tana app.

The UI is `web/` (Vite + React + TypeScript, plain CSS). It runs inside the Tauri app (`apps/bana-desktop`) or in a browser against `bana serve`. The old egui UI in `legacy/egui` is reference only: its tests are the porting checklist.

## Sources (clean side only)

- `../tanacraft-spec/10-visual-design.md`, `11-visual-states.md`, `12-screens-visual.md`, `DESIGN.md`, `13-design-principles.md`, `14-motion.md`, `15-asset-inventory.md` (when they exist).
- `../tanacraft-spec/screens/` (1512x949 window shots) and `screens/visual/`.
- `../tanacraft-spec/DIFFS.md` for open differences.
- Never open the Tana app or inspect it. See `../WALL.md`.

## Rules

1. **Tokens, not literals.** Every colour, size, radius, spacing, weight and duration is a CSS custom property in `web/src/tokens.css` (dark in `:root`, light in `:root[data-theme='light']`). A raw hex or px value for a design decision in `app.css` or a component is a bug. Add a token.
2. **Both themes.** Check dark and light shots (`?theme=light` in the URL forces light).
3. **Icons** are our own SVGs in `web/src/icons.tsx` (16x16 grid, stroke 1.35, `currentColor`). Pictures follow the `bana-assets` skill.
4. **States.** Every clickable thing has rest, hover, pressed, focused and disabled looks; every row hover and selected.
5. **The UI never owns data.** Every edit is an engine command (`engine.ts` queue). See "Data safety" below before touching editors.

## The screenshot loop

1. Read the spec section and open the spec screenshot.
2. Change tokens or layout.
3. `cargo build -p bana-cli && pnpm --dir web build && pnpm --dir web shots` (or `cargo xtask shots`): PNGs land in `docs/images/web/` at 2x (3024x1898). Add a test to `web/tests/shots.spec.ts` for a new screen.
4. Read the PNG next to the spec screenshot (spec shots are 1x, 1512x949: halve our pixel numbers). Compare alignment, spacing, row height, text size and weight, colours, icon size, empty space. Sample spec colours with a script (darkest/lightest pixel in a box) rather than guessing.
5. Repeat until it matches. Then `cargo xtask ci`, `cargo xtask web`, commit, one screen per commit.

## Data safety (DIFFS N15, N16, N26, N27, N32)

- `engine.ts`: one ordered queue for every request. A rename waiting behind a rename of the same line is merged (never the one in flight).
- Write-ahead log: every edit (`run`, `fieldDraft`, `panel.open/close`) is written to `sessionStorage` (`bana.wal`) in the same task as the key press, tagged `wal: [client, seq]` (client = this page load), and leaves the log only when the engine answers it (or a later edit of the same page). The next load replays what is left (`replayWal`) before drawing; `Host::handle` runs each (client, seq) once, so the request in flight at a reload, the unload beacon and the replay never double an edit. On unload everything still queued also goes out as one `sendBeacon` batch (a closed tab drops sessionStorage).
- `controller.ts`: commands that move the caret run through `exec`, which raises a barrier: keys pressed meanwhile are buffered and replayed (in order) once the new snapshot is drawn. The buffer is written to `sessionStorage` (`bana.keys`) whenever it changes and typed again after the log replay. `exec` never awaits between a key and its logged command (the `focus.set` before a command is queued, not awaited).
- Windows (DIFFS N60): every request carries `window` (an id kept in the tab's sessionStorage). The first window uses the plain panel names; every other window's panels are `<window>:<name>` in `Host`, so pages, carets and selections never mix. Line renames carry `base` (the text the edit started from; a merged rename keeps the first one's); `Host` rebases the change over text another window wrote since (`rebase`, one splice per side), and the editor adopts another window's text keeping its own caret (`shiftedOffset`). `rev` stale checks are per client. Tests: `windows.spec.ts`.
- Durable answers (DIFFS N77): the engine appends every edit to `graph.bana.log` (Loro updates, length-prefixed, fsynced) before it answers, with the request's `wal` number in the document (`Graph::note_applied`), so a page may drop an edit on the answer and a resend after a crash runs once; a full save folds the log into the file. Enter carries its line and caret (`node.enter {node, caret}`); after an outage a caret on a line that is gone is cleared, so nothing lands in another line. While offline, keys waiting behind a command show under the line (`waitingText`, N78). Test: `offline.spec.ts` kills the engine 0 to 800 ms after typing (`KILL_SEEDS`).
- Offline (DIFFS N73): the HTTP transport throws `Offline` when the engine cannot be reached; the queue keeps that request first (and logged), shows the offline notice (`.toast.offline`) and retries with backoff. When the engine answers again the `onResume` hook (controller) puts back each panel's page and the caret before the queued edits go (a restarted engine forgot them), then everything redraws. Test: `offline.spec.ts` kills and restarts the server mid-typing.
- A click on a line made while keys still wait in the buffer is queued behind them (`KeyInfo.click`); keys pressed before a click or a reload never leave typing going on in a line (`staleKeys`).
- Another window splitting a line (N74): `Host` records the split; carets of other windows past the split move to the second half, and renames sent from the old text that change only text past the split go there (`follow_split`).
- The page title is in the same pipeline: its keys are buffered while earlier keys wait, and buffered keys drained while the title has the keyboard go through the title's own key handler (`TitleHandle.key`), so a replayed Down leaves the title as a typed one does.
- Before a command, `exec` tells the engine where the caret is (`focus.set`) if it moved without a command.
- Line editors (`LineEditor` in `Outline.tsx`) send `node.setName` on every input and adopt the engine's text only when none of their own edits are pending.
- The outline is virtualized; a row never holds unsaved text. A key pressed while the focused line is scrolled away scrolls it back (`focusScroll`) and is replayed into it.
- Playwright fuzz tests (`web/tests/fuzz.spec.ts`) check that no typed character is lost, also across reloads and force-kills. Run them after any editor change; `FUZZ_SEEDS=40` for a long run. The reload test runs with the CPU throttled 4 to 6 times (CDP `Emulation.setCPUThrottlingRate`), so 40+ keys are still queued at the reload; it has no retry.

## Web and Tauri gotchas

- `.panel .title` is either an `h1` (fixed titles) or a `textarea.title-input`; Playwright's `toHaveValue` fails at once on the `h1`, so use `.title-input`.
- Playwright types far faster than people: anything async between a key and the next must keep order (queue + barrier), or keys land in the wrong line.
- Tauri: `cargo run -p bana-desktop` embeds `web/dist` (the `custom-protocol` feature is on by default); `pnpm --dir web tauri dev` uses the Vite dev server instead. A fresh checkout without `web/dist` gets a placeholder page from `build.rs`.
- The HTTP server refuses other origins and Host headers (DNS rebinding); `sendBeacon` uses `text/plain` so it stays a simple request.
- Each Playwright test starts its own `bana serve` (fixtures.ts) with a fixed today (2026-10-10); build the CLI first.
- `useStore(selector)` must return state that already exists (a primitive, or an object found in the state). A selector that builds a new object every call loops forever (React error #185).
- Keys must wait while earlier keys wait: `inBarrier()` is true while the buffer is non-empty, not only while a command runs, or a fast key overtakes buffered ones.
- A page read that a newer read overtook is dropped (`refreshSeq`); drawing it would mount an editor on a line that no longer has the caret, and keys would land there. Callers of `refresh()` still wait until a read at least as new is drawn.
- After a command that moves the caret, keys wait until the new line's editor registers (`holdKeysForEditor`), because React registers editors in an effect after the paint.
- Editors unregister only themselves (`unregisterEditor(h)`): a line editor unmounting after a field editor mounted must not drop the field editor.
- Pickers opened from buttons (not from `#`/`@` typed in a line) get their own search box (`at < 0`).
- Playwright `testIgnore`/`testMatch` regexes match the whole path: anchor them (`[/\\](shots|measure)\.spec\.ts$`), or `zz.spec.ts` also hides `fuzz.spec.ts`.
- The `app` fixture fails a test on any error thrown in the page.
- Measure layout with `pnpm exec playwright test --project=measure` (prints element boxes in 1x points) and compare with `../tanacraft-spec/12-screens-visual.md`; sample spec colours with a darkest/lightest-pixel script.
- Inter Variable from `@fontsource-variable/inter/opsz.css` (optical size axis): body and title widths match the spec at the automatic size (`font-optical-sizing: auto`); never pin `opsz` (a pinned 20 made headings 4 % wide, N59.7).
- Typing with nothing focused (after a click elsewhere or a reload) puts the caret back in the line that had the caret last and types nothing, keys already waiting included (N61); only an Escape-cleared selection (N27, N34) lets typing go on in that line.
- Keys only reach an editor the person can see: a caret or selection in rows the panel does not show (a table, board or calendar view, a configure panel) takes no keys (`outlineVisible`), and typing with the keyboard on a button goes nowhere (N35). The fuzz test "typed keys land only in a visibly focused editor" checks it.
- Commands that move the caret inside the same line (format marks, joins) hand their text and caret to the editor at once (`adopt`); do not make every refresh re-place the caret, it races with typing (seen as picker flakes under load).
- Run the pickers and editing specs with `--repeat-each=4 --workers=4` after touching the key path: some races only show under load.
- A field name typed after `>` is sent as `fieldDraft`; saves write it in as a field so a force-kill keeps it (N26). Renames and field values carry `rev`; the engine drops older ones.
- `PAGE_STACKS=1` makes the fixture report page errors with their stack (find the source in `dist/assets/*.js` at the line and column).
- Every tool and menu on a page is a pointer target too: menus fit the window (`NodeMenu` measures itself), popovers that hold submenus need `overflow: visible`.
- DIFFS value pairs ("A vs B") are not always Tana first (N59.33 to 35 and N67 give the clone first). Settle each with spec 12 and the paired `screens/diffs/dNN` image (Tana on the left) before changing anything.
- Playwright HTML5 drags with `mouse.down/move/up`: the first moves start the drag, and a pause before `mouse.up` loses the drop unless one more `mouse.move` comes first. Measure boxes after the 250 ms modal entry (scale 0.95 to 1).
- Never rebuild `web/dist` while another agent's Playwright run is going: the tests serve it, and a rebuild mid-run fails tests that pass alone.
- DIFFS gives newer measurements than spec 12 (round 10: the page menu opens right under its button, not 7 px below); the newer reading wins.
- Local tools and tests reach the engine with the install's token (`BANA_HOME/token`; the fixtures give each test engine its own `BANA_HOME`). A request from a page carries its Origin and needs no token.
- Files on nodes travel as base64 in `file.attach` and are not written to the session log (too big); everything else still is.
- A line editor takes the engine's text only from a page read at least as new as its own last answered edit (`acked`; `run` results carry the engine `version`). Under load a read issued before the edit was drawn after it and put the caret back one letter (two windows typing in one line).
- Flake hunts: `pnpm exec playwright test --project=tests --repeat-each 5`, then `CPU_THROTTLE=4` (the fixture slows every page 4 times through CDP). Fix the cause; no retries, skips or longer timeouts.
- Tests that change data with `server.run` race the UI: wait until the change is drawn before clicking by position (a table cell `nth(1)` clicked before the new column existed hit the row's empty last cell), and do not follow a UI action with a `server.run` that depends on it (the window's own queued request can reach the engine later); use the UI instead.
- The capture window (UC-08.14) is the same page at `#capture`: `main.tsx` draws only `QuickAdd` with `onDone` (Tauri `capture_done`, else `window.close`). Its Tauri label `capture` must stay in `capabilities/default.json`; menu events go to `main` only (`emit_to`).
- Command buttons (UC-06.33): `row.buttons` on node and field rows (`.cmd-btn`), set with `commandNode.button`, configured in Configure supertag and on the field page (Commands).
- A command result with a `message` string is shown as a toast by `exec` (`node.count`). Inline references are `[[id:N]]` or `[[id:N|alias]]` in both parsers (`format.rs`, `format.ts`); rewrite tokens with `format::map_refs`, never a string replace of `[[id:N]]` (it misses aliases).
- Node links (`node.link`) are `http://127.0.0.1:PORT/?node=N`: `main.tsx` opens the node and drops the parameter; pasted into a line they become an inline reference.
- The account menu holds one entry Tana's has not (Export everything as Markdown, UC-08.13); its submenu needs the menu's `overflow: visible`.
- A locked node (UC-06.35) is refused by `Session::check_lock` for every mutating command with `node` in its params or a locked `parent`; its editor is `readOnly`.
