# 07 Workspaces, teams and sharing

Tana separates private and shared workspaces, lets members work in one graph, and publishes
pages to the web. Bana is local and single-user so far. Scenarios:
`web/tests/use-cases/07-teams-sharing.spec.ts`.

### UC-07.01 A separate workspace: WORKS
- **Story:** I keep my allotment club apart from my private notes.
- **Steps:** Manage workspaces, type a name, Enter; open it in the sidebar.
- **Expected:** A second workspace with its own Library, Schema and Trash.
- **Source:** https://outliner.tana.inc/learn/features/workspaces
- **Bana:** WORKS.

### UC-07.02 Rename a workspace: WORKS
- **Source:** https://outliner.tana.inc/learn/features/workspaces
- **Bana:** WORKS.

### UC-07.03 Move a draft into another workspace: WORKS
- **Story:** I draft privately, then move the note into the shared workspace.
- **Steps:** Select the line, Move to, pick the other workspace.
- **Expected:** The node and its children now belong to that workspace.
- **Source:** https://outliner.tana.inc/learn/features/workspaces (Moving nodes between workspaces)
- **Bana:** WORKS. Move to lists every workspace's Library by the workspace's name.

### UC-07.04 Two people (or windows) in one note: PARTIAL
- **Story:** Two of us type in the same meeting note at once.
- **Steps:** Open the same page in two windows and type.
- **Expected:** Both edits kept, live.
- **Source:** https://outliner.tana.inc/learn/features/workspaces (Working collaboratively)
- **Bana:** PARTIAL. Two windows on one engine keep both words in one line. No sync between devices or people.

### UC-07.05 Invite members: OUT OF SCOPE candidate (cloud)
- **Source:** https://outliner.tana.inc/learn/features/workspaces (Workspace membership)
- **Bana:** not built.

### UC-07.06 Sync between my computers: MISSING
- **Story:** My laptop and desktop share one graph.
- **Source:** https://outliner.tana.inc/learn/features/tana-for-desktop
- **Bana:** MISSING; the Loro store already merges copies edited apart, network sync (iroh) is the next milestone.

### UC-07.07 Notify a member: MISSING
- **Source:** https://outliner.tana.inc/learn/features/sidebar (Notifications)
- **Bana:** MISSING.

### UC-07.08 Publish a page to the web: MISSING
- **Story:** I share a project plan or an article with people who do not use the app.
- **Steps:** Publish preview, Publish; optionally a password.
- **Expected:** A read-only web page (outline view or article), updated on republish.
- **Source:** https://outliner.tana.inc/learn/features/tana-publish
- **Bana:** MISSING. A local "export as a static site" would cover much of it without a server.

### UC-07.09 Which workspaces a workspace may draw from: MISSING
- **Source:** https://outliner.tana.inc/learn/features/workspaces (Allow content from)
- **Bana:** MISSING.

### UC-07.10 A link that opens a node from another app: WORKS
- **Story:** I paste a link to a node in my calendar invite and it opens the node.
- **Source:** https://outliner.tana.inc/learn/features/copy-paste-and-export (Copy node link)
- **Bana:** WORKS. Copy node link (node menu, selection toolbar; `node.link`) copies `http://127.0.0.1:PORT/?node=N`, which opens the node in the engine's web UI from any app while Bana runs; pasted into a line it becomes an inline reference. spec-gap: the link opens a browser window, not the desktop window (that needs the bundled app's URL scheme), and it follows the port (8787 unless taken).

### UC-07.11 Save a view for everyone: OUT OF SCOPE candidate (needs members)
- **Source:** https://outliner.tana.inc/learn/features/views (Save for everyone)
- **Bana:** not built.
