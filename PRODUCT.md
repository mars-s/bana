# TanaCraft: product brief

## What it is

A local-first, keyboard-first outliner. Every line is a node; nodes nest, carry supertags with
fields, reference each other and live under daily notes. One graph file per person (a Loro
document, so copies edited apart merge). Every action is a command with an id, so the keyboard,
the command line, the CLI, the MCP server and the JSON API all do the same things.

It targets the behaviour and the look and feel of the Tana outliner, rebuilt clean-room from a
black-box spec (`../tanacraft-spec/`) with our own code, icons and pictures.

## Who it is for

People who think in outlines and want structure without forms: writers of daily notes, people
running tasks and meetings as tagged nodes, and agents that read and write the same graph.

## What it must feel like

The look is set by `DESIGN.md` (adopted from the spec team's observations) and the principles in
`../tanacraft-spec/13-design-principles.md`. In one breath: **the quiet workbench**. A dense,
flat, grey frame; a slightly different page sheet; colour only for tags, links, selection, AI
and done; Inter at 15 px on a 3.75 px step; rows 28.5 px tall; controls that appear on hover;
almost nothing animates.

Non-negotiables, in order:

1. **No typed character is ever lost.** Not on a fast keyboard, not on a row scrolled away, not
   on reload, quit or a force-kill. The web UI never owns data; the engine does.
2. **The keyboard never waits.** Navigation, expand and collapse, menus and the command line are
   instant (`../tanacraft-spec/14-motion.md`).
3. **The page is the content.** Chrome stays quiet; nothing on the page casts a shadow.
4. **Both themes are tuned, not inverted.** Regular weight 375 in dark, 400 in light.

## Out of scope for now

AI features, sync over the network (merging works offline; transport comes later), Android. The iPhone app (`apps/tanacraft-ios`) is a first version on the same engine.

## Where things are

| Concern | File |
|---|---|
| Design tokens (every colour, size, radius, duration) | `web/src/tokens.css` |
| Design system (observed facts) | `DESIGN.md` |
| Icons (our own SVGs) | `web/src/icons.tsx` |
| Screens and their tests | `web/src/*.tsx`, `web/tests/` |
| The engine | `crates/core`, `crates/commands`, `crates/server` |
