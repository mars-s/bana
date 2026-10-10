// Use cases: people, companies and a light CRM (docs/use-cases/04-crm.md).
import { test, expect, missing, works, tree, fieldOf, idOf, library } from './helpers'
import { pageWith, focus } from '../fixtures'
import type { Server } from '../fixtures'

/** #company, and #person gets Company (options from #company), Last contacted and Next contact dates. */
async function crmSchema(server: Server) {
  await server.run('tag.create', { name: 'company', color: 'orange' })
  await server.run('field.create', { name: 'Company', type: 'options_from_tag' })
  await server.run('field.setSourceTag', { field: 'Company', tag: 'company' })
  await server.run('field.create', { name: 'Last contacted', type: 'date' })
  await server.run('field.create', { name: 'Next contact', type: 'date' })
  for (const f of ['Company', 'Last contacted', 'Next contact']) await server.run('tag.addField', { tag: 'person', field: f })
  const lib = await library(server)
  const acme = (await server.run('node.add', { name: 'Greenhouse Co', parent: lib })).result.id
  await server.run('tag.add', { node: acme, tag: 'company' })
  return acme as number
}

test('UC-04.01 link a person to their company; the company page lists its people', works('UC-04.01'), async ({ app, server }) => {
  const acme = await crmSchema(server)
  const robin = await idOf(server, 'Robin Ash')
  await server.run('field.set', { node: robin, field: 'Company', value: 'Greenhouse Co' })
  await server.run('nav.open', { node: acme })
  await expect(app.locator('.references .reference-row', { hasText: 'Robin Ash' })).toBeVisible()
})

test('UC-04.02 typing a new company in the field makes it a #company node', works('UC-04.02'), async ({ app, server }) => {
  await crmSchema(server)
  const { ids } = await pageWith(server, app, ['Sam Ortiz'])
  await server.run('tag.add', { node: ids[0], tag: 'person' })
  await server.run('node.expand', { node: ids[0] })
  await app.locator('.row.field', { hasText: 'Company' }).locator('.field-value').first().dispatchEvent('mousedown')
  await expect(app.locator('.field-input')).toBeFocused()
  await app.keyboard.type('Seed Savers Ltd')
  await app.keyboard.press('Enter')
  await app.keyboard.press('Escape')
  await expect.poll(async () => {
    const hits = await server.rpc('find', { query: 'Seed Savers Ltd', limit: 5 })
    const n = hits.find((h: any) => h.name === 'Seed Savers Ltd')
    return n ? (await tree(server, n.id)).tags : null
  }).toEqual(['company'])
})

test('UC-04.03 a people table sorted by when you last talked', works('UC-04.03'), async ({ app, server }) => {
  await crmSchema(server)
  await server.run('field.set', { node: await idOf(server, 'Robin Ash'), field: 'Last contacted', value: '2026-09-01' })
  await server.run('field.set', { node: await idOf(server, 'Mia Chen'), field: 'Last contacted', value: '2026-10-05' })
  const s = (await server.run('search.create', { query: '#person', name: 'People' })).result.id
  await server.run('view.set', { node: s, kind: 'table' })
  await server.run('view.sort', { node: s, by: 'Last contacted', desc: true })
  await server.run('nav.open', { node: s })
  await expect(app.locator('.t-row[data-id]').first()).toContainText('Mia Chen')
  await server.run('view.sort', { node: s, by: 'Last contacted', desc: false })
  await expect(app.locator('.t-row[data-id]').first()).toContainText('Robin Ash')
})

test('UC-04.04 who to reconnect with: people whose next contact date has passed', works('UC-04.04'), async ({ app, server }) => {
  await crmSchema(server)
  await server.run('field.set', { node: await idOf(server, 'Robin Ash'), field: 'Next contact', value: '2026-10-01' })
  await server.run('field.set', { node: await idOf(server, 'Mia Chen'), field: 'Next contact', value: '2026-12-01' })
  const s = (await server.run('search.create', { query: '#person "Next contact"<today', name: 'Reconnect' })).result
  expect(s.results).toBe(1)
  await server.run('nav.open', { node: s.id })
  await expect(app.locator('.panel .title-input')).toHaveValue('Reconnect')
  await expect(app.locator(`.row.node[data-id="${await idOf(server, 'Robin Ash')}"]`)).toBeVisible()
  await expect(app.locator('.row.node', { hasText: 'Mia Chen' })).toHaveCount(0)
})

test('UC-04.05 a deal pipeline: cards grouped by stage, a new deal added in a column', works('UC-04.05'), async ({ app, server }) => {
  await server.run('tag.create', { name: 'deal', color: 'green' })
  await server.run('field.create', { name: 'Stage', type: 'options' })
  await server.run('field.setOptions', { field: 'Stage', options: ['Lead', 'Proposal', 'Won'] })
  await server.run('tag.addField', { tag: 'deal', field: 'Stage' })
  const { page } = await pageWith(server, app, [], 'Pipeline')
  const s = (await server.run('search.create', { query: '#deal', name: 'Deals', parent: page })).result.id
  await server.run('view.set', { node: s, kind: 'cards' })
  await server.run('view.group', { node: s, by: 'Stage' })
  await server.run('view.addItem', { node: s, name: 'Café planters', field: 'Stage', value: 'Proposal', stay: true })
  await server.run('nav.open', { node: s })
  await expect(app.locator('.board-col', { hasText: 'Proposal' }).locator('.card', { hasText: 'Café planters' })).toBeVisible()
  await app.locator('.card', { hasText: 'Café planters' }).dragTo(app.locator('.board-col', { hasText: 'Won' }))
  const id = await idOf(server, 'Café planters')
  await expect.poll(() => fieldOf(server, id, 'Stage')).toBe('Won')
})

test('UC-04.06 an email field warns about a value that is not an address', works('UC-04.06'), async ({ app, server }) => {
  const mia = await idOf(server, 'Mia Chen')
  await server.run('field.set', { node: mia, field: 'Email', value: 'mia at example' })
  await server.run('nav.open', { node: mia })
  await expect(app.locator('.row.field', { hasText: 'Email' }).locator('.field-warn')).toBeVisible()
  await server.run('field.set', { node: mia, field: 'Email', value: 'mia@example.org' })
  await expect(app.locator('.row.field', { hasText: 'Email' }).locator('.field-warn')).toHaveCount(0)
})

test('UC-04.07 everything that mentions a person, as a search (LINKS TO)', works('UC-04.07'), async ({ app, server }) => {
  const robin = await idOf(server, 'Robin Ash')
  const { ids } = await pageWith(server, app, ['Gift idea: pruning saw'])
  await server.run('node.reference', { target: robin, parent: ids[0] })
  const s = (await server.run('search.create', { query: `LINKS:${robin}`, name: 'About Robin' })).result
  expect(s.results).toBeGreaterThanOrEqual(2)
  await server.run('nav.open', { node: s.id })
  await expect(app.locator('.row.node', { hasText: 'Gift idea: pruning saw' })).toBeVisible()
})

test('UC-04.08 log an interaction on today\'s page with @person; it shows on their page', works('UC-04.08'), async ({ app, server }) => {
  const day = (await server.run('nav.today')).result.zoom
  const id = (await server.run('node.add', { name: '', parent: day })).result.id
  await focus(server, app, id, 0)
  await app.keyboard.type('Coffee with ')
  await app.keyboard.type('@Mia')
  await expect(app.locator('.menu.picker .menu-item.sel')).toContainText('Mia Chen')
  await app.keyboard.press('Enter')
  await app.keyboard.type(' about the tool library')
  await expect.poll(async () => (await tree(server, id)).name).toContain('about the tool library')
  await server.run('nav.open', { node: await idOf(server, 'Mia Chen') })
  await expect(app.locator('.references .reference-row', { hasText: 'about the tool library' })).toBeVisible()
})

missing('UC-04.09', 'sync contacts and deals from HubSpot or Pipedrive', 'out-of-scope')
missing('UC-04.10', 'scan a business card on the phone into a #person', 'out-of-scope')
test('UC-04.11 merge two duplicate #person nodes into one (Merge into)', works('UC-04.11'), async ({ app, server }) => {
  const { page, ids } = await pageWith(server, app, ['Dana Reyes', 'Dana Reyes (dup)', 'Lunch with '], 'People')
  for (const id of ids.slice(0, 2)) await server.run('tag.add', { node: id, tag: 'person' })
  await server.run('field.add', { node: ids[1], field: 'Phone', value: '555 0101' })
  // A line that mentions the duplicate.
  await server.run('node.setName', { node: ids[2], name: `Lunch with [[id:${ids[1]}]]` })
  await app.locator(`.row.node[data-id="${ids[1]}"] .bullet`).click({ button: 'right' })
  await app.locator('.node-menu .menu-item', { hasText: 'Merge into' }).click()
  await app.keyboard.type('Dana Reyes')
  await expect(app.locator('.menu.picker .menu-item.sel')).toContainText('Dana Reyes #person')
  await app.keyboard.press('Enter')
  // One Dana remains, with the phone number, and the mention points at her.
  await expect.poll(async () => (await tree(server, page)).children.map((c: any) => c.name)).toEqual(['Dana Reyes', `Lunch with [[id:${ids[0]}]]`])
  expect(await fieldOf(server, ids[0], 'Phone')).toBe('555 0101')
})
