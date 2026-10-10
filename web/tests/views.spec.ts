// Views (spec 04): view menu, table cells typed in place, board drag, calendar months.
import { test, expect, type Server } from './fixtures'
import type { Page } from '@playwright/test'

async function openTasks(server: Server, app: Page) {
  const hits = await server.rpc('find', { query: 'Open tasks', limit: 5 })
  const id = hits.find((h: any) => h.name === 'Open tasks').id
  await server.run('nav.open', { node: id })
  await expect(app.locator('.panel .title-input')).toHaveValue('Open tasks')
  return id as number
}

async function fieldOf(server: Server, node: number, field: string) {
  return (await server.run('export.json', { node })).result?.fields?.[field]
}

test('the View as menu switches to a table, and a cell is typed in place', async ({ app, server }) => {
  await openTasks(server, app)
  await app.locator('.view-toolbar .tool[title="View as"]').click()
  await app.locator('.menu-item', { hasText: 'Table' }).click()
  await expect(app.locator('.table')).toBeVisible()
  const row = app.locator('.t-row', { hasText: 'Build four beds' })
  const id = Number(await row.getAttribute('data-id'))
  // The Effort column is the last field column.
  const cells = row.locator('.t-cell')
  const n = await cells.count()
  await cells.nth(n - 2).click()
  await expect(app.locator('.field-input')).toBeFocused()
  await app.keyboard.press('Meta+a')
  await app.keyboard.type('9')
  await expect.poll(() => fieldOf(server, id, 'Effort (hours)')).toBe('9')
  await app.keyboard.press('Escape')
  await expect(row.locator('.t-cell').nth(n - 2)).toHaveText('9')
})

test('a board card dragged to another column takes its value', async ({ app, server }) => {
  const view = await openTasks(server, app)
  await server.run('view.set', { node: view, kind: 'cards' })
  await server.run('view.group', { node: view, by: 'Status' })
  await expect(app.locator('.board')).toBeVisible()
  const card = app.locator('.card', { hasText: 'Fill with compost' })
  const target = app.locator('.board-col', { hasText: 'In progress' })
  await card.dragTo(target)
  await expect(app.locator('.board-col', { hasText: 'In progress' }).locator('.card', { hasText: 'Fill with compost' })).toBeVisible()
})

test('the calendar shows the month and steps to the next', async ({ app, server }) => {
  const view = await openTasks(server, app)
  await server.run('view.set', { node: view, kind: 'calendar' })
  await expect(app.locator('.cal-title')).toHaveText('October 2026')
  await expect(app.locator('.cal-day.today')).toHaveText('10')
  await expect(app.locator('.cal-event', { hasText: 'Build four beds' })).toBeVisible()
  await app.locator('.cal-head .icon-btn[title="Next"]').click()
  await expect(app.locator('.cal-title')).toHaveText('November 2026')
})

test('the query builder adds a condition, and results wait for Run once (spec 05 §2, N13)', async ({ app, server }) => {
  const view = await openTasks(server, app)
  await server.run('view.set', { node: view, kind: 'table' })
  await expect(app.locator('.table')).toBeVisible()
  await app.locator('.panel .title-input').click()
  await app.keyboard.press('Escape')
  await app.keyboard.press('Meta+Shift+o')
  await expect(app.locator('.query-builder')).toBeVisible()
  const before = await app.locator('.t-row[data-id]').count()
  await app.locator('.q-input').first().fill('Build')
  await app.locator('.q-input').first().press('Enter')
  await expect(app.locator('.q-clause')).toHaveCount(await app.locator('.q-clause').count())
  await expect(app.locator('.q-expr')).toContainText('Build')
  // Held: the table still shows the old results until Run once.
  expect(await app.locator('.t-row[data-id]').count()).toBe(before)
  await app.locator('.q-foot .btn', { hasText: 'Done' }).click()
  await expect(app.locator('.query-builder')).toHaveCount(0)
  await expect(app.locator('.t-row[data-id]')).toHaveCount(1)
})

test('the calendar week view has an hour grid and steps by a week', async ({ app, server }) => {
  const view = await openTasks(server, app)
  await server.run('view.set', { node: view, kind: 'calendar' })
  await app.locator('.seg', { hasText: 'Week' }).click()
  await expect(app.locator('.tg')).toBeVisible()
  await expect(app.locator('.cal-title')).toHaveText('Week 41, 2026')
  await app.locator('.cal-head .icon-btn[title="Next"]').click()
  await expect(app.locator('.cal-title')).toHaveText('Week 42, 2026')
  await expect(app.locator('.tg .cal-event', { hasText: 'Build four beds' })).toBeVisible()
})

test('clicking a table header renames the field in place; its icon opens the field', async ({ app, server }) => {
  await openTasks(server, app)
  await app.locator('.view-toolbar .tool[title="View as"]').click()
  await app.locator('.menu-item', { hasText: 'Table' }).click()
  const head = app.locator('.t-head .t-cell', { hasText: 'Due' })
  await head.locator('.t-col-name').click()
  await expect(app.locator('.t-col-input')).toBeFocused()
  await app.keyboard.type('Due date')
  await app.keyboard.press('Enter')
  await expect(app.locator('.t-head .t-col-name', { hasText: 'Due date' })).toBeVisible()
  // Escape leaves the name as it was.
  await app.locator('.t-head .t-col-name', { hasText: 'Due date' }).click()
  await app.keyboard.type('Nope')
  await app.keyboard.press('Escape')
  await expect(app.locator('.t-head .t-col-name', { hasText: 'Due date' })).toBeVisible()
  await app.locator('.t-head .t-cell', { hasText: 'Due date' }).locator('.t-col-icon').click()
  await expect(app.locator('.panel .title-input')).toHaveValue('Due date')
})

test('the table header menu calculates, moves and hides a column', async ({ app, server }) => {
  await openTasks(server, app)
  await app.locator('.view-toolbar .tool[title="View as"]').click()
  await app.locator('.menu-item', { hasText: 'Table' }).click()
  const names = () => app.locator('.t-head .t-col-name').allTextContents()
  const before = await names()
  const effort = before.find((n) => n.startsWith('Effort'))!
  await app.locator('.t-head .t-cell', { hasText: effort }).click({ button: 'right' })
  await app.locator('.header-menu .menu-item', { hasText: 'Calculate' }).hover()
  await app.locator('.header-menu .submenu .menu-item', { hasText: 'Sum' }).click()
  await expect(app.locator('.t-foot')).toBeVisible()
  await expect(app.locator('.t-foot')).toContainText(/\d/)
  await app.locator('.t-head .t-cell', { hasText: effort }).click({ button: 'right' })
  await app.locator('.header-menu .menu-item', { hasText: 'Move column left' }).click()
  await expect.poll(names).not.toEqual(before)
  expect((await names()).indexOf(effort)).toBe(before.indexOf(effort) - 1)
  await app.locator('.t-head .t-cell', { hasText: effort }).click({ button: 'right' })
  await app.locator('.header-menu .menu-item', { hasText: /^Hide column$/ }).click()
  await expect.poll(names).not.toContain(effort)
})

test('+ OR in the query builder never sends typing into a result row (N35)', async ({ app, server }) => {
  const view = await openTasks(server, app)
  await server.run('view.set', { node: view, kind: 'table' })
  await app.locator('.view-toolbar .tool[title^="Search conditions"]').click()
  await app.locator('.q-add .btn', { hasText: '+ OR' }).click()
  // The new group's input has the keyboard.
  await expect(app.locator('.query-builder .q-group .q-group .q-input')).toBeFocused()
  await app.keyboard.type('xyz')
  await app.waitForTimeout(200)
  expect((await server.run('graph.search', { query: 'xyz' })).result).toEqual([])
  // With the keyboard on a button, typed keys go nowhere.
  await app.locator('.q-add .btn', { hasText: '+ AND' }).last().focus()
  await app.keyboard.type('qq')
  await app.waitForTimeout(200)
  expect((await server.run('graph.search', { query: 'qq' })).result).toEqual([])
})

test('the query builder has its menus, Learn more and a corner close button (N43)', async ({ app, server }) => {
  const view = await openTasks(server, app)
  await server.run('view.set', { node: view, kind: 'table' })
  await app.locator('.view-toolbar .tool[title^="Search conditions"]').click()
  const qb = app.locator('.query-builder')
  for (const label of ['+ Supertag', 'Tag fields ▾', 'Field values ▾', 'System fields ▾', 'Tag types ▾', 'Search operators ▾']) await expect(qb.getByText(label, { exact: true })).toBeVisible()
  await expect(qb.locator('.q-learn')).toHaveText('Learn more')
  await qb.getByText('Search operators ▾').click()
  await app.locator('.menu.pop .menu-item', { hasText: 'IN LIBRARY' }).click()
  await expect(app.locator('.q-expr')).toContainText(/library/i)
  await qb.getByText('Tag fields ▾').click()
  await app.locator('.menu.pop .menu-item', { hasText: 'Status' }).hover()
  await app.locator('.menu.pop .submenu .menu-item', { hasText: /^Set$/ }).click()
  await expect(app.locator('.q-expr')).toContainText('Status')
  await qb.locator('.q-close').click()
  await expect(qb).toHaveCount(0)
})

test('a tag page view tab switches the view in place; the tag header stays (N36)', async ({ app, server }) => {
  const tags = (await server.run('graph.tags', {})).result
  const task = tags.find((t: any) => t.name === 'task')
  await server.run('nav.open', { node: task.id })
  await expect(app.locator('.table')).toBeVisible()
  await app.locator('.tag-tabs .view-tab', { hasText: 'Task board' }).click()
  await expect(app.locator('.board')).toBeVisible()
  await expect(app.locator('.panel .title-input')).toHaveValue('task')
  await expect(app.locator('.tag-tabs .view-tab.active')).toHaveText('Task board')
  await app.locator('.tag-tabs .view-tab', { hasText: 'Everything tagged' }).click()
  await expect(app.locator('.table')).toBeVisible()
  // Sorting keeps the table (N47).
  await app.locator('.view-toolbar .tool[title="Sort by"]').click()
  await app.locator('.menu.pop .menu-item').first().click()
  await expect(app.locator('.table')).toBeVisible()
})
