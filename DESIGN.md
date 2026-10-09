---
name: TanaCraft (Tana Outliner look, observed)
description: Dense, quiet outliner chrome where colour belongs to tags; measured from Tana Outliner 1.531.0, dark and light.
colors:
  dark-chrome: "#232526"
  dark-panel: "#1a1c1e"
  dark-raised: "#1e2022"
  dark-button: "#282a2b"
  dark-hover: "#353636"
  dark-pressed: "#434444"
  dark-popover: "#232526"
  dark-tooltip: "#2e2f30"
  dark-divider: "#2e2f30"
  dark-outline: "#3b3c3d"
  dark-text-primary: "#cdcdcd"
  dark-text-strong: "#eaeaea"
  dark-text-secondary: "#b2b2b3"
  dark-text-tertiary: "#a9a9a9"
  dark-text-muted: "#828282"
  dark-text-faint: "#6e6f6f"
  dark-link: "#5c97e5"
  dark-link-hover: "#9bbef0"
  dark-focus: "#297dd9"
  dark-selection: "#21365a"
  dark-selection-ring: "#2a5897"
  dark-active-item: "#2a5897"
  dark-drop-line: "#6a96df"
  dark-toggle-on: "#437c5d"
  dark-confirm-text: "#94cba0"
  dark-ai: "#d66ed1"
  dark-code-text: "#dc9544"
  dark-code-bg: "#2b2d30"
  dark-highlight: "#615a19"
  light-chrome: "#f4f4f4"
  light-panel: "#ffffff"
  light-raised: "#f4f4f4"
  light-button: "#f4f4f4"
  light-hover: "#e3e3e3"
  light-pressed: "#f8f8f8"
  light-popover: "#fefefe"
  light-tooltip: "#3b3c3d"
  light-divider: "#efefef"
  light-outline: "#dcdddd"
  light-text-primary: "#282a2b"
  light-text-strong: "#141719"
  light-text-secondary: "#535454"
  light-text-tertiary: "#6e6f6f"
  light-text-muted: "#8c8c8c"
  light-text-faint: "#bcbcbc"
  light-link: "#297dd9"
  light-link-hover: "#2b64ac"
  light-focus: "#5c97e5"
  light-selection: "#d7e6fa"
  light-selection-ring: "#c3d9f7"
  light-active-item: "#438ae1"
  light-drop-line: "#a2bdec"
  light-toggle-on: "#7ac090"
  light-confirm-text: "#437c5d"
  light-ai: "#c745b9"
  light-code-text: "#cd7389"
  light-code-bg: "#f6f6f6"
  light-highlight: "#f8e7a2"
  done-green: "#5aa67a"
  today-marker: "#d34e2c"
  quick-add-accent: "#aa9615"
  tag-pink-dark-fg: "#fa3ba4"
  tag-pink-dark-tint: "#5b032f"
  tag-pink-dark-tint-strong: "#79053f"
  tag-pink-light-fg: "#ea0079"
  tag-pink-light-tint: "#fbe9f2"
  tag-pink-light-tint-strong: "#f9ddeb"
  tag-blue-dark-fg: "#0089ff"
  tag-blue-dark-tint: "#002965"
  tag-blue-dark-tint-strong: "#003789"
  tag-blue-light-fg: "#0062ff"
  tag-blue-light-tint: "#e3eeff"
  tag-blue-light-tint-strong: "#d5e6ff"
  tag-amber-dark-fg: "#c87a00"
  tag-amber-dark-tint: "#3d2804"
  tag-amber-light-fg: "#a16100"
  tag-amber-light-tint: "#f8ecd7"
  tag-neutral-dark-fg: "#8c8c8c"
  tag-neutral-dark-tint: "#353636"
  tag-neutral-light-fg: "#828282"
  tag-neutral-light-tint: "#f4f4f4"
typography:
  page-title:
    fontFamily: "Inter Variable, Inter, system-ui, sans-serif"
    fontSize: "29px"
    fontWeight: 550
    lineHeight: "33.4px"
  section-title:
    fontFamily: "Inter Variable, Inter, system-ui, sans-serif"
    fontSize: "19px"
    fontWeight: 600
    lineHeight: "21.9px"
  body:
    fontFamily: "Inter Variable, Inter, system-ui, sans-serif"
    fontSize: "15px"
    fontWeight: 375
    lineHeight: "21px"
  body-light-theme:
    fontFamily: "Inter Variable, Inter, system-ui, sans-serif"
    fontSize: "15px"
    fontWeight: 400
    lineHeight: "21px"
  heading-inline:
    fontFamily: "Inter Variable, Inter, system-ui, sans-serif"
    fontSize: "15px"
    fontWeight: 600
    lineHeight: "17.25px"
  dense:
    fontFamily: "Inter Variable, Inter, system-ui, sans-serif"
    fontSize: "14px"
    fontWeight: 375
    lineHeight: "19.6px"
  label:
    fontFamily: "Inter Variable, Inter, system-ui, sans-serif"
    fontSize: "12px"
    fontWeight: 375
    lineHeight: "16.8px"
  button-small:
    fontFamily: "Inter Variable, Inter, system-ui, sans-serif"
    fontSize: "12px"
    fontWeight: 450
    lineHeight: "13.2px"
  meta:
    fontFamily: "Inter Variable, Inter, system-ui, sans-serif"
    fontSize: "10px"
    fontWeight: 375
    lineHeight: "11px"
  caps-label:
    fontFamily: "Inter Variable, Inter, system-ui, sans-serif"
    fontSize: "10px"
    fontWeight: 600
    lineHeight: "11.5px"
    letterSpacing: "0.4px"
  code:
    fontFamily: "ui-monospace, JetBrains Mono, monospace"
    fontSize: "12.75px"
    lineHeight: "17.85px"
rounded:
  hair: "1.875px"
  sm: "3.75px"
  md: "5.625px"
  menu: "6px"
  lg: "7.5px"
  xl: "10.5px"
  panel: "11.25px"
  card-ai: "12px"
  full: "9999px"
spacing:
  q1: "3.75px"
  q1-5: "5.625px"
  q2: "7.5px"
  q3: "11.25px"
  q4: "15px"
  q5: "18.75px"
  q6: "22.5px"
  q8: "30px"
  q10: "37.5px"
  indent: "32.25px"
components:
  outline-row:
    typography: "{typography.body}"
    textColor: "{colors.dark-text-primary}"
    padding: "3.75px 0"
    height: "28.5px"
  bullet:
    size: "15px"
    rounded: "{rounded.full}"
  bullet-collapsed:
    backgroundColor: "{colors.dark-pressed}"
    size: "15px"
    rounded: "{rounded.full}"
  checkbox:
    backgroundColor: "#ffffff26"
    size: "18px"
    rounded: "{rounded.sm}"
  checkbox-checked:
    backgroundColor: "{colors.done-green}"
    textColor: "#ffffff"
    size: "18px"
    rounded: "{rounded.sm}"
  field-row:
    backgroundColor: "{colors.dark-raised}"
    rounded: "{rounded.lg}"
    padding: "0 7.5px"
    height: "34.5px"
  tag-chip:
    backgroundColor: "{colors.tag-pink-dark-tint}"
    textColor: "{colors.tag-pink-dark-fg}"
    typography: "{typography.label}"
    rounded: "{rounded.sm}"
    padding: "0.75px 3.75px"
    height: "17px"
  sidebar-item:
    textColor: "{colors.dark-text-secondary}"
    rounded: "{rounded.sm}"
    padding: "7.5px"
    height: "30.4px"
    width: "195px"
  sidebar-item-hover:
    backgroundColor: "{colors.dark-hover}"
    textColor: "{colors.dark-text-primary}"
  button-small:
    backgroundColor: "{colors.dark-button}"
    textColor: "{colors.dark-text-primary}"
    typography: "{typography.button-small}"
    rounded: "{rounded.sm}"
    padding: "3.6px 4.8px"
    height: "21.4px"
  button-small-pressed:
    backgroundColor: "{colors.dark-pressed}"
    textColor: "{colors.dark-text-strong}"
  button-icon:
    rounded: "{rounded.sm}"
    padding: "7.5px"
    size: "29px"
  button-ghost:
    textColor: "{colors.dark-text-tertiary}"
    typography: "{typography.button-small}"
    rounded: "{rounded.sm}"
    padding: "3.6px 4.8px"
  menu:
    backgroundColor: "{colors.dark-popover}"
    rounded: "{rounded.menu}"
    padding: "5.625px"
    width: "210px"
  menu-item:
    textColor: "{colors.dark-text-secondary}"
    typography: "{typography.label}"
    rounded: "{rounded.sm}"
    padding: "5.625px 11.25px 5.625px 30px"
    height: "28px"
  menu-item-hover:
    backgroundColor: "{colors.dark-hover}"
  command-line:
    backgroundColor: "{colors.dark-popover}"
    rounded: "{rounded.md}"
    width: "500px"
  command-item-active:
    backgroundColor: "{colors.dark-active-item}"
    textColor: "#ffffff"
    typography: "{typography.dense}"
    padding: "11.25px 22.5px"
    height: "42.1px"
  modal:
    backgroundColor: "{colors.dark-panel}"
    rounded: "{rounded.lg}"
    padding: "22.5px 30px"
    width: "630px"
  search-dialog:
    backgroundColor: "{colors.dark-panel}"
    rounded: "{rounded.xl}"
    width: "585px"
  search-input:
    backgroundColor: "#141719"
    textColor: "{colors.dark-text-secondary}"
    rounded: "{rounded.md}"
    padding: "6px 10.5px 6px 31.5px"
    height: "34px"
  toast:
    backgroundColor: "{colors.dark-popover}"
    textColor: "{colors.dark-text-secondary}"
    typography: "{typography.dense}"
    rounded: "{rounded.xl}"
    padding: "14px"
  tooltip:
    backgroundColor: "{colors.dark-tooltip}"
    textColor: "{colors.dark-text-strong}"
    typography: "{typography.label}"
    rounded: "{rounded.sm}"
    padding: "3.75px 7.5px"
    height: "24.3px"
  tab-pill-active:
    backgroundColor: "{colors.dark-hover}"
    textColor: "{colors.dark-text-strong}"
    typography: "{typography.label}"
    rounded: "{rounded.md}"
    height: "26.3px"
    width: "178px"
  view-tab-active:
    backgroundColor: "{colors.dark-hover}"
    textColor: "{colors.dark-text-strong}"
    typography: "{typography.label}"
    rounded: "{rounded.sm}"
    padding: "4.2px 6px"
    height: "25.2px"
  board-card:
    backgroundColor: "{colors.dark-raised}"
    rounded: "{rounded.md}"
    width: "300px"
  table-row:
    typography: "{typography.dense}"
    height: "31.1px"
  toggle-off:
    backgroundColor: "#4b4b4c"
    rounded: "{rounded.full}"
    width: "29.4px"
    height: "18.2px"
  toggle-on:
    backgroundColor: "{colors.dark-toggle-on}"
    rounded: "{rounded.full}"
    width: "29.4px"
    height: "18.2px"
  segment-active:
    backgroundColor: "{colors.dark-selection}"
    textColor: "#c3d9f7"
    typography: "{typography.button-small}"
    padding: "3.6px 4.8px"
---

# Design System: TanaCraft (Tana Outliner look, observed)

All values were measured in the running Tana Outliner desktop app at 1x points, dark and light theme. Component tokens in the frontmatter are written with the dark-theme colours; the light equivalent of every `dark-*` colour is the matching `light-*` token. Full tables: `10-visual-design.md`; states: `11-visual-states.md`; motion: `14-motion.md`; icons and pictures: `15-asset-inventory.md`.

## Overview

**Creative North Star: "The Quiet Workbench"**

A dense, flat, grey workspace where the only things that light up are the things you named: tags, links and your current selection. The frame (top bar and sidebar) is one undivided grey; the page is a slightly different sheet with a softly rounded bottom; everything floating gets a real shadow, and nothing else does.

Type does the work: one family (Inter), a small ladder (29 / 19 / 15 / 14 / 12 / 10 px), and hierarchy by weight and grey level more than by size. Rows are tight (28.5 px) but the column is narrow (780 px max), so the page breathes sideways instead of vertically. Controls hide until the pointer is near them. Almost nothing animates.

**Key Characteristics:**
- 15 px root and a 3.75 px spacing step (every gap is a multiple of a quarter of 15).
- Regular weight 375 in dark, 400 in light.
- Colour reserved for tags (four derived tones per hue), blue system state, green done, magenta AI.
- Flat page, shadowed floats.
- Instant navigation, instant expand/collapse, instant menus.

## Colors

A neutral grey system in two tuned themes plus a twelve-hue tag palette.

### Primary
- **System Blue** (`dark-link` / `light-link`): links and references; the same family drives selection fill (`*-selection`), selection ring, active list item, active segment, focus ring and the drop line. It never appears as a large fill except under a selected row.

### Secondary
- **Tag hues** (`tag-*`): each tag colour is rendered through fg, tint, tint-strong and a header glow. Chip = fg text on tint; tagged bullet = tint-strong halo with an fg ring and dot; tag tile = tint-strong. The full twelve-hue table is in 10 §1.5.

### Tertiary
- **Done Green** (`done-green`): checked checkboxes; toggles use `*-toggle-on`; confirm buttons use `*-confirm-text`.
- **AI Magenta** (`*-ai`): only the AI chat item and sparkle.
- **Today Vermilion** (`today-marker`) and **Quick-add Gold** (`quick-add-accent`): one place each.

### Neutral
- **Chrome Grey** (`*-chrome`): window, top bar, sidebar.
- **Page Sheet** (`*-panel`): panels, dialogs, inputs.
- **Raised Tone** (`*-raised`): field boxes, side widget, view-tab strip, settings nav, cards.
- **Button / Hover / Pressed** (`*-button`, `*-hover`, `*-pressed`).
- **Hairlines** (`*-divider`, `*-outline`).
- **Text ladder** (`*-text-strong` → `primary` → `secondary` → `tertiary` → `muted` → `faint`).

### Named Rules
**The Earned Colour Rule.** Saturated colour appears only for a tag, a link or selection, AI, or success. Everything else is grey.

**The Chip Rule.** A tag chip is always its own colour on a tint of itself; never white text on a saturated fill.

## Typography

**Display Font:** Inter Variable (with system-ui)
**Body Font:** Inter Variable (with system-ui)
**Label/Mono Font:** system monospace (ui-monospace) for inline code only

**Character:** one neutral grotesque used at unusual intermediate weights (375, 450, 550) so the page reads soft in dark and crisp in light.

### Hierarchy
- **Display** (550, 29 px, 33.4 px): page titles and the day title. 600 on the Supertags overview.
- **Headline** (600, 19 px, 21.9 px): view titles, configure-section titles, quick-add title, heading nodes (26.6 px line), settings section titles. Settings page title is 23 px 600.
- **Title** (600, 15 px, 17.25 px): list group headers, references header, sub-section headings.
- **Body** (375 dark / 400 light, 15 px, 21 px): every outline line, field name and value. Max column 780 px.
- **Dense body** (375 / 400, 14 px, 19.6 px): tables, cards, toasts, command line, search results, configure-panel prose.
- **Label** (375 / 400, 12 px, 16.8 px): tabs, menus, breadcrumbs (13.2 px line), help text; buttons at 450 weight; config labels at 600.
- **Meta** (375 / 400, 10 px): result crumbs, event times; caps labels at 600 with 0.4 px tracking.

### Named Rules
**The Weight Before Size Rule.** To emphasise, raise the weight or step up the grey ladder before reaching for a bigger size.

## Layout

- Window: top bar 45 px tall (traffic-light inset 85 px); sidebar 225 px (mini rail 45 px, or hidden); panel area inset 6 px from the right and bottom window edges.
- Panels split the panel area evenly with a 6 px gap. Each panel: 40 px breadcrumb row, 37.5 px side padding.
- Outline: column max 780 px centred; 757.6 px when a 403 px side widget sits 45 px to its right. Rows 28.5 px; indent 32.25 px per level; 1 px guide line under each expanded parent's bullet.
- Field boxes: name column 172.5 px, values from 201 px.
- Tables, boards, calendars use the full panel width. Board columns 300 px with 30 px gaps; month cells 172 × 54.5; week hour rows 45 px.
- Spacing scale: multiples of 3.75 px (`spacing` tokens). Common: 3.75 row padding, 7.5 control padding, 11.25 gaps, 15 section padding, 22.5 / 30 modal padding, 37.5 page padding.

## Elevation & Depth

Flat by default, with one shadow recipe for everything that floats. Depth on the page comes from tone: the page sheet is one step off the chrome, and raised content (field boxes, cards, widgets) is one step off the page.

### Shadow Vocabulary
- **Float** (three layers: y 1.5 blur 1.5 black 20 %; y 1.5 blur 1.5 black 25 %; y 11.25 blur 22.5 black 30 %; light theme 5 % / 3 % / 4 %): menus, popovers, command line, selection toolbar, quick add, search dialog, toasts. Paired with a 1 px outline (`*-outline`).
- **Tooltip** (first two layers of Float only).
- **Chip lift** (y 0.5, spread 0.5, black 20 %): under a hovered tag chip.
- **Knob** (y 2.8, blur 8.4, black 30 %): toggle knob.
- **Scrim**: black 75 % (dark) / 40 % (light) behind quick add and search, never behind the command line.

### Named Rules
**The Flat Page Rule.** Nothing that lives on the page casts a shadow. Only things floating above it do.

## Shapes

Soft, consistent corners that grow with the object: 1.875 px for breadcrumb buttons and inputs, 3.75 px for nearly every control (buttons, chips, menu items, checkboxes, selection), 5.625 px for tabs, cards and command line, 6 px menus, 7.5 px field boxes, side widget and modals, 10.5 px search dialog and toasts, 11.25 px panel bottoms, 12 px AI suggestion cards, full circles for bullets, avatars, toggles and the today marker, 20 % squircles for tag "#" squares. Borders are 1 px hairlines (0.5 px on buttons); a 3 px left bar marks calendar events; a 5 px top bar marks quick add.

## Components

### Buttons
- **Shape:** gently rounded (3.75 px).
- **Small filled:** raised button grey with primary text, 21.4–26.2 px tall, padding 3.6 × 4.8 px (small) or 4.9 × 7 px (regular), 12 px / 450 or 14 px / 450.
- **Hover / Focus:** a 0.5 px border appears (`#5c5c5d` / `#cdcdcd`) and the text brightens; pressed fill steps up one grey; keyboard focus draws a 1 px solid blue ring, no offset.
- **Ghost:** no fill, tertiary text, fills with the hover grey.
- **Icon:** 29 px square, 14 px icon, padding 7.5 px.
- **Segmented:** butted segments; selected one gets the selection fill, blue text and a blue hairline.

### Chips
- **Style:** two joined pills ("#" and name) in the tag tint with fg text, 12 px, 17–21 px tall.
- **State:** on hover a "×" part appears at the left and the corners round; a hairline lift appears. Active view filters use the blue selection fill with an "×" at the right.

### Cards / Containers
- **Corner Style:** 5.625 px (board cards), 7.5 px (field boxes, side widget, tag tiles), 12 px (AI suggestion cards).
- **Background:** raised tone.
- **Shadow Strategy:** none (Flat Page Rule).
- **Border:** 1 px outline on cards; field boxes have none in dark and a 1 px `#efefef` outline in light.
- **Internal Padding:** 7.5 × 11.25 px in card sections, divided by 1 px hairlines.

### Inputs / Fields
- **Style:** page-sheet fill with a 1 px inset hairline ring, 1.875 px radius, 7.5 px padding; dialog search inputs use a darker well (`#141719` / `#f8f8f8`) with 5.625 px radius and a magnifier inside.
- **Focus:** no ring or glow; only the caret.
- **Placeholders:** faint grey, phrased as an instruction.
- **Toggle:** 29.4 × 18.2 px pill, 14 px knob, green when on.
- **Checkbox:** 18 px, translucent fill, green with a white tick when checked.

### Navigation
- **Sidebar:** 195 px rows, 14 px secondary text with 14 px icons; hover and active rows fill with the hover grey; pressed text turns link blue. Section headings 14 px 600 muted.
- **Tabs:** 178 × 26.3 px pills in the top bar; active one filled.
- **Breadcrumbs:** 12 px muted crumbs separated by slashes; long paths collapse into a "…" crumb.
- **View tabs:** a raised strip with 25 px tabs; active tab filled.

### Outline row (signature)
A 15 px bullet, then text 22.5 px to its right. Leaf: 5 px grey dot. Collapsed parent: the dot sits in a 15 px grey halo. Tagged: halo in the tag's strong tint with an fg ring and dot. Search node: a tiny magnifier in the halo. On row hover a round expand button appears 21 px to the left. Selected rows fill blue with a 1 px blue ring and a floating action bar 7.5 px above.

### Menus and command line
Menus 210 px wide, 28 px items with a 30 px icon inset, shortcuts right-aligned in muted text, hairline separators, hover fill. The command line is a 500 px float without a scrim; its active item is a full-bleed blue bar with white text.

## Do's and Don'ts

### Do:
- **Do** build on a 15 px root and multiples of 3.75 px for every gap, size and radius.
- **Do** use weight 375 for regular text in dark theme and 400 in light theme.
- **Do** derive four tones per tag colour (fg, tint, strong tint, glow) and use them exactly as in 10 §1.5.
- **Do** keep outline rows at 28.5 px and indent at 32.25 px.
- **Do** show row and header controls only on hover (header "Icon" / "Banner" after a 400 ms delay).
- **Do** keep navigation, expand/collapse, menus and panel changes instant.
- **Do** use the single Float shadow plus a 1 px outline for every floating surface.

### Don't:
- **Don't** give a drop shadow to anything that sits on the page (field boxes, board cards and widgets use tone and at most a 1 px outline).
- **Don't** use saturated colour for anything that is not a tag, link or selection, AI or success.
- **Don't** put white text on a tag colour; chips are fg on tint.
- **Don't** animate expand/collapse, page changes or menu open/close.
- **Don't** add illustrations to empty states; one muted sentence is the pattern.
- **Don't** use pure white text or pure black backgrounds in dark mode (`#cdcdcd` on `#1a1c1e`).
