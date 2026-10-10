---
name: tanacraft-assets
description: Use when TanaCraft needs a raster image (app icon, empty-state art, page banner, onboarding art, sample covers) or a new UI icon.
---

# TanaCraft assets

Two kinds of asset, made two different ways.

| Asset | How | Why |
|---|---|---|
| UI icons (sidebar, toolbar, bullets, menus, chips) | Draw as inline SVG in `web/src/icons.tsx` (16x16 grid, `currentColor`) | Stays crisp at 12 to 20 px, follows the theme colour, no file |
| App icon, empty states, banners, onboarding, sample covers | Generate with Codex CLI (gpt-image-2) | Pictures, not glyphs |

Never use AI images for UI icons. They blur at small sizes.

## The originality rule

Every asset is ours. Never pass a Tana screenshot, Tana icon or Tana art to the image model, never ask it for "Tana's icon" or "in Tana's style", and never trace Tana art. Describe what you want in plain words: shape, colours, mood. Matching Tana's layout and colours in the UI is fine; copying its pictures is not.

## Pictures that mirror Tana's (clean-room flow)

For backdrops, wallpapers, banners, empty states and onboarding art that should feel like Tana's:

1. Take the brief for that picture (id like `P01`) from `../tanacraft-spec/15-asset-inventory.md`. It gives job, size, mood, palette and style in words.
2. Generate from those words only. Never give the model a Tana image or ask for "Tana's" anything.
3. Name the file after the brief id (`p01-day-banner.png`) and list it in `ATTRIBUTION.md` with the brief id.
4. The spec team compares each result with the original and flags any that look like copies. Regenerate flagged ones with a different composition.

## Generate an image

Run Codex one-shot with stdin closed, from the folder the image should land in:

```bash
cd assets/images && codex exec --skip-git-repo-check -s workspace-write --enable image_generation \
  "Generate one image with your image generation tool (gpt-image-2): <description>. Size <W>x<H>. No text. Save it as <name>.png in the current directory, then print the path." \
  < /dev/null > /tmp/codex-img.log 2>&1
```

- It takes about 30 to 90 s. Codex also keeps a copy under `~/.codex/generated_images/`.
- If the log goes silent for minutes, it is waiting on input: kill it and rerun with `< /dev/null`.
- **Open the PNG and look at it** before using it. Regenerate if it has text, artefacts or the wrong mood.
- Make several in one call when they belong together ("generate 4 images ... save as empty-1.png ... empty-4.png") so the style matches.

## After generating

1. Resize or convert down to the size used (keep a 2x copy for Retina). Keep files small: banners under ~300 KB, use WebP or compressed PNG.
2. Put files in `assets/images/` (pictures) or `assets/icon/` (app icon set, `.icns` via `iconutil`).
3. Add a row to `ATTRIBUTION.md`: file, "generated with gpt-image-2 via Codex CLI for TanaCraft", date, prompt summary.
4. Web pictures go in `web/src/assets/` and are imported from the component (Vite bundles them; the Tauri app embeds `web/dist`). The app icon set: `pnpm --dir web exec tauri icon <png> -o ../apps/tanacraft-desktop/icons`.
5. Run `cargo xtask shots`, look at every screen that shows the asset, in dark and light.

## Style brief (keep every picture consistent)

Flat, quiet, editorial. Soft geometric shapes, generous empty space, one or two accent colours from the theme tokens in `web/src/tokens.css`, works on the dark background and the light one. No text, no people's faces, no logos.
