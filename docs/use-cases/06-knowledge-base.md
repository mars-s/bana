# 06 Knowledge base: writing, structure, supertags, search and views

The core outliner, used as a personal knowledge base or wiki. Scenarios:
`web/tests/use-cases/06-knowledge-base.spec.ts`. Sources are pages under
`https://outliner.tana.inc/learn/features/` unless a full URL is given.

## Writing and structure

| ID | Story | Steps | Expected | Status | Source |
|---|---|---|---|---|---|
| UC-06.01 | I outline my thoughts fast with the keyboard. | Type, Enter, Tab, Shift+Tab. | Nested lines in order. | WORKS | outline-editor |
| UC-06.02 | I reorder ideas without the mouse. | Cmd+Shift+Up and Down. | The line and its children move. | WORKS | outline-editor |
| UC-06.03 | Any line can become the page I focus on. | Cmd+. to zoom in, Cmd+, back; breadcrumbs. | The line is the page title; crumbs lead back. | WORKS | navigation |
| UC-06.04 | Hide detail I do not need now. | Cmd+Down and Cmd+Up on a line. | Children show and hide. | WORKS | outline-editor |
| UC-06.05 | Emphasis in a note. | Cmd+B, Cmd+I, Cmd+U, code, Cmd+Shift+X. | Formatted text. | WORKS (strikethrough: UC-06.45) | nodes-and-references |
| UC-06.06 | Headings and a short description under a line. | `!` on an empty line; Ctrl+I. | Heading style; grey description. | WORKS | nodes-and-references |
| UC-06.07 | One note in many places, edited once. | Select, Cmd+C; Cmd+V on an empty line. | A reference; edits show everywhere. | WORKS | copy-paste-and-export |
| UC-06.08 | See how widely a note is used. | Settings, Reference counter. | A count beside referenced lines. | WORKS | nodes-and-references |
| UC-06.09 | Notes about a reference that belong to this place only. | Cmd+Alt+Down on a reference. | Context notes under it; the original is untouched. | WORKS | nodes-and-references (Contextual content) |
| UC-06.10 | Copy a checklist to reuse it. | Cmd+Shift+D. | An independent copy with children. | WORKS | copy-paste-and-export |
| UC-06.11 | Undo a deletion days later. | Cmd+Shift+Backspace; Trash; Restore from the menu. | The line is back where it was. | WORKS | workspaces (Trash) |
| UC-06.12 | Who changed this and when. | Show edit log. | Created and edited entries. | WORKS (one local user) | nodes-and-references |
| UC-06.13 | Pages with an emoji and a banner. | Hover the title, Icon, Banner. | Icon by the title; banner on top. | PARTIAL: banners are colours or one shipped picture, no own image | nodes-and-references (Banners and Icons) |
| UC-06.14 | A wiki with a menu of sections. | View as Side menu, Tabs or List; click an item. | The item's content beside the menu. | WORKS: side menu and tabs show the picked item's outline in place; a list opens it beside | views (Navigation views) |
| UC-06.15 | Find any note by its words. | Cmd+S, type, Enter. | The node opens. | WORKS | sidebar (Global Search) |
| UC-06.16 | Get back to what I was just working on. | Sidebar, Recents. | Recently opened pages. | WORKS | sidebar (Recents) |
| UC-06.17 | Keep key pages one click away. | Pin or unpin in sidebar. | The page under Pinned. | WORKS | sidebar (Pinned nodes) |
| UC-06.18 | Two notes side by side, or in tabs. | Cmd+M (Tana: Shift+click); New tab. | A second panel; a second tab. | WORKS | navigation (Tabs and Panels) |
| UC-06.19 | My own shortcut for a command. | Bind a key in Settings, Keybindings. | The key runs the command. | WORKS | keyboard-shortcuts |
| UC-06.20 | Run anything by name. | Cmd+K, type, Enter. | The command runs on the line. | WORKS | command-line |
| UC-06.21 | Take back a mistake. | Cmd+Z, Cmd+Shift+Z. | Undo and redo. | WORKS | keyboard-shortcuts |

## Supertags and fields

| ID | Story | Steps | Expected | Status | Source |
|---|---|---|---|---|---|
| UC-06.22 | Turn a list I made into a reusable type. | Convert to supertag. | A supertag; children become its template. | WORKS | supertags |
| UC-06.23 | A "design task" is a task with extra fields. | Extend `#task`. | The new tag has the task's fields and checkbox. | WORKS | supertags (Extend) |
| UC-06.24 | Rarely used fields out of the way. | Optional fields on the supertag. | Offered by a button, not shown as rows. | WORKS | supertags (Optional fields) |
| UC-06.25 | A number field stays in its range. | Field min and max. | A warning on a value outside the range. | WORKS: kept, with the warning mark on the field row | fields (Number) |
| UC-06.26 | Template fields only when filled. | Hide field: when empty. | Hidden until it has a value. | WORKS | fields (Hide field) |
| UC-06.27 | A field with several values. | Add values to one field. | All values shown. | WORKS | fields |
| UC-06.28 | See every type I have defined. | Sidebar, Supertags; click one. | The tag page with its nodes. | WORKS | supertags (The supertag page) |

## Search and views

| ID | Story | Steps | Expected | Status | Source |
|---|---|---|---|---|---|
| UC-06.29 | A precise query with OR and NOT. | Query builder with nested groups. | Only matching nodes. | WORKS | search-nodes (Query builder) |
| UC-06.30 | A table of notes with my own columns. | Table view, add a column, type a cell. | The field is set on the row's node. | WORKS | views (Table view) |
| UC-06.31 | Groups I can fold. | Group by a field; fold a group. | A folded group hides its rows. | WORKS: table and list group headers fold (`view.toggleGroupFolded`), board columns hide; an outline view does not group | views (Group); changelog 2026 wk 37-39 |
| UC-06.32 | Light or dark. | Switch theme. | The theme changes. | WORKS | account-settings |
| UC-06.34 | Merge duplicates. | Node menu Merge into…; Configure supertag or a field page, Merge into…. | Children, tags, values and every reference move to the kept one; the other goes to the trash. | WORKS | nodes-and-references (Merging duplicate nodes) |
| UC-06.44 | Content width per page. | The panel's width button: Medium or Full width. | The page uses the column or the whole panel; kept per page. | WORKS | views (Content widths) |
| UC-06.43 | Rolling periods in searches. | `Due:"this week"`, `Due:"NEXT 14 DAYS"`, `LAST 7 DAYS`, next or last week or month, this year. | Results follow today. | WORKS (no FOR RELATIVE DATE; on day pages PARENT is the day, UC-01.15) | dates-and-calendar-nodes; changelog 2026 wk 37-39 |
| UC-06.35 | Keep a finished note from accidental edits. | Node menu, Lock (Unlock to edit again). | A lock mark; typing, Enter, tags, fields, children, moves and deletes are refused with a note. | WORKS (`node.toggleLock`; folding still works; command nodes cannot get round it) | nodes-and-references (Lock nodes) |
| UC-06.36 | How long is this page? | Command line, Count words. | Words, characters and nodes of the page, or of the selected lines. | WORKS (`node.count`; names only, not field values) | outline-editor; changelog 2026 wk 37-39 |
| UC-06.37 | A reference that reads well in my sentence. | Right-click an inline reference: Alias, or Turn into text. | The reference shows my words (`[[id:N\|alias]]`) and still points at the node; or it becomes plain text. | WORKS (`node.refAlias`, `node.unlinkRef`; exports write `[[Name\|alias]]`) | nodes-and-references (Alias, Remove an inline reference) |
| UC-06.38 | A quick emoji in a line. | Type `:` and a word at the start of a word, Enter. | The emoji replaces `:word`. | WORKS (about 100 common emoji by English names; `10:30` and `Note:` open nothing) | nodes-and-references (Add emojis) |
| UC-06.41 | Find text by pattern. | A search with `/pattern/` or `/pattern/i`. | Nodes whose text matches. | WORKS (spaces allowed inside the slashes; a broken pattern is refused) | search-nodes (regular expressions) |
| UC-06.42 | Look for something inside one page. | Search and open, pick a result, Tab, type. | Only results under that node; Backspace searches everywhere again. | WORKS | sidebar (Global Search) |
| UC-06.45 | Cross something out. | Select, Cmd+Shift+X. | `~~text~~`, drawn struck through; exports as `<s>`. | WORKS (spec-gap: the key is ours) | nodes-and-references (Formatting text) |
| UC-06.33 | One action runs my usual steps. | A command node with steps; run it. | All steps apply to the node. | PARTIAL: listed by name in the command line ("Run …"), run on events (UC-02.20) and from buttons on a supertag's nodes or a field's rows (Configure supertag or the field page, Commands; `commandNode.button`); no node filter | command-nodes |

## Gaps (MISSING)

| ID | Story | Source |
|---|---|---|
| UC-06.39 | Code blocks with a language; Mermaid diagrams. | copy-paste-and-export; changelog 2026 wk 21-22 |
| UC-06.40 | A Part of tree searched with COMPONENTS REC. | fields (Part of) |
