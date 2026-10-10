// Bana ID inside workerd with the real Durable Objects: registration and sign-in checked against a
// software authenticator with PRF, the one-time code exchange, replays refused, rate limits, the
// wrapped keys, recovery, devices and account deletion wiping the relay's data.
import { SELF, env, runInDurableObject } from 'cloudflare:test'
import { describe, expect, it } from 'vitest'
import type { Accounts } from '../src/id'
import { ID_LIMITS } from '../src/limits'
import { SoftAuthenticator, appKeys, ceremony, dec, enc, type Post } from './authenticator'

const ORIGIN = 'https://relay.test'
const RET = 'http://127.0.0.1:4567/auth/callback'
let ips = 0

/** A client from its own IP address (rate limits are per address). */
function client(ip = `10.0.0.${++ips}`) {
  const call = async (method: string, path: string, body?: unknown, token?: string) => {
    const headers: Record<string, string> = { 'cf-connecting-ip': ip, 'content-type': 'application/json' }
    if (token) headers.authorization = `Bearer ${token}`
    const r = await SELF.fetch(`${ORIGIN}/id/api/${path}`, { method, headers, body: body === undefined ? undefined : JSON.stringify(body) })
    return { status: r.status, body: (await r.json()) as any }
  }
  const post: Post = (path, body) => call('POST', path, body)
  return { call, post }
}

/** Creates a Bana ID with a fresh authenticator: answers everything the app would hold. */
async function create(auth = new SoftAuthenticator(), c = client()) {
  const app = await appKeys()
  const fin = await ceremony(c.post, auth, ORIGIN, { mode: 'create', ret: RET, app: app.pub, name: 'Avi' })
  expect(fin.status, JSON.stringify(fin.body)).toBe(200)
  expect(fin.body.ret).toBe(RET)
  const ex = await c.post('exchange', { code: fin.body.code })
  expect(ex.status, JSON.stringify(ex.body)).toBe(200)
  return { auth, c, app, code: fin.body.code as string, ex: ex.body }
}

const accounts = () => env.ACCOUNTS.get(env.ACCOUNTS.idFromName('accounts-v1'))
// (Typed loosely: the RPC stub's full type is too deep for tsc here.)
const room = (id: string): any => env.ROOMS.get(env.ROOMS.idFromName(id))
const inRoom = <T,>(r: string, f: (o: any) => T): Promise<T> => (runInDurableObject as any)(room(r), f)
let rooms = 0
const newRoom = () => `ab${(++rooms).toString(16).padStart(30, '0')}`

describe('Bana ID', () => {
  it('serves the passkey page with a strict content security policy', async () => {
    const r = await SELF.fetch(`${ORIGIN}/id?mode=create`)
    expect(r.status).toBe(200)
    expect(r.headers.get('content-security-policy')).toMatch(/script-src 'nonce-/)
    const html = await r.text()
    expect(html).toContain('navigator.credentials.create')
    expect(html).not.toContain('mode=create')
  })

  it('creates a Bana ID, hands back the PRF output masked for the app only, and keeps wraps', async () => {
    const { auth, c, app, ex } = await create()
    expect(ex.mode).toBe('create')
    expect(ex.session).toMatch(/^[A-Za-z0-9_-]{40,}$/)
    // The app unmasks exactly the passkey's PRF output; the relay only ever carried it masked.
    const want = await auth.prfOf(auth.creds[0], dec((await c.post('options', { mode: 'signin', ret: RET, app: app.pub })).body.prfSalt))
    expect([...(await app.unmask(ex.page, ex.masked))]).toEqual([...want])
    expect(ex.masked).not.toBe(enc(want))
    // The app stores the room, a wrap for this passkey and the recovery wrap.
    const r = newRoom()
    const v = await c.call('PUT', 'vault', { room: r, wrap: { credential: ex.credential, blob: 'AQID-wrapped' }, recovery: { proof: 'proof-of-code', blob: 'recovery-wrapped' } }, ex.session)
    expect(v.status, JSON.stringify(v.body)).toBe(200)
    expect(v.body.room).toBe(r)
    expect(v.body.recovery).toBe(true)
    expect(v.body.credentials).toEqual([expect.objectContaining({ id: ex.credential, unlocks: true })])
    expect(v.body.devices).toEqual([expect.objectContaining({ sid: ex.sid, me: true })])
    // Nothing secret is stored in the clear: the recovery proof only as a hash.
    const stored = await runInDurableObject(accounts(), (o: Accounts) => JSON.stringify(o.sql.exec('SELECT * FROM accounts').toArray()))
    expect(stored).not.toContain('proof-of-code')
  })

  it('signs in on a second device with the same passkey: the wrap and a room invite come back', async () => {
    const { auth, ex } = await create()
    const a = client()
    const r = newRoom()
    await a.call('PUT', 'vault', { room: r, wrap: { credential: ex.credential, blob: 'the-wrap' } }, ex.session)
    // Device B: a new app key, the same passkey.
    const b = client()
    const appB = await appKeys()
    const fin = await ceremony(b.post, auth, ORIGIN, { mode: 'signin', ret: 'bana://auth/callback', app: appB.pub })
    expect(fin.status, JSON.stringify(fin.body)).toBe(200)
    expect(fin.body.ret).toBe('bana://auth/callback')
    const got = (await b.post('exchange', { code: fin.body.code })).body
    expect(got).toMatchObject({ mode: 'signin', account: ex.account, credential: ex.credential, wrap: 'the-wrap', room: r })
    expect(got.session).not.toBe(ex.session)
    expect([...(await appB.unmask(got.page, got.masked))]).toEqual([...(await auth.prfOf(auth.creds[0], dec((await b.post('options', { mode: 'signin', ret: RET, app: appB.pub })).body.prfSalt)))])
    // The invite is waiting in the room, as a hash of the secret only B was given.
    const hashes = await inRoom(r, (o: any) => o.sql.exec('SELECT hash FROM invites').toArray().map((x: any) => x.hash))
    const secret = Uint8Array.from(atob(got.invite), (ch) => ch.charCodeAt(0))
    expect(hashes).toContain(btoa(String.fromCharCode(...new Uint8Array(await crypto.subtle.digest('SHA-256', secret)))))
    const acct = (await b.call('GET', 'account', undefined, got.session)).body
    expect(acct.devices).toHaveLength(2)
  })

  it('a code works once, a challenge once, and an expired or unknown code never', async () => {
    const { c, code } = await create()
    const again = await c.post('exchange', { code })
    expect(again.status).toBe(400)
    expect(again.body.error).toMatch(/used or has expired/)
    expect((await c.post('exchange', { code: 'made-up-code' })).status).toBe(400)
    // Replaying a finished ceremony (same challenge) is refused.
    const auth = new SoftAuthenticator()
    const app = await appKeys()
    const o = await c.post('options', { mode: 'create', ret: RET, app: app.pub })
    const r = await auth.create(o.body.publicKey, ORIGIN, dec(o.body.prfSalt))
    const body = { challengeId: o.body.challengeId, id: r.id, response: r.response }
    expect((await c.post('finish', body)).status).toBe(200)
    expect((await c.post('finish', body)).body.error).toMatch(/expired; start again/)
  })

  it('refuses another origin, another relying party, a changed signature and a bad return address', async () => {
    const c = client()
    const app = await appKeys()
    const { auth, ex } = await create()
    // Signed for another site.
    const evil = await ceremony(c.post, auth, 'https://evil.test', { mode: 'signin', ret: RET, app: app.pub })
    expect(evil.body.error).toMatch(/wrong origin/)
    // A passkey made for another relying party id.
    const other = new SoftAuthenticator()
    const o = await c.post('options', { mode: 'create', ret: RET, app: app.pub })
    const r = await other.create({ ...o.body.publicKey, rp: { id: 'evil.test' } }, ORIGIN, dec(o.body.prfSalt))
    expect((await c.post('finish', { challengeId: o.body.challengeId, id: r.id, response: r.response })).body.error).toMatch(/relying party/)
    // A signature changed on the way.
    const o2 = await c.post('options', { mode: 'signin', ret: RET, app: app.pub })
    const g = await auth.get(o2.body.publicKey, ORIGIN, dec(o2.body.prfSalt))
    const sig = dec(g.response.signature)
    sig[sig.length - 1] ^= 1
    expect((await c.post('finish', { challengeId: o2.body.challengeId, id: g.id, response: { ...g.response, signature: enc(sig) } })).status).toBe(400)
    // Codes go only to the app (its scheme or a loopback port), never to a web site.
    for (const ret of ['https://evil.test/auth/callback', 'http://evil.test:80/auth/callback', 'bana://auth/x?y=1', 'javascript:alert(1)']) {
      expect((await c.post('options', { mode: 'signin', ret, app: app.pub })).body.error).toMatch(/return address/)
    }
    // A passkey that is not on this relay.
    const stranger = new SoftAuthenticator()
    stranger.creds = auth.creds.map((x) => ({ ...x, id: crypto.getRandomValues(new Uint8Array(16)) }))
    expect((await ceremony(c.post, stranger, ORIGIN, { mode: 'signin', ret: RET, app: app.pub })).body.error).toMatch(/not a Bana ID/)
    expect(ex.account).toBeTruthy()
  })

  it('a counter that goes back looks cloned and is refused', async () => {
    const { auth, c, app } = await create()
    expect((await ceremony(c.post, auth, ORIGIN, { mode: 'signin', ret: RET, app: app.pub })).status).toBe(200)
    auth.creds[0].counter = 0
    expect((await ceremony(c.post, auth, ORIGIN, { mode: 'signin', ret: RET, app: app.pub })).body.error).toMatch(/cloned/)
  })

  it('a platform without PRF still signs in, with no masked output', async () => {
    const auth = new SoftAuthenticator()
    auth.prf = false
    const { ex } = await create(auth)
    expect(ex.masked).toBeNull()
    expect(ex.page).toBeNull()
  })

  it('rate limits sign-in steps, new accounts and recovery tries per address', async () => {
    const c = client()
    const app = await appKeys()
    let last = 0
    for (let i = 0; i <= ID_LIMITS.auth.burst; i++) last = (await c.post('options', { mode: 'signin', ret: RET, app: app.pub })).status
    expect(last).toBe(429)
    const d = client()
    const statuses: number[] = []
    for (let i = 0; i <= ID_LIMITS.create.burst; i++) statuses.push((await d.post('options', { mode: 'create', ret: RET, app: app.pub })).status)
    expect(statuses.slice(0, ID_LIMITS.create.burst).every((s) => s === 200)).toBe(true)
    expect(statuses.at(-1)).toBe(429)
    const e = client()
    const tries: number[] = []
    for (let i = 0; i <= ID_LIMITS.recover.burst; i++) tries.push((await e.post('recover', { proof: `wrong-${i}` })).status)
    expect(tries.slice(0, -1).every((s) => s === 400)).toBe(true)
    expect(tries.at(-1)).toBe(429)
  })

  it('recovers with the recovery code proof, and only with it', async () => {
    const { c, ex } = await create()
    const r = newRoom()
    await c.call('PUT', 'vault', { room: r, recovery: { proof: 'the-right-proof', blob: 'rec-wrap' } }, ex.session)
    const nc = client()
    expect((await nc.post('recover', { proof: 'a-wrong-proof' })).status).toBe(400)
    const got = (await nc.post('recover', { proof: 'the-right-proof' })).body
    expect(got).toMatchObject({ mode: 'recover', account: ex.account, recoveryWrap: 'rec-wrap', room: r })
    expect(got.invite).toBeTruthy()
    expect((await nc.call('GET', 'account', undefined, got.session)).status).toBe(200)
    // A new room (sync set up again) drops wraps made for the old key.
    const v = (await c.call('PUT', 'vault', { room: newRoom() }, ex.session)).body
    expect(v.recovery).toBe(false)
  })

  it('adds a second passkey with a ticket, removes one, and keeps at least one', async () => {
    const { auth, c, app, ex } = await create()
    const t = (await c.call('POST', 'ticket', undefined, ex.session)).body.ticket
    expect((await c.post('options', { mode: 'add', ret: RET, app: app.pub })).body.error).toMatch(/ticket/)
    const fin = await ceremony(c.post, auth, ORIGIN, { mode: 'add', ret: RET, app: app.pub, ticket: t })
    expect(fin.status, JSON.stringify(fin.body)).toBe(200)
    const added = (await c.post('exchange', { code: fin.body.code })).body
    expect(added.mode).toBe('add')
    expect(added.session).toBeUndefined()
    expect(added.masked).toBeTruthy()
    // The ticket worked once.
    expect((await c.post('options', { mode: 'add', ret: RET, app: app.pub, ticket: t })).body.error).toMatch(/expired/)
    let acct = (await c.call('GET', 'account', undefined, ex.session)).body
    expect(acct.credentials).toHaveLength(2)
    acct = (await c.call('DELETE', `credentials/${ex.credential}`, undefined, ex.session)).body
    expect(acct.credentials.map((x: any) => x.id)).toEqual([added.credential])
    expect((await c.call('DELETE', `credentials/${added.credential}`, undefined, ex.session)).status).toBe(409)
  })

  it('renames and removes devices; a removed device is shut out of the room; signing out ends the session', async () => {
    const { auth, c, ex } = await create()
    const r = newRoom()
    await c.call('PUT', 'vault', { room: r }, ex.session)
    const b = client()
    const appB = await appKeys()
    const got = (await b.post('exchange', { code: (await ceremony(b.post, auth, ORIGIN, { mode: 'signin', ret: RET, app: appB.pub })).body.code })).body
    await b.call('PATCH', `devices/${got.sid}`, { name: 'sealed-name-b', device: 'device-b-id' }, got.session)
    const renamed = (await c.call('PATCH', `devices/${got.sid}`, { name: 'sealed-new-name' }, ex.session)).body
    expect(renamed.devices.find((d: any) => d.sid === got.sid)).toMatchObject({ name: 'sealed-new-name', device: 'device-b-id' })
    await inRoom(r, (o: any) => o.sql.exec("INSERT INTO devices (device) VALUES ('device-b-id')"))
    expect((await c.call('DELETE', `devices/${got.sid}`, undefined, ex.session)).status).toBe(200)
    expect((await b.call('GET', 'account', undefined, got.session)).status).toBe(401)
    const revoked = await inRoom(r, (o: any) => o.sql.exec("SELECT revoked FROM devices WHERE device = 'device-b-id'").one().revoked)
    expect(revoked).toBe(1)
    // Signing out this device.
    expect((await c.call('DELETE', `devices/${ex.sid}`, undefined, ex.session)).status).toBe(200)
    expect((await c.call('GET', 'account', undefined, ex.session)).status).toBe(401)
  })

  it('deleting a Bana ID wipes its rows and its sync room from the relay', async () => {
    const { c, ex } = await create()
    const r = newRoom()
    await c.call('PUT', 'vault', { room: r, recovery: { proof: 'p-del', blob: 'b' } }, ex.session)
    await inRoom(r, (o: any) => {
      o.sql.exec("INSERT INTO devices (device) VALUES ('d1')")
      o.sql.exec("INSERT INTO updates (seq, sender, mid, i, n, data) VALUES (1, 'd1', 'm', 0, 1, 'ciphertext')")
    })
    expect(await room(r).rows()).toBeGreaterThan(0)
    expect(await runInDurableObject(accounts(), (o: Accounts) => o.rowsFor(ex.account))).toBeGreaterThan(0)
    const del = await c.call('DELETE', 'account', undefined, ex.session)
    expect(del.body).toEqual({ ok: true, deleted: ex.account })
    expect(await runInDurableObject(accounts(), (o: Accounts) => o.rowsFor(ex.account))).toBe(0)
    expect(await room(r).rows()).toBe(0)
    expect((await c.call('GET', 'account', undefined, ex.session)).status).toBe(401)
    expect((await client().post('recover', { proof: 'p-del' })).status).toBe(400)
  })

  it('refuses calls without a session and junk bodies', async () => {
    const c = client()
    expect((await c.call('GET', 'account')).status).toBe(401)
    expect((await c.call('GET', 'account', undefined, 'x'.repeat(43))).status).toBe(401)
    const r = await SELF.fetch(`${ORIGIN}/id/api/options`, { method: 'POST', body: '[1,2', headers: { 'cf-connecting-ip': '10.9.9.9' } })
    expect(r.status).toBe(400)
  })
})
