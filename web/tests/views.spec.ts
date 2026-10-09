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
  await app.locator('.view-toolbar .tool').first().click()
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
