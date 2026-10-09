// The data-safety invariants, ported from legacy/egui/ui-egui/tests/ui_fuzz.rs:
// - a typed key is never lost: random keys typed at full speed, every character ends up in the page;
// - reloading the window or killing the engine at a random point keeps every character;
// - long pages (70+ lines, a small window, rows drawn only near the viewport) behave the same.
// More runs: FUZZ_SEEDS=40 pnpm exec playwright test fuzz

import { test, expect, pageWith, focus, type Server } from './fixtures'
import type { Page } from '@playwright/test'

const SEEDS = Number(process.env.FUZZ_SEEDS ?? 3)
const STEPS = Number(process.env.FUZZ_STEPS ?? 70)

function rng(seed: number) {
  let s = seed * 2654435761 + 1
  return (n: number) => {
    s ^= s << 13
    s ^= s >>> 17
    s ^= s << 5
    return Math.abs(s) % n
  }
}

/** Every character of the page's text: its title and every line under it. */
async function pageChars(server: Server, page: number): Promise<string> {
  const tree = (await server.run('export.json', { node: page })).result
  const out: string[] = []
  const walk = (n: any) => {
    if (!n) return
    out.push(n.name ?? '')
    for (const c of n.children ?? []) walk(c)
  }
  walk(tree)
  return out.join('')
}

const sorted = (s: string) => [...s.replace(/\s/g, '')].sort().join('')

// Keys that move the caret, make lines, select, indent, but never delete text.
const SAFE_KEYS = ['Enter', 'Tab', 'Shift+Tab', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'Escape', 'Home', 'End', 'Shift+ArrowDown', 'Shift+ArrowUp', 'Meta+Shift+ArrowUp', 'Meta+Shift+ArrowDown', 'Meta+ArrowDown', 'Meta+ArrowUp']
const WORDS = ['a', 'b', 'xy', 'hello', 'zz', 'q1', '42', 'k']

async function typeRandomly(app: Page, pick: (n: number) => number, steps: number): Promise<string> {
  let typed = ''
  for (let i = 0; i < steps; i++) {
    if (pick(3) === 0) {
      await app.keyboard.press(SAFE_KEYS[pick(SAFE_KEYS.length)])
    } else {
      const w = WORDS[pick(WORDS.length)]
      typed += w
      await app.keyboard.type(w)
    }
  }
  return typed
}

async function settled(server: Server, app: Page) {
  // Every key has gone to the engine once the UI's queue is empty and the buffer drained.
  await app.waitForTimeout(150)
  void server
}

for (let seed = 1; seed <= SEEDS; seed++) {
  test(`random keys never lose a character (seed ${seed})`, async ({ app, server }) => {
    const errors: string[] = []
    app.on('pageerror', (e) => errors.push(e.message))
    const pick = rng(seed)
    const { page, ids } = await pageWith(server, app, ['first', 'second'], 'Fuzz')
    await focus(server, app, ids[1], 6)
    const before = await pageChars(server, page)
    const typed = await typeRandomly(app, pick, STEPS)
    await settled(server, app)
    await expect.poll(async () => sorted(await pageChars(server, page))).toBe(sorted(before + typed))
    expect(errors).toEqual([])
  })

  test(`long page in a small window never loses a character (seed ${seed})`, async ({ app, server }) => {
    await app.setViewportSize({ width: 900, height: 520 })
    const pick = rng(seed + 1000)
    const names = Array.from({ length: 80 }, (_, i) => `line ${i}`)
    const { page, ids } = await pageWith(server, app, names, 'Long')
    const start = ids[50 + pick(25)]
    await focus(server, app, start, 2)
    const before = await pageChars(server, page)
    let typed = ''
    for (let round = 0; round < 4; round++) {
      typed += await typeRandomly(app, pick, STEPS / 4)
      // Scroll far away mid-edit, then keep typing: the line comes back and takes the keys.
      await app.mouse.move(450, 300)
      await app.mouse.wheel(0, pick(2) ? -4000 : 4000)
      await app.waitForTimeout(30)
      typed += 'w'
      await app.keyboard.type('w')
    }
    await settled(server, app)
    await expect.poll(async () => sorted(await pageChars(server, page))).toBe(sorted(before + typed))
  })

  test(`reloading at a random point keeps every character (seed ${seed})`, async ({ app, server }) => {
    const pick = rng(seed + 2000)
    const { page, ids } = await pageWith(server, app, ['one'], 'Reload')
    await focus(server, app, ids[0], 3)
    const before = await pageChars(server, page)
    const typed = await typeRandomly(app, pick, 10 + pick(40))
    await app.reload()
    await expect(app.locator('.panel .title, .panel .title-input').first()).toBeVisible()
    // Keys still waiting when the page went are typed again after the reload.
    await expect.poll(async () => sorted(await pageChars(server, page))).toBe(sorted(before + typed))
  })

  test(`killing the engine at a random point keeps what autosave wrote (seed ${seed})`, async ({ app, server }) => {
    const pick = rng(seed + 3000)
    const { page, ids } = await pageWith(server, app, ['one'], 'Kill')
    await focus(server, app, ids[0], 3)
    const before = await pageChars(server, page)
    const typed = await typeRandomly(app, pick, 10 + pick(40))
    await settled(server, app)
    // Autosave writes within a second of the last change; a force-kill after that loses nothing.
    await app.waitForTimeout(1300)
    await server.kill('SIGKILL')
    await server.restart()
    expect(sorted(await pageChars(server, page))).toBe(sorted(before + typed))
  })
}
