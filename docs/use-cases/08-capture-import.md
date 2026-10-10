# 08 Capture, import, export and data safety

Getting things in fast, bringing old notes over, getting data out, and never losing it.
Scenarios: `web/tests/use-cases/08-capture-import.spec.ts`.

### UC-08.01 Capture to an Inbox: WORKS
- **Story:** From a script or another tool I drop a thought into an Inbox to sort later.
- **Steps:** `tanacraft-cli capture TEXT` (or the `node.capture` command); open the capture inbox.
- **Expected:** The item waits in the Inbox.
- **Source:** https://outliner.tana.inc/learn/features/sidebar (Inbox)
- **TanaCraft:** WORKS. The Inbox is reached from the command line, not a sidebar row.

### UC-08.02 Paste lines as nodes: WORKS
- **Story:** I paste a list from an email and get one node per line.
- **Source:** https://outliner.tana.inc/learn/features/copy-paste-and-export
- **TanaCraft:** WORKS.

### UC-08.03 Paste a nested Markdown list: WORKS
- **Story:** An indented list keeps its nesting.
- **Source:** https://outliner.tana.inc/changelog (2026 wk 33-36: pasted outlines expand every branch)
- **TanaCraft:** WORKS.

### UC-08.04 Import from another outliner: WORKS
- **Story:** I bring my Workflowy (OPML) or Markdown notes over.
- **Steps:** Account menu, Import content, pick a .opml or .md file.
- **Expected:** The outline appears, with `#tags`, checkboxes and `Field:: value` lines kept.
- **Source:** https://outliner.tana.inc/learn/features/import-data-into-tana (Workflowy/OPML)
- **TanaCraft:** WORKS. Imports go into the Library rather than a new workspace.

### UC-08.05 Export a page: WORKS
- **Story:** I take a project out as Markdown, OPML or JSON with its fields.
- **Steps:** Export commands (command line, CLI `export`).
- **Expected:** The text holds children and field values.
- **Source:** https://outliner.tana.inc/learn/features/copy-paste-and-export
- **TanaCraft:** WORKS.

### UC-08.06 Offline and crash-safe: WORKS
- **Story:** My notes live on my computer and survive a crash.
- **Steps:** Type, kill the app, start it again.
- **Expected:** Everything typed is there.
- **Source:** https://outliner.tana.inc/learn/features/tana-for-desktop (Work offline)
- **TanaCraft:** WORKS (local-first; autosave within a second plus a write-ahead log).

### UC-08.07 Notes from the terminal: WORKS
- **Story:** A script adds tasks to today and searches my notes.
- **Steps:** `tanacraft-cli add NAME --today --tag task`, `search`, `export today`.
- **Source:** https://outliner.tana.inc/learn/features/input-api (the closest Tana feature)
- **TanaCraft:** WORKS.

### UC-08.08 Paste code: PARTIAL
- **Story:** I paste a snippet and want a code block with syntax colours.
- **Source:** https://outliner.tana.inc/learn/features/copy-paste-and-export (Paste as code)
- **TanaCraft:** PARTIAL. Paste as code keeps it as inline code; no block, language or highlighting.

### UC-08.09 Imports land in the Library: WORKS
- **Source:** https://outliner.tana.inc/learn/features/workspaces (Library)
- **TanaCraft:** WORKS.

### UC-08.10 Import Notion, Roam or Logseq: WORKS
- **Source:** https://outliner.tana.inc/learn/features/import-data-into-tana, https://outliner.tana.inc/learn/guides/import-from-notion
- **TanaCraft:** WORKS. A Notion or Logseq export folder (`tanacraft-cli import FOLDER`, `import.folder`) becomes pages named after the files (Notion ids and Logseq namespaces cleaned up), folders as parent pages; Roam's JSON (`import.roam`, or Import content with a .json file) becomes pages with their blocks, TODO and DONE as checkboxes. `[[links]]` become links. Notion databases (CSV) are not read.

### UC-08.11 Tana Paste: WORKS
- **Story:** A script or an AI writes plain text that becomes tagged nodes, fields, dates and searches.
- **Source:** https://outliner.tana.inc/learn/features/tana-paste
- **TanaCraft:** WORKS. Pasting text that starts with `%%tana%%` (or `import.tanaPaste`, or importing such a file) makes the nodes: `#tag`, `#[[two words]]`, `Field:: value`, `[[Field]]::` with value bullets, `[[Name]]` links, `[[date:...]]` (single, range), `[ ]` and `[x]`, `!!` headings, `%%search%%` with the query below and `%%view:...%%`. Not read: Tana node ids, images, tables.

### UC-08.12 Paste spreadsheet cells as a table: WORKS
- **Story:** I copy rows from a spreadsheet and get one node per row, the columns as fields.
- **Source:** https://outliner.tana.inc/learn/features/copy-paste-and-export (Pasting from Google Sheets)
- **TanaCraft:** WORKS. Tab-separated lines with a header row: the first column names the nodes, the others become fields (new ones are made); View as Table shows them as columns. Values are plain text.

### UC-08.13 Whole-workspace Markdown ZIP: WORKS
- **Story:** Export everything as folders of Markdown (outline, by supertag, or a vault for Obsidian).
- **Steps:** Account menu, Export everything as Markdown: By page, By supertag, or As an Obsidian vault (or `tanacraft-cli export --zip OUT --shape vault`).
- **Source:** https://outliner.tana.inc/changelog (2026 wk 33-36: three shapes for Markdown export)
- **TanaCraft:** WORKS (`export.zip`). By page: `Library/` and `Daily notes/`; by supertag: a `#tag/` folder per supertag; vault: by page with tags and fields as front matter. Inline references are `[[Name]]` links. The ZIP is stored, not compressed. The browser download is tested; the desktop window's download was not tried.

### UC-08.14 Capture from any app: WORKS
- **Story:** In another app I press a key, type a thought, and go back to what I was doing.
- **Source:** https://outliner.tana.inc/learn/features/tana-for-desktop (Global Clipper)
- **TanaCraft:** WORKS. In the desktop app Cmd+Shift+E (Ctrl+Shift+E elsewhere) opens a small Quick add window of its own over any app, without the main window. Each line goes to today's page as it is typed; Enter starts the next line, Escape or a click elsewhere puts the window away and the keyboard goes back to the app that had it. Not built: changing the key. The page is tested in the web suite (`#capture`); the window and its permission by Rust tests; the shortcut was tried by hand.

### UC-08.15 Templates gallery: MISSING
- **Story:** I install a ready-made OKR or habit tracker setup.
- **Source:** https://outliner.tana.inc/learn/features/tana-templates
- **TanaCraft:** MISSING ("Browse templates" shows a note that there is no gallery).

### UC-08.16 Capture on the phone: OUT OF SCOPE candidate (mobile)
- **Source:** https://outliner.tana.inc/learn/features/tana-mobile
- **TanaCraft:** not built.

### UC-08.17 Voice memos turned into nodes: OUT OF SCOPE candidate (AI)
- **Source:** https://outliner.tana.inc/learn/features/mobile-voice-memos
- **TanaCraft:** not built.
