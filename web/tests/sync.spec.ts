// End to end: the relay Worker on local workerd (`wrangler dev`, nothing deployed) and two engines
// (`bana serve`, as the desktop app and as a phone would run them), each in its own browser
// window. Sync is set up and joined through Settings > Sync, then what is typed in window A shows
// in window B, and typing in the same line on both sides keeps both and never moves a caret.
// Run with `cargo xtask sync` (or `pnpm exec playwright test --project=sync`).

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
  // Each relay its own ports (and devtools port), so tests run side by side.
  const proc = spawn('npx', ['wrangler', 'dev', '--port', String(port), '--inspector-port', String(inspector), '--ip', '127.0.0.1', '--persist-to', state, '--show-interactive-dev-session=false'], {
    cwd: join(root, 'cloudflare/sync'),
    stdio: 'ignore',
    env: { ...process.env, WRANGLER_SEND_METRICS: 'false', CI: '1' },
  })
  const url = `http://127.0.0.1:${port}`
  for (let i = 0; i < 600; i++) {
    try {
      if ((await fetch(url)).ok) return { url, proc }
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
  file: string
  proc: ChildProcess
  run(command: string, params?: unknown): Promise<any>
  rpc(method: string, params?: unknown): Promise<any>
}

async function engine(args: string[]): Promise<Engine> {
  const port = await freePort()
  const file = join(mkdtempSync(join(tmpdir(), 'bana-sync-')), 'graph.bana')
  const proc = await startServer(file, port, args)
  const rpc = async (method: string, params: unknown = {}) =>
    (await fetch(`http://127.0.0.1:${port}/rpc`, { method: 'POST', body: JSON.stringify({ method, params }), headers: { authorization: `Bearer ${liveToken(file)}` } })).json()
  return { url: `http://127.0.0.1:${port}/`, file, proc, rpc, run: (command, params = {}) => rpc('run', { command, params }) }
}

async function openSyncSettings(page: Page) {
  await page.locator('.topbar-right [aria-label="Settings"]').click()
  await page.locator('.account-menu .menu-item', { hasText: 'Settings' }).click()
  await page.locator('.settings-nav .view-tab', { hasText: 'Sync' }).click()
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

test.setTimeout(120_000)

test('set up on one engine, join from another, and type in window A to see it in window B', async ({ relay, a, b, browser }) => {
  const pa = await browser.newPage({ viewport: { width: 1512, height: 949 } })
  const pb = await browser.newPage({ viewport: { width: 1512, height: 949 } })
  const errors: string[] = []
  for (const p of [pa, pb]) p.on('pageerror', (e) => errors.push(e.message))
  await pa.goto(a.url)
  await pb.goto(b.url)
  await expect(pa.locator('.panel .title').first()).toBeVisible()

  // A: Settings > Sync > Set up sync.
  await openSyncSettings(pa)
  await pa.getByLabel('Relay address').fill(relay.url)
  await pa.getByLabel('Device name').fill('Desk')
  await pa.getByRole('button', { name: 'Set up sync' }).click()
  await expect(pa.locator('.sync-status')).toHaveAttribute('data-sync', 'synced', { timeout: 20_000 })
  await expect(pa.locator('.topbar-right .sync-btn')).toHaveAttribute('data-sync', 'synced')
  await pa.getByRole('button', { name: 'Add a device' }).click()
  await expect(pa.getByLabel('Pairing QR code').locator('svg')).toBeVisible()
  const code = await pa.getByLabel('Pairing code').inputValue()
  // SYNC_SHOTS=1: pictures for the docs (the code in them is from a throwaway local relay).
  if (process.env.SYNC_SHOTS) await pa.screenshot({ path: join(root, 'docs/images/web/sync-add-device.png') })
  expect(code).toMatch(/^bana-sync:/)

  // B: Settings > Sync > Join, with the code pasted.
  await openSyncSettings(pb)
  await pb.getByLabel('Sync code').fill(code)
  await pb.getByLabel('Device name').fill('Phone')
  await pb.getByRole('button', { name: 'Join' }).click()
  await expect(pb.locator('.sync-status')).toHaveAttribute('data-sync', 'synced', { timeout: 20_000 })
  await expect(pb.locator('.sync-device')).toHaveCount(2)
  await expect(pb.locator('.sync-device[data-device="Desk"]')).toBeVisible()
  if (process.env.SYNC_SHOTS) await pb.screenshot({ path: join(root, 'docs/images/web/sync-devices.png') })
  // B's notes are now A's: the same page by id.
  const lib = (await a.rpc('sidebar')).workspaces[0].library
  const pageId = (await a.run('node.add', { name: 'Synced page', parent: lib })).result.id
  const line = (await a.run('node.add', { name: 'Garden', parent: pageId })).result.id
  await expect.poll(async () => (await b.run('graph.inspect', { node: line })).result?.name, { timeout: 15_000 }).toBe('Garden')
  // (In each window's main panel: Settings is open beside it.)
  await a.rpc('run', { command: 'nav.open', params: { node: pageId }, panel: 'main' })
  await b.rpc('run', { command: 'nav.open', params: { node: pageId }, panel: 'main' })
  await expect(pa.locator('.panel .title-input').first()).toHaveValue('Synced page')
  await expect(pb.locator('.panel .title-input').first()).toHaveValue('Synced page')

  // Typed in A, shown in B.
  await pa.locator('.row.node .text', { hasText: 'Garden' }).first().click()
  await pa.keyboard.press('End')
  await pa.keyboard.type(' beds', { delay: 30 })
  await expect(pb.locator('.row.node .text', { hasText: 'Garden beds' })).toBeVisible({ timeout: 15_000 })

  // Both type in the same line: both words stay, and each caret stays after its own words.
  await pb.locator('.row.node .text', { hasText: 'Garden beds' }).first().click()
  await pb.keyboard.press('Home')
  await expect(pb.locator(`.editor-input[data-node="${line}"]`)).toBeFocused()
  await Promise.all([pa.keyboard.type(' AAA', { delay: 60 }), pb.keyboard.type('BBB ', { delay: 60 })])
  const both = 'BBB Garden beds AAA'
  await expect.poll(async () => (await a.run('graph.inspect', { node: line })).result?.name, { timeout: 15_000 }).toBe(both)
  await expect.poll(async () => (await b.run('graph.inspect', { node: line })).result?.name, { timeout: 15_000 }).toBe(both)
  await expect(pa.locator(`.editor-input[data-node="${line}"]`)).toHaveValue(both)
  await expect(pb.locator(`.editor-input[data-node="${line}"]`)).toHaveValue(both)
  await pa.keyboard.type('!')
  await pb.keyboard.type('?')
  const end = 'BBB ?Garden beds AAA!'
  await expect.poll(async () => (await a.run('graph.inspect', { node: line })).result?.name, { timeout: 15_000 }).toBe(end)
  await expect.poll(async () => (await b.run('graph.inspect', { node: line })).result?.name, { timeout: 15_000 }).toBe(end)
  expect(errors).toEqual([])
})

test('an engine offline catches up when it comes back, and a removed device stops syncing', async ({ relay, a, b }) => {
  await a.rpc('sync.setup', { relay: relay.url, name: 'Desk' })
  await expect.poll(async () => (await a.rpc('sync.status')).state, { timeout: 20_000 }).toBe('synced')
  const { code } = await a.rpc('sync.invite')
  await b.rpc('sync.join', { code, name: 'Phone' })
  await expect.poll(async () => (await b.rpc('sync.status')).state, { timeout: 20_000 }).toBe('synced')
  const lib = (await a.rpc('sidebar')).workspaces[0].library
  // B goes away; A keeps writing.
  b.proc.kill('SIGKILL')
  const ids: number[] = []
  for (let i = 0; i < 5; i++) ids.push((await a.run('node.add', { name: `while away ${i}`, parent: lib })).result.id)
  // B comes back on the same file and catches up.
  const port = Number(new URL(b.url).port)
  b.proc = await startServer(b.file, port, [])
  await expect.poll(async () => (await b.run('graph.inspect', { node: ids[4] })).result?.name, { timeout: 20_000 }).toBe('while away 4')
  // A removes B: B stops with a clear message, and its edits no longer reach A.
  const me = (await b.rpc('sync.status')).device
  await a.rpc('sync.revoke', { device: me })
  await expect.poll(async () => (await b.rpc('sync.status')).state, { timeout: 20_000 }).toBe('error')
  expect((await b.rpc('sync.status')).message).toMatch(/removed/)
  const late = (await b.run('node.add', { name: 'after removal', parent: lib })).result.id
  await new Promise((r) => setTimeout(r, 1500))
  expect((await a.run('graph.inspect', { node: late })).result?.name ?? null).not.toBe('after removal')
})
