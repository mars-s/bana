// Bana ID (docs/bana-id.md): optional accounts for sync and recovery, signed in with passkeys only.
//
// One Durable Object ("Accounts", SQLite) holds every Bana ID on this relay: passkeys (credential
// id and public key), sessions (one per signed-in device, with its name sealed by the app), the
// workspace key WRAPPED once per passkey (by a key from the passkey's PRF output) and once by the
// recovery code, and the account's sync room. It never sees an unwrapped key, a PRF output or a
// recovery code: only a hash of the code's sign-in proof.
// ponytail: one object for all accounts (a personal relay has a handful); shard by account id if
// a relay ever serves thousands.
//
// The flow: the app opens GET /id (the page in src/page.ts) in the system sign-in sheet or the
// browser. The page runs WebAuthn here, on the relay's own origin (the relying party, RP_ID), and
// sends the PRF output masked for the app (X25519). The relay answers a one-time code; the page
// hands it to the app through bana://auth/... (iPhone) or http://127.0.0.1:<port>/auth/callback
// (desktop). The app exchanges the code once for a session.

import { DurableObject } from 'cloudflare:workers'
import { ID_LIMITS as L } from './limits'
import { b64url, fromB64url, sha256, verifyAssertion, verifyRegistration } from './webauthn'
import type { Env } from './index'

type Json = Record<string, any>
type Mode = 'create' | 'signin' | 'add'

const json = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json', 'cache-control': 'no-store' } })
const fail = (status: number, error: string) => json({ error }, status)

const rand = (n: number) => b64url(crypto.getRandomValues(new Uint8Array(n)))
const hash = async (s: string) => b64url(await sha256(s))
/** Standard base64 (the room's invite hashes and secrets use it, as crates/sync does). */
const b64 = (b: Uint8Array) => btoa(String.fromCharCode(...b))

const str = (v: unknown, max: number): v is string => typeof v === 'string' && v.length > 0 && v.length <= max
const ROOM = /^[0-9a-f]{32}$/

/** Where the page may send the one-time code: the iPhone app's scheme or a loopback port on this
 * computer. Nothing else, so a link made by someone else cannot send a code to their server. */
export function okReturn(r: unknown): r is string {
  return typeof r === 'string' && r.length <= 200 && (/^bana:\/\/auth\/[A-Za-z0-9/_-]*$/.test(r) || /^http:\/\/(127\.0\.0\.1|localhost):\d{1,5}\/auth\/callback$/.test(r))
}

/** The relying party: RP_ID from the config, else this Worker's own host. */
export const rpIdOf = (env: Env, url: URL) => (env.RP_ID || url.hostname).toLowerCase()

/** Pages on the relying party's own origin (and http://localhost in local development). */
const originOk = (rpId: string) => (o: string) => o === `https://${rpId}` || (rpId === 'localhost' && /^http:\/\/localhost(:\d{1,5})?$/.test(o))

/** The Worker's side: /id/api/* goes to the one Accounts object. */
export function idApi(req: Request, env: Env): Promise<Response> {
  return env.ACCOUNTS.get(env.ACCOUNTS.idFromName('accounts-v1')).fetch(req)
}

interface Session {
  sid: string
  account: string
}

export class Accounts extends DurableObject<Env> {
  sql: SqlStorage

  constructor(ctx: DurableObjectState, env: Env) {
    super(ctx, env)
    this.sql = ctx.storage.sql
    this.sql.exec(`
      CREATE TABLE IF NOT EXISTS accounts (id TEXT PRIMARY KEY, created INTEGER NOT NULL, room TEXT, recovery_auth TEXT UNIQUE, recovery_wrap TEXT);
      CREATE TABLE IF NOT EXISTS creds (id TEXT PRIMARY KEY, account TEXT NOT NULL, jwk TEXT NOT NULL, counter INTEGER NOT NULL, created INTEGER NOT NULL, used INTEGER NOT NULL, wrap TEXT);
      CREATE TABLE IF NOT EXISTS sessions (sid TEXT PRIMARY KEY, token TEXT UNIQUE NOT NULL, account TEXT NOT NULL, cred TEXT, name TEXT, device TEXT, created INTEGER NOT NULL, seen INTEGER NOT NULL);
      CREATE TABLE IF NOT EXISTS challenges (id TEXT PRIMARY KEY, challenge TEXT NOT NULL, mode TEXT NOT NULL, account TEXT, ret TEXT NOT NULL, app TEXT NOT NULL, expires INTEGER NOT NULL);
      CREATE TABLE IF NOT EXISTS codes (hash TEXT PRIMARY KEY, body TEXT NOT NULL, expires INTEGER NOT NULL);
      CREATE TABLE IF NOT EXISTS tickets (hash TEXT PRIMARY KEY, account TEXT NOT NULL, expires INTEGER NOT NULL);
      CREATE TABLE IF NOT EXISTS rate (k TEXT PRIMARY KEY, tokens REAL NOT NULL, at INTEGER NOT NULL);
    `)
  }

  /** Takes one token from bucket `k`; false when it is empty. */
  take(k: string, lim: { burst: number; perMinute: number }, now = Date.now()): boolean {
    const r = this.sql.exec<{ tokens: number; at: number }>('SELECT tokens, at FROM rate WHERE k = ?', k).toArray()[0]
    const tokens = Math.min(lim.burst, (r ? r.tokens : lim.burst) + ((now - (r?.at ?? now)) * lim.perMinute) / 60_000)
    if (tokens < 1) return false
    this.sql.exec('INSERT OR REPLACE INTO rate (k, tokens, at) VALUES (?, ?, ?)', k, tokens - 1, now)
    return true
  }

  room(roomId: string) {
    return this.env.ROOMS.get(this.env.ROOMS.idFromName(roomId))
  }

  async fetch(req: Request): Promise<Response> {
    const url = new URL(req.url)
    const ip = req.headers.get('cf-connecting-ip') ?? 'local'
    const path = url.pathname.replace(/^\/id\/api\//, '')
    const now = Date.now()
    this.sql.exec('DELETE FROM challenges WHERE expires <= ?', now)
    this.sql.exec('DELETE FROM codes WHERE expires <= ?', now)
    this.sql.exec('DELETE FROM tickets WHERE expires <= ?', now)
    let body: Json = {}
    if (req.method !== 'GET' && req.method !== 'DELETE') {
      const text = await req.text()
      if (text.length > L.body) return fail(413, 'too big')
      try {
        body = text ? JSON.parse(text) : {}
      } catch {
        return fail(400, 'not JSON')
      }
      if (!body || typeof body !== 'object' || Array.isArray(body)) return fail(400, 'not a JSON object')
    }
    const rpId = rpIdOf(this.env, url)
    try {
      if (req.method === 'POST' && (path === 'options' || path === 'finish' || path === 'exchange')) {
        if (!this.take(`ip:${ip}:auth`, L.auth, now)) return fail(429, 'too many tries; wait a minute')
        if (path === 'options') return await this.options(body, rpId, ip, now)
        if (path === 'finish') return await this.finish(body, rpId, now)
        return await this.exchange(body, now)
      }
      if (req.method === 'POST' && path === 'recover') {
        if (!this.take(`ip:${ip}:recover`, L.recover, now)) return fail(429, 'too many tries; wait an hour')
        return await this.recover(body, now)
      }
      const s = await this.session(req, now)
      if (!s) return fail(401, 'sign in again')
      if (!this.take(`acct:${s.account}`, L.session, now)) return fail(429, 'too many requests; slow down')
      const parts = path.split('/')
      if (req.method === 'GET' && path === 'account') return json(this.account(s))
      if (req.method === 'PUT' && path === 'vault') return await this.vault(s, body)
      if (req.method === 'POST' && path === 'ticket') return await this.ticket(s, now)
      if (req.method === 'DELETE' && parts[0] === 'credentials' && parts.length === 2) return this.removeCredential(s, parts[1])
      if (req.method === 'PATCH' && parts[0] === 'devices' && parts.length === 2) return this.renameDevice(s, parts[1], body)
      if (req.method === 'DELETE' && parts[0] === 'devices' && parts.length === 2) return await this.removeDevice(s, parts[1])
      if (req.method === 'DELETE' && path === 'account') return await this.deleteAccount(s)
      return fail(404, 'not found')
    } catch (e) {
      return fail(400, e instanceof Error ? e.message : 'bad request')
    }
  }

  // ---- ceremonies -------------------------------------------------------------------------

  async options(b: Json, rpId: string, ip: string, now: number): Promise<Response> {
    const mode = b.mode as Mode
    if (mode !== 'create' && mode !== 'signin' && mode !== 'add') return fail(400, 'mode: create, signin or add')
    if (!okReturn(b.ret)) return fail(400, 'that return address is not allowed')
    if (!fromB64url(b.app) || fromB64url(b.app)!.length !== 32) return fail(400, 'app: the app’s X25519 public key')
    const challenge = rand(32)
    const id = rand(16)
    let account: string | null = null
    let publicKey: Json
    if (mode === 'signin') {
      publicKey = { challenge, rpId, userVerification: 'required', allowCredentials: [], timeout: 120_000 }
    } else {
      let exclude: string[] = []
      if (mode === 'add') {
        if (!str(b.ticket, 100)) return fail(400, 'adding a passkey needs a ticket from the app')
        const h = await hash(b.ticket)
        const t = this.sql.exec<{ account: string }>('SELECT account FROM tickets WHERE hash = ? AND expires > ?', h, now).toArray()[0]
        this.sql.exec('DELETE FROM tickets WHERE hash = ?', h)
        if (!t) return fail(400, 'that link expired; start again from the app')
        account = t.account
        exclude = this.sql.exec<{ id: string }>('SELECT id FROM creds WHERE account = ?', account).toArray().map((c) => c.id)
        if (exclude.length >= L.credentials) return fail(409, `a Bana ID holds at most ${L.credentials} passkeys`)
      } else {
        if (!this.take(`ip:${ip}:create`, L.create, now)) return fail(429, 'too many new Bana IDs from here; wait a while')
        account = rand(16)
      }
      const name = typeof b.name === 'string' && b.name.trim() ? b.name.trim().slice(0, 64) : 'Bana'
      publicKey = {
        challenge,
        rp: { id: rpId, name: 'Bana' },
        user: { id: account, name: `${name} (Bana ID)`, displayName: name },
        pubKeyCredParams: [{ type: 'public-key', alg: -7 }],
        authenticatorSelection: { residentKey: 'required', requireResidentKey: true, userVerification: 'required' },
        attestation: 'none',
        excludeCredentials: exclude.map((id) => ({ type: 'public-key', id })),
        timeout: 120_000,
      }
    }
    this.sql.exec('INSERT INTO challenges (id, challenge, mode, account, ret, app, expires) VALUES (?, ?, ?, ?, ?, ?, ?)', id, challenge, mode, account, b.ret, b.app, now + L.challengeMs)
    return json({ challengeId: id, mode, publicKey, prfSalt: b64url(await sha256('bana-id/prf/v1')) })
  }

  async finish(b: Json, rpId: string, now: number): Promise<Response> {
    if (!str(b.challengeId, 64)) return fail(400, 'no challenge')
    const c = this.sql.exec<{ challenge: string; mode: Mode; account: string | null; ret: string; app: string }>('SELECT challenge, mode, account, ret, app FROM challenges WHERE id = ?', b.challengeId).toArray()[0]
    // A challenge is used once, whatever happens next.
    this.sql.exec('DELETE FROM challenges WHERE id = ?', b.challengeId)
    if (!c) return fail(400, 'that sign-in expired; start again')
    const r = b.response ?? {}
    const clientData = fromB64url(r.clientDataJSON)
    if (!clientData) return fail(400, 'no client data')
    if (b.masked != null && fromB64url(b.masked)?.length !== 32) return fail(400, 'bad PRF answer')
    if (b.masked != null && fromB64url(b.page)?.length !== 32) return fail(400, 'bad page key')
    const expect = { challenge: c.challenge, rpId, origins: originOk(rpId) }
    let account: string
    let cred: string
    if (c.mode === 'signin') {
      const auth = fromB64url(r.authenticatorData)
      const sig = fromB64url(r.signature)
      if (!auth || !sig || !str(b.id, 1400)) return fail(400, 'bad assertion')
      const row = this.sql.exec<{ account: string; jwk: string; counter: number }>('SELECT account, jwk, counter FROM creds WHERE id = ?', b.id).toArray()[0]
      if (!row) return fail(400, 'this passkey is not a Bana ID on this relay (it may have been removed)')
      if (r.userHandle && r.userHandle !== row.account) return fail(400, 'this passkey belongs to another account')
      const counter = await verifyAssertion(clientData, auth, sig, JSON.parse(row.jwk), { ...expect, type: 'webauthn.get' })
      if (counter !== 0 && counter <= row.counter) return fail(400, 'this passkey looks cloned (its counter went back)')
      this.sql.exec('UPDATE creds SET counter = ?, used = ? WHERE id = ?', counter, now, b.id)
      account = row.account
      cred = b.id
    } else {
      const att = fromB64url(r.attestationObject)
      if (!att || !c.account) return fail(400, 'bad registration')
      const reg = await verifyRegistration(clientData, att, { ...expect, type: 'webauthn.create' })
      if (this.sql.exec('SELECT 1 FROM creds WHERE id = ?', reg.credId).toArray().length) return fail(409, 'this passkey is already registered')
      if (c.mode === 'create') {
        if (Number(this.sql.exec<{ n: number }>('SELECT COUNT(*) AS n FROM accounts').one().n) >= L.accounts) return fail(409, 'this relay holds as many Bana IDs as it can')
        this.sql.exec('INSERT INTO accounts (id, created) VALUES (?, ?)', c.account, now)
      } else if (!this.sql.exec('SELECT 1 FROM accounts WHERE id = ?', c.account).toArray().length) {
        return fail(400, 'that Bana ID was deleted')
      }
      this.sql.exec('INSERT INTO creds (id, account, jwk, counter, created, used) VALUES (?, ?, ?, ?, ?, ?)', reg.credId, c.account, JSON.stringify(reg.jwk), reg.counter, now, now)
      account = c.account
      cred = reg.credId
    }
    const code = rand(32)
    const hand = { mode: c.mode, account, cred, app: c.app, page: b.masked != null ? b.page : null, masked: b.masked ?? null }
    this.sql.exec('INSERT INTO codes (hash, body, expires) VALUES (?, ?, ?)', await hash(code), JSON.stringify(hand), now + L.codeMs)
    return json({ code, ret: c.ret })
  }

  /** The app trades the one-time code for a session (create, sign in) or the new passkey (add). */
  async exchange(b: Json, now: number): Promise<Response> {
    if (!str(b.code, 100)) return fail(400, 'no code')
    const h = await hash(b.code)
    const row = this.sql.exec<{ body: string }>('SELECT body FROM codes WHERE hash = ?', h).toArray()[0]
    this.sql.exec('DELETE FROM codes WHERE hash = ?', h)
    if (!row) return fail(400, 'that code was used or has expired; sign in again')
    const hand = JSON.parse(row.body) as { mode: Mode; account: string; cred: string; app: string; page: string | null; masked: string | null }
    const out: Json = { mode: hand.mode, account: hand.account, credential: hand.cred, page: hand.page, masked: hand.masked }
    if (hand.mode === 'add') return json(out)
    Object.assign(out, await this.newSession(hand.account, hand.cred, now))
    const wrap = this.sql.exec<{ wrap: string | null }>('SELECT wrap FROM creds WHERE id = ?', hand.cred).toArray()[0]?.wrap ?? null
    return json({ ...out, wrap })
  }

  async recover(b: Json, now: number): Promise<Response> {
    if (!str(b.proof, 100)) return fail(400, 'no recovery proof')
    const a = this.sql.exec<{ id: string; recovery_wrap: string }>('SELECT id, recovery_wrap FROM accounts WHERE recovery_auth = ?', await hash(b.proof)).toArray()[0]
    if (!a) return fail(400, 'that recovery code does not match a Bana ID on this relay')
    return json({ mode: 'recover', account: a.id, recoveryWrap: a.recovery_wrap, ...(await this.newSession(a.id, null, now)) })
  }

  async newSession(account: string, cred: string | null, now: number): Promise<Json> {
    const n = Number(this.sql.exec<{ n: number }>('SELECT COUNT(*) AS n FROM sessions WHERE account = ?', account).one().n)
    if (n >= L.sessions) throw new Error(`a Bana ID can be signed in on at most ${L.sessions} devices; remove one on the profile page`)
    const sid = rand(9)
    const token = rand(32)
    this.sql.exec('INSERT INTO sessions (sid, token, account, cred, created, seen) VALUES (?, ?, ?, ?, ?, ?)', sid, await hash(token), account, cred, now, now)
    const room = this.sql.exec<{ room: string | null }>('SELECT room FROM accounts WHERE id = ?', account).one().room
    // Signed in: this device may join the account's sync room (one invite, for a few minutes).
    let invite: string | null = null
    if (room) {
      const secret = crypto.getRandomValues(new Uint8Array(32))
      await this.room(room).allow(b64(await sha256(secret)), now + L.ticketMs)
      invite = b64(secret)
    }
    return { session: token, sid, room, invite }
  }

  // ---- with a session -----------------------------------------------------------------------

  async session(req: Request, now: number): Promise<Session | null> {
    const m = (req.headers.get('authorization') ?? '').match(/^Bearer ([A-Za-z0-9_-]{20,100})$/)
    if (!m) return null
    const s = this.sql.exec<{ sid: string; account: string }>('SELECT sid, account FROM sessions WHERE token = ?', await hash(m[1])).toArray()[0]
    if (!s) return null
    this.sql.exec('UPDATE sessions SET seen = ? WHERE sid = ?', now, s.sid)
    return s
  }

  account(s: Session): Json {
    const a = this.sql.exec<{ created: number; room: string | null; recovery_auth: string | null }>('SELECT created, room, recovery_auth FROM accounts WHERE id = ?', s.account).one()
    return {
      account: s.account,
      created: a.created,
      room: a.room,
      recovery: !!a.recovery_auth,
      credentials: this.sql
        .exec<{ id: string; created: number; used: number; wrap: string | null }>('SELECT id, created, used, wrap FROM creds WHERE account = ? ORDER BY created', s.account)
        .toArray()
        .map((c) => ({ id: c.id, created: c.created, used: c.used, unlocks: !!c.wrap })),
      devices: this.sql
        .exec<{ sid: string; name: string | null; device: string | null; created: number; seen: number }>('SELECT sid, name, device, created, seen FROM sessions WHERE account = ? ORDER BY created', s.account)
        .toArray()
        .map((d) => ({ ...d, me: d.sid === s.sid })),
    }
  }

  /** The wrapped keys: `room` (a new room drops wraps made for the old key), `wrap` for one
   * passkey, `recovery` (proof and wrap) for the recovery code. */
  async vault(s: Session, b: Json): Promise<Response> {
    let recoveryAuth: string | null = null
    if (b.recovery !== undefined) {
      if (!str(b.recovery?.proof, 100) || !str(b.recovery?.blob, 512)) return fail(400, 'bad recovery')
      recoveryAuth = await hash(b.recovery.proof)
    }
    if (b.room !== undefined) {
      if (typeof b.room !== 'string' || !ROOM.test(b.room)) return fail(400, 'bad room')
      const old = this.sql.exec<{ room: string | null }>('SELECT room FROM accounts WHERE id = ?', s.account).one().room
      if (old !== b.room) {
        this.sql.exec('UPDATE accounts SET room = ?, recovery_auth = NULL, recovery_wrap = NULL WHERE id = ?', b.room, s.account)
        this.sql.exec('UPDATE creds SET wrap = NULL WHERE account = ?', s.account)
      }
    }
    if (b.wrap !== undefined) {
      if (!str(b.wrap?.credential, 1400) || !str(b.wrap?.blob, 512)) return fail(400, 'bad wrap')
      const n = this.sql.exec('UPDATE creds SET wrap = ? WHERE id = ? AND account = ?', b.wrap.blob, b.wrap.credential, s.account).rowsWritten
      if (!n) return fail(404, 'no such passkey')
    }
    if (recoveryAuth) this.sql.exec('UPDATE accounts SET recovery_auth = ?, recovery_wrap = ? WHERE id = ?', recoveryAuth, b.recovery.blob, s.account)
    return json(this.account(s))
  }

  async ticket(s: Session, now: number): Promise<Response> {
    const t = rand(24)
    this.sql.exec('INSERT INTO tickets (hash, account, expires) VALUES (?, ?, ?)', await hash(t), s.account, now + L.ticketMs)
    return json({ ticket: t })
  }

  removeCredential(s: Session, id: string): Response {
    const n = Number(this.sql.exec<{ n: number }>('SELECT COUNT(*) AS n FROM creds WHERE account = ?', s.account).one().n)
    if (!this.sql.exec('SELECT 1 FROM creds WHERE id = ? AND account = ?', id, s.account).toArray().length) return fail(404, 'no such passkey')
    if (n <= 1) return fail(409, 'keep at least one passkey (or delete the Bana ID)')
    this.sql.exec('DELETE FROM creds WHERE id = ? AND account = ?', id, s.account)
    return json(this.account(s))
  }

  renameDevice(s: Session, sid: string, b: Json): Response {
    if (b.name !== undefined && !str(b.name, 1024)) return fail(400, 'bad name')
    if (b.device !== undefined && !str(b.device, 64)) return fail(400, 'bad device')
    const n = this.sql.exec('UPDATE sessions SET name = COALESCE(?, name), device = COALESCE(?, device) WHERE sid = ? AND account = ?', b.name ?? null, b.device ?? null, sid, s.account).rowsWritten
    if (!n) return fail(404, 'no such device')
    return json(this.account(s))
  }

  /** Signs a device out (this one, or another from the profile page); another one is also
   * removed from the sync room. */
  async removeDevice(s: Session, sid: string): Promise<Response> {
    const d = this.sql.exec<{ device: string | null }>('SELECT device FROM sessions WHERE sid = ? AND account = ?', sid, s.account).toArray()[0]
    if (!d) return fail(404, 'no such device')
    this.sql.exec('DELETE FROM sessions WHERE sid = ?', sid)
    const room = this.sql.exec<{ room: string | null }>('SELECT room FROM accounts WHERE id = ?', s.account).one().room
    if (sid !== s.sid && d.device && room) await this.room(room).revokeDevice(d.device)
    return json({ ok: true })
  }

  /** Deletes the Bana ID and its sync room from the relay. Notes on the devices stay. */
  async deleteAccount(s: Session): Promise<Response> {
    const room = this.sql.exec<{ room: string | null }>('SELECT room FROM accounts WHERE id = ?', s.account).one().room
    this.ctx.storage.transactionSync(() => {
      this.sql.exec('DELETE FROM sessions WHERE account = ?', s.account)
      this.sql.exec('DELETE FROM creds WHERE account = ?', s.account)
      this.sql.exec('DELETE FROM tickets WHERE account = ?', s.account)
      this.sql.exec('DELETE FROM rate WHERE k = ?', `acct:${s.account}`)
      this.sql.exec('DELETE FROM accounts WHERE id = ?', s.account)
    })
    if (room) await this.room(room).wipe()
    return json({ ok: true, deleted: s.account })
  }

  /** For tests: how many rows mention an account. */
  rowsFor(account: string): number {
    let n = 0
    for (const t of ['accounts', 'creds', 'sessions', 'tickets']) {
      n += Number(this.sql.exec<{ n: number }>(`SELECT COUNT(*) AS n FROM ${t} WHERE ${t === 'accounts' ? 'id' : 'account'} = ?`, account).one().n)
    }
    return n
  }
}
