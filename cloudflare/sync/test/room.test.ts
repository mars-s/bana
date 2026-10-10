// The relay inside workerd (Miniflare) with the real Durable Object: sign-in, invites, backlog
// pages, live broadcast, hibernation, pruning with a snapshot, revoking, and the limits.
import { SELF, env, evictDurableObject, runInDurableObject } from 'cloudflare:test'
import { describe, expect, it } from 'vitest'
import type { Room } from '../src/index'
import { LIMITS } from '../src/limits'

let rooms = 0
/** A fresh room id (32 hex) per test. */
const newRoom = () => (++rooms).toString(16).padStart(32, '0')

const b64 = (b: ArrayBuffer | Uint8Array) => btoa(String.fromCharCode(...new Uint8Array(b instanceof Uint8Array ? b : new Uint8Array(b))))

type Msg = Record<string, any>

class Device {
  keys!: CryptoKeyPair
  id = ''
  static async make(): Promise<Device> {
    const d = new Device()
    d.keys = (await crypto.subtle.generateKey({ name: 'Ed25519' }, true, ['sign', 'verify'])) as CryptoKeyPair
    d.id = b64(await crypto.subtle.exportKey('raw', d.keys.publicKey) as ArrayBuffer)
    return d
  }
  async sign(room: string, nonce: string) {
    return b64(await crypto.subtle.sign({ name: 'Ed25519' }, this.keys.privateKey, new TextEncoder().encode(`bana-sync/hello/v1:${room}:${nonce}`)))
  }
}

class Conn {
  ws!: WebSocket
  inbox: Msg[] = []
  waiters: (() => void)[] = []
  closed: { code: number; reason: string } | null = null
  taken: Msg[] = []
  static async open(room: string): Promise<Conn> {
    const res = await SELF.fetch(`https://relay.test/v1/rooms/${room}`, { headers: { Upgrade: 'websocket' } })
    expect(res.status).toBe(101)
    const c = new Conn()
    c.ws = res.webSocket!
    c.ws.accept()
    c.ws.addEventListener('message', (e) => {
      c.inbox.push(typeof e.data === 'string' && e.data !== 'pong' ? JSON.parse(e.data) : { t: e.data })
      c.waiters.splice(0).forEach((w) => w())
    })
    c.ws.addEventListener('close', (e) => {
      c.closed = { code: e.code, reason: e.reason }
      c.waiters.splice(0).forEach((w) => w())
    })
    return c
  }
  send(m: Msg | string) {
    this.ws.send(typeof m === 'string' ? m : JSON.stringify(m))
  }
  /** The next message of type `t` (earlier ones of other types are skipped). */
  async next(t: string, ms = 2000): Promise<Msg> {
    const end = Date.now() + ms
    for (;;) {
      const i = this.inbox.findIndex((m) => m.t === t)
      if (i >= 0) {
        this.taken = this.inbox.splice(0, i + 1)
        return this.taken[i]
      }
      if (this.closed && t !== 'close') throw new Error(`closed (${this.closed.code} ${this.closed.reason}) waiting for ${t}; inbox ${JSON.stringify(this.inbox)}`)
      if (Date.now() > end) throw new Error(`no ${t}; inbox ${JSON.stringify(this.inbox)}`)
      await new Promise<void>((r) => {
        this.waiters.push(r)
        setTimeout(r, 50)
      })
    }
  }
  async waitClosed(ms = 2000) {
    const end = Date.now() + ms
    while (!this.closed && Date.now() < end) await new Promise((r) => setTimeout(r, 20))
    return this.closed
  }
  /** Everything up to and including the next message of type `t`. */
  async until(t: string): Promise<Msg[]> {
    const before = this.inbox.length
    await this.next(t)
    void before
    return this.taken
  }
  /** Messages of type `t` that arrived (without waiting). */
  got(t: string) {
    return this.inbox.filter((m) => m.t === t)
  }
}

/** Connects and signs in; returns the connection after `synced`. */
async function signIn(room: string, d: Device, extra: Msg = {}): Promise<{ c: Conn; welcome: Msg; synced: Msg }> {
  const c = await Conn.open(room)
  const { nonce } = await c.next('challenge')
  c.send({ t: 'hello', device: d.id, sig: await d.sign(room, nonce), cursor: 0, name: 'c2VhbGVk', ...extra })
  const welcome = await c.next('welcome')
  const synced = await c.next('synced')
  return { c, welcome, synced }
}

async function invite(by: Conn): Promise<string> {
  const secret = crypto.getRandomValues(new Uint8Array(32))
  const hash = b64(await crypto.subtle.digest('SHA-256', secret))
  by.send({ t: 'invite', hash })
  expect((await by.next('invited')).hash).toBe(hash)
  return b64(secret)
}

const stub = (room: string) => env.ROOMS.get(env.ROOMS.idFromName(room))

async function push(c: Conn, data: string, id = crypto.randomUUID().slice(0, 8)) {
  c.send({ t: 'push', id, i: 0, n: 1, data })
  return (await c.next('ok')).seq as number
}

describe('routing', () => {
  it('answers health, refuses bad paths and plain requests', async () => {
    expect((await SELF.fetch('https://relay.test/')).status).toBe(200)
    expect((await SELF.fetch('https://relay.test/v1/rooms/not-a-room', { headers: { Upgrade: 'websocket' } })).status).toBe(404)
    expect((await SELF.fetch(`https://relay.test/v1/rooms/${newRoom()}`)).status).toBe(426)
  })
})

describe('sign-in', () => {
  it('the first device creates the room; others need a one-time invite', async () => {
    const room = newRoom()
    const [mac, phone, stranger, pad] = await Promise.all([Device.make(), Device.make(), Device.make(), Device.make()])
    const a = await signIn(room, mac)
    expect(a.welcome.head).toBe(0)
    expect(a.welcome.devices.map((d: Msg) => d.device)).toEqual([mac.id])
    // No invite: refused.
    const s = await Conn.open(room)
    const { nonce } = await s.next('challenge')
    s.send({ t: 'hello', device: stranger.id, sig: await stranger.sign(room, nonce), cursor: 0 })
    expect((await s.next('error')).code).toBe('not-member')
    expect((await s.waitClosed())?.code).toBe(4001)
    // With an invite: in, and the others hear about it.
    const code = await invite(a.c)
    const b = await signIn(room, phone, { invite: code })
    expect(b.welcome.devices).toHaveLength(2)
    expect((await a.c.next('devices')).devices).toHaveLength(2)
    // The same invite twice: refused.
    const p = await Conn.open(room)
    const n2 = (await p.next('challenge')).nonce
    p.send({ t: 'hello', device: pad.id, sig: await pad.sign(room, n2), cursor: 0, invite: code })
    expect((await p.next('error')).code).toBe('bad-invite')
    expect((await p.waitClosed())?.code).toBe(4001)
  })

  it('refuses a wrong signature, a replayed one, frames before sign-in, and expired invites', async () => {
    const room = newRoom()
    const [mac, pad] = await Promise.all([Device.make(), Device.make()])
    const c = await Conn.open(room)
    const { nonce } = await c.next('challenge')
    c.send({ t: 'hello', device: mac.id, sig: await pad.sign(room, nonce), cursor: 0 })
    expect((await c.next('error')).code).toBe('bad-signature')
    // A signature for another challenge does not work either.
    const d = await Conn.open(room)
    await d.next('challenge')
    d.send({ t: 'hello', device: mac.id, sig: await mac.sign(room, nonce), cursor: 0 })
    expect((await d.next('error')).code).toBe('bad-signature')
    const e = await Conn.open(room)
    await e.next('challenge')
    e.send({ t: 'push', id: 'x', i: 0, n: 1, data: 'AA==' })
    expect((await e.next('error')).code).toBe('not-signed-in')
    // An invite past its day.
    const a = await signIn(room, mac)
    const code = await invite(a.c)
    await runInDurableObject(stub(room), (_r: Room, state) => {
      state.storage.sql.exec('UPDATE invites SET expires = 1')
    })
    const f = await Conn.open(room)
    const n = (await f.next('challenge')).nonce
    f.send({ t: 'hello', device: pad.id, sig: await pad.sign(room, n), cursor: 0, invite: code })
    expect((await f.next('error')).code).toBe('bad-invite')
  })

  it('an invite never creates a room', async () => {
    const room = newRoom()
    const pad = await Device.make()
    const c = await Conn.open(room)
    const n = (await c.next('challenge')).nonce
    c.send({ t: 'hello', device: pad.id, sig: await pad.sign(room, n), cursor: 0, invite: b64(new Uint8Array(32)) })
    expect((await c.next('error')).code).toBe('bad-invite')
  })
})

describe('updates', () => {
  it('stores in order, broadcasts to caught-up devices, and serves the backlog after a cursor', async () => {
    const room = newRoom()
    const [mac, phone, pad] = await Promise.all([Device.make(), Device.make(), Device.make()])
    const a = await signIn(room, mac)
    const b = await signIn(room, phone, { invite: await invite(a.c) })
    expect(await push(a.c, 'AQ==')).toBe(1)
    const u = await b.c.next('update')
    expect(u).toMatchObject({ seq: 1, from: mac.id, n: 1, data: 'AQ==' })
    expect(a.c.got('update')).toEqual([])
    expect(await push(b.c, 'Ag==')).toBe(2)
    expect((await a.c.next('update')).seq).toBe(2)
    expect(await push(a.c, 'Aw==')).toBe(3)
    // A device back with cursor 1 gets 2 and 3, then synced at head 3.
    const c = await signIn(room, pad, { invite: await invite(a.c) })
    expect(c.synced).toEqual({ t: 'synced', head: 3, upto: 3 })
    const d = await Conn.open(room)
    const n = (await d.next('challenge')).nonce
    d.send({ t: 'hello', device: phone.id, sig: await phone.sign(room, n), cursor: 1 })
    await d.next('welcome')
    expect((await d.next('update')).seq).toBe(2)
    expect((await d.next('update')).seq).toBe(3)
    expect(await d.next('synced')).toEqual({ t: 'synced', head: 3, upto: 3 })
  })

  it('pages a long backlog, and live updates wait until the device has caught up', async () => {
    const room = newRoom()
    const [mac, phone] = await Promise.all([Device.make(), Device.make()])
    const a = await signIn(room, mac)
    const code = await invite(a.c)
    const rows = LIMITS.pageRows + 50
    await runInDurableObject(stub(room), (_r: Room, state) => {
      for (let s = 1; s <= rows; s++) state.storage.sql.exec('INSERT INTO updates (seq, sender, mid, i, n, data) VALUES (?, ?, ?, 0, 1, ?)', s, mac.id, `m${s}`, 'AA==')
    })
    await evictDurableObject(stub(room))
    const c = await Conn.open(room)
    const n = (await c.next('challenge')).nonce
    c.send({ t: 'hello', device: phone.id, sig: await phone.sign(room, n), cursor: 0, invite: code })
    await c.next('welcome')
    const page1 = await c.until('synced')
    expect(page1.at(-1)).toEqual({ t: 'synced', head: rows, upto: LIMITS.pageRows })
    expect(page1.filter((m) => m.t === 'update')).toHaveLength(LIMITS.pageRows)
    c.inbox = []
    // A push now is not sent live to the device still catching up; it comes with the next page.
    await push(a.c, 'AQ==')
    c.send({ t: 'pull', after: LIMITS.pageRows })
    const page2 = await c.until('synced')
    expect(page2.at(-1)).toEqual({ t: 'synced', head: rows + 1, upto: rows + 1 })
    const seqs = page2.filter((m) => m.t === 'update').map((u) => u.seq)
    expect(seqs).toEqual(Array.from({ length: 51 }, (_, i) => LIMITS.pageRows + 1 + i))
    // Caught up: now live.
    await push(a.c, 'Ag==')
    expect((await c.next('update')).seq).toBe(rows + 2)
  })

  it('records acknowledgements and tells the others', async () => {
    const room = newRoom()
    const [mac, phone] = await Promise.all([Device.make(), Device.make()])
    const a = await signIn(room, mac)
    const b = await signIn(room, phone, { invite: await invite(a.c) })
    await push(a.c, 'AQ==')
    b.c.send({ t: 'ack', seq: 1, vv: 'c2VhbGVk' })
    expect(await a.c.next('acked')).toEqual({ t: 'acked', device: phone.id, seq: 1, vv: 'c2VhbGVk' })
    // An ack past the head counts up to the head.
    b.c.send({ t: 'ack', seq: 99 })
    expect((await a.c.next('acked')).seq).toBe(1)
  })
})

describe('hibernation', () => {
  it('keeps signed-in sockets working after the object is evicted, and answers ping without it', async () => {
    const room = newRoom()
    const [mac, phone] = await Promise.all([Device.make(), Device.make()])
    const a = await signIn(room, mac)
    const b = await signIn(room, phone, { invite: await invite(a.c) })
    await push(a.c, 'AQ==')
    await b.c.next('update')
    await evictDurableObject(stub(room))
    // State came back from storage and the socket attachments: who is who, and who is live.
    expect(await push(a.c, 'Ag==')).toBe(2)
    expect((await b.c.next('update')).seq).toBe(2)
    a.c.send('ping')
    expect((await a.c.next('pong')).t).toBe('pong')
  })
})

describe('pruning', () => {
  it('replaces updates every device has with a snapshot; new devices start from it', async () => {
    const room = newRoom()
    const [mac, phone, pad] = await Promise.all([Device.make(), Device.make(), Device.make()])
    const a = await signIn(room, mac)
    const b = await signIn(room, phone, { invite: await invite(a.c) })
    for (const d of ['AQ==', 'Ag==', 'Aw==']) await push(a.c, d)
    // The phone has only 2: a snapshot up to 3 is refused.
    b.c.send({ t: 'ack', seq: 2 })
    await a.c.next('acked')
    a.c.send({ t: 'ack', seq: 3 })
    a.c.send({ t: 'snap', upto: 3, id: 's1', i: 0, n: 1, data: 'U05BUA==' })
    expect((await a.c.next('error')).code).toBe('snapshot')
    // Up to 2 in two parts: accepted once whole.
    a.c.send({ t: 'snap', upto: 2, id: 's2', i: 0, n: 2, data: 'UDA=' })
    a.c.send({ t: 'snap', upto: 2, id: 's2', i: 1, n: 2, data: 'UDE=' })
    expect(await a.c.next('pruned')).toEqual({ t: 'pruned', upto: 2 })
    expect(await b.c.next('pruned')).toEqual({ t: 'pruned', upto: 2 })
    const left = await runInDurableObject(stub(room), (_r: Room, s) => s.storage.sql.exec<{ seq: number }>('SELECT seq FROM updates').toArray().map((r) => r.seq))
    expect(left).toEqual([3])
    // A new device: the snapshot parts, then update 3.
    const c = await Conn.open(room)
    const n = (await c.next('challenge')).nonce
    c.send({ t: 'hello', device: pad.id, sig: await pad.sign(room, n), cursor: 0, invite: await invite(a.c) })
    expect((await c.next('welcome')).snapshot).toEqual({ upto: 2, n: 2 })
    expect(await c.next('snapshot')).toEqual({ t: 'snapshot', upto: 2, from: mac.id, i: 0, n: 2, data: 'UDA=' })
    expect((await c.next('snapshot')).i).toBe(1)
    expect((await c.next('update')).seq).toBe(3)
    expect(await c.next('synced')).toEqual({ t: 'synced', head: 3, upto: 3 })
    // After pruning everything, numbering goes on.
    for (const conn of [a.c, b.c, c]) conn.send({ t: 'ack', seq: 3 })
    await new Promise((r) => setTimeout(r, 100))
    a.c.send({ t: 'snap', upto: 3, id: 's3', i: 0, n: 1, data: 'UzM=' })
    await a.c.next('pruned')
    await evictDurableObject(stub(room))
    expect(await push(a.c, 'BA==')).toBe(4)
  })
})

describe('revoking', () => {
  it('closes the device and keeps it out', async () => {
    const room = newRoom()
    const [mac, phone] = await Promise.all([Device.make(), Device.make()])
    const a = await signIn(room, mac)
    const b = await signIn(room, phone, { invite: await invite(a.c) })
    a.c.send({ t: 'revoke', device: phone.id })
    expect((await b.c.next('error')).code).toBe('revoked')
    expect((await b.c.waitClosed())?.code).toBe(4003)
    let list = (await a.c.next('devices')).devices
    if (!list.find((d: Msg) => d.device === phone.id).revoked) list = (await a.c.next('devices')).devices
    expect(list.find((d: Msg) => d.device === phone.id).revoked).toBe(true)
    const c = await Conn.open(room)
    const n = (await c.next('challenge')).nonce
    c.send({ t: 'hello', device: phone.id, sig: await phone.sign(room, n), cursor: 0 })
    expect((await c.next('error')).code).toBe('revoked')
    // Pruning no longer waits for it.
    await push(a.c, 'AQ==')
    a.c.send({ t: 'ack', seq: 1 })
    a.c.send({ t: 'snap', upto: 1, id: 's', i: 0, n: 1, data: 'Uw==' })
    expect((await a.c.next('pruned')).upto).toBe(1)
  })
})

describe('limits', () => {
  it('closes on a frame that is too big, binary frames and floods', async () => {
    const room = newRoom()
    const mac = await Device.make()
    const a = await signIn(room, mac)
    a.c.send('x'.repeat(LIMITS.frame + 1))
    expect((await a.c.waitClosed())?.code).toBe(4008)
    const b = await signIn(room, mac)
    b.c.ws.send(new Uint8Array([1, 2, 3]))
    expect((await b.c.waitClosed())?.code).toBe(1003)
    const c = await signIn(room, mac)
    for (let i = 0; i < LIMITS.burst + 20; i++) c.c.send({ t: 'ack', seq: 0 })
    expect((await c.c.next('error', 5000)).code).toBe('rate')
    expect((await c.c.waitClosed())?.code).toBe(4008)
  })

  it('refuses pushes into a full room, oversized payloads and too many parts', async () => {
    const room = newRoom()
    const mac = await Device.make()
    const a = await signIn(room, mac)
    a.c.send({ t: 'push', id: 'big', i: 0, n: LIMITS.parts + 1, data: 'AA==' })
    expect((await a.c.next('error')).code).toBe('bad-message')
    const b = await signIn(room, mac)
    await runInDurableObject(stub(room), (_r: Room, state) => {
      state.storage.sql.exec('INSERT INTO updates (seq, sender, mid, i, n, data) VALUES (1, ?, ?, 0, 1, ?)', mac.id, 'fill', 'A'.repeat(1024))
    })
    await evictDurableObject(stub(room))
    const saved = LIMITS.roomBytes
    LIMITS.roomBytes = 1500
    try {
      const c = await signIn(room, mac)
      c.c.send({ t: 'push', id: 'x', i: 0, n: 1, data: 'B'.repeat(1000) })
      expect((await c.c.next('error')).code).toBe('full')
      expect(c.c.closed).toBeNull()
    } finally {
      LIMITS.roomBytes = saved
    }
    void b
  })

  it('caps the devices in a room and the sockets on it', async () => {
    const room = newRoom()
    const mac = await Device.make()
    const a = await signIn(room, mac)
    await runInDurableObject(stub(room), (_r: Room, state) => {
      for (let i = 1; i < LIMITS.devices; i++) state.storage.sql.exec('INSERT INTO devices (device) VALUES (?)', `d${i}`)
    })
    const more = await Device.make()
    const c = await Conn.open(room)
    const n = (await c.next('challenge')).nonce
    c.send({ t: 'hello', device: more.id, sig: await more.sign(room, n), cursor: 0, invite: await invite(a.c) })
    expect((await c.next('error')).msg).toMatch(/as many devices/)
    const open: Conn[] = []
    for (let i = 0; i < LIMITS.sockets + 2; i++) {
      const res = await SELF.fetch(`https://relay.test/v1/rooms/${room}`, { headers: { Upgrade: 'websocket' } })
      if (res.status === 503) return
      res.webSocket?.accept()
      void open
    }
    throw new Error('no socket limit')
  })
})
