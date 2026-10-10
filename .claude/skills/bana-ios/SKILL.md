---
name: bana-ios
description: Use when working on the Bana iPhone app (Swift).
category: frontend
---

# Bana iPhone app

The iPhone app is native SwiftUI in `apps/bana-ios`, built for iOS 26. Glass goes on the chrome that floats over notes. Outline rows stay flat.

## Liquid Glass

- The deployment target is iOS 26. Do not add an iOS 18 fallback. Tell the user that phones on older iOS cannot run the app.
- Glass covers the top nav bar, the bottom bar, the keyboard indent and outdent bar, the quick-capture button, the search field and sheets.
- Use system pieces first: NavigationStack, toolbar and TabView pick up glass in Xcode 26. For custom controls use `.glassEffect()`, `GlassEffectContainer` with `glassEffectID` to morph grouped controls, and `.buttonStyle(.glass)` or `.glassProminent`.
- Let the outline scroll under the bars, so the glass has content to blur.
- Respect Reduce Transparency and Increase Contrast.
- Prove it with simulator screenshots in light and dark, with notes scrolled under the bars.

## Tokens

- `apps/bana-ios/Bana/Tokens.swift` is generated. Never hand-edit it.
- After any change to `web/src/tokens.css`, run the generator `gen-ios-tokens.py`. `cargo xtask ci` runs it with `--check` and must fail when the Swift file is stale.

## Build noise

- Red SourceKit errors in a fresh worktree ("No such module 'UIKit'", "Cannot find type 'Engine' in scope") are index noise until a real build runs. Judge by `xcodebuild`, not by the editor.

## Design source

- Use `../tanacraft-spec/` and Tana's public docs only. Never ask the user to install the Tana iPhone app or send its screen recordings. See the wall in `../WALL.md`.
- Load `apple-design` and `write-swift` before writing Swift.
