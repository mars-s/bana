// TanaCraft remote MCP connector: claude.ai and ChatGPT reach the TanaCraft app on your Mac.
//
// The Worker is an OAuth 2.1 provider for MCP clients (dynamic client registration, PKCE), signs
// you in with GitHub and lets exactly one GitHub account in. Authorized MCP requests are passed to
// the app's own MCP endpoint (POST /mcp) through a Cloudflare Tunnel, with the install's local
// token added here, so the token never reaches the client. No notes are stored in the cloud: when
// the Mac sleeps or the app is closed, tools answer that it is unreachable.
//
// Bindings (wrangler.jsonc, `wrangler secret put`):
//   OAUTH_KV              KV namespace for the OAuth provider (clients, grants, tokens)
//   PUBLIC_ORIGIN         this Worker's origin, e.g. https://tanacraft-mcp.example.workers.dev
//   MAC_MCP_URL           the tunnel's address of the app's endpoint, e.g. https://mac.example.org/mcp
//   ALLOWED_GITHUB_ID     the one GitHub account (numeric id) that may connect
//   GITHUB_CLIENT_ID      secret: the GitHub OAuth app
//   GITHUB_CLIENT_SECRET  secret
//   MAC_TOKEN             secret: the app's local token (~/TanaCraft/token on the Mac)
//   CF_ACCESS_CLIENT_ID, CF_ACCESS_CLIENT_SECRET  optional secrets: a Cloudflare Access service
//                         token, when the tunnel's hostname is behind Access as well

import OAuthProvider, { OAuthError, authorizationErrorRedirect, type OAuthHelpers, type OAuthResourceAuth } from '@cloudflare/workers-oauth-provider'

export interface Env {
  OAUTH_KV?: KVNamespace
  OAUTH_PROVIDER?: OAuthHelpers
  PUBLIC_ORIGIN?: string
  MAC_MCP_URL?: string
  ALLOWED_GITHUB_ID?: string
  GITHUB_CLIENT_ID?: string
  GITHUB_CLIENT_SECRET?: string
  MAC_TOKEN?: string
  CF_ACCESS_CLIENT_ID?: string
  CF_ACCESS_CLIENT_SECRET?: string
}

const SCOPE = 'notes'
/** What a grant carries: who signed in. */
type Identity = { github_id: string }
/** MCP clients whose browser pages may call /mcp directly. */
const CLIENT_ORIGINS = ['https://claude.ai', 'https://chatgpt.com', 'https://chat.openai.com']
const BODY_LIMIT = 1024 * 1024

const json = (body: unknown, status: number) => Response.json(body, { status, headers: { 'Cache-Control': 'no-store', 'X-Content-Type-Options': 'nosniff' } })
const escape = (v: string) => v.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`)

function allowed(props: unknown, env: Env): props is Identity {
  const p = props as Partial<Identity> | undefined
  return !!p && !!env.ALLOWED_GITHUB_ID && p.github_id === env.ALLOWED_GITHUB_ID
}

async function s256(verifier: string) {
  const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(verifier))
  return btoa(String.fromCharCode(...new Uint8Array(digest))).replace(/=/g, '').replace(/\+/g, '-').replace(/\//g, '_')
}

/** Sign-in: a consent page, then GitHub (PKCE), then only the allowed account gets a grant. */
async function signIn(request: Request, env: Env): Promise<Response> {
  const url = new URL(request.url)
  const origin = env.PUBLIC_ORIGIN!
  const oauth = env.OAUTH_PROVIDER!
  try {
    if (url.pathname === '/authorize' && request.method === 'GET') {
      const req = await oauth.parseAuthRequest(request)
      if (req.scope.some((s) => s !== SCOPE && s !== 'offline_access')) return json({ error: 'Only the notes scope is offered.' }, 400)
      const facts = await oauth.describeConsent(req)
      const consent = await oauth.beginConsent(req)
      const headers = consent.headers
      headers.set('Content-Type', 'text/html; charset=utf-8')
      // The form posts here, then redirects to GitHub (or back to the client on Deny).
      headers.set('Content-Security-Policy', `default-src 'none'; form-action 'self' https://github.com ${new URL(req.redirectUri).origin}; base-uri 'none'; frame-ancestors 'none'`)
      const page = `<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>TanaCraft access</title>
<h1>Let ${escape(facts.clientName)} use your TanaCraft notes?</h1>
<p>It can read and change the notes in the TanaCraft app on your Mac while the Mac is awake and the app is open.</p>
<p>Access is sent back to ${escape(facts.redirectHost)}.${facts.redirectIsLoopback ? ' This is an app on this computer: check that you started it.' : ''}</p>
<form method="post" action="/authorize"><input type="hidden" name="handle" value="${escape(consent.handle)}">
<button name="decision" value="allow">Sign in with GitHub and allow</button> <button name="decision" value="deny">Deny</button></form></html>`
      return new Response(page, { headers })
    }
    if (url.pathname === '/authorize' && request.method === 'POST') {
      if (request.headers.get('origin') !== origin) return json({ error: 'Consent must come from this page.' }, 403)
      const form = await request.formData()
      const handle = String(form.get('handle') ?? '')
      if (form.get('decision') === 'deny') {
        const denied = await oauth.denyConsent(request, handle)
        return new Response(null, { status: 302, headers: denied.headers })
      }
      if (form.get('decision') !== 'allow') return json({ error: 'Consent required.' }, 400)
      const approved = await oauth.approveConsent(request, handle, { scope: [SCOPE, 'offline_access'] })
      const verifier = crypto.randomUUID() + crypto.randomUUID()
      const upstream = await oauth.beginUpstream(approved.request, { data: { verifier }, headers: approved.headers })
      const target = new URL('https://github.com/login/oauth/authorize')
      // No GitHub scopes: only who you are is needed.
      target.search = new URLSearchParams({
        client_id: env.GITHUB_CLIENT_ID!,
        redirect_uri: origin + '/callback',
        scope: '',
        state: upstream.state,
        code_challenge: await s256(verifier),
        code_challenge_method: 'S256',
        allow_signup: 'false',
      }).toString()
      upstream.headers.set('Location', target.href)
      return new Response(null, { status: 302, headers: upstream.headers })
    }
    if (url.pathname === '/callback' && request.method === 'GET') {
      const resumed = await oauth.finishUpstream<{ verifier: string }>(request)
      const deny = () => {
        resumed.headers.set('Location', authorizationErrorRedirect(resumed.request, 'access_denied'))
        return new Response(null, { status: 302, headers: resumed.headers })
      }
      const code = url.searchParams.get('code')
      if (url.searchParams.has('error') || !code) return deny()
      const exchange = await fetch('https://github.com/login/oauth/access_token', {
        method: 'POST',
        redirect: 'manual',
        signal: AbortSignal.timeout(15000),
        headers: { Accept: 'application/json', 'Content-Type': 'application/x-www-form-urlencoded' },
        body: new URLSearchParams({ client_id: env.GITHUB_CLIENT_ID!, client_secret: env.GITHUB_CLIENT_SECRET!, code, redirect_uri: origin + '/callback', code_verifier: resumed.data.verifier }),
      })
      if (!exchange.ok) return deny()
      const got = (await exchange.json()) as { access_token?: string; token_type?: string; scope?: string }
      // A GitHub token with scopes means the app asks for more than it should: refuse.
      if (!got.access_token || got.token_type !== 'bearer' || got.scope?.trim()) return deny()
      const who = await fetch('https://api.github.com/user', {
        redirect: 'manual',
        signal: AbortSignal.timeout(15000),
        headers: { Authorization: 'Bearer ' + got.access_token, Accept: 'application/vnd.github+json', 'User-Agent': 'TanaCraftConnector/0.1' },
      })
      if (!who.ok) return deny()
      const user = (await who.json()) as { id?: number }
      if (!Number.isSafeInteger(user.id) || String(user.id) !== env.ALLOWED_GITHUB_ID) return deny()
      // The GitHub token is not kept: the grant holds only the account id.
      const done = await oauth.completeAuthorization({
        request: resumed.request,
        userId: String(user.id),
        metadata: {},
        scope: [SCOPE, 'offline_access'],
        props: { github_id: String(user.id) } satisfies Identity,
      })
      resumed.headers.set('Location', done.redirectTo)
      return new Response(null, { status: 302, headers: resumed.headers })
    }
    if (url.pathname === '/' || url.pathname === '/health') return json({ ok: true, service: 'TanaCraft remote MCP connector' }, 200)
    return json({ error: 'Not found.' }, 404)
  } catch {
    // Never echo library errors, cookies, codes or URLs.
    return json({ error: 'Sign-in could not be completed. Start the connection again.' }, 400)
  }
}

/** An authorized MCP request goes to the Mac through the tunnel, with the local token. */
async function toMac(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
  const c = ctx as ExecutionContext & { props?: Identity; auth?: OAuthResourceAuth }
  if (!allowed(c.props, env)) return json({ error: 'This connector belongs to another account.' }, 403)
  if (!c.auth?.scope.includes(SCOPE)) return json({ error: 'The notes scope is required.' }, 403)
  const source = request.headers.get('origin')
  if (source && ![env.PUBLIC_ORIGIN, ...CLIENT_ORIGINS].includes(source)) return json({ error: 'Origin denied.' }, 403)
  // Only POST carries messages (no server-sent stream, no sessions in the app).
  if (request.method !== 'POST') return new Response(null, { status: 405, headers: { Allow: 'POST' } })
  const body = await request.arrayBuffer()
  if (body.byteLength > BODY_LIMIT) return json({ error: 'Request too large.' }, 413)
  const headers: Record<string, string> = { 'Content-Type': 'application/json', Accept: 'application/json', Authorization: 'Bearer ' + env.MAC_TOKEN }
  const version = request.headers.get('mcp-protocol-version')
  if (version) headers['MCP-Protocol-Version'] = version
  if (env.CF_ACCESS_CLIENT_ID && env.CF_ACCESS_CLIENT_SECRET) {
    headers['CF-Access-Client-Id'] = env.CF_ACCESS_CLIENT_ID
    headers['CF-Access-Client-Secret'] = env.CF_ACCESS_CLIENT_SECRET
  }
  let mac: Response
  try {
    mac = await fetch(env.MAC_MCP_URL!, { method: 'POST', headers, body, redirect: 'manual', signal: AbortSignal.timeout(30000) })
  } catch {
    return unreachable(body)
  }
  // The tunnel answers 502 to 530 when nothing listens behind it (the Mac asleep, the app closed).
  if (mac.status >= 500 || mac.status === 401) return mac.status === 401 ? json({ error: 'The app refused the connector token; set MAC_TOKEN again (docs/mcp.md).' }, 502) : unreachable(body)
  return new Response(mac.body, { status: mac.status, headers: { 'Content-Type': mac.headers.get('content-type') ?? 'application/json', 'Cache-Control': 'no-store' } })
}

/** A JSON-RPC error for each request when the Mac cannot be reached. */
function unreachable(body: ArrayBuffer): Response {
  const error = { code: -32001, message: 'TanaCraft on the Mac is not reachable: the Mac is asleep or offline, or the app is closed.' }
  let msg: unknown = null
  try {
    msg = JSON.parse(new TextDecoder().decode(body))
  } catch {
    /* not JSON */
  }
  const reply = (m: unknown) => ({ jsonrpc: '2.0', id: (m as { id?: unknown })?.id ?? null, error })
  const out = Array.isArray(msg) ? msg.filter((m) => m?.id != null).map(reply) : reply(msg)
  return json(out, 200)
}

/** The OAuth provider for this Worker's origin (made once per isolate; it needs the origin). */
let provider: { origin: string; p: OAuthProvider<Env> } | null = null
function oauthFor(origin: string): OAuthProvider<Env> {
  if (provider?.origin === origin) return provider.p
  const p = new OAuthProvider<Env>({
  apiRoute: '/mcp',
  apiHandler: { fetch: toMac },
  defaultHandler: { fetch: signIn },
  authorizeEndpoint: '/authorize',
  tokenEndpoint: '/oauth/token',
  clientRegistrationEndpoint: '/oauth/register',
  clientIdMetadataDocumentEnabled: false,
  scopesSupported: [SCOPE, 'offline_access'],
  resourceMetadata: { resource: origin + '/mcp', authorization_servers: [origin], resource_name: 'TanaCraft notes on your Mac' },
  requiredScopes: [SCOPE],
  accessTokenTTL: 3600,
  refreshTokenTTL: 30 * 86400,
  clientRegistrationTTL: 90 * 86400,
  onError: () => {},
  tokenExchangeCallback(options) {
    if (!allowed(options.props, options.env) || options.scope.some((s) => s !== SCOPE && s !== 'offline_access')) throw new OAuthError('invalid_grant', { description: 'Grant not allowed.' })
  },
})
  provider = { origin, p }
  return p
}

export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
    // Fail closed until every setting is in place (the wizard sets them).
    const missing = (['OAUTH_KV', 'PUBLIC_ORIGIN', 'MAC_MCP_URL', 'ALLOWED_GITHUB_ID', 'GITHUB_CLIENT_ID', 'GITHUB_CLIENT_SECRET', 'MAC_TOKEN'] as const).filter((k) => !env[k])
    if (missing.length) return json({ error: 'The connector is not set up yet.', missing }, 503)
    if (new URL(request.url).origin !== env.PUBLIC_ORIGIN) return json({ error: 'Unknown host.' }, 403)
    if (request.method === 'POST' && Number(request.headers.get('content-length') ?? 0) > BODY_LIMIT) return json({ error: 'Request too large.' }, 413)
    return oauthFor(env.PUBLIC_ORIGIN!).fetch(request, env, ctx)
  },
}
