// Use cases: API, MCP and integrations (docs/use-cases/10-integrations-api.md).
import { spawn } from 'node:child_process'
import { createInterface } from 'node:readline'
import { join, resolve } from 'node:path'
import { test, expect, missing, works, partial, tree } from './helpers'
import { pageWith, liveToken } from '../fixtures'

const bin = join(process.env.CARGO_TARGET_DIR ?? resolve(import.meta.dirname, '../../../target'), 'debug', 'tanacraft-cli')

/** An MCP client on stdio: `tanacraft-cli --file FILE mcp` on a graph file. */
function mcp(file: string) {
  const proc = spawn(bin, ['--file', file, 'mcp'], { stdio: ['pipe', 'pipe', 'inherit'] })
  const lines = createInterface({ input: proc.stdout! })
  const waiting = new Map<number, (v: any) => void>()
  lines.on('line', (l) => {
    const m = JSON.parse(l)
    waiting.get(m.id)?.(m)
  })
  let next = 1
  return {
    call(method: string, params: unknown = {}): Promise<any> {
      const id = next++
      proc.stdin!.write(JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n')
      return new Promise((r) => waiting.set(id, r))
    },
    close: () => proc.kill(),
  }
}

test('UC-10.01 an AI assistant (Claude, Cursor, Codex) adds a task through MCP; it shows in the open app', works('UC-10.01'), async ({ app, server }) => {
  // MCP finds the running engine through the live file beside the graph and works on it (UC-10.11).
  const c = mcp(server.file)
  const init = await c.call('initialize', { protocolVersion: '2025-03-26' })
  expect(init.result.serverInfo.name).toBe('tanacraft')
  const tools = (await c.call('tools/list')).result.tools.map((t: any) => t.name)
  expect(tools).toEqual(expect.arrayContaining(['list_commands', 'execute', 'add_node', 'export_markdown']))
  await c.call('tools/call', { name: 'execute', arguments: { command: 'nav.today' } })
  const r = await c.call('tools/call', { name: 'add_node', arguments: { name: 'Prune the apple tree', tag: 'task' } })
  expect(r.result.isError ?? false).toBe(false)
  const md = await c.call('tools/call', { name: 'export_markdown', arguments: {} })
  expect(JSON.stringify(md.result)).toContain('Prune the apple tree')
  c.close()
  // No reload: the open window shows it.
  await expect(app.locator('.row.node', { hasText: 'Prune the apple tree' })).toBeVisible()
  await expect(app.locator('.row.node', { hasText: 'Prune the apple tree' }).locator('.chip')).toContainText('task')
})

test('UC-10.11 MCP works on the running app; on the file only when no app has it open', works('UC-10.11'), async ({ app, server }) => {
  const { page } = await pageWith(server, app, ['Raised beds'], 'Live plan')
  const c = mcp(server.file)
  await c.call('initialize', {})
  await c.call('tools/call', { name: 'add_node', arguments: { name: 'Water the beds', parent: page } })
  c.close()
  await expect(app.locator('.row.node', { hasText: 'Water the beds' })).toBeVisible()
  // The person's page and caret stay where they were (MCP has its own window).
  await expect(app.locator('.panel .title-input')).toHaveValue('Live plan')
  // With the app gone, MCP edits the file itself.
  await app.waitForTimeout(1200)
  await server.kill('SIGTERM')
  const f = mcp(server.file)
  await f.call('initialize', {})
  const r = await f.call('tools/call', { name: 'add_node', arguments: { name: 'Offline note', parent: page } })
  expect(r.result.isError ?? false).toBe(false)
  f.close()
  await server.restart()
  expect((await tree(server, page)).children.map((x: any) => x.name)).toContain('Offline note')
})

test('UC-10.02 an AI assistant reads a page as Markdown through MCP', works('UC-10.02'), async ({ app, server }) => {
  await pageWith(server, app, ['Raised beds', 'Seed list'], 'Garden plan')
  await app.waitForTimeout(1500)
  await server.kill('SIGTERM')
  const c = mcp(server.file)
  await c.call('initialize', {})
  const find = await c.call('tools/call', { name: 'execute', arguments: { command: 'graph.search', params: { query: 'Garden plan' } } })
  const id = Number(/"id":\s*(\d+)/.exec(JSON.stringify(find.result.structuredContent ?? JSON.parse(find.result.content[0].text)))?.[1])
  expect(id, JSON.stringify(find)).toBeGreaterThan(0)
  const r = await c.call('tools/call', { name: 'export_markdown', arguments: { node: id } })
  expect(JSON.stringify(r.result)).toContain('Raised beds')
  c.close()
})

test('UC-10.03 a script posts to the local JSON API and the node appears live', works('UC-10.03'), async ({ app, server }) => {
  const today = (await server.run('nav.today')).result.zoom
  const headers = { authorization: `Bearer ${liveToken(server.file)}` }
  const res = await fetch(`http://127.0.0.1:${server.port}/rpc`, {
    method: 'POST',
    headers,
    body: JSON.stringify({ method: 'run', params: { command: 'node.add', params: { name: 'From my script', parent: today } } }),
  })
  const id = (await res.json()).result.id
  await fetch(`http://127.0.0.1:${server.port}/rpc`, { method: 'POST', headers, body: JSON.stringify({ method: 'run', params: { command: 'field.add', params: { node: id, field: 'Source', value: 'cron job' } } }) })
  await expect(app.locator('.row.node', { hasText: 'From my script' })).toBeVisible()
  expect((await tree(server, id)).fields.Source).toBe('cron job')
})

test('UC-10.04 the API only listens on this computer and asks local tools for the token', partial('UC-10.04'), async ({ server }) => {
  const without = await fetch(`http://127.0.0.1:${server.port}/rpc`, { method: 'POST', body: '{"method":"version"}' })
  expect(without.status).toBe(401)
  const r = await fetch(`http://127.0.0.1:${server.port}/rpc`, { method: 'POST', body: '{"method":"version"}', headers: { authorization: `Bearer ${liveToken(server.file)}` } })
  expect(r.ok).toBe(true)
  // Gap: one token per running app (in the live file beside the graph), not tokens per workspace;
  // no hosted (remote) MCP.
})

missing('UC-10.05', 'hosted (remote) MCP server reachable from cloud agents', 'out-of-scope')
missing('UC-10.06', 'two-way sync with Google Calendar or Outlook (events, contacts)', 'out-of-scope')
missing('UC-10.07', 'Readwise highlights synced into #book and #highlight nodes')
missing('UC-10.08', 'a command node that calls a web API (Make API request)')
missing('UC-10.09', 'API tokens per workspace that can be created and revoked')
missing('UC-10.10', 'GitHub, Linear, Jira, Slack, HubSpot and Pipedrive connections', 'out-of-scope')
