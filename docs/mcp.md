# MCP: assistants working on your notes

TanaCraft speaks the Model Context Protocol, so an assistant (Claude, ChatGPT, Cursor, Codex) can
read and change your notes with the same commands the keyboard runs. There are three ways in.

| Where the assistant runs | How it connects | Setup |
|---|---|---|
| On this Mac (Claude Code, Claude Desktop, Cursor) | `tanacraft-mcp` on stdio | one line of config |
| A script or tool on this Mac | `POST /mcp` on 127.0.0.1 (streamable HTTP) with the local token | none |
| In the cloud (claude.ai, ChatGPT) | the remote connector: a Cloudflare Worker and a Cloudflare Tunnel | `cloudflare/wizard.sh` |

In every case the assistant works on the **running app**: what it adds shows in your open window at
once, in its own window of the engine, so your page and caret stay where they are (DIFFS N60).

## The tools

`list_commands`, `execute` (any command by id, see `tanacraft-cli commands`), `view`, `inspect`,
`search`, `add_node`, `export_markdown`; resources `tanacraft://commands` and `tanacraft://page`.
Tana Paste (`execute import.tanaPaste`) is the quickest way for an assistant to write structure.

## On this Mac: stdio

```json
{ "mcpServers": { "tanacraft": { "command": "/path/to/tanacraft-mcp" } } }
```

`tanacraft-mcp` (or `tanacraft-cli mcp`) looks for the running app that has the graph open: the
desktop app and `tanacraft-cli serve` write `graph.tanacraft.live` beside the graph (port and pid)
and the client authenticates with the install's local token. When no app is running it opens the
file itself. `--file FILE` picks another graph; `--connect 127.0.0.1:PORT --token T` names an app
directly.

## The local token

One per install, in the data folder: `~/TanaCraft/token` (or `$TANACRAFT_HOME/token`), readable by
you only, made the first time the app starts. Requests to `POST /rpc` and `POST /mcp` that do not
come from the app's own pages must send `Authorization: Bearer <token>`. The API listens on
127.0.0.1 only. To change the token, delete the file and restart the app (then set it again in the
remote connector: `wrangler secret put MAC_TOKEN`).

## Streamable HTTP on the Mac

The app serves MCP at `http://127.0.0.1:8787/mcp` (the desktop app takes 8787 when it is free, else
any free port; `--serve PORT` fixes it). It follows the streamable HTTP transport (2025-06-18) in
its simplest form: `POST` a JSON-RPC message or batch, get JSON back (202 for notifications only);
no server-sent stream (`GET` is 405) and no sessions. Pages on other origins are refused. The
token, not the Host header, is the authority here, so the endpoint also answers through a tunnel.

```sh
curl -s http://127.0.0.1:8787/mcp -H "Authorization: Bearer $(cat ~/TanaCraft/token)" \
  -H 'Content-Type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'
```

## The remote connector (claude.ai, ChatGPT)

```
claude.ai / ChatGPT ──OAuth 2.1──▶ Cloudflare Worker ──Cloudflare Tunnel──▶ TanaCraft on your Mac
                                    (cloudflare/worker.ts)                   POST /mcp + local token
```

- The Worker (`cloudflare/worker.ts`) is an OAuth 2.1 provider for MCP clients (Cloudflare's
  `workers-oauth-provider`): dynamic client registration, PKCE, refresh tokens (1 hour access, 30
  days refresh). Signing in shows a consent page, then GitHub; only the one GitHub account named in
  `ALLOWED_GITHUB_ID` gets a grant, and GitHub is asked for no scopes (a GitHub token with scopes
  is refused). The GitHub token is not kept.
- Authorized MCP requests are passed to `MAC_MCP_URL` (the tunnel's address of `/mcp`) with the
  local token added by the Worker; the assistant never sees it. Optional `CF_ACCESS_CLIENT_ID` and
  `CF_ACCESS_CLIENT_SECRET` add a Cloudflare Access service token when the tunnel's hostname is
  behind Access too.
- The tunnel config the wizard writes passes only `/mcp` to the app; everything else is 404.
- **The Mac must be awake and TanaCraft open.** Otherwise each request gets a JSON-RPC error
  "TanaCraft on the Mac is not reachable". No notes are stored in the cloud; keeping a cloud copy
  is a separate decision.
- The Worker stays closed (503) until every setting is in place.

### Setting it up

Nothing is created for you: the cloud accounts, the GitHub OAuth app, the tunnel and the deploy are
yours. The wizard walks you through each step and checks the result:

```sh
tanacraft/cloudflare/wizard.sh
```

It signs Wrangler in, creates the KV namespace, looks up your GitHub id, has you register the
GitHub OAuth app (callback `https://tanacraft-mcp.<subdomain>.workers.dev/callback`), creates and
routes the tunnel (`tanacraft.<your domain>`), deploys the Worker with its secrets, and shows where
to add the connector URL in claude.ai and ChatGPT.

### Tests

```sh
cargo build -p tanacraft-cli
cd cloudflare && npm install && npm test && npm run typecheck
```

`test.mjs` runs the Worker on local workerd (Miniflare) with a stand-in GitHub, and end to end: the
official MCP SDK client registers, signs in, and its tool calls pass through the Worker to a real
`tanacraft-cli serve` on this machine. It also checks the other GitHub account and scoped GitHub
grants are refused, consent escaping and origin checks, refresh and revocation, the "not
reachable" answer when the Mac is down, and that the tunnel endpoint refuses requests without the
local token. Rust tests cover `/mcp` (token, batch, 202, a tunnel's Host, other origins) and live
discovery.
