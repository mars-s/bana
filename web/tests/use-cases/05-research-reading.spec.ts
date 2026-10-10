// Use cases: research and reading notes (docs/use-cases/05-research-reading.md).
import { test, expect, missing, works, partial, tree, fieldOf, idOf, library } from './helpers'
import { pageWith, focus, nameOf } from '../fixtures'

test('UC-05.01 save a link: a line that is just a URL shows a preview card (no video embed)', partial('UC-05.01'), async ({ app, server }) => {
  await pageWith(server, app, ['https://www.youtube.com/watch?v=abc123', 'my notes on it'])
  await expect(app.locator('.link-card .link-site')).toHaveText('youtube.com')
  // Gap: no embedded player for YouTube, Vimeo, Loom or X links.
  await expect(app.locator('iframe')).toHaveCount(0)
})

test('UC-05.02 quotes nested under a book take its Author by themselves', works('UC-05.02'), async ({ app, server }) => {
  await server.run('tag.create', { name: 'book', color: 'blue' })
  await server.run('tag.create', { name: 'quote', color: 'yellow' })
  await server.run('field.create', { name: 'Author', type: 'plain' })
  await server.run('field.setAutoInit', { field: 'Author', mode: 'ancestor_value' })
  await server.run('tag.addField', { tag: 'book', field: 'Author' })
  await server.run('tag.addField', { tag: 'quote', field: 'Author' })
  const { ids } = await pageWith(server, app, ['Teaming with Microbes'])
  await server.run('tag.add', { node: ids[0], tag: 'book' })
  await server.run('field.set', { node: ids[0], field: 'Author', value: 'Lowenfels' })
  const q = (await server.run('node.add', { name: '', parent: ids[0] })).result.id
  await server.run('node.expand', { node: ids[0] })
  await focus(server, app, q, 0)
  await app.keyboard.type('Plants feed the soil life #quote')
  await expect(app.locator('.menu-item.sel')).toContainText('quote')
  await app.keyboard.press('Enter')
  await expect.poll(() => fieldOf(server, q, 'Author')).toBe('Lowenfels')
})

test('UC-05.03 highlight and bold the key phrase in a note', works('UC-05.03'), async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['compost needs air'])
  await focus(server, app, ids[0], 17)
  await app.keyboard.press('Shift+Alt+ArrowLeft')
  await app.keyboard.press('Control+h')
  await expect.poll(() => nameOf(server, ids[0])).toBe('compost needs ==air==')
  await app.keyboard.press('Home')
  await app.keyboard.press('Shift+Alt+ArrowRight')
  await app.keyboard.press('Meta+b')
  await expect.poll(() => nameOf(server, ids[0])).toBe('**compost** needs ==air==')
  await expect(app.locator(`.row.node[data-id="${ids[0]}"] mark, .row.node[data-id="${ids[0]}"] .hl`).first()).toBeVisible()
})

test('UC-05.04 link a new concept inline with @ and Cmd+Enter; it lands in the Library', works('UC-05.04'), async ({ app, server }) => {
  const { ids } = await pageWith(server, app, [''])
  await focus(server, app, ids[0], 0)
  await app.keyboard.type('Fungi trade sugar, see ')
  await app.keyboard.type('@Mycorrhiza')
  await expect(app.locator('.menu.picker')).toBeVisible()
  await app.keyboard.press('Meta+Enter')
  await app.keyboard.type(' for more')
  await expect(app.locator(`.row.node[data-id="${ids[0]}"] .inline-ref, .row.node[data-id="${ids[0]}"] .editor`).first()).toBeVisible()
  const lib = await library(server)
  await expect.poll(async () => (await tree(server, lib)).children.map((c: any) => c.name)).toContain('Mycorrhiza')
  const concept = await idOf(server, 'Mycorrhiza')
  await server.run('nav.open', { node: concept })
  await expect(app.locator('.references .reference-row')).toHaveCount(1)
})

test('UC-05.05 a reading list table filtered to what is still unread', works('UC-05.05'), async ({ app, server }) => {
  await server.run('tag.create', { name: 'article', color: 'teal' })
  await server.run('field.create', { name: 'Read', type: 'checkbox' })
  await server.run('tag.addField', { tag: 'article', field: 'Read' })
  const { page, ids } = await pageWith(server, app, ['No-dig gardening', 'Seed saving basics', 'Rainwater tanks'], 'Reading list')
  for (const id of ids) await server.run('tag.add', { node: id, tag: 'article' })
  await server.run('field.set', { node: ids[1], field: 'Read', value: 'Yes' })
  await server.run('view.set', { node: page, kind: 'table' })
  expect((await server.run('view.addFilter', { node: page, clause: 'NOT Read:Yes' })).error).toBeUndefined()
  await server.run('nav.open', { node: page })
  await expect(app.locator('.t-row[data-id]')).toHaveCount(2)
  await expect(app.locator('.t-row', { hasText: 'Seed saving basics' })).toHaveCount(0)
})

test('UC-05.06 unlinked mentions: lines that name this page without linking it, linked in one click', works('UC-05.06'), async ({ app, server }) => {
  const lib = (await server.rpc('sidebar')).workspaces[0].library
  const concept = (await server.run('node.add', { name: 'Companion planting', parent: lib })).result.id
  const line = (await server.run('node.add', { name: 'Try companion planting with basil', parent: lib })).result.id
  await server.run('nav.open', { node: concept })
  await app.locator('.mentions .references-head').click()
  const row = app.locator('.mentions .reference-row', { hasText: 'Try companion planting with basil' })
  await expect(row).toBeVisible()
  await row.locator('.btn', { hasText: 'Link' }).click()
  await expect.poll(async () => (await server.run('graph.inspect', { node: line })).result.name).toBe(`Try [[id:${concept}]] with basil`)
  await expect(app.locator('.mentions .reference-row')).toHaveCount(0)
  // It is a reference now.
  await expect(app.locator('.references:not(.mentions) .reference-row')).toHaveCount(1)
})
missing('UC-05.07', 'Readwise sync of book and article highlights')
missing('UC-05.08', 'attach a PDF or an image to a node and preview it')
missing('UC-05.09', 'expand an inline reference in place (Shift+click) to read it without leaving the line')
missing('UC-05.10', 'record a lecture and get a transcript and summary (AI)', 'ai')
