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
| [01-daily-journal.md](01-daily-journal.md) | Day pages, weeks, journaling, dates | 9 | 3 | 4 | 0 | 0 |
| [02-tasks-projects.md](02-tasks-projects.md) | Checkboxes, tasks, projects, boards | 14 | 2 | 4 | 0 | 0 |
| [03-meetings.md](03-meetings.md) | Meeting notes, attendees, action items | 8 | 0 | 1 | 2 | 2 |
| [04-crm.md](04-crm.md) | People, companies, pipeline | 8 | 0 | 1 | 2 | 0 |
| [05-research-reading.md](05-research-reading.md) | Books, quotes, links, concepts | 4 | 1 | 4 | 0 | 1 |
| [06-knowledge-base.md](06-knowledge-base.md) | Outline editing, references, supertags, fields, search, views, navigation | 27 | 6 | 12 | 0 | 0 |
| [07-teams-sharing.md](07-teams-sharing.md) | Workspaces, collaboration, publishing | 2 | 2 | 5 | 2 | 0 |
| [08-capture-import.md](08-capture-import.md) | Capture, import, export, data safety | 8 | 1 | 6 | 1 | 1 |
| [09-ai.md](09-ai.md) | AI chat, commands, autofill, voice, meetings | 0 | 0 | 0 | 0 | 17 |
| [10-integrations-api.md](10-integrations-api.md) | MCP, local API, CLI, integrations | 4 | 1 | 3 | 3 | 0 |
| [11-new-tana.md](11-new-tana.md) | The new agent-first Tana | 0 | 0 | 2 | 5 | 3 |
| **Total (181)** | | **84** | **16** | **42** | **15** | **24** |

Out of the 142 use cases that need neither AI nor a cloud or mobile service, 100 work fully or in
part (84 + 16) and 42 are missing.

## Top 15 gaps by user value

Ranked by how often a Tana user would hit them in a normal week, and how much they unlock.

1. **Sync between computers** (UC-07.06). A notes app people trust must follow them from laptop
   to desktop. The Loro store already merges copies; the network part (iroh) is next.
2. ~~**PARENT in template and day searches** (UC-01.15, UC-03.05, UC-03.08).~~ Done: PARENT is
   the page related content shows on, and a day stands for its date. "Tasks due this day",
   "meetings of the day", "this person's meetings" all fail because PARENT is not bound to the
   day page or the tagged node. One engine fix unlocks Tana's most copied dashboards.
3. **Dates typed in words** (UC-01.08b). `@tomorrow`, `@next friday`, `@Nov 21 to Nov 30` in any
   line; today only a command inserts today's link.
4. **Moving in from Tana and others** (UC-08.11, UC-08.10). Tana Paste, and Notion, Roam and
   Logseq imports. Tana Paste matters most: it is how Tana users and AI tools write structure.
5. **Files on nodes** (UC-05.08). Images and PDFs dropped on a node, with previews.
6. ~~**MCP on the running app** (UC-10.11, UC-10.01).~~ Done: MCP finds the running app through
   the live file beside the graph and works on it live.
7. **Tag a whole selection** (UC-02.15). Only one of the selected lines gets the tag.
8. **Automation** (UC-02.20, UC-06.33). Commands on events (tag added, checked, child added) and
   command nodes as buttons and in the web command line.
9. **Publish a page** (UC-07.08). A read-only web page or article; a local static export would
   cover most of it.
10. **Rolling dates in searches** (UC-06.43). This week, NEXT 14 DAYS, FOR RELATIVE DATE.
11. **Add to day** (UC-01.12). Send a node to a future day as a reminder.
12. **Unlinked mentions** (UC-05.06). Find and link notes that name a page.
13. **Merge duplicates** (UC-06.34). Nodes, supertags and fields.
14. **Progress at a glance** (UC-02.03, UC-02.04). The bar should appear by itself under
    checkboxes, and pinned projects should show a ring in the sidebar.
15. **Capture from anywhere** (UC-08.14). A system-wide quick capture in the desktop app.

Smaller defects found on the way (worth a ticket each): the Content width button has no action
(UC-06.44); the date picker ignores the Sunday week start that calendar views follow (UC-01.13);
Move to cannot reach another workspace (UC-07.03); a number outside its range is refused rather
than kept with a warning (UC-06.25); side menu and tabs views open the item in a second panel
instead of in place (UC-06.14); outline and list group headers do not fold (UC-06.31); with
`serve --sample --file`, the sample reaches the file only after the first edit.

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
