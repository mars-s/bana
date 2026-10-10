// The field definition page and supertag configuration (spec 03 §5, §6, §8).
import { test, expect, type Server } from './fixtures'

async function inspectSchema(server: Server, node: number) {
  return server.rpc('schema', { node })
}

test('a field page changes the type, adds options and makes it required', async ({ app, server }) => {
  const f = (await server.run('field.create', { name: 'Mood', type: 'plain' })).result
  const id = f.field ?? f.id ?? f.node
  await server.run('nav.open', { node: id })
  await expect(app.locator('.cfg-grid')).toBeVisible()
  await app.locator('select[aria-label="Field type"]').selectOption('options')
  await expect.poll(async () => (await inspectSchema(server, id)).type).toBe('options')
  await app.locator('.cfg-input[placeholder="Add an option"]').fill('Calm')
  await app.locator('.cfg-input[placeholder="Add an option"]').press('Enter')
  await expect.poll(async () => (await inspectSchema(server, id)).ownOptions).toEqual(['Calm'])
  await app.locator('button[aria-label="Required"]').click()
  await expect.poll(async () => (await inspectSchema(server, id)).required).toBe(true)
})

test('configure a supertag: colour, checkbox and a template field', async ({ app, server }) => {
  const tag = (await server.run('tag.create', { name: 'idea', color: 'grey' })).result
  const id = tag.tag ?? tag.id ?? tag.node
  await server.run('nav.open', { node: id })
  await app.locator('.head-actions .btn[title="Options"]').click()
  await app.locator('.node-menu .menu-item', { hasText: 'Configure supertag' }).click()
  // It opens in a second panel to the right of the tag page (spec 03 §5).
  await expect(app.locator('.panel')).toHaveCount(2)
  await expect(app.locator('.panel').nth(1).locator('.configure')).toBeVisible()
  await expect(app.locator('.panel').nth(0).locator('.configure')).toHaveCount(0)
  await app.locator('.swatch[title="green"]').click()
  await expect.poll(async () => (await inspectSchema(server, id)).color).toBe('green')
  // Sections start folded; a click on a header opens it.
  await expect(app.locator('button[aria-label="Show a checkbox"]')).toHaveCount(0)
  await app.locator('.cfg-card-head', { hasText: 'Building blocks' }).click()
  await app.locator('button[aria-label="Show a checkbox"]').click()
  await expect.poll(async () => (await inspectSchema(server, id)).checkbox).toBe(true)
  await app.locator('.cfg-card-head', { hasText: 'Content template' }).click()
  await app.locator('.cfg-fields .ghost-btn').click()
  await expect(app.locator('.menu.picker')).toBeVisible()
  await app.keyboard.type('Impact')
  await expect(app.locator('.menu.picker .menu-item.sel')).toContainText('Impact')
  await app.keyboard.press('Enter')
  await expect.poll(async () => (await inspectSchema(server, id)).fields.map((x: any) => x.name)).toEqual(['Impact'])
})

test('Configure supertag from the node menu opens one second panel; its close button closes it', async ({ app, server }) => {
  const tag = (await server.run('tag.create', { name: 'idea', color: 'grey' })).result
  const id = tag.tag ?? tag.id ?? tag.node
  const lib = (await server.rpc('sidebar')).workspaces[0].library
  await server.run('node.move', { node: id, parent: lib })
  await server.run('nav.open', { node: lib })
  const bullet = app.locator(`.row.node[data-id="${id}"] .bullet`).first()
  await bullet.click({ button: 'right' })
  await app.locator('.node-menu .menu-item', { hasText: 'Configure supertag' }).click()
  await expect(app.locator('.panel')).toHaveCount(2)
  await expect(app.locator('.panel').nth(1).locator('.configure')).toBeVisible()
  // Asking again reuses the panel.
  await bullet.click({ button: 'right' })
  await app.locator('.node-menu .menu-item', { hasText: 'Configure supertag' }).click()
  await expect(app.locator('.panel')).toHaveCount(2)
  await app.locator('.panel').nth(1).locator('.crumbs [title="Close panel"]').click()
  await expect(app.locator('.panel')).toHaveCount(1)
})

test('a field page reads Schema / #supertag / field, and its description is typed in place', async ({ app, server }) => {
  const hits = await server.rpc('find', { query: 'Status', limit: 10 })
  const id = hits.find((h: any) => h.name === 'Status').id
  await server.run('nav.open', { node: id })
  await expect(app.locator('.crumbs .crumb')).toHaveText(['Schema', '#task', 'Status'])
  await app.locator('.field-head .page-desc').fill('Where a task is')
  await expect.poll(async () => (await server.run('graph.inspect', { node: id })).result?.description).toBe('Where a task is')
})

test('a field page lists sources of options and adds one; Configure supertag ends with Used N times and Delete (N59.32, N59.33)', async ({ app, server }) => {
  const hits = await server.rpc('find', { query: 'Status', limit: 10 })
  const id = hits.find((h: any) => h.name === 'Status').id
  await server.run('nav.open', { node: id })
  const row = app.locator('.cfg-row', { hasText: 'Sources of options' })
  await row.locator('.ghost-btn', { hasText: '+ Add source' }).click()
  await app.keyboard.type('Ideas')
  await app.locator('.menu.picker .menu-item', { hasText: 'Ideas' }).first().click()
  await expect(row.locator('.cfg-option', { hasText: 'Ideas' })).toBeVisible()
  // The control column starts 157.5 from the labels.
  const lx = (await app.locator('.cfg-label').first().boundingBox())!.x
  const cx = (await app.locator('.cfg-control').first().boundingBox())!.x
  expect(Math.abs(cx - lx - 157.5)).toBeLessThan(1)
  const tags = (await server.run('graph.tags', {})).result
  await server.run('nav.open', { node: tags.find((t: any) => t.name === 'task').id })
  await app.locator('.head-actions [title="Options"]').first().click()
  await app.locator('.node-menu .menu-item', { hasText: 'Configure supertag' }).click()
  await expect(app.locator('.cfg-used')).toHaveText(/Used \d+ times/)
})

test('option values show the option marker, not a bullet (N59.24)', async ({ app, server }) => {
  const hits = await server.rpc('find', { query: 'Build four beds', limit: 5 })
  await server.run('nav.open', { node: hits.find((h: any) => h.name === 'Build four beds').id })
  const status = app.locator('.row.field', { hasText: 'Status' })
  await expect(status.locator('.option-mark')).toBeVisible()
  await expect(status.locator('.field-value .dot')).toHaveCount(0)
})
