// Use cases: meetings (docs/use-cases/03-meetings.md). Recording, transcripts and the meeting bot
// need AI or a cloud calendar and are listed as gaps.
import { test, expect, missing, works, partial, tree, fieldOf, idOf } from './helpers'
import { focus } from '../fixtures'
import type { Page } from '@playwright/test'
import type { Server } from '../fixtures'

/** A #meeting with an Attendees field (options from #person) and a Date set to the day it is written on. */
async function meetingSchema(server: Server) {
  await server.run('field.create', { name: 'Attendees', type: 'options_from_tag' })
  await server.run('field.setSourceTag', { field: 'Attendees', tag: 'person' })
  await server.run('field.create', { name: 'Date', type: 'date' })
  await server.run('field.setAutoInit', { field: 'Date', mode: 'ancestor_day' })
  await server.run('tag.addField', { tag: 'meeting', field: 'Attendees' })
  await server.run('tag.addField', { tag: 'meeting', field: 'Date' })
}

/** A new line on today's page, typed and tagged #meeting from the keyboard. */
async function newMeeting(server: Server, app: Page, name: string) {
  const day = (await server.run('nav.today')).result.zoom
  const id = (await server.run('node.add', { name: '', parent: day })).result.id
  await focus(server, app, id, 0)
  await app.keyboard.type(`${name} #meeting`)
  await expect(app.locator('.menu-item.sel')).toContainText('meeting')
  await app.keyboard.press('Enter')
  await expect.poll(async () => (await tree(server, id)).tags).toContain('meeting')
  return id as number
}

test('UC-03.01 tag a line #meeting and its template (Agenda, Notes, fields) fills in', works('UC-03.01'), async ({ app, server }) => {
  const id = await newMeeting(server, app, 'Board sync')
  await expect.poll(async () => (await tree(server, id)).children?.map((c: any) => c.name)).toEqual(['Agenda', 'Notes'])
  await server.run('node.expand', { node: id })
  await expect(app.locator('.row.field', { hasText: 'Outdoors' }).nth(1)).toBeVisible()
})

test('UC-03.02 pick attendees from your #person nodes in a field', works('UC-03.02'), async ({ app, server }) => {
  await meetingSchema(server)
  const id = await newMeeting(server, app, 'Shed planning')
  await server.run('node.expand', { node: id })
  const field = app.locator(`.row.field[data-owner="${id}"]`, { hasText: 'Attendees' })
  await field.locator('.field-value').first().dispatchEvent('mousedown')
  await expect(app.locator('.field-input')).toBeFocused()
  await app.keyboard.type('Robin')
  await expect(app.locator('.field-input')).toHaveValue('Robin')
  await expect(app.locator('.menu.picker .menu-item.sel')).toContainText('Robin Ash')
  await app.keyboard.press('Enter')
  await app.keyboard.press('Escape')
  await expect.poll(async () => JSON.stringify(await fieldOf(server, id, 'Attendees'))).toMatch(/Robin Ash/)
})

test('UC-03.03 the meeting date fills in from the day page it was written on', works('UC-03.03'), async ({ app, server }) => {
  await meetingSchema(server)
  const id = await newMeeting(server, app, 'Volunteer call')
  await expect.poll(() => fieldOf(server, id, 'Date')).toBe('2026-10-10')
})

test('UC-03.04 action items written in a meeting show up with all other open tasks', works('UC-03.04'), async ({ app, server }) => {
  const id = await newMeeting(server, app, 'Committee')
  await server.run('node.expand', { node: id })
  const notes = (await tree(server, id)).children[1].id
  const item = (await server.run('node.add', { name: '', parent: notes })).result.id
  await server.run('node.expand', { node: notes })
  await focus(server, app, item, 0)
  await app.keyboard.type('Email the council #task')
  await app.keyboard.press('Enter')
  await expect.poll(async () => (await tree(server, item)).tags).toContain('task')
  await server.run('nav.open', { node: await idOf(server, 'Open tasks') })
  await expect(app.locator('.row.node', { hasText: 'Email the council' })).toBeVisible()
})

test('UC-03.05 a person page lists the meetings they attended (a search on that page; not yet from the #person template)', partial('UC-03.05'), async ({ app, server }) => {
  await meetingSchema(server)
  const id = await newMeeting(server, app, 'Tool library chat')
  await server.run('field.set', { node: id, field: 'Attendees', value: 'Robin Ash' })
  const robin = await idOf(server, 'Robin Ash')
  const s = (await server.run('search.create', { query: '#meeting Attendees:PARENT', name: 'Meetings', parent: robin })).result.id
  await server.run('nav.open', { node: robin })
  await server.run('node.expand', { node: s })
  await expect(app.locator('.row.node', { hasText: 'Tool library chat' })).toBeVisible()
  // Gap: the same search as related content on #person (tag.addRelated) finds nothing, as PARENT
  // is not bound to each person.
  await server.run('tag.addRelated', { tag: 'person', query: '#meeting Attendees:PARENT', name: 'Met with' })
  await expect(app.locator('.side-card', { hasText: 'Met with' })).toContainText('Nothing here yet')
})

test('UC-03.06 meeting titles built from fields (title expression)', works('UC-03.06'), async ({ app, server }) => {
  await meetingSchema(server)
  await server.run('tag.setTitleExpression', { tag: 'meeting', expression: '${name} with ${Attendees}' })
  const id = await newMeeting(server, app, 'Catch-up')
  await server.run('field.set', { node: id, field: 'Attendees', value: 'Mia Chen' })
  await expect(app.locator(`.row.node[data-id="${id}"]`)).toContainText('Catch-up with Mia Chen')
})

test('UC-03.07 copy the meeting notes out as text to paste in an email', works('UC-03.07'), async ({ app, server, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write'])
  const id = await newMeeting(server, app, 'Budget review')
  await focus(server, app, id, 0)
  await app.keyboard.press('Escape')
  await app.keyboard.press('Meta+c')
  await expect.poll(() => app.evaluate(() => navigator.clipboard.readText())).toContain('Budget review')
  expect(await app.evaluate(() => navigator.clipboard.readText())).toContain('Agenda')
})

missing('UC-03.08', 'meetings of the day on each day page (a #day search with Date = PARENT)')
missing('UC-03.09', 'calendar events (Google or Outlook) appear on the day page as an Agenda', 'out-of-scope')
missing('UC-03.10', 'record a meeting without a bot and get a transcript and summary (AI notetaker)', 'ai')
missing('UC-03.11', 'a meeting agent bot joins the call and files notes, tasks and people (AI)', 'ai')
missing('UC-03.12', 'desktop notification when a calendar meeting starts, one click to record', 'out-of-scope')
missing('UC-03.13', 'notify a workspace member about an action item (Notify @person)')
