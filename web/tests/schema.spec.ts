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
  await app.locator('button[aria-label="Show a checkbox"]').click()
  await expect.poll(async () => (await inspectSchema(server, id)).checkbox).toBe(true)
  await app.locator('.cfg-fields .ghost-btn').click()
  await expect(app.locator('.menu.picker')).toBeVisible()
  await app.keyboard.type('Impact')
  await expect(app.locator('.menu.picker .menu-item.sel')).toContainText('Impact')
  await app.keyboard.press('Enter')
  await expect.poll(async () => (await inspectSchema(server, id)).fields.map((x: any) => x.name)).toEqual(['Impact'])
})

test('Configure supertag from the node menu opens one second panel; Done closes it', async ({ app, server }) => {
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
  await app.locator('.panel').nth(1).locator('.tag-tabs .btn', { hasText: 'Done' }).click()
  await expect(app.locator('.panel')).toHaveCount(1)
})
