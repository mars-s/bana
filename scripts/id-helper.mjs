#!/usr/bin/env node
// A stand-in for the passkey sheet, for tests only (scripts/ios-test.sh id): the iPhone simulator
// cannot make passkeys in a UI test, so the app (launched with `-idHelper <this address>`) hands
// the Bana ID page's address here instead of opening ASWebAuthenticationSession. This runs the
// page's steps with a software authenticator with PRF (cloudflare/sync/test/authenticator.ts)
// against the real relay and answers the address the page would have returned to
// (`bana://auth/callback?code=...`). The code exchange, unwrapping and joining in the app are
// the real ones.
//
//   node scripts/id-helper.mjs 8798
//   POST /complete {"url": "<page address>", "deliver": false}  -> {"callback": "..."}
//   (deliver: true fetches a 127.0.0.1 callback itself, as a desktop browser would.)
// One authenticator for the helper's life: a passkey made for one device signs in on the next.

import { createServer } from 'node:http'
import { SoftAuthenticator, ceremony } from '../cloudflare/sync/test/authenticator.ts'

const port = Number(process.argv[2] ?? 8798)
const auth = new SoftAuthenticator()

async function complete(pageUrl, deliver) {
  const u = new URL(pageUrl)
  const q = Object.fromEntries(u.searchParams)
  const post = async (path, body) => {
    const r = await fetch(`${u.origin}/id/api/${path}`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) })
    return { status: r.status, body: await r.json() }
  }
  const fin = await ceremony(post, auth, u.origin, { mode: q.mode, ret: q.ret, app: q.app, ticket: q.ticket, name: q.name })
  if (fin.status !== 200) throw new Error(fin.body.error ?? `finish answered ${fin.status}`)
  const callback = `${fin.body.ret}?code=${encodeURIComponent(fin.body.code)}&state=${encodeURIComponent(q.state ?? '')}`
  if (deliver) await (await fetch(callback)).text()
  return callback
}

createServer((req, res) => {
  let body = ''
  req.on('data', (c) => (body += c))
  req.on('end', async () => {
    try {
      if (req.method !== 'POST' || req.url !== '/complete') throw new Error('POST /complete')
      const { url, deliver } = JSON.parse(body)
      const callback = await complete(url, !!deliver)
      res.writeHead(200, { 'content-type': 'application/json' }).end(JSON.stringify({ callback }))
    } catch (e) {
      res.writeHead(400, { 'content-type': 'application/json' }).end(JSON.stringify({ error: String(e?.message ?? e) }))
    }
  })
}).listen(port, '127.0.0.1', () => console.log(`id helper on http://127.0.0.1:${port}`))
