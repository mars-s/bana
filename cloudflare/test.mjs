// The connector Worker on local workerd (Miniflare) with a stand-in GitHub, and end to end: the
// official MCP SDK client signs in, then its requests pass through the Worker to a real
// `tanacraft-cli serve` on this machine (the tunnel is replaced by a forward to 127.0.0.1).
// Run: npm test (after `cargo build -p tanacraft-cli`).

import { build } from 'esbuild'
import { Miniflare, convertV4MiniflareOptions } from 'miniflare'
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { readFile, rm, mkdtemp } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { createServer } from 'node:net'
import { Client } from '@modelcontextprotocol/sdk/client/index.js'
import { StreamableHTTPClientTransport } from '@modelcontextprotocol/sdk/client/streamableHttp.js'

const ORIGIN = 'https://tanacraft-mcp.example.workers.dev'
const RESOURCE = ORIGIN + '/mcp'
const MAC = 'https://mac.example.org/mcp'
const CALLBACK = 'https://claude.ai/api/mcp/auth_callback'

// ---- a real engine on this machine ----
const root = resolve(import.meta.dirname, '..')
const bin = join(process.env.CARGO_TARGET_DIR ?? join(root, 'target'), 'debug', 'tanacraft-cli')
const home = await mkdtemp(join(tmpdir(), 'tanacraft-connector-'))
const port = await new Promise((done) => {
  const s = createServer().listen(0, '127.0.0.1', () => {
    const p = s.address().port
    s.close(() => done(p))
  })
})
let engine = spawn(bin, ['--file', join(home, 'graph.tanacraft'), 'serve', '--sample', '--today', '2026-10-10', '--port', String(port)], { stdio: 'ignore', env: { ...process.env, TANACRAFT_HOME: home } })
let macToken = ''
for (let i = 0; i < 200 && !macToken; i++) {
  await new Promise((r) => setTimeout(r, 50))
  macToken = await readFile(join(home, 'token'), 'utf8').then((t) => t.trim(), () => '')
}
assert.ok(macToken, 'the engine made its token')
let macUp = true
let macCalls = 0

// ---- the Worker ----
await build({ entryPoints: ['worker.ts'], outfile: '.test-worker.mjs', bundle: true, platform: 'node', format: 'esm', external: ['cloudflare:workers'] })
let githubId = 42
let githubScope = ''
const options = (bindings) =>
  convertV4MiniflareOptions({
    modules: true,
    scriptPath: '.test-worker.mjs',
    compatibilityDate: '2026-09-30',
    compatibilityFlags: ['nodejs_compat'],
    kvNamespaces: ['OAUTH_KV'],
    bindings,
    outboundService: async (req) => {
      const url = new URL(req.url)
      if (url.href === 'https://github.com/login/oauth/access_token') {
        const body = await req.formData()
        assert.equal(body.get('client_id'), 'TEST-client')
        assert.ok(body.get('code_verifier'))
        return Response.json({ access_token: 'TEST-upstream', token_type: 'bearer', scope: githubScope })
      }
      if (url.href === 'https://api.github.com/user') return Response.json({ id: githubId })
      if (url.href === MAC) {
        // The tunnel: the Mac's engine, or a tunnel error page when it is down.
        macCalls++
        if (!macUp) return new Response('origin unreachable', { status: 530 })
        return fetch(`http://127.0.0.1:${port}/mcp`, { method: 'POST', headers: req.headers, body: await req.arrayBuffer() })
      }
      throw new Error('unexpected request to ' + url.href)
    },
  })
const bindings = {
  PUBLIC_ORIGIN: ORIGIN,
  MAC_MCP_URL: MAC,
  ALLOWED_GITHUB_ID: '42',
  GITHUB_CLIENT_ID: 'TEST-client',
  GITHUB_CLIENT_SECRET: 'TEST-secret',
  MAC_TOKEN: macToken,
}
const mf = new Miniflare(options(bindings))

const cookies = new Map()
async function request(path, { method = 'GET', body, headers = {}, cookie = true } = {}) {
  const r = await mf.dispatchFetch(ORIGIN + path, {
    method,
    redirect: 'manual',
    headers: { ...(body && typeof body !== 'string' ? { 'Content-Type': 'application/x-www-form-urlencoded' } : {}), ...(cookie ? { Cookie: [...cookies].map(([k, v]) => k + '=' + v).join('; ') } : {}), ...headers },
    body,
  })
  for (const c of r.headers.getSetCookie()) {
    const [pair] = c.split(';')
    const i = pair.indexOf('=')
    cookies.set(pair.slice(0, i), pair.slice(i + 1))
  }
  return r
}
const verifier = 'TEST-pkce-verifier-that-is-long-enough-for-the-spec-0123456789'
const challenge = createHash('sha256').update(verifier).digest('base64url')
let clientId
async function consent(extra = {}) {
  const q = new URLSearchParams({ response_type: 'code', client_id: clientId, redirect_uri: CALLBACK, scope: 'notes offline_access', state: 'client-state', code_challenge: challenge, code_challenge_method: 'S256', resource: RESOURCE, ...extra })
  const r = await request('/authorize?' + q)
  return { r, html: await r.text() }
}
const handleOf = (html) => html.match(/name="handle" value="([^"]+)"/)[1]
/** Consent, allow, and the GitHub redirect: returns the callback path GitHub would call. */
async function toGithub() {
  const { r, html } = await consent()
  assert.equal(r.status, 200)
  const next = await request('/authorize', { method: 'POST', body: new URLSearchParams({ handle: handleOf(html), decision: 'allow' }), headers: { Origin: ORIGIN } })
  assert.equal(next.status, 302)
  const gh = new URL(next.headers.get('location'))
  assert.equal(gh.origin, 'https://github.com')
  assert.equal(gh.searchParams.get('scope'), '')
  assert.equal(gh.searchParams.get('code_challenge_method'), 'S256')
  return '/callback?' + new URLSearchParams({ code: 'TEST-code', state: gh.searchParams.get('state') })
}
async function signedIn() {
  const r = await request(await toGithub())
  assert.equal(r.status, 302)
  const back = new URL(r.headers.get('location'))
  assert.equal(back.origin + back.pathname, CALLBACK)
  const t = await request('/oauth/token', { method: 'POST', body: new URLSearchParams({ grant_type: 'authorization_code', client_id: clientId, code: back.searchParams.get('code'), redirect_uri: CALLBACK, code_verifier: verifier, resource: RESOURCE }) })
  assert.equal(t.status, 200)
  return t.json()
}

try {
  await test('remote MCP connector', async (t) => {
    await t.test('discovery, registration and a 401 with the resource metadata', async () => {
      const r = await request('/mcp', { method: 'POST', body: '{}', headers: { 'Content-Type': 'application/json' } })
      assert.equal(r.status, 401)
      assert.match(r.headers.get('www-authenticate'), /oauth-protected-resource/)
      assert.equal((await request('/.well-known/oauth-protected-resource/mcp')).status, 200)
      const meta = await (await request('/.well-known/oauth-authorization-server')).json()
      assert.ok(meta.registration_endpoint.endsWith('/oauth/register'))
      assert.ok(meta.code_challenge_methods_supported.includes('S256'))
      const reg = await request('/oauth/register', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ client_name: 'Claude <b>', redirect_uris: [CALLBACK], token_endpoint_auth_method: 'none', grant_types: ['authorization_code', 'refresh_token'], response_types: ['code'] }) })
      assert.equal(reg.status, 201)
      clientId = (await reg.json()).client_id
    })
    await t.test('the consent page escapes the client name; bad redirects and scopes are refused', async () => {
      const { r, html } = await consent()
      assert.ok(html.includes('&#60;b&#62;') && !html.includes('<b>'))
      assert.match(r.headers.get('content-security-policy'), /frame-ancestors 'none'/)
      assert.equal((await consent({ redirect_uri: 'https://attacker.example/cb' })).r.status, 400)
      assert.equal((await consent({ scope: 'everything' })).r.status, 400)
      const other = await request('/authorize', { method: 'POST', body: new URLSearchParams({ handle: handleOf(html), decision: 'allow' }), headers: { Origin: 'https://attacker.example' } })
      assert.equal(other.status, 403)
    })
    await t.test('another GitHub account, or a GitHub grant with scopes, is turned away', async () => {
      githubId = 43
      assert.match((await request(await toGithub())).headers.get('location'), /access_denied/)
      githubId = 42
      githubScope = 'repo'
      assert.match((await request(await toGithub())).headers.get('location'), /access_denied/)
      githubScope = ''
    })
    await t.test('end to end: the MCP SDK client works on the notes in the app on the Mac', async () => {
      const tokens = await signedIn()
      const sdk = new Client({ name: 'Test assistant', version: '1.0.0' })
      const transport = new StreamableHTTPClientTransport(new URL(RESOURCE), { requestInit: { headers: { Authorization: 'Bearer ' + tokens.access_token } }, fetch: (url, init) => mf.dispatchFetch(url, init) })
      await sdk.connect(transport)
      const tools = (await sdk.listTools()).tools.map((x) => x.name)
      assert.ok(tools.includes('add_node') && tools.includes('export_markdown'), tools.join(','))
      const added = await sdk.callTool({ name: 'add_node', arguments: { name: 'Plant garlic from Claude', tag: 'task' } })
      assert.notEqual(added.isError, true, JSON.stringify(added))
      const found = await sdk.callTool({ name: 'search', arguments: { query: 'garlic from Claude' } })
      assert.match(JSON.stringify(found), /Plant garlic from Claude/)
      await sdk.close()
      // The local token never reaches the client; a client's own token never reaches the Mac.
      assert.ok(!JSON.stringify(tokens).includes(macToken))
      // Refresh works; a revoked token stops working.
      const refreshed = await request('/oauth/token', { method: 'POST', body: new URLSearchParams({ grant_type: 'refresh_token', client_id: clientId, refresh_token: tokens.refresh_token, resource: RESOURCE }) })
      assert.equal(refreshed.status, 200)
      const fresh = (await refreshed.json()).access_token
      assert.equal((await request('/oauth/token', { method: 'POST', body: new URLSearchParams({ client_id: clientId, token: fresh, token_type_hint: 'access_token' }) })).status, 200)
      const after = await request('/mcp', { method: 'POST', body: '{"jsonrpc":"2.0","id":1,"method":"ping"}', headers: { Authorization: 'Bearer ' + fresh, 'Content-Type': 'application/json' } })
      assert.equal(after.status, 401)
    })
    await t.test('when the Mac sleeps, tools answer that it is unreachable', async () => {
      const tokens = await signedIn()
      macUp = false
      const r = await request('/mcp', { method: 'POST', body: '{"jsonrpc":"2.0","id":7,"method":"tools/list"}', headers: { Authorization: 'Bearer ' + tokens.access_token, 'Content-Type': 'application/json' } })
      const body = await r.json()
      assert.equal(body.id, 7)
      assert.match(body.error.message, /not reachable/)
      macUp = true
      // A page on another site cannot use a token it got hold of.
      const evil = await request('/mcp', { method: 'POST', body: '{"jsonrpc":"2.0","id":1,"method":"ping"}', headers: { Authorization: 'Bearer ' + tokens.access_token, Origin: 'https://attacker.example', 'Content-Type': 'application/json' } })
      assert.equal(evil.status, 403)
    })
    await t.test('the tunnel endpoint itself refuses requests without the local token', async () => {
      const r = await fetch(`http://127.0.0.1:${port}/mcp`, { method: 'POST', body: '{"jsonrpc":"2.0","id":1,"method":"ping"}', headers: { 'Content-Type': 'application/json', Authorization: 'Bearer client-token' } })
      assert.equal(r.status, 401)
    })
    await t.test('without its settings the Worker stays closed', async () => {
      const closed = new Miniflare(options({ PUBLIC_ORIGIN: ORIGIN }))
      try {
        const r = await closed.dispatchFetch(RESOURCE, { method: 'POST', body: '{}' })
        assert.equal(r.status, 503)
      } finally {
        await closed.dispose()
      }
    })
  })
} finally {
  await mf.dispose()
  engine.kill('SIGKILL')
  await rm(home, { recursive: true, force: true })
  await rm('.test-worker.mjs', { force: true })
}
