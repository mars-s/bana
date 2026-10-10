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
| UC-06.05 | Emphasis in a note. | Cmd+B, Cmd+I, Cmd+U, code. | Formatted text. | PARTIAL: no strikethrough (UC-06.45) | nodes-and-references |
| UC-06.06 | Headings and a short description under a line. | `!` on an empty line; Ctrl+I. | Heading style; grey description. | WORKS | nodes-and-references |
| UC-06.07 | One note in many places, edited once. | Select, Cmd+C; Cmd+V on an empty line. | A reference; edits show everywhere. | WORKS | copy-paste-and-export |
| UC-06.08 | See how widely a note is used. | Settings, Reference counter. | A count beside referenced lines. | WORKS | nodes-and-references |
| UC-06.09 | Notes about a reference that belong to this place only. | Cmd+Alt+Down on a reference. | Context notes under it; the original is untouched. | WORKS | nodes-and-references (Contextual content) |
| UC-06.10 | Copy a checklist to reuse it. | Cmd+Shift+D. | An independent copy with children. | WORKS | copy-paste-and-export |
| UC-06.11 | Undo a deletion days later. | Cmd+Shift+Backspace; Trash; Restore from the menu. | The line is back where it was. | WORKS | workspaces (Trash) |
| UC-06.12 | Who changed this and when. | Show edit log. | Created and edited entries. | WORKS (one local user) | nodes-and-references |
| UC-06.13 | Pages with an emoji and a banner. | Hover the title, Icon, Banner. | Icon by the title; banner on top. | PARTIAL: banners are colours or one shipped picture, no own image | nodes-and-references (Banners and Icons) |
| UC-06.14 | A wiki with a menu of sections. | View as Side menu, Tabs or List; click an item. | The item's content beside the menu. | PARTIAL: every navigation view opens the item in a second panel; side menu and tabs do not show it in place | views (Navigation views) |
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
| UC-06.25 | A number field stays in its range. | Field min and max. | A warning on a value outside the range. | PARTIAL: the value is refused instead of kept with a warning | fields (Number) |
| UC-06.26 | Template fields only when filled. | Hide field: when empty. | Hidden until it has a value. | WORKS | fields (Hide field) |
| UC-06.27 | A field with several values. | Add values to one field. | All values shown. | WORKS | fields |
| UC-06.28 | See every type I have defined. | Sidebar, Supertags; click one. | The tag page with its nodes. | WORKS | supertags (The supertag page) |

## Search and views

| ID | Story | Steps | Expected | Status | Source |
|---|---|---|---|---|---|
| UC-06.29 | A precise query with OR and NOT. | Query builder with nested groups. | Only matching nodes. | WORKS | search-nodes (Query builder) |
| UC-06.30 | A table of notes with my own columns. | Table view, add a column, type a cell. | The field is set on the row's node. | WORKS | views (Table view) |
| UC-06.31 | Groups I can fold. | Group by a field; fold a group. | A folded group hides its rows. | PARTIAL: board columns hide, outline and list group headers do not fold | views (Group); changelog 2026 wk 37-39 |
| UC-06.32 | Light or dark. | Switch theme. | The theme changes. | WORKS | account-settings |
| UC-06.43 | Rolling periods in searches. | `Due:"this week"`, `Due:"NEXT 14 DAYS"`, `LAST 7 DAYS`, next or last week or month, this year. | Results follow today. | WORKS (no FOR RELATIVE DATE; on day pages PARENT is the day, UC-01.15) | dates-and-calendar-nodes; changelog 2026 wk 37-39 |
| UC-06.33 | One action runs my usual steps. | A command node with steps; run it. | All steps apply to the node. | PARTIAL: runs through the engine, CLI and MCP; not listed by name in the web command line; no buttons on tags or fields, no node filter | command-nodes |

## Gaps (MISSING)

| ID | Story | Source |
|---|---|---|
| UC-06.34 | Merge duplicate nodes, supertags or fields. | nodes-and-references (Merging duplicate nodes) |
| UC-06.35 | Lock a node so it cannot be edited. | nodes-and-references (Lock nodes) |
| UC-06.36 | Count words, characters and nodes. | outline-editor; changelog 2026 wk 37-39 |
| UC-06.37 | Give an inline reference an alias, or turn it back into text. | nodes-and-references (Alias, Remove an inline reference) |
| UC-06.38 | Type `:` and a word to pick an emoji. | nodes-and-references (Add emojis) |
| UC-06.39 | Code blocks with a language; Mermaid diagrams. | copy-paste-and-export; changelog 2026 wk 21-22 |
| UC-06.40 | A Part of tree searched with COMPONENTS REC. | fields (Part of) |
| UC-06.41 | Regular expressions in searches. | search-nodes (regular expressions) |
| UC-06.42 | Search inside one node from the search box (Tab). | sidebar (Global Search) |
| UC-06.44 | Content width per page (Medium, Full). The panel button has no action. | views (Content widths) |
| UC-06.45 | Strikethrough. | nodes-and-references (Formatting text) |
