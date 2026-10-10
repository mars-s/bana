# 10 API, MCP and integrations

Scenarios: `web/tests/use-cases/10-integrations-api.spec.ts`.

### UC-10.01 An AI assistant writes through MCP: PARTIAL
- **Story:** Claude, Cursor or Codex adds tasks and reads pages in my graph.
- **Steps:** Point the client at `tanacraft-cli --file graph.tanacraft mcp`; call `add_node`, `execute`, `export_markdown`.
- **Expected:** The node appears in the app.
- **Source:** https://outliner.tana.inc/learn/features/local-api-mcp
- **TanaCraft:** PARTIAL. MCP works on the graph file, so the app must be closed while it writes; the node shows when the app opens again. `mcp --connect` has no app port to connect to (UC-10.11).

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
- **TanaCraft:** PARTIAL. The API listens on 127.0.0.1 only; there are no tokens (UC-10.09).

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

### UC-10.11 MCP on the running app: MISSING
- **Story:** My assistant adds a task and I watch it appear in the open app.
- **Source:** https://outliner.tana.inc/learn/features/local-api-mcp (the desktop app hosts the local server)
- **TanaCraft:** MISSING. The desktop app and `serve` have no MCP endpoint; the CLI's `mcp --connect` targets a port nothing opens.
