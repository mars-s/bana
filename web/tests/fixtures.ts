// A fresh engine per test: `bana serve` on its own port with the sample graph and a fixed
// today (Sat, 10 Oct 2026), optionally backed by a file so quitting and reloading can be tested.

import { test as base, expect, type Page } from '@playwright/test'
import { spawn, type ChildProcess } from 'node:child_process'
import { mkdtempSync, readFileSync } from 'node:fs'
import { createServer } from 'node:net'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'

const root = resolve(import.meta.dirname, '../..')
const bin = join(process.env.CARGO_TARGET_DIR ?? join(root, 'target'), 'debug', 'bana')
let nextPort = 9100

export interface Server {
  url: string
  port: number
  file: string
  proc: ChildProcess
  /** Starts the server again on the same file (after `kill`). */
  restart(): Promise<void>
  kill(signal?: NodeJS.Signals): Promise<void>
  rpc(method: string, params?: unknown): Promise<any>
  run(command: string, params?: unknown): Promise<any>
}

/** Whether nothing listens on `port` (another app, or someone else's `bana serve`). */
function portFree(port: number): Promise<boolean> {
  return new Promise((done) => {
    const probe = createServer()
    probe.once('error', () => done(false))
    probe.listen(port, '127.0.0.1', () => probe.close(() => done(true)))
  })
}

/** The install's local token (local tools send it, UC-10.11). Each test's engine has its own
 * data folder (BANA_HOME = the graph's folder), so tests never touch ~/Bana. */
export function liveToken(file: string): string {
  try {
    return readFileSync(join(dirname(file), 'token'), 'utf8').trim()
  } catch {
    return ''
  }
}

async function startServer(file: string, port: number, args: string[]): Promise<ChildProcess> {
  const proc = spawn(bin, ['--file', file, 'serve', ...args, '--today', '2026-10-10', '--port', String(port), '--web', join(root, 'web/dist')], {
    stdio: ['ignore', 'ignore', 'pipe'],
    env: { ...process.env, BANA_HOME: dirname(file) },
  })
  for (let i = 0; i < 400; i++) {
    if (proc.exitCode !== null) throw new Error(`server on ${port} exited (port taken?)`)
    try {
      const r = await fetch(`http://127.0.0.1:${port}/rpc`, { method: 'POST', body: '{"method":"version"}', headers: { authorization: `Bearer ${liveToken(file)}` } })
      if (r.ok) return proc
    } catch {
      /* not up yet */
    }
    await new Promise((r) => setTimeout(r, 50))
  }
  throw new Error(`server on ${port} did not start`)
}

export const test = base.extend<{ server: Server; app: Page; serveArgs: string[] }>({
  serveArgs: [['--sample'], { option: true }],
  server: async ({ serveArgs }, use, info) => {
    let port = nextPort + info.workerIndex * 200 + (info.repeatEachIndex % 200)
    nextPort += 1
    // Never talk to a server this test did not start.
    while (!(await portFree(port))) port = nextPort++ + info.workerIndex * 200
    const file = join(mkdtempSync(join(tmpdir(), 'bana-')), 'graph.bana')
    let proc = await startServer(file, port, serveArgs)
    const s: Server = {
      url: `http://127.0.0.1:${port}/`,
      port,
      file,
      get proc() {
        return proc
      },
      async restart() {
        proc = await startServer(file, port, serveArgs)
      },
      async kill(signal = 'SIGKILL') {
        const p = proc
        const done = new Promise((r) => p.once('exit', r))
        p.kill(signal)
        await done
      },
      async rpc(method, params = {}) {
        const r = await fetch(`http://127.0.0.1:${port}/rpc`, { method: 'POST', body: JSON.stringify({ method, params }), headers: { authorization: `Bearer ${liveToken(file)}` } })
        return r.json()
      },
      async run(command, params = {}) {
        return s.rpc('run', { command, params })
      },
    }
    await use(s)
    if (proc.exitCode === null) proc.kill('SIGKILL')
  },
  app: async ({ page, server }, use) => {
    // Any error thrown in the page fails the test.
    const errors: string[] = []
    page.on('pageerror', (e) => errors.push(process.env.PAGE_STACKS ? String(e.stack) : e.message))
    // CPU_THROTTLE=4 runs the page 4 times slower, to find races that only show under load.
    if (process.env.CPU_THROTTLE) await (await page.context().newCDPSession(page)).send('Emulation.setCPUThrottlingRate', { rate: Number(process.env.CPU_THROTTLE) })
    await page.goto(server.url)
    await expect(page.locator('.panel .title')).toBeVisible()
    await use(page)
    expect(errors, 'errors thrown in the page').toEqual([])
  },
})

export { expect }

/** The text of every line in the outline, in order. */
export async function lines(page: Page): Promise<string[]> {
  return page.locator('.row.node .line > .text, .row.node .line > .editor .editor-input').evaluateAll((els) =>
    els.map((e) => (e instanceof HTMLTextAreaElement ? e.value : (e.textContent ?? '').replace(/​/g, ''))),
  )
}

/** The engine's name of a node. */
export async function nameOf(server: Server, id: number): Promise<string> {
  const r = await server.run('graph.inspect', { node: id })
  return r.result?.name ?? ''
}

/** Clicks at the end of the line whose text is `text`. */
export async function clickLine(page: Page, text: string) {
  const el = page.locator('.row.node .text', { hasText: text }).first()
  const box = await el.boundingBox()
  if (!box) throw new Error(`no line ${text}`)
  await page.mouse.click(box.x + box.width - 1, box.y + box.height / 2)
  await expect(page.locator('.editor-input')).toBeFocused()
}

/** A new page in the Library with these lines, opened in the main panel. Returns the line ids. */
export async function pageWith(server: Server, app: Page, names: string[], title = 'Test page'): Promise<{ page: number; ids: number[] }> {
  const lib = (await server.rpc('sidebar')).workspaces[0].library
  const p = (await server.run('node.add', { name: title, parent: lib })).result.id
  const ids: number[] = []
  for (const n of names) ids.push((await server.run('node.add', { name: n, parent: p })).result.id)
  await server.run('nav.open', { node: p })
  await expect(app.locator('.panel .title-input')).toHaveValue(title)
  return { page: p, ids }
}

/** Puts the caret in line `id` at `caret` (as a click would), and waits for the editor. */
export async function focus(server: Server, app: Page, id: number, caret: number) {
  await server.run('focus.set', { node: id, caret })
  await expect(app.locator(`.editor-input[data-node="${id}"]`)).toBeFocused()
}

/** Waits until the engine has every edit sent so far. */
export async function settle(app: Page) {
  await app.waitForTimeout(50)
  await app.evaluate(() => new Promise((r) => setTimeout(r, 50)))
}
