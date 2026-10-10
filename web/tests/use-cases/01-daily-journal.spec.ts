// Use cases: daily journal and calendar pages (docs/use-cases/01-daily-journal.md).
// The fixture's today is Sat, 10 Oct 2026.
import { test, expect, missing, works, partial, idOf, library, palette, page } from './helpers'

const title = (app: import('@playwright/test').Page) => app.locator('.panel .title').first()

test('UC-01.01 open today from anywhere (sidebar Today, Ctrl+Shift+D)', works('UC-01.01'), async ({ app, server }) => {
  await expect(title(app)).toContainText('Sat, 10 Oct')
  await server.run('nav.open', { node: await library(server) })
  await expect(title(app)).toContainText('Library')
  await app.locator('.sidebar .side-row', { hasText: 'Today' }).click()
  await expect(title(app)).toContainText('Sat, 10 Oct')
  await server.run('nav.open', { node: await library(server) })
  await expect(title(app)).toContainText('Library')
  await app.locator('body').click({ position: { x: 700, y: 900 } })
  await app.keyboard.press('Control+Shift+d')
  await expect(title(app)).toContainText('Sat, 10 Oct')
})

test('UC-01.02 step to the previous and next day', works('UC-01.02'), async ({ app }) => {
  await app.locator('.day-nav [title="Previous day"]').click()
  await expect(title(app)).toContainText('Fri, 9 Oct')
  await app.locator('.day-nav [title="Next day"]').click()
  await app.locator('.day-nav [title="Next day"]').click()
  await expect(title(app)).toContainText('Sun, 11 Oct')
  await app.locator('.day-nav .btn', { hasText: 'Today' }).click()
  await expect(title(app)).toContainText('Sat, 10 Oct')
})

test('UC-01.03 jump to any date with the date picker', works('UC-01.03'), async ({ app }) => {
  await app.locator('.day-nav [title="Go to date"]').click()
  await app.locator('.date-picker .dp-day:not(.out)', { hasText: /^22$/ }).click()
  await app.locator('.date-picker .btn', { hasText: 'OK' }).click()
  await expect(title(app)).toContainText('22 Oct')
})

test('UC-01.04 a day page sits in its week and year; the breadcrumbs open them', works('UC-01.04'), async ({ app }) => {
  await app.locator('.crumb', { hasText: 'Week 41' }).click()
  await expect(app.locator('.panel .title-input, .panel .title').first()).toContainText('Week 41')
  await expect(app.locator('.row.node', { hasText: 'Sat, 10 Oct' })).toBeVisible()
  await app.locator('.crumb', { hasText: '2026' }).click()
  await expect(app.locator('.row.node', { hasText: 'Week 41' })).toBeVisible()
})

test('UC-01.05 every new day gets the #day template (tag and its Open tasks card)', works('UC-01.05'), async ({ app, server }) => {
  await app.locator('.day-nav [title="Next day"]').click()
  await expect(title(app)).toContainText('Sun, 11 Oct')
  await expect(app.locator('.panel .chip', { hasText: 'day' }).first()).toBeVisible()
  await expect(app.locator('.side-card h3', { hasText: 'Open tasks' })).toBeVisible()
  expect((await page(server)).tags.map((t: any) => t.name)).toContain('day')
})

test('UC-01.06 quick add drops a thought on today without leaving the page', works('UC-01.06'), async ({ app, server }) => {
  await server.run('nav.open', { node: await library(server) })
  await expect(title(app)).toContainText('Library')
  await app.keyboard.press('Meta+e')
  const input = app.locator('.dialog.quick input')
  await expect(input).toBeFocused()
  await app.keyboard.type('Buy twine for the bean poles')
  await app.keyboard.press('Enter')
  await expect(title(app)).toContainText('Library')
  await server.run('nav.today')
  await expect(app.locator('.row.node', { hasText: 'Buy twine for the bean poles' })).toBeVisible()
})

test('UC-01.07 a node dated by a field shows in that day\'s references', works('UC-01.07'), async ({ app, server }) => {
  const task = await idOf(server, 'Build four beds')
  await server.run('field.set', { node: task, field: 'Due', value: '2026-10-12' })
  await server.run('nav.day', { date: '2026-10-12' })
  await expect(title(app)).toContainText('Mon, 12 Oct')
  await expect(app.locator('.references .reference-row', { hasText: 'Build four beds' })).toBeVisible()
})

test('UC-01.08 put a date in a line (Insert date command)', works('UC-01.08'), async ({ app, server }) => {
  const day = (await server.run('nav.today')).result.zoom
  const id = (await server.run('node.add', { name: 'Plant garlic ', parent: day })).result.id
  await server.run('focus.set', { node: id, caret: 13 })
  await expect(app.locator(`.editor-input[data-node="${id}"]`)).toBeFocused()
  await palette(app, 'Insert date')
  await expect.poll(async () => (await server.run('graph.inspect', { node: id })).result.name).toMatch(/Plant garlic \S+/)
})

test('UC-01.08b type @next friday or @Nov 21 to 30 and the words become date links', works('UC-01.08b'), async ({ app, server }) => {
  const day = (await server.run('nav.today')).result.zoom
  const id = (await server.run('node.add', { name: 'Call the plumber', parent: day })).result.id
  await server.run('focus.set', { node: id, caret: 16 })
  await expect(app.locator(`.editor-input[data-node="${id}"]`)).toBeFocused()
  await app.keyboard.type(' @next friday')
  await expect(app.locator('.menu.picker .menu-item').first()).toContainText('Fri, 16 Oct')
  await app.keyboard.press('Enter')
  await app.keyboard.type(' ok')
  const name = async () => (await server.run('graph.inspect', { node: id })).result.name as string
  await expect.poll(name).toMatch(/^Call the plumber \[\[id:\d+\]\] ok$/)
  const target = Number(/id:(\d+)/.exec(await name())![1])
  expect((await server.run('graph.inspect', { node: target })).result.name).toBe('Fri, 16 Oct')
  // A range links both days.
  await app.keyboard.type(' @Nov 21 to 30')
  await expect(app.locator('.menu.picker .menu-item').first()).toContainText('Sat, 21 Nov to Mon, 30 Nov')
  await app.keyboard.press('Enter')
  await expect.poll(name).toMatch(/ok \[\[id:\d+\]\] to \[\[id:\d+\]\]$/)
})

test('UC-01.10 habit check-off: tick a habit each day and count what was done recently', partial('UC-01.10'), async ({ app, server }) => {
  const day = (await server.run('nav.today')).result.zoom
  const h = (await server.run('node.add', { name: 'Stretch 10 minutes', parent: day })).result.id
  await server.run('focus.set', { node: h, caret: 0 })
  await expect(app.locator(`.editor-input[data-node="${h}"]`)).toBeFocused()
  await app.keyboard.press('Meta+Enter')
  await app.keyboard.press('Meta+Enter')
  await expect.poll(async () => (await server.run('graph.inspect', { node: h })).result.done).toBe(true)
  const s = (await server.run('search.create', { query: 'Stretch DONE:7', name: 'Stretched this week', parent: day })).result
  expect(s.results).toBe(1)
  // Gap: no streak or calendar overview of a habit (the template's chain view).
})

test('UC-01.11 weekly review page: week nodes carry their own #week tag', works('UC-01.11'), async ({ app }) => {
  await app.locator('.crumb', { hasText: 'Week 41' }).click()
  await expect(app.locator('.panel .chip', { hasText: 'week' }).first()).toBeVisible()
})

test('UC-01.13 week start and 12/24-hour time are settings; calendar views follow the week start', partial('UC-01.13'), async ({ app, server }) => {
  const tasks = await idOf(server, 'Open tasks')
  await server.run('view.set', { node: tasks, kind: 'calendar' })
  await server.run('nav.open', { node: tasks })
  await expect(app.locator('.cal-dow').first()).toContainText('Mon')
  await app.locator('.topbar [title="Account"]').click()
  await app.locator('.account-menu .menu-item', { hasText: 'Settings' }).click()
  const settings = app.locator('.panel.settings-panel')
  await settings.locator('.settings-nav .view-tab', { hasText: 'Date and time' }).click()
  await settings.locator('.seg', { hasText: 'Sunday' }).click()
  await settings.locator('.seg', { hasText: '12h' }).click()
  await expect.poll(async () => (await server.run('settings.get')).result.week_start).toBe('sunday')
  await expect.poll(async () => (await server.run('settings.get')).result.time_format).toBe('12h')
  await expect(app.locator('.cal-dow').first()).toContainText('Sun')
  // Gap: the date picker still starts its weeks on Monday.
})

missing('UC-01.09', 'journal prompts: a field that auto-fills with a random node of a supertag')
test('UC-01.12 Add to day (Remind me): a reference of the node on the chosen day', works('UC-01.12'), async ({ app, server }) => {
  const day = (await server.run('nav.today')).result.zoom
  const id = (await server.run('node.add', { name: 'Read the soil article', parent: day })).result.id
  await app.locator(`.row.node[data-id="${id}"] .bullet`).click({ button: 'right' })
  await app.locator('.node-menu .menu-item', { hasText: 'Add to day' }).hover()
  await app.locator('.node-menu .menu-item', { hasText: /^Tomorrow$/ }).click()
  await app.locator('.day-nav .btn[title="Next day"]').click()
  await expect(app.locator('.panel .title').first()).toContainText('Sun, 11 Oct')
  await expect(app.locator('.row.node', { hasText: 'Read the soil article' })).toBeVisible()
})
missing('UC-01.14', 'daily notes for a second workspace (Today follows the main workspace only)')
test('UC-01.15 every day page lists the tasks due that day and the overdue ones (PARENT is the day)', works('UC-01.15'), async ({ app, server }) => {
  await server.run('field.set', { node: await idOf(server, 'Build four beds'), field: 'Due', value: '2026-10-12' })
  await server.run('field.set', { node: await idOf(server, 'Fill with compost'), field: 'Due', value: '2026-10-02' })
  await server.run('tag.addRelated', { tag: 'day', query: '#task Due:PARENT', name: 'Due this day' })
  await server.run('tag.addRelated', { tag: 'day', query: '#task NOT DONE Due<PARENT', name: 'Overdue' })
  await server.run('nav.day', { date: '2026-10-12' })
  await expect(app.locator('.side-card', { hasText: 'Due this day' })).toContainText('Build four beds')
  await expect(app.locator('.side-card', { hasText: 'Overdue' })).toContainText('Fill with compost')
  // A past day: what was due then.
  await server.run('nav.day', { date: '2026-10-02' })
  await expect(app.locator('.side-card', { hasText: 'Due this day' })).toContainText('Fill with compost')
  await expect(app.locator('.side-card', { hasText: 'Overdue' })).not.toContainText('Fill with compost')
})
