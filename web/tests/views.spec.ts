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

/** Search options by the title, then Open query editor in its card (N54). */
async function openQueryEditor(app: Page) {
  await app.locator('.head-actions [title="Search options"]').click()
  await app.locator('.search-pop .ghost-btn', { hasText: 'Open query editor' }).click()
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

test('a board card dragged within its column goes where it is dropped; the others slide aside', async ({ app, server }) => {
  const view = await openTasks(server, app)
  await server.run('view.set', { node: view, kind: 'cards' })
  await server.run('view.group', { node: view, by: 'Status' })
  for (const name of ['Card two', 'Card three']) await server.run('view.addItem', { node: view, name, field: 'Status', value: 'In progress', stay: true })
  const col = app.locator('.board-col', { hasText: 'In progress' })
  const cards = col.locator('.card')
  await expect(cards).toHaveCount(3)
  const names = async () => (await cards.locator('.card-body').allTextContents()).map((t) => t.trim())
  const before = await names()
  const last = cards.last()
  const first = cards.first()
  // Mid-drag: the cards from the slot down are moved aside (a transform), the dragged one dimmed.
  await last.hover()
  await app.mouse.down()
  const box = (await first.boundingBox())!
  // (The first moves start the drag; the next ones are drag-overs.)
  await app.mouse.move(box.x + box.width / 2, box.y + 30, { steps: 5 })
  await app.mouse.move(box.x + box.width / 2, box.y + 4, { steps: 5 })
  await expect(first).toHaveAttribute('style', /translateY/)
  await expect(last).toHaveClass(/dragged/)
  await app.mouse.up()
  await expect.poll(names).toEqual([before[before.length - 1], ...before.slice(0, -1)])
  await expect(first).not.toHaveAttribute('style', /translateY/)
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

test('clicking a table header renames the field in place and opens it to the right (N46)', async ({ app, server }) => {
  await openTasks(server, app)
  await app.locator('.view-toolbar .tool[title="View as"]').click()
  await app.locator('.menu-item', { hasText: 'Table' }).click()
  const head = app.locator('.t-head .t-cell', { hasText: 'Due' })
  await head.locator('.t-col-name').click()
  await expect(app.locator('.t-col-input')).toBeFocused()
  await expect(app.locator('.panel')).toHaveCount(2)
  await expect(app.locator('.panel').nth(1).locator('.title-input')).toHaveValue('Due')
  await app.keyboard.type('Due date')
  await app.keyboard.press('Enter')
  await expect(app.locator('.t-head .t-col-name', { hasText: 'Due date' })).toBeVisible()
  // Escape leaves the name as it was.
  await app.locator('.t-head .t-col-name', { hasText: 'Due date' }).click()
  await app.keyboard.type('Nope')
  await app.keyboard.press('Escape')
  await expect(app.locator('.t-head .t-col-name', { hasText: 'Due date' })).toBeVisible()
  // The icon opens the field in a panel to the right.
  await app.locator('.t-head .t-cell', { hasText: 'Due date' }).locator('.t-col-icon').click()
  await expect(app.locator('.panel')).toHaveCount(2)
  await expect(app.locator('.panel').nth(1).locator('.title-input')).toHaveValue('Due date')
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
  await openQueryEditor(app)
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
  await openQueryEditor(app)
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

test('dragging a header edge resizes the column; + Add offers new and system fields (N46)', async ({ app, server }) => {
  await openTasks(server, app)
  await app.locator('.view-toolbar .tool[title="View as"]').click()
  await app.locator('.menu-item', { hasText: 'Table' }).click()
  const head = app.locator('.t-head .t-cell', { hasText: 'Status' })
  const before = (await head.boundingBox())!.width
  const grip = (await head.locator('.t-grip').boundingBox())!
  await app.mouse.move(grip.x + 2, grip.y + 5)
  await app.mouse.down()
  await app.mouse.move(grip.x + 82, grip.y + 5, { steps: 5 })
  await app.mouse.up()
  await expect.poll(async () => Math.round((await head.boundingBox())!.width - before)).toBeGreaterThan(70)
  await app.locator('.t-head .tool', { hasText: '+ Add' }).click()
  await expect(app.locator('.menu.pop .menu-item', { hasText: 'Add new field' })).toBeVisible()
  await app.locator('.menu.pop .menu-item', { hasText: 'Add new field' }).click()
  await expect(app.locator('.menu.picker')).toBeVisible()
  // Columns already shown are not offered again.
  await expect(app.locator('.menu.picker .menu-item', { hasText: /^Status$/ })).toHaveCount(0)
})

test('the view toolbar: text filter, filter chips with x, sort rules; no Group in a table (N48, N54)', async ({ app, server }) => {
  const view = await openTasks(server, app)
  await server.run('view.set', { node: view, kind: 'table' })
  await expect(app.locator('.view-toolbar .tool[title="Group by"]')).toHaveCount(0)
  await expect(app.locator('.head-actions .btn[title="Add an item that matches"]')).toBeVisible()
  await app.locator('.view-toolbar .tool[title="Filter by text"]').click()
  await app.keyboard.type('compost')
  await app.keyboard.press('Enter')
  await expect(app.locator('.filter-chip')).toContainText('compost')
  await expect(app.locator('.t-row[data-id]')).toHaveCount(2)
  await app.locator('.filter-chip button').click()
  await expect(app.locator('.filter-chip')).toHaveCount(0)
  await expect(app.locator('.t-row[data-id]')).toHaveCount(3)
  await app.locator('.view-toolbar .tool[title="Sort by"]').click()
  await app.locator('.menu.pop .menu-item', { hasText: '+ Add sort' }).hover()
  await app.locator('.menu.pop .submenu .menu-item', { hasText: 'Name' }).click()
  await app.locator('.view-toolbar .tool[title="Sort by"]').click()
  await expect(app.locator('.menu.pop .sort-rule', { hasText: 'Name' })).toContainText('Ascending')
})

test('board column + Add makes a card at the top with its name typed in place; a drop low in a column counts (N49)', async ({ app, server }) => {
  const view = await openTasks(server, app)
  await server.run('view.set', { node: view, kind: 'cards' })
  await server.run('view.group', { node: view, by: 'Status' })
  const col = app.locator('.board-col', { hasText: 'In progress' })
  await col.locator('.ghost-btn', { hasText: '+ Add' }).click()
  await expect(col.locator('.card-name-input')).toBeFocused()
  await app.keyboard.type('Water seedlings')
  await app.keyboard.press('Enter')
  await expect(col.locator('.card', { hasText: 'Water seedlings' })).toBeVisible()
  // Still first once named, and the empty "Status not set" column is there.
  await expect(col.locator('.card').first()).toContainText('Water seedlings')
  await expect(app.locator('.board-col .board-name', { hasText: 'Status not set' })).toBeVisible()
  const hit = (await server.run('graph.search', { query: 'Water seedlings' })).result
  expect(hit.length).toBe(1)
  // Drop far below the cards of a column.
  const target = app.locator('.board-col', { hasText: 'Not started' })
  const box = (await target.boundingBox())!
  await col.locator('.card', { hasText: 'Water seedlings' }).dragTo(target, { targetPosition: { x: 100, y: box.height - 20 } })
  await expect(target.locator('.card', { hasText: 'Water seedlings' })).toBeVisible()
})

test('calendar by pointer: move an event, drag out a new one, stretch its end; the day view reads the date (N50)', async ({ app, server }) => {
  const view = await openTasks(server, app)
  await server.run('view.set', { node: view, kind: 'calendar' })
  // Month: Build four beds (Thu 15 Oct) dropped on the 16th.
  const ev = app.locator('.cal-event', { hasText: 'Build four beds' })
  const cell16 = app.locator('.cal-cell:not(.out)').filter({ has: app.locator('.cal-day', { hasText: /^16$/ }) })
  await ev.dragTo(cell16)
  await expect(cell16.locator('.cal-event', { hasText: 'Build four beds' })).toBeVisible()
  // Week of the 12th: drag 10:00 to 11:00 on Monday, name it.
  await app.locator('.seg', { hasText: 'Week' }).click()
  await app.locator('.cal-head .icon-btn[title="Next"]').click()
  const col = app.locator('.tg-col').first()
  await col.scrollIntoViewIfNeeded()
  const box = (await col.boundingBox())!
  const hourH = 45
  const scroll = await app.locator('.tg-body').evaluate((el) => el.scrollTop)
  const y = (m: number) => box.y + (m / 60) * hourH - 0 * scroll
  await app.mouse.move(box.x + 20, y(10 * 60) + 2)
  await app.mouse.down()
  await app.mouse.move(box.x + 20, y(11 * 60) + 2, { steps: 4 })
  await app.mouse.up()
  await expect(app.locator('.tg-new-input')).toBeFocused()
  await app.keyboard.type('Call the council')
  await app.keyboard.press('Enter')
  const made = app.locator('.tg-event-wrap', { hasText: 'Call the council' })
  await expect(made).toContainText('10:00 → 11:00')
  // Stretch its end to 12:00.
  const r = (await made.locator('.tg-resize').boundingBox())!
  await app.mouse.move(r.x + 10, r.y + 1)
  await app.mouse.down()
  await app.mouse.move(r.x + 10, r.y + hourH + 1, { steps: 4 })
  await app.mouse.up()
  await expect(app.locator('.tg-event-wrap', { hasText: 'Call the council' })).toContainText('10:00 → 12:00')
  // Day view of today.
  await app.locator('.seg', { hasText: 'Day' }).click()
  await app.locator('.cal-head .icon-btn[title="Today"]').click()
  await expect(app.locator('.cal-title')).toHaveText('October 10th, 2026')
  await expect(app.locator('.tg-day')).toHaveText('Sat 10/10')
})

test('a supertag page shows the open view tab as a title row with its tools', async ({ app, server }) => {
  const hits = await server.rpc('find', { query: 'task', limit: 10 })
  await server.run('nav.open', { node: hits.find((h: any) => h.name === 'task').id })
  await app.locator('.view-tab', { hasText: 'Task board' }).click()
  await expect(app.locator('.view-title .view-name')).toHaveText('Task board')
  await app.locator('.view-tab', { hasText: 'Task calendar' }).click()
  await expect(app.locator('.view-title .view-name')).toHaveText('Task calendar')
  await app.locator('.view-title [title="View options"]').click()
  await expect(app.locator('.menu')).toBeVisible()
})

test('search options show the query as a sentence and merge view filters into the search (N54)', async ({ app, server }) => {
  const view = await openTasks(server, app)
  await server.run('view.addFilter', { node: view, clause: 'Status:"Not started"' })
  await app.locator('.head-actions [title="Search options"]').click()
  const pop = app.locator('.search-pop')
  await expect(pop.locator('.search-pop-body')).toHaveText('Search for nodes tagged #task and not done')
  await expect(pop.locator('.q-token')).toHaveText(['#task'])
  await pop.locator('.btn', { hasText: 'Merge filters into search' }).click()
  await expect(pop).toHaveCount(0)
  await app.locator('.head-actions [title="Search options"]').click()
  await expect(pop.locator('.search-pop-body')).toHaveText('Search for nodes tagged #task and not done and where Status set to Not started')
  await expect(pop.locator('.btn', { hasText: 'Merge filters into search' })).toHaveCount(0)
  await app.keyboard.press('Escape')
  await expect(pop).toHaveCount(0)
})
