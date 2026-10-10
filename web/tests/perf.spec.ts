// A 50,000-node workspace with everything expanded: the outline draws only rows near the viewport,
// scrolling and typing stay quick, and a line far down takes typed text like any other.
import { test, expect, nameOf } from './fixtures'

test.use({ serveArgs: ['--large', '50000'] })

test('50k rows: scroll, click a line far down and type', async ({ app, server }) => {
  await server.run('view.expandAll')
  await expect.poll(async () => (await server.rpc('page', { count: 1 })).rows.total).toBeGreaterThan(40_000)
  await app.reload()
  await expect(app.locator('.row.node').first()).toBeVisible()
  const total = (await server.rpc('page', { count: 1 })).rows.total
  // Only a screenful of rows is in the DOM.
  expect(await app.locator('.row.node').count()).toBeLessThan(200)
  // Jump to the middle.
  const t0 = Date.now()
  await app.locator('.scroller').evaluate((el, f) => (el.scrollTop = el.scrollHeight * f), 0.6)
  await expect(app.locator('.row.node .text').first()).toBeVisible()
  await expect.poll(async () => (await app.locator('.vrow .row.placeholder').count()) === 0).toBe(true)
  const scrollMs = Date.now() - t0
  // The list may still settle after the jump (rows measured as they draw): take a row that is
  // well inside the visible part of the panel right now.
  let id = 0
  let box: { x: number; y: number; width: number; height: number } | null = null
  for (let i = 0; i < 20 && !box; i++) {
    const hit = await app.evaluate(() => {
      const sc = document.querySelector('.scroller')!.getBoundingClientRect()
      for (const t of document.querySelectorAll<HTMLElement>('.row.node .text')) {
        const r = t.getBoundingClientRect()
        if (r.top > sc.top + 120 && r.bottom < sc.bottom - 40 && r.width > 4) return { id: Number(t.closest<HTMLElement>('.row.node')!.dataset.id), box: { x: r.x, y: r.y, width: r.width, height: r.height } }
      }
      return null
    })
    if (hit) ({ id, box } = hit)
    else await app.waitForTimeout(100)
  }
  if (!box) throw new Error('no row on screen after the jump')
  const before = await nameOf(server, id)
  await app.mouse.click(box.x + box.width - 1, box.y + box.height / 2)
  await expect(app.locator('.editor-input')).toBeFocused()
  const t1 = Date.now()
  await app.keyboard.type(' typed far down')
  await expect.poll(() => nameOf(server, id)).toBe(`${before} typed far down`)
  const typeMs = Date.now() - t1
  console.log(`50k: ${total} rows; scroll to 60% and draw: ${scrollMs} ms; 15 keys to the engine: ${typeMs} ms`)
  expect(typeMs).toBeLessThan(3000)
})
