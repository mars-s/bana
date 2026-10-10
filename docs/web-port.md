# Porting the egui UI tests to the web UI

The old egui front end (`legacy/egui`, tag `egui-final`) had 92 UI tests. They are the checklist
for the web UI (`web/`, Playwright tests in `web/tests/`). The engine behind both is the same, so
what moves is the UI half: which keys and clicks run which commands, and that no typed character
is lost on the way.

Status: **ported** (a Playwright test covers it), **works** (behaves, no web test yet), **todo**.

## Data safety (ui_fuzz.rs)

| egui test | Status | Web test |
|---|---|---|
| random_keys_never_crash_the_ui | ported | `fuzz.spec.ts` random keys (no page error, every character kept) |
| random_keys_on_long_pages_never_lose_text | ported | `fuzz.spec.ts` long page in a small window, scrolled away mid-edit |
| quitting_at_a_random_point_keeps_typed_text | ported | `fuzz.spec.ts` reload at a random point; force-kill of the engine after autosave; `offline.spec.ts` engine killed and restarted mid-typing |
| damaged_files_draw_in_every_view | ported | `damaged.spec.ts` (the engine side is `crates/commands/tests/fuzz.rs`) |
| 50k-node frame test (perf.rs) | ported | `perf.spec.ts` 51,550 rows: scroll, click far down, type |

## Editing and keys (typing.rs)

| egui test | Status | Web test |
|---|---|---|
| typing_an_outline_with_the_keyboard | ported | `editing.spec.ts` |
| shift_enter_and_bold_shortcut | ported (bold), works (Shift+Enter) | `editing.spec.ts` bold |
| clicking_text_puts_the_caret_there | ported | `editing.spec.ts` |
| long_lines_wrap_and_typing_never_adds_line_breaks | ported | `editing.spec.ts` |
| diffs_n3 Escape then Down/Up | ported | `editing.spec.ts` |
| diffs_n4 Cmd+. puts the caret in the title | ported | `editing.spec.ts` |
| diffs_n27 typing on a selected line | ported | `editing.spec.ts` |
| diffs_n34 typing after a cleared selection | ported | `editing.spec.ts` |
| keys_and_menus_hand_the_keyboard_back | ported (Cmd+K, Escape) | `editing.spec.ts` |
| diffs_p2 command line steps for the focused node | ported | `editing.spec.ts` |
| diffs_p2 Delete moves to the next line | works | |
| inline_reference_left_right_and_select_all | works (Left/Right, Cmd+A twice) | `editing.spec.ts` left/right |
| shift_down_selects_and_tab_indents_the_selection | works | |
| diffs_p1 Escape selects, Backspace deletes | works | `menus.spec.ts` (N20 path) |
| bold_with_a_collapsed_caret / diffs_p1 Cmd+B / diffs_n19 | engine | the engine's tests |

## Pickers and fields

| egui test | Status | Web test |
|---|---|---|
| `#` menu, Cmd+Enter creates (diffs_p2, diffs_p3) | ported (Enter), works (Cmd+Enter) | `pickers.spec.ts` |
| diffs_p1 `>` on an empty line makes a field row | ported | `pickers.spec.ts` |
| field_name_tab_goes_to_the_value_and_enter_starts_a_node | ported | `pickers.spec.ts` |
| enter_in_a_field_value_adds_another / diffs_n1 | ported | `pickers.spec.ts` |
| diffs_n15 field values saved as typed | ported | `pickers.spec.ts` |
| diffs_n16_n17 Shift+Tab out of a value | ported | `pickers.spec.ts` |
| diffs_n2 `>` field row stays where it was typed | works | |
| `@` reference and inline reference | ported | `pickers.spec.ts` |
| `!` heading | ported | `pickers.spec.ts` |
| diffs_n28, n31, n33 selected field rows | ported | `pickers.spec.ts` |
| options_from_supertag picker, invalid value warning | works (picker), warning shown | |
| date_picker_takes_a_day_and_a_time / add end | ported (our own picker: day, start and end time, Add end) | `pickers.spec.ts` |
| hidden_fields_fold_to_a_label | works (engine decides, UI shows "+ Field") | |

## Shell, menus, pages

| egui test | Status | Web test |
|---|---|---|
| node_menu_has_filter_sort_group_display_and_copy | ported (menu, Add supertag) | `menus.spec.ts` |
| diffs_n11 selection toolbar | ported | `menus.spec.ts` |
| descriptions_are_edited_in_place | ported | `menus.spec.ts` |
| drag_a_bullet_onto_another_row / dragging a selection | ported | `menus.spec.ts` |
| copying selected nodes, diffs_n12 paste as references | ported | `menus.spec.ts` |
| diffs_n20 deleting a node used elsewhere asks first | ported | `menus.spec.ts` |
| tabs_and_panels_keep_their_own_pages | works | |
| search_and_open | works | |
| diffs_p2 supertags overview | works (shot) | `shots.spec.ts` |
| diffs_p3 tag page breadcrumb reads Schema | works (shot) | |
| settings pages, recents, help | works (shot) | |
| every_menu_bar_entry_runs_a_known_command | ported (Rust test in the desktop app) | `apps/bana-desktop` `menu_bar_follows_the_spec...` |
| diffs_n6_n14 workspaces side by side, workspace rename | ported (create, folded rows), works (rename) | `menus.spec.ts` |
| content width, panel split, mini sidebar flyouts | ported (flyouts), works (width, split) | `shell.spec.ts` |
| ctrl_shift_page_keys_move_the_tab, custom keybindings | ported (keys and drag; bindings saved with the graph, set in Settings) | `shell.spec.ts` |
| configure supertag panel, field definition page | ported (Configure opens a second panel; node menu too) | `schema.spec.ts` |
| reference counter, warn on editing a referenced node, url embeds | ported | `references.spec.ts` |
| context notes and fields on references | ported (heading, fields under the reference) | `references.spec.ts` |

## Views

| egui test | Status | Web test |
|---|---|---|
| every_view_renders_on_search_tag_and_plain_pages | ported (table, board, calendar) | `views.spec.ts`, `shots.spec.ts` |
| table cells typed in place | ported | `views.spec.ts` |
| board drag between columns | ported | `views.spec.ts` |
| query_builder (nested groups, Run once, Done, Cmd+Shift+O) | ported | `views.spec.ts` |
| table_header_renames_the_field_in_place, header menu | ported (click the name; right-click for type, calculate, sort, move, hide, delete) | `views.spec.ts` |
| list_view_opens_the_selected_item_in_the_next_panel | works (opens a new panel) | |
| calendar week and day grid, drag to create and resize | ported (month and week drag, drag out a new event, stretch its end) | `views.spec.ts` |
| lanes drag, week page schedule, new results flash | todo | |
