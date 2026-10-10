// The popover layer (Popover.tsx): menus render on <body>, never clipped by a table or panel, and
// stay inside the window by flipping and shifting.
import { test, expect, pageWith, type Server } from './fixtures'
import type { Locator, Page } from '@playwright/test'

/** The box is inside the window and the popover is what the pointer would hit at its centre. */
async function fullyVisible(app: Page, pop: Locator) {
  await expect(pop).toBeVisible()
  const box = (await pop.boundingBox())!
  const vp = app.viewportSize()!
  expect(box.x).toBeGreaterThanOrEqual(0)
  expect(box.y).toBeGreaterThanOrEqual(0)
  expect(box.x + box.width).toBeLessThanOrEqual(vp.width)
  expect(box.y + box.height).toBeLessThanOrEqual(vp.height)
  const top = await pop.evaluate((el) => {
    const r = el.getBoundingClientRect()
    return el.contains(document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2))
  })
  expect(top).toBe(true)
}

/** A new supertag with two tagged nodes, its page open: a table with only the title column. */
async function narrowTable(server: Server, app: Page) {
  const lib = (await server.rpc('sidebar')).workspaces[0].library
  for (const n of ['goon', 'goon']) {
    const id = (await server.run('node.add', { name: n, parent: lib })).result.id
    await server.run('tag.add', { node: id, tag: 'goon' })
  }
  const tag = (await server.run('graph.tags', {})).result.find((t: any) => t.name === 'goon')
  await server.run('nav.open', { node: tag.id })
  await expect(app.locator('.t-row[data-id]')).toHaveCount(2)
  return tag.id as number
}

test('the table "+ Add" menu is not clipped by a narrow table, also in a small window', async ({ app, server }) => {
  await narrowTable(server, app)
  for (const size of [null, { width: 620, height: 360 }]) {
    if (size) await app.setViewportSize(size)
    await app.locator('.t-head .tool', { hasText: '+ Add' }).click()
    const menu = app.locator('.menu.pop', { hasText: 'Add new field' })
    await fullyVisible(app, menu)
    // Its submenu too.
    await menu.locator('.menu-item', { hasText: 'System fields' }).hover()
    await fullyVisible(app, app.locator('.submenu'))
    await app.keyboard.press('Escape')
    await expect(menu).toHaveCount(0)
  }
})

test('the table "+ Add" sits after the last column, outside the row rules (spec 12: Table)', async ({ app, server }) => {
  await narrowTable(server, app)
  const head = (await app.locator('.t-head').boundingBox())!
  const add = (await app.locator('.t-add .tool').boundingBox())!
  expect(add.x).toBeGreaterThanOrEqual(head.x + head.width)
  expect(Math.round(head.height * 10) / 10).toBe(27.1)
  // A tag without a checkbox has no checkbox slot by its title.
  await expect(app.locator('.title-slot')).toHaveCount(0)
})

test('a menu opened at the right edge shifts left; one opened near the bottom stays in the window', async ({ app, server }) => {
  const names = Array.from({ length: 14 }, (_, i) => `Line ${i + 1}`)
  const { ids } = await pageWith(server, app, names)
  // The page's Options button is at the window's right edge; its menu starts at the button's left.
  await app.locator('.head-actions [title="Options"]').click()
  await fullyVisible(app, app.locator('.node-menu'))
  await app.keyboard.press('Escape')
  await expect(app.locator('.node-menu')).toHaveCount(0)
  // A short window: the node menu of a line near the bottom moves up (and scrolls if it must).
  await app.setViewportSize({ width: 900, height: 420 })
  const last = app.locator(`.row.node[data-id="${ids[8]}"] .bullet`)
  const b = (await last.boundingBox())!
  expect(b.y).toBeGreaterThan(300)
  await last.click({ button: 'right' })
  await fullyVisible(app, app.locator('.node-menu'))
  // The view-options submenu near the bottom also fits.
  await app.locator('.node-menu .menu-item', { hasText: 'Add to day' }).hover()
  await fullyVisible(app, app.locator('.submenu'))
})

test('the account menu at the top-right corner opens inside the window', async ({ app }) => {
  await app.locator('[title="Account"]').click()
  await fullyVisible(app, app.locator('.account-menu'))
})
