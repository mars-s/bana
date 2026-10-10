# 10 API, MCP and integrations

Scenarios: `web/tests/use-cases/10-integrations-api.spec.ts`.

### UC-10.01 An AI assistant writes through MCP: WORKS
- **Story:** Claude, Cursor or Codex adds tasks and reads pages in my graph.
- **Steps:** Point the client at `tanacraft-cli --file graph.tanacraft mcp`; call `add_node`, `execute`, `export_markdown`.
- **Expected:** The node appears in the app.
- **Source:** https://outliner.tana.inc/learn/features/local-api-mcp
- **TanaCraft:** WORKS. MCP works on the running app (UC-10.11); the node shows in the open window at once.

### UC-10.02 An AI assistant reads a page: WORKS
- **Steps:** `export_markdown` on a node.
- **Expected:** The subtree as Markdown.
- **Source:** https://outliner.tana.inc/learn/features/local-api-mcp (Read operations)
- **TanaCraft:** WORKS.

### UC-10.03 A script posts to a local API: WORKS
- **Story:** A cron job adds a node with a field, and I see it appear while the app is open.
- **Steps:** `POST /rpc` with `{"method":"run","params":{"command":"node.add",...}}`.
- **Expected:** The node shows at once.
- **Source:** https://outliner.tana.inc/learn/features/input-api
- **TanaCraft:** WORKS for the browser UI served by `tanacraft-cli serve`.

### UC-10.04 Access control for the API: PARTIAL
- **Story:** Only tools I allow can write to my graph.
- **Source:** https://outliner.tana.inc/learn/features/input-api (API tokens)
- **TanaCraft:** PARTIAL. The API listens on 127.0.0.1 only; requests from outside the app's own pages must send the running app's token, written to the live file beside the graph (`graph.tanacraft.live`, this user only). No tokens per workspace to create and revoke (UC-10.09).

### UC-10.05 Hosted MCP: OUT OF SCOPE candidate (cloud)
- **Source:** https://outliner.tana.inc/changelog (2026 wk 23-30: Remote MCP)
- **TanaCraft:** not built.

### UC-10.06 Calendar sync: OUT OF SCOPE candidate (cloud)
- **Source:** https://outliner.tana.inc/learn/features/calendar-integration
- **TanaCraft:** not built.

### UC-10.07 Readwise: MISSING
- **Source:** https://outliner.tana.inc/learn/features/readwise-integration
- **TanaCraft:** MISSING.

### UC-10.08 Call a web API from a command: MISSING
- **Source:** https://outliner.tana.inc/learn/features/ai-command-nodes (Make API request)
- **TanaCraft:** MISSING.

### UC-10.09 API tokens: MISSING
- **Source:** https://outliner.tana.inc/learn/features/account-settings (API keys)
- **TanaCraft:** MISSING.

### UC-10.10 GitHub, Linear, Jira, Slack, HubSpot, Pipedrive: OUT OF SCOPE candidate (new Tana, cloud)
- **Source:** https://tana.inc/integrations
- **TanaCraft:** not built.

### UC-10.11 MCP on the running app: WORKS
- **Story:** My assistant adds a task and I watch it appear in the open app.
- **Source:** https://outliner.tana.inc/learn/features/local-api-mcp (the desktop app hosts the local server)
- **TanaCraft:** WORKS. The desktop app and `tanacraft-cli serve` serve their engine on 127.0.0.1 and write a live file beside the graph (port, token). `tanacraft-mcp` and `tanacraft-cli mcp` find it and drive the running engine in a window of their own (the person's page and caret stay put); with no app running they work on the file. `--connect HOST:PORT --token T` names one directly.
