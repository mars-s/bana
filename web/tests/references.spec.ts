// References (spec 01 §3, 08): the reference counter, the warning on editing a referenced node,
// link previews, and notes and fields on a reference.
import { test, expect, pageWith, focus, nameOf, type Server } from './fixtures'

async function tree(server: Server, id: number): Promise<any> {
  return (await server.run('export.json', { node: id })).result
}

test('the reference counter shows how many places point at a line', async ({ app, server }) => {
  const { page, ids } = await pageWith(server, app, ['Recipe', 'Plan', 'Shop'])
  await server.run('node.reference', { target: ids[0], parent: ids[1] })
  await server.run('node.reference', { target: ids[0], parent: ids[2] })
  await expect(app.locator('.ref-count')).toHaveCount(0)
  await server.run('settings.set', { key: 'ref_counter', value: true })
  await server.run('nav.open', { node: page })
  await expect(app.locator(`.row.node[data-id="${ids[0]}"] .ref-count`)).toHaveText('2')
  // From Settings too.
  await app.locator('.topbar [title="Account"]').click()
  await app.locator('.account-menu .menu-item', { hasText: 'Settings' }).click()
  await app.locator('button[aria-label="Reference counter"]').click()
  await expect.poll(async () => (await server.run('settings.get')).result.ref_counter).toBe(false)
})

test('editing a referenced node warns once that the edit shows everywhere', async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['Recipe', 'Plan'])
  await server.run('node.reference', { target: ids[0], parent: ids[1] })
  await focus(server, app, ids[0], 6)
  await app.keyboard.type('s')
  await expect(app.locator('.toast')).toContainText('also appears in 1 other place')
  await expect.poll(() => nameOf(server, ids[0])).toBe('Recipes')
  // Not for a line nobody points at.
  await app.locator('.toast').evaluate((e) => e.remove())
  await focus(server, app, ids[1], 4)
  await app.keyboard.type('x')
  await expect.poll(() => nameOf(server, ids[1])).toBe('Planx')
  await expect(app.locator('.toast')).toHaveCount(0)
})

test('a line that is just a link shows a preview card; the setting hides it', async ({ app, server }) => {
  await pageWith(server, app, ['https://www.example.org/garden/plan', 'not a link'])
  const card = app.locator('.link-card')
  await expect(card).toHaveCount(1)
  await expect(card.locator('.link-site')).toHaveText('example.org')
  await server.run('settings.set', { key: 'url_embeds', value: false })
  await app.reload()
  await expect(app.locator('.row.node', { hasText: 'not a link' })).toBeVisible()
  await expect(card).toHaveCount(0)
})

test('Mod+Alt+Down on a reference writes notes for this place only; context fields show under it', async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['Recipe', 'Plan'])
  const r = (await server.run('node.reference', { target: ids[0], parent: ids[1] })).result.id
  await server.run('node.expand', { node: ids[1] })
  await focus(server, app, r, 0)
  await app.keyboard.press('Meta+Alt+ArrowDown')
  await app.keyboard.type('for Sunday')
  await app.keyboard.press('Enter')
  await app.keyboard.type('double it')
  await expect(app.getByText('Notes and fields about Recipe, stored in Plan')).toBeVisible()
  await expect.poll(async () => (await tree(server, ids[1])).children?.[0]?.notes?.map((c: any) => c.name)).toEqual(['for Sunday', 'double it'])
  expect((await tree(server, ids[0])).children ?? []).toEqual([])
  await server.run('node.contextField', { node: r, field: 'Role', value: 'Lead' })
  await expect(app.locator('.row.field', { hasText: 'Role' })).toContainText('Lead')
})

test('inline references read as names in titles, tabs and reference lists (N79)', async ({ app, server }) => {
  const hits = (await server.run('graph.search', { query: 'Robin' })).result as { id: number; line: string }[]
  const ask = hits.find((h) => h.line.startsWith('Ask '))!.id
  const robin = hits.find((h) => h.line.startsWith('Robin Ash'))!.id
  // Robin's page: the line that mentions her reads with her name.
  await server.run('nav.open', { node: robin })
  await expect(app.locator('.reference-row', { hasText: 'shed key' })).toContainText('Ask Robin Ash for the shed key')
  await expect(app.locator('.references').first()).not.toContainText('[[id:')
  // Zoomed into that line: the title and the tab read the name; editing shows the raw text.
  await server.run('nav.open', { node: ask })
  await expect(app.locator('.title-rich')).toContainText('Ask Robin Ash for the shed key')
  await expect(app.locator('.tab.active .tab-title')).toContainText('Ask Robin Ash')
  await expect(app.locator('.tab.active .tab-title')).not.toContainText('[[id:')
  await app.locator('.panel .title-input').focus()
  await expect(app.locator('.title-rich')).toHaveCount(0)
})
