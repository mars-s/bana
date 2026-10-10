// Bana ID end to end: the relay on local workerd (`wrangler dev`, relying party `localhost`), two
// engines as two desktops, and Chromium's virtual authenticator with PRF (CDP WebAuthn). Desktop
// A creates a Bana ID (a passkey) on its profile page; desktop B signs in with the same passkey
// and A's notes arrive through sync. Run with `cargo xtask sync` (project `sync`).

import { test as base, expect, type Page } from '@playwright/test'
import { spawn, type ChildProcess } from 'node:child_process'
import { mkdtempSync } from 'node:fs'
import { createServer } from 'node:net'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { liveToken, startServer } from './fixtures'

const root = resolve(import.meta.dirname, '../..')

function freePort(): Promise<number> {
  return new Promise((done) => {
    const s = createServer().listen(0, '127.0.0.1', () => {
      const p = (s.address() as { port: number }).port
      s.close(() => done(p))
    })
  })
}

async function startRelay(): Promise<{ url: string; proc: ChildProcess }> {
  const [port, inspector] = [await freePort(), await freePort()]
  const state = mkdtempSync(join(tmpdir(), 'bana-relay-'))
  // Passkeys need a domain, not an IP: the page is served on http://localhost (a secure context).
  const proc = spawn(
    'npx',
    ['wrangler', 'dev', '--port', String(port), '--inspector-port', String(inspector), '--ip', '127.0.0.1', '--persist-to', state, '--var', 'RP_ID:localhost', '--show-interactive-dev-session=false'],
    { cwd: join(root, 'cloudflare/sync'), stdio: 'ignore', env: { ...process.env, WRANGLER_SEND_METRICS: 'false', CI: '1' } },
  )
  const url = `http://localhost:${port}`
  for (let i = 0; i < 600; i++) {
    try {
      if ((await fetch(`http://127.0.0.1:${port}/`)).ok) return { url, proc }
    } catch {
      /* not up yet */
    }
    await new Promise((r) => setTimeout(r, 100))
  }
  proc.kill()
  throw new Error('wrangler dev did not start')
}

interface Engine {
  url: string
  proc: ChildProcess
  rpc(method: string, params?: unknown): Promise<any>
  run(command: string, params?: unknown): Promise<any>
}

async function engine(args: string[]): Promise<Engine> {
  const port = await freePort()
  const file = join(mkdtempSync(join(tmpdir(), 'bana-id-')), 'graph.bana')
  const proc = await startServer(file, port, args)
  const rpc = async (method: string, params: unknown = {}) =>
    (await fetch(`http://127.0.0.1:${port}/rpc`, { method: 'POST', body: JSON.stringify({ method, params }), headers: { authorization: `Bearer ${liveToken(file)}` } })).json()
  return { url: `http://127.0.0.1:${port}/`, proc, rpc, run: (command, params = {}) => rpc('run', { command, params }) }
}

const test = base.extend<{ relay: { url: string }; a: Engine; b: Engine }>({
  relay: async ({}, use) => {
    const r = await startRelay()
    await use(r)
    r.proc.kill()
  },
  a: async ({}, use) => {
    const e = await engine(['--sample'])
    await use(e)
    e.proc.kill('SIGKILL')
  },
  b: async ({}, use) => {
    const e = await engine([])
    await use(e)
    e.proc.kill('SIGKILL')
  },
})

test.setTimeout(150_000)

async function openProfile(page: Page) {
  await page.locator('.topbar-right [aria-label="Settings"]').click()
  await page.locator('.account-menu .menu-item', { hasText: 'Profile and Bana ID' }).click()
  await expect(page.locator('.profile-settings')).toBeVisible()
}

/** Clicks `button` on the profile page; the passkey page opens in a new tab, which the test moves
 * to the tab that holds the virtual authenticator, and presses the passkey button there. */
async function passkey(page: Page, button: string, authTab: Page, pageButton: string) {
  const popup = page.context().waitForEvent('page')
  await page.locator('.profile-settings').getByRole('button', { name: button }).click()
  const tab = await popup
  await tab.waitForURL(/\/id\?mode=/)
  const url = tab.url()
  await tab.close()
  await authTab.goto(url)
  await authTab.getByRole('button', { name: pageButton }).click()
  // The page hands the code to the engine's one-shot listener on 127.0.0.1.
  await authTab.waitForURL(/127\.0\.0\.1:\d+\/auth\/callback/, { timeout: 20_000 })
  await expect(authTab.locator('body')).toContainText('Done.')
}

test('create a Bana ID on desktop A, sign in on desktop B with the same passkey, and see A’s notes arrive', async ({ relay, a, b, browser }) => {
  const ctxA = await browser.newContext({ viewport: { width: 1512, height: 949 } })
  const ctxB = await browser.newContext({ viewport: { width: 1512, height: 949 } })
  const pa = await ctxA.newPage()
  const pb = await ctxB.newPage()
  const errors: string[] = []
  for (const p of [pa, pb]) {
    p.on('pageerror', (e) => errors.push(e.message))
    p.on('dialog', (d) => void d.accept())
  }
  // One tab with a virtual platform authenticator (PRF on): the "iCloud Keychain" both desktops share.
  const authCtx = await browser.newContext()
  const authTab = await authCtx.newPage()
  const cdp = await authCtx.newCDPSession(authTab)
  await cdp.send('WebAuthn.enable')
  const { authenticatorId } = await cdp.send('WebAuthn.addVirtualAuthenticator', {
    options: { protocol: 'ctap2', transport: 'internal', hasResidentKey: true, hasUserVerification: true, isUserVerified: true, hasPrf: true, automaticPresenceSimulation: true },
  })

  // A: Profile > Bana ID on the local relay > Create Bana ID.
  await pa.goto(a.url)
  await expect(pa.locator('.panel .title').first()).toBeVisible()
  await openProfile(pa)
  await pa.locator('.relay-choice summary').click()
  await pa.getByLabel('Bana ID relay').fill(relay.url)
  await passkey(pa, 'Create Bana ID', authTab, 'Create passkey')
  await expect(pa.locator('[data-bana-id="signed-in"]')).toBeVisible({ timeout: 20_000 })
  const code = await pa.getByTestId('recovery-code').textContent()
  expect(code).toMatch(/^[0-9A-Z]{5}(-[0-9A-Z]{5}){4}$/)
  // SYNC_SHOTS=1: pictures for the docs (a throwaway local relay and passkey).
  if (process.env.SYNC_SHOTS) await pa.screenshot({ path: join(root, 'docs/images/web/bana-id-recovery.png') })
  await pa.getByRole('button', { name: 'I saved it' }).click()
  await expect(pa.getByTestId('recovery-code')).toHaveCount(0)
  await expect(pa.locator('.passkey-row')).toHaveCount(1)
  await expect(pa.locator('.passkey-row')).not.toContainText('cannot unlock')
  expect((await cdp.send('WebAuthn.getCredentials', { authenticatorId })).credentials).toHaveLength(1)
  await expect.poll(async () => (await a.rpc('sync.status')).state, { timeout: 20_000 }).toBe('synced')

  // A's notes.
  const lib = (await a.rpc('sidebar')).workspaces[0].library
  const pageId = (await a.run('node.add', { name: 'From desk A', parent: lib })).result.id
  const line = (await a.run('node.add', { name: 'Seeds to order', parent: pageId })).result.id

  // B: a fresh desktop signs in with the same passkey; its notes become the synced ones.
  await pb.goto(b.url)
  await expect(pb.locator('.panel .title').first()).toBeVisible()
  await openProfile(pb)
  await pb.locator('.relay-choice summary').click()
  await pb.getByLabel('Bana ID relay').fill(relay.url)
  await passkey(pb, 'Sign in with a passkey', authTab, 'Sign in with passkey')
  await expect(pb.locator('[data-bana-id="signed-in"]')).toBeVisible({ timeout: 20_000 })
  await expect.poll(async () => (await b.run('graph.inspect', { node: line })).result?.name, { timeout: 30_000 }).toBe('Seeds to order')
  await b.rpc('run', { command: 'nav.open', params: { node: pageId }, panel: 'main' })
  await expect(pb.locator('.panel .title-input').first()).toHaveValue('From desk A')

  // Both devices are on the account; A sees B (names are sealed on the relay, readable here).
  await expect(pb.locator('.id-device')).toHaveCount(2)
  if (process.env.SYNC_SHOTS) await pb.screenshot({ path: join(root, 'docs/images/web/bana-id-profile.png') })
  await pa.reload()
  await openProfile(pa)
  await expect(pa.locator('.id-device')).toHaveCount(2)
  // Typed on B, it reaches A.
  await b.run('node.add', { name: 'Back from B', parent: pageId })
  await expect.poll(async () => JSON.stringify((await a.run('graph.inspect', { node: pageId })).result), { timeout: 15_000 }).toContain('Back from B')
  expect(errors).toEqual([])
  await Promise.all([ctxA.close(), ctxB.close(), authCtx.close()])
})
