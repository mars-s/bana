# 07 Workspaces, teams and sharing

Tana separates private and shared workspaces, lets members work in one graph, and publishes
pages to the web. TanaCraft is local and single-user so far. Scenarios:
`web/tests/use-cases/07-teams-sharing.spec.ts`.

### UC-07.01 A separate workspace: WORKS
- **Story:** I keep my allotment club apart from my private notes.
- **Steps:** Manage workspaces, type a name, Enter; open it in the sidebar.
- **Expected:** A second workspace with its own Library, Schema and Trash.
- **Source:** https://outliner.tana.inc/learn/features/workspaces
- **TanaCraft:** WORKS.

### UC-07.02 Rename a workspace: WORKS
- **Source:** https://outliner.tana.inc/learn/features/workspaces
- **TanaCraft:** WORKS.

### UC-07.03 Move a draft into another workspace: PARTIAL
- **Story:** I draft privately, then move the note into the shared workspace.
- **Steps:** Select the line, Move to, pick the other workspace.
- **Expected:** The node and its children now belong to that workspace.
- **Source:** https://outliner.tana.inc/learn/features/workspaces (Moving nodes between workspaces)
- **TanaCraft:** PARTIAL. The engine moves it (CLI, MCP, API), but the Move to menu finds nothing in other workspaces.

### UC-07.04 Two people (or windows) in one note: PARTIAL
- **Story:** Two of us type in the same meeting note at once.
- **Steps:** Open the same page in two windows and type.
- **Expected:** Both edits kept, live.
- **Source:** https://outliner.tana.inc/learn/features/workspaces (Working collaboratively)
- **TanaCraft:** PARTIAL. Two windows on one engine keep both words in one line. No sync between devices or people.

### UC-07.05 Invite members: OUT OF SCOPE candidate (cloud)
- **Source:** https://outliner.tana.inc/learn/features/workspaces (Workspace membership)
- **TanaCraft:** not built.

### UC-07.06 Sync between my computers: MISSING
- **Story:** My laptop and desktop share one graph.
- **Source:** https://outliner.tana.inc/learn/features/tana-for-desktop
- **TanaCraft:** MISSING; the Loro store already merges copies edited apart, network sync (iroh) is the next milestone.

### UC-07.07 Notify a member: MISSING
- **Source:** https://outliner.tana.inc/learn/features/sidebar (Notifications)
- **TanaCraft:** MISSING.

### UC-07.08 Publish a page to the web: MISSING
- **Story:** I share a project plan or an article with people who do not use the app.
- **Steps:** Publish preview, Publish; optionally a password.
- **Expected:** A read-only web page (outline view or article), updated on republish.
- **Source:** https://outliner.tana.inc/learn/features/tana-publish
- **TanaCraft:** MISSING. A local "export as a static site" would cover much of it without a server.

### UC-07.09 Which workspaces a workspace may draw from: MISSING
- **Source:** https://outliner.tana.inc/learn/features/workspaces (Allow content from)
- **TanaCraft:** MISSING.

### UC-07.10 A link that opens a node from another app: MISSING
- **Story:** I paste a link to a node in my calendar invite and it opens the node.
- **Source:** https://outliner.tana.inc/learn/features/copy-paste-and-export (Copy node link)
- **TanaCraft:** MISSING. Copy node link copies an internal `[[id:N]]` token; no deep link.

### UC-07.11 Save a view for everyone: OUT OF SCOPE candidate (needs members)
- **Source:** https://outliner.tana.inc/learn/features/views (Save for everyone)
- **TanaCraft:** not built.
