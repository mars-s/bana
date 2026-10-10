# Tana use cases, checked against TanaCraft

Tana's public feature set turned into user stories, each tried in the running TanaCraft web UI
(`tanacraft-cli serve --sample`) and written down as a Playwright scenario in
`web/tests/use-cases/`. Checked on 10 Oct 2026.

## Sources

Everything here comes from Tana's public pages, written in our own words:

- The outliner's docs: every page under https://outliner.tana.inc/learn/features/, the guides and
  tutorials under https://outliner.tana.inc/learn/, the help center
  (https://outliner.tana.inc/help/...), the product pages (daily notes, supertags, views, search
  nodes, outline editor, knowledge graph, PKM, personal, students, solo entrepreneurs, desktop,
  mobile, publish, voice memos).
- The changelog, https://outliner.tana.inc/changelog, every release from Oct 2025 (wk 41-43) to
  9 Oct 2026, and the monthly "Tana Current" blog posts.
- The new product at https://tana.inc (spaces, types, docs, meetings, agents, integrations), in
  `11-new-tana.md`.

## Status words

- **WORKS**: tried in the app; its scenario passes.
- **PARTIAL**: the core works and its scenario passes; what is missing is written next to it.
- **MISSING**: not there; listed with `test.fixme`.
- **OUT OF SCOPE candidate**: needs AI, voice, a cloud service, other people, or a phone. AI items
  are kept apart (tag `@ai`) because the AI decision is open.

## Areas

| File | Area | Works | Partial | Missing | Out of scope | AI |
|---|---|---|---|---|---|---|
| [01-daily-journal.md](01-daily-journal.md) | Day pages, weeks, journaling, dates | 13 | 2 | 1 | 0 | 0 |
| [02-tasks-projects.md](02-tasks-projects.md) | Checkboxes, tasks, projects, boards | 19 | 1 | 0 | 0 | 0 |
| [03-meetings.md](03-meetings.md) | Meeting notes, attendees, action items | 8 | 0 | 1 | 2 | 2 |
| [04-crm.md](04-crm.md) | People, companies, pipeline | 9 | 0 | 0 | 2 | 0 |
| [05-research-reading.md](05-research-reading.md) | Books, quotes, links, concepts | 7 | 1 | 1 | 0 | 1 |
| [06-knowledge-base.md](06-knowledge-base.md) | Outline editing, references, supertags, fields, search, views, navigation | 42 | 2 | 1 | 0 | 0 |
| [07-teams-sharing.md](07-teams-sharing.md) | Workspaces, collaboration, publishing | 4 | 1 | 4 | 2 | 0 |
| [08-capture-import.md](08-capture-import.md) | Capture, import, export, data safety | 14 | 1 | 0 | 1 | 1 |
| [09-ai.md](09-ai.md) | AI chat, commands, autofill, voice, meetings | 0 | 0 | 0 | 0 | 17 |
| [10-integrations-api.md](10-integrations-api.md) | MCP, local API, CLI, integrations | 4 | 2 | 3 | 2 | 0 |
| [11-new-tana.md](11-new-tana.md) | The new agent-first Tana | 0 | 0 | 2 | 5 | 3 |
| **Total (181)** | | **120** | **10** | **13** | **14** | **24** |

Out of the 143 use cases that need neither AI nor a cloud or mobile service, 130 work fully or in
part (120 + 10) and 13 are missing.

## Top 15 gaps by user value

Ranked by how often a Tana user would hit them in a normal week, and how much they unlock.

1. **Sync between computers** (UC-07.06). A notes app people trust must follow them from laptop
   to desktop. The Loro store already merges copies; the network part (iroh) is next.
2. ~~**PARENT in template and day searches** (UC-01.15, UC-03.05, UC-03.08).~~ Done: PARENT is
   the page related content shows on, and a day stands for its date.
3. ~~**Dates typed in words** (UC-01.08b).~~ Done: `@` reads dates and ranges in words.
4. ~~**Moving in from Tana and others** (UC-08.11, UC-08.10).~~ Done: Tana Paste, Notion and
   Logseq folders, Roam JSON.
5. ~~**Files on nodes** (UC-05.08).~~ Done: dropped or pasted files, kept beside the graph, with
   picture previews.
6. ~~**MCP on the running app** (UC-10.11, UC-10.01).~~ Done: MCP works on the running app
   (stdio and streamable HTTP), plus a remote connector for claude.ai and ChatGPT (docs/mcp.md).
7. ~~**Tag a whole selection** (UC-02.15).~~ Done: # Tag and Add tag tag every selected line.
8. ~~**Automation** (UC-02.20, UC-06.33).~~ Done: commands on events, command nodes by name in the
   command line, command buttons on supertags and fields; a node filter remains.
9. **Publish a page** (UC-07.08). A read-only web page or article; a local static export would
   cover most of it.
10. ~~**Rolling dates in searches** (UC-06.43).~~ Done: this week, NEXT 14 DAYS, LAST 7 DAYS and more.
11. ~~**Add to day** (UC-01.12).~~ Done: node menu Add to day, `node.addToDay`.
12. ~~**Unlinked mentions** (UC-05.06).~~ Done.
13. ~~**Merge duplicates** (UC-06.34).~~ Done: nodes, supertags and fields (`node.merge`).
14. ~~**Progress at a glance** (UC-02.03, UC-02.04).~~ Done: bars by themselves, rings on pinned
    projects.
15. ~~**Capture from anywhere** (UC-08.14).~~ Done: Cmd+Shift+E in any app opens a small Quick add
    window of its own; Escape or a click elsewhere puts it away and gives the keyboard back.

Also done since: Cmd+Shift+M opens Move to (UC-02.19), merging duplicate contacts (UC-04.11),
strikethrough (UC-06.45), counting words (UC-06.36), regular expressions in searches (UC-06.41),
searching inside one node from the search box (UC-06.42), node links that open from other apps
(UC-07.10), spreadsheet cells pasted as nodes with fields (UC-08.12), aliases on inline references
(UC-06.37), reading one in place with Shift+click (UC-05.09) the whole workspace as a ZIP of
Markdown files (UC-08.13), locked nodes (UC-06.35), emoji after `:` (UC-06.38), a templates
gallery (UC-08.15), journal prompts (UC-01.09) and Part of searches (UC-06.40).

Smaller defects found on the way, all fixed since: the Content width button sets Medium or Full
width per page (UC-06.44); side menu and tabs views show the item in place (UC-06.14); table and
list group headers fold (UC-06.31); the date picker
follows the Sunday week start (UC-01.13), Move to reaches other workspaces (UC-07.03), a number
outside its range is kept with a warning (UC-06.25), and `serve --sample --file` writes the sample
at once.

## AI-dependent features (decision open)

Listed apart so they can be taken or dropped as one decision. Details in [09-ai.md](09-ai.md).

- Chat with AI about your notes, with web search (UC-09.01)
- Ask AI and AI command nodes with prompts (UC-09.02)
- Autofill of titles, fields and descriptions; AI-enhanced fields (UC-09.03, UC-09.04)
- Suggested fields for new supertags; autotag (UC-09.05, UC-09.06)
- AI chat agents (UC-09.07); new-Tana agents with skills and proposals (UC-11.06)
- Meeting notetaker without a bot, meeting agent bot (UC-09.08, UC-09.09, UC-03.10, UC-03.11)
- Live transcription, voice memos, voice chat (UC-09.10, UC-09.11, UC-09.12, UC-08.17)
- Lecture transcription and summaries (UC-05.10)
- Rewrite transcripts into summaries, articles, memos (UC-09.13)
- Image and banner generation (UC-09.14)
- Model choice, own key, credits (UC-09.15); help that answers from the docs (UC-09.16)
- Extraction of tasks, people and dates from text (UC-09.17); typed meeting outcomes (UC-11.09)
- Artifacts such as slides made from a doc (UC-11.07)

What already helps if AI comes: every action is a command with an id and the MCP server exposes
them, so an outside assistant can read and write the graph today (UC-10.01, UC-10.02).

## Running the scenarios

```sh
pnpm --dir web build
cargo build -p tanacraft-cli
cd web && pnpm exec playwright test tests/use-cases --project=tests
pnpm exec playwright test tests/use-cases --project=tests --grep @partial   # one status
pnpm exec playwright test tests/use-cases --project=tests --grep @UC-02.09  # one use case
pnpm exec playwright test tests/use-cases --project=tests --list --grep @missing
```

Each scenario starts its own engine on the sample graph (fixed today: Sat, 10 Oct 2026). Every
`@works` and `@partial` scenario must pass; `@missing`, `@out-of-scope` and `@ai` ones are
`test.fixme` and listed as skipped. They also run in `cargo xtask web`.
