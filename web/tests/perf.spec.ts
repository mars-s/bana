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
  const row = app.locator('.row.node').nth(10)
  const id = Number(await row.getAttribute('data-id'))
  const before = await nameOf(server, id)
  const text = app.locator(`.row.node[data-id="${id}"] .text`)
  await text.scrollIntoViewIfNeeded()
  const box = (await text.boundingBox())!
  await app.mouse.click(box.x + box.width - 1, box.y + box.height / 2)
  await expect(app.locator('.editor-input')).toBeFocused()
  const t1 = Date.now()
  await app.keyboard.type(' typed far down')
  await expect.poll(() => nameOf(server, id)).toBe(`${before} typed far down`)
  const typeMs = Date.now() - t1
  console.log(`50k: ${total} rows; scroll to 60% and draw: ${scrollMs} ms; 15 keys to the engine: ${typeMs} ms`)
  expect(typeMs).toBeLessThan(3000)
})
