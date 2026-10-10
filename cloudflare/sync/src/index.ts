// The Bana sync relay (docs/sync.md). A Worker routes `GET /v1/rooms/<room>` (a WebSocket) to
// one Durable Object per room. The room keeps encrypted updates in order, hands each device what it
// has not seen, broadcasts new ones to devices that are caught up, and replaces old updates with an
// encrypted snapshot once every device has them. It never sees a key or a note: payloads, device
// names and versions are ciphertext made on the devices (crates/sync).
//
// Free plan: SQLite-backed Durable Objects and the WebSocket Hibernation API (an idle connection
// costs nothing; "ping" is answered without waking the object). One stored row per push.
//
// Protocol: JSON text frames, the same as crates/sync/src/proto.rs.

import { DurableObject } from 'cloudflare:workers'
import { CLOSE_AUTH, CLOSE_LIMIT, CLOSE_REVOKED, LIMITS } from './limits'

export interface Env {
  ROOMS: DurableObjectNamespace<Room>
}

const ROOM = /^[0-9a-f]{32}$/

export default {
  async fetch(req: Request, env: Env): Promise<Response> {
    const url = new URL(req.url)
    if (url.pathname === '/' || url.pathname === '/health') return new Response('Bana sync relay: it stores encrypted updates and cannot read them.\n')
    const m = url.pathname.match(/^\/v1\/rooms\/([^/]+)$/)
    if (!m || !ROOM.test(m[1])) return new Response('not found\n', { status: 404 })
    if (req.headers.get('Upgrade')?.toLowerCase() !== 'websocket') return new Response('expected a WebSocket\n', { status: 426 })
    return env.ROOMS.get(env.ROOMS.idFromName(m[1])).fetch(req)
  },
} satisfies ExportedHandler<Env>

/** What a socket remembers across hibernation. */
interface Att {
  nonce: string
  device: string | null
  live: boolean
  tokens: number
  at: number
}

type Msg = Record<string, unknown>

const enc = new TextEncoder()

function b64d(s: string): Uint8Array | null {
  try {
    return Uint8Array.from(atob(s), (c) => c.charCodeAt(0))
  } catch {
    return null
  }
}

function b64e(b: ArrayBuffer | Uint8Array): string {
  const u = b instanceof Uint8Array ? b : new Uint8Array(b)
  let s = ''
  for (const x of u) s += String.fromCharCode(x)
  return btoa(s)
}

const str = (v: unknown, max = LIMITS.data): v is string => typeof v === 'string' && v.length <= max
const int = (v: unknown): v is number => typeof v === 'number' && Number.isSafeInteger(v) && v >= 0

async function verifyHello(device: string, room: string, nonce: string, sig: string): Promise<boolean> {
  const pk = b64d(device)
  const s = b64d(sig)
  if (!pk || pk.length !== 32 || !s || s.length !== 64) return false
  try {
    const key = await crypto.subtle.importKey('raw', pk, { name: 'Ed25519' }, false, ['verify'])
    return await crypto.subtle.verify({ name: 'Ed25519' }, key, s, enc.encode(`bana-sync/hello/v1:${room}:${nonce}`))
  } catch {
    return false
  }
}

export class Room extends DurableObject<Env> {
  sql: SqlStorage
  /** Cached between events (recomputed after waking from hibernation). */
  #head: number | null = null
  #bytes: number | null = null

  constructor(ctx: DurableObjectState, env: Env) {
    super(ctx, env)
    this.sql = ctx.storage.sql
    this.sql.exec(`
      CREATE TABLE IF NOT EXISTS meta (k TEXT PRIMARY KEY, v TEXT);
      CREATE TABLE IF NOT EXISTS devices (device TEXT PRIMARY KEY, name TEXT, cursor INTEGER NOT NULL DEFAULT 0, vv TEXT, seen INTEGER NOT NULL DEFAULT 0, revoked INTEGER NOT NULL DEFAULT 0);
      CREATE TABLE IF NOT EXISTS updates (seq INTEGER PRIMARY KEY, sender TEXT NOT NULL, mid TEXT NOT NULL, i INTEGER NOT NULL, n INTEGER NOT NULL, data TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS snapshot (i INTEGER PRIMARY KEY, n INTEGER NOT NULL, upto INTEGER NOT NULL, sender TEXT NOT NULL, data TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS snap_parts (device TEXT NOT NULL, id TEXT NOT NULL, i INTEGER NOT NULL, n INTEGER NOT NULL, upto INTEGER NOT NULL, data TEXT NOT NULL, PRIMARY KEY (device, id, i));
      CREATE TABLE IF NOT EXISTS invites (hash TEXT PRIMARY KEY, expires INTEGER NOT NULL);
    `)
    // Keep-alive pings are answered by the runtime: they never wake the object.
    ctx.setWebSocketAutoResponse(new WebSocketRequestResponsePair('ping', 'pong'))
  }

  // ---- state ------------------------------------------------------------------------------

  room(): string {
    return (this.sql.exec<{ v: string }>(`SELECT v FROM meta WHERE k = 'room'`).toArray()[0]?.v as string) ?? ''
  }

  snapUpto(): number {
    return Number(this.sql.exec<{ u: number }>('SELECT MAX(upto) AS u FROM snapshot').one().u ?? 0)
  }

  head(): number {
    this.#head ??= Math.max(Number(this.sql.exec<{ s: number }>('SELECT MAX(seq) AS s FROM updates').one().s ?? 0), this.snapUpto())
    return this.#head
  }

  bytes(): number {
    this.#bytes ??=
      Number(this.sql.exec<{ b: number }>('SELECT COALESCE(SUM(length(data)), 0) AS b FROM updates').one().b) +
      Number(this.sql.exec<{ b: number }>('SELECT COALESCE(SUM(length(data)), 0) AS b FROM snapshot').one().b)
    return this.#bytes
  }

  devices(): Msg[] {
    return this.sql
      .exec<{ device: string; name: string | null; cursor: number; vv: string | null; seen: number; revoked: number }>('SELECT device, name, cursor, vv, seen, revoked FROM devices ORDER BY rowid')
      .toArray()
      .map((d) => ({ device: d.device, name: d.name, cursor: d.cursor, vv: d.vv, seen: d.seen, revoked: !!d.revoked }))
  }

  sockets(filter: (a: Att) => boolean, except?: WebSocket): WebSocket[] {
    return this.ctx.getWebSockets().filter((w) => w !== except && filter(w.deserializeAttachment() as Att))
  }

  send(ws: WebSocket, m: Msg) {
    try {
      ws.send(JSON.stringify(m))
    } catch {
      /* gone */
    }
  }

  broadcast(m: Msg, except?: WebSocket, liveOnly = false) {
    const text = JSON.stringify(m)
    for (const w of this.sockets((a) => !!a.device && (a.live || !liveOnly), except)) {
      try {
        w.send(text)
      } catch {
        /* gone */
      }
    }
  }

  fail(ws: WebSocket, code: number, err: string, msg: string) {
    this.send(ws, { t: 'error', code: err, msg })
    try {
      ws.close(code, msg.slice(0, 120))
    } catch {
      /* already closed */
    }
  }

  // ---- connections ------------------------------------------------------------------------

  async fetch(req: Request): Promise<Response> {
    const room = new URL(req.url).pathname.split('/').pop() ?? ''
    if (!ROOM.test(room)) return new Response('not found\n', { status: 404 })
    if (this.ctx.getWebSockets().length >= LIMITS.sockets) return new Response('too many connections\n', { status: 503 })
    if (!this.room()) this.sql.exec(`INSERT OR REPLACE INTO meta (k, v) VALUES ('room', ?)`, room)
    const [client, server] = Object.values(new WebSocketPair())
    this.ctx.acceptWebSocket(server)
    const nonce = b64e(crypto.getRandomValues(new Uint8Array(32)))
    const att: Att = { nonce, device: null, live: false, tokens: LIMITS.burst, at: Date.now() }
    server.serializeAttachment(att)
    this.send(server, { t: 'challenge', nonce })
    return new Response(null, { status: 101, webSocket: client })
  }

  async webSocketMessage(ws: WebSocket, raw: string | ArrayBuffer) {
    const a = ws.deserializeAttachment() as Att
    if (typeof raw !== 'string') return this.fail(ws, 1003, 'bad-message', 'text frames only')
    if (raw.length > LIMITS.frame) return this.fail(ws, CLOSE_LIMIT, 'too-big', 'frame too big')
    const now = Date.now()
    a.tokens = Math.min(LIMITS.burst, a.tokens + ((now - a.at) * LIMITS.perSecond) / 1000)
    a.at = now
    if (a.tokens < 1) return this.fail(ws, CLOSE_LIMIT, 'rate', 'too many messages; slow down')
    a.tokens -= 1
    ws.serializeAttachment(a)
    let m: Msg
    try {
      m = JSON.parse(raw)
    } catch {
      return this.fail(ws, 1003, 'bad-message', 'not JSON')
    }
    if (!a.device) {
      if (m.t !== 'hello') return this.fail(ws, CLOSE_AUTH, 'not-signed-in', 'sign in first')
      return this.hello(ws, a, m, now)
    }
    // A device revoked while connected is closed at once; this catches a frame already on its way.
    const me = this.sql.exec<{ revoked: number }>('SELECT revoked FROM devices WHERE device = ?', a.device).toArray()[0]
    if (!me || me.revoked) return this.fail(ws, CLOSE_REVOKED, 'revoked', 'this device was removed')
    switch (m.t) {
      case 'invite':
        return this.invite(ws, m, now)
      case 'push':
        return this.push(ws, a.device, m)
      case 'ack':
        return this.ack(ws, a.device, m)
      case 'pull':
        if (!int(m.after)) return this.fail(ws, 1003, 'bad-message', 'pull needs after')
        return this.page(ws, a, m.after)
      case 'snap':
        return this.snap(ws, a.device, m)
      case 'revoke':
        return this.revoke(m)
      case 'hello':
        return
      default:
        return this.fail(ws, 1003, 'bad-message', 'unknown message')
    }
  }

  async webSocketClose(ws: WebSocket, code: number) {
    try {
      ws.close(code === 1005 ? 1000 : code, 'bye')
    } catch {
      /* closed */
    }
  }

  async webSocketError() {}

  // ---- messages ---------------------------------------------------------------------------

  async hello(ws: WebSocket, a: Att, m: Msg, now: number) {
    const { device, sig, cursor, invite, name } = m
    if (!str(device, 64) || !str(sig, 128) || !int(cursor) || (invite !== undefined && !str(invite, 64)) || (name !== undefined && name !== null && !str(name, 1024))) {
      return this.fail(ws, CLOSE_AUTH, 'bad-message', 'bad hello')
    }
    if (!(await verifyHello(device, this.room(), a.nonce, sig))) return this.fail(ws, CLOSE_AUTH, 'bad-signature', 'the signature does not match')
    const row = this.sql.exec<{ revoked: number }>('SELECT revoked FROM devices WHERE device = ?', device).toArray()[0]
    let joined = false
    if (row?.revoked) return this.fail(ws, CLOSE_REVOKED, 'revoked', 'this device was removed')
    if (!row) {
      let valid = false
      if (typeof invite === 'string') {
        const secret = b64d(invite)
        if (secret) {
          const hash = b64e(await crypto.subtle.digest('SHA-256', secret))
          const inv = this.sql.exec<{ expires: number }>('SELECT expires FROM invites WHERE hash = ?', hash).toArray()[0]
          this.sql.exec('DELETE FROM invites WHERE hash = ?', hash)
          valid = !!inv && inv.expires > now
        }
        if (!valid) return this.fail(ws, CLOSE_AUTH, 'bad-invite', 'the invite is not valid (used, expired or unknown)')
      }
      const count = Number(this.sql.exec<{ c: number }>('SELECT COUNT(*) AS c FROM devices').one().c)
      // The first device creates the room; everyone after needs an invite.
      if (!valid && count > 0) return this.fail(ws, CLOSE_AUTH, 'not-member', 'this device is not in the room')
      if (Number(this.sql.exec<{ c: number }>('SELECT COUNT(*) AS c FROM devices WHERE revoked = 0').one().c) >= LIMITS.devices) {
        return this.fail(ws, CLOSE_AUTH, 'not-member', 'the room has as many devices as it can hold; remove one first')
      }
      this.sql.exec('INSERT INTO devices (device) VALUES (?)', device)
      joined = true
    }
    this.sql.exec('UPDATE devices SET seen = ?, name = COALESCE(?, name) WHERE device = ?', now, typeof name === 'string' ? name : null, device)
    a.device = device
    ws.serializeAttachment(a)
    const n = Number(this.sql.exec<{ c: number }>('SELECT COUNT(*) AS c FROM snapshot').one().c)
    this.send(ws, { t: 'welcome', head: this.head(), devices: this.devices(), snapshot: n ? { upto: this.snapUpto(), n } : null })
    this.page(ws, a, cursor)
    if (joined) this.broadcast({ t: 'devices', devices: this.devices() }, ws)
  }

  /** The backlog after `after`: the snapshot when `after` is older than it, then a page of updates. */
  page(ws: WebSocket, a: Att, after: number) {
    const upto0 = this.snapUpto()
    if (after < upto0) {
      for (const p of this.sql.exec<{ i: number; n: number; upto: number; sender: string; data: string }>('SELECT i, n, upto, sender, data FROM snapshot ORDER BY i')) {
        this.send(ws, { t: 'snapshot', upto: p.upto, from: p.sender, i: p.i, n: p.n, data: p.data })
      }
      after = upto0
    }
    let upto = after
    let bytes = 0
    for (const r of this.sql.exec<{ seq: number; sender: string; mid: string; i: number; n: number; data: string }>('SELECT seq, sender, mid, i, n, data FROM updates WHERE seq > ? ORDER BY seq LIMIT ?', after, LIMITS.pageRows)) {
      if (bytes > 0 && bytes + r.data.length > LIMITS.pageBytes) break
      this.send(ws, { t: 'update', seq: r.seq, from: r.sender, id: r.mid, i: r.i, n: r.n, data: r.data })
      bytes += r.data.length
      upto = r.seq
    }
    const head = this.head()
    this.send(ws, { t: 'synced', head, upto })
    // Caught up: from now on new updates are sent as they come.
    a.live = upto >= head
    ws.serializeAttachment(a)
  }

  invite(ws: WebSocket, m: Msg, now: number) {
    if (!str(m.hash, 64)) return this.fail(ws, 1003, 'bad-message', 'invite needs a hash')
    this.sql.exec('DELETE FROM invites WHERE expires <= ?', now)
    if (Number(this.sql.exec<{ c: number }>('SELECT COUNT(*) AS c FROM invites').one().c) >= LIMITS.invites) {
      return this.send(ws, { t: 'error', code: 'invites', msg: 'too many open invites; wait for one to expire' })
    }
    this.sql.exec('INSERT OR REPLACE INTO invites (hash, expires) VALUES (?, ?)', m.hash, now + LIMITS.inviteMs)
    this.send(ws, { t: 'invited', hash: m.hash })
  }

  push(ws: WebSocket, device: string, m: Msg) {
    const { id, i, n, data } = m
    if (!str(id, 64) || !int(i) || !int(n) || n < 1 || n > LIMITS.parts || i >= n || !str(data)) return this.fail(ws, 1003, 'bad-message', 'bad push')
    if (this.bytes() + data.length > LIMITS.roomBytes) {
      return this.send(ws, { t: 'error', code: 'full', msg: 'the relay room is full: remove devices that no longer sync so old updates can be dropped' })
    }
    const seq = this.head() + 1
    this.sql.exec('INSERT INTO updates (seq, sender, mid, i, n, data) VALUES (?, ?, ?, ?, ?, ?)', seq, device, id, i, n, data)
    this.#head = seq
    this.#bytes = this.bytes() + data.length
    this.send(ws, { t: 'ok', id, i, seq })
    this.broadcast({ t: 'update', seq, from: device, id, i, n, data }, ws, true)
  }

  ack(ws: WebSocket, device: string, m: Msg) {
    const { seq, vv } = m
    if (!int(seq) || (vv !== undefined && vv !== null && !str(vv, 64 * 1024))) return this.fail(ws, 1003, 'bad-message', 'bad ack')
    const head = this.head()
    this.sql.exec('UPDATE devices SET cursor = MAX(cursor, ?), vv = COALESCE(?, vv) WHERE device = ?', Math.min(seq, head), typeof vv === 'string' ? vv : null, device)
    const cursor = this.sql.exec<{ cursor: number }>('SELECT cursor FROM devices WHERE device = ?', device).one().cursor
    this.broadcast({ t: 'acked', device, seq: cursor, vv: typeof vv === 'string' ? vv : null }, ws)
  }

  snap(ws: WebSocket, device: string, m: Msg) {
    const { upto, id, i, n, data } = m
    if (!int(upto) || !str(id, 64) || !int(i) || !int(n) || n < 1 || n > LIMITS.parts || i >= n || !str(data)) return this.fail(ws, 1003, 'bad-message', 'bad snap')
    const min = Number(this.sql.exec<{ m: number }>('SELECT MIN(cursor) AS m FROM devices WHERE revoked = 0').one().m ?? 0)
    if (upto > min || upto <= this.snapUpto() || upto > this.head()) {
      return this.send(ws, { t: 'error', code: 'snapshot', msg: 'not every device has that yet' })
    }
    // One snapshot in the making per device.
    this.sql.exec('DELETE FROM snap_parts WHERE device = ? AND id != ?', device, id)
    this.sql.exec('INSERT OR REPLACE INTO snap_parts (device, id, i, n, upto, data) VALUES (?, ?, ?, ?, ?, ?)', device, id, i, n, upto, data)
    const have = Number(this.sql.exec<{ c: number }>('SELECT COUNT(*) AS c FROM snap_parts WHERE device = ? AND id = ?', device, id).one().c)
    if (have < n) return
    this.ctx.storage.transactionSync(() => {
      this.sql.exec('DELETE FROM snapshot')
      this.sql.exec('INSERT INTO snapshot (i, n, upto, sender, data) SELECT i, n, upto, device, data FROM snap_parts WHERE device = ? AND id = ?', device, id)
      this.sql.exec('DELETE FROM snap_parts WHERE device = ?', device)
      this.sql.exec('DELETE FROM updates WHERE seq <= ?', upto)
    })
    this.#bytes = null
    this.#head = null
    this.broadcast({ t: 'pruned', upto })
  }

  revoke(m: Msg) {
    if (!str(m.device, 64)) return
    this.sql.exec('UPDATE devices SET revoked = 1 WHERE device = ?', m.device)
    for (const w of this.sockets((a) => a.device === m.device)) this.fail(w, CLOSE_REVOKED, 'revoked', 'this device was removed')
    this.broadcast({ t: 'devices', devices: this.devices() })
  }
}
