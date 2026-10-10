// Use cases: tasks and projects (docs/use-cases/02-tasks-projects.md).
import { test, expect, works, partial, tree, fieldOf, idOf, palette } from './helpers'
import { pageWith, focus, nameOf } from '../fixtures'

const done = async (server: any, id: number) => (await server.run('graph.inspect', { node: id })).result.done

test('UC-02.01 Cmd+Enter cycles a line through checkbox, done, no checkbox', works('UC-02.01'), async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['Water the seedlings'])
  await focus(server, app, ids[0], 0)
  await app.keyboard.press('Meta+Enter')
  await expect(app.locator(`.row.node[data-id="${ids[0]}"] .checkbox`)).toBeVisible()
  await expect.poll(() => done(server, ids[0])).toBe(false)
  await app.keyboard.press('Meta+Enter')
  await expect.poll(() => done(server, ids[0])).toBe(true)
  await expect(app.locator(`.editor-input[data-node="${ids[0]}"]`)).toBeFocused()
  await app.keyboard.press('Meta+Enter')
  await expect.poll(async () => (await done(server, ids[0])) ?? null).toBe(null)
  await expect(app.locator(`.row.node[data-id="${ids[0]}"] .checkbox`)).toHaveCount(0)
})

test('UC-02.02 Enter after a checkbox line starts another checkbox (a quick to-do list)', works('UC-02.02'), async ({ app, server }) => {
  const { page, ids } = await pageWith(server, app, ['Buy compost'])
  await focus(server, app, ids[0], 11)
  await app.keyboard.press('Meta+Enter')
  await app.keyboard.press('Enter')
  await app.keyboard.type('Buy mulch')
  await expect.poll(async () => (await tree(server, page)).children.map((c: any) => [c.name, c.done])).toEqual([
    ['Buy compost', false],
    ['Buy mulch', false],
  ])
})

test('UC-02.03 a parent shows a progress bar of the checkboxes below it by itself', works('UC-02.03'), async ({ app, server }) => {
  const { page, ids } = await pageWith(server, app, ['Build day'])
  for (const n of ['Saw boards', 'Screw frames', 'Level ground']) {
    const c = (await server.run('node.add', { name: n, parent: ids[0] })).result.id
    await server.run('node.setDone', { node: c, done: false })
  }
  const first = (await tree(server, ids[0])).children[0]
  await server.run('nav.open', { node: page })
  // As soon as a checkbox sits below (no Show progress bar needed).
  await expect(app.locator(`.row.node[data-id="${ids[0]}"] .progress`)).toHaveAttribute('title', '0 of 3 done')
  await server.run('node.expand', { node: ids[0] })
  const firstId = (await server.rpc('find', { query: first.name, limit: 5 }))[0].id
  await app.locator(`.row.node[data-id="${firstId}"] .checkbox`).click()
  await expect(app.locator(`.row.node[data-id="${ids[0]}"] .progress`)).toHaveAttribute('title', '1 of 3 done')
  // On a line of its own with its count (DIFFS N85).
  await expect(app.locator(`.row.node[data-id="${ids[0]}"] .progress-count`)).toHaveText('1 / 3')
})

test('UC-02.05 typing #task makes a task: checkbox and template fields appear', works('UC-02.05'), async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['Fix the gate latch'])
  await focus(server, app, ids[0], 18)
  await app.keyboard.type(' #task')
  await expect(app.locator('.menu-item.sel')).toContainText('task')
  await app.keyboard.press('Enter')
  const row = app.locator(`.row.node[data-id="${ids[0]}"]`)
  await expect(row.locator('.chip')).toContainText('task')
  await expect(row.locator('.checkbox')).toBeVisible()
  await server.run('node.expand', { node: ids[0] })
  await expect(app.locator('.row.field', { hasText: 'Status' }).first()).toBeVisible()
  await expect.poll(() => nameOf(server, ids[0])).toBe('Fix the gate latch')
})

test('UC-02.06 checking a task sets its Status field (done mapped to a field)', works('UC-02.06'), async ({ app, server }) => {
  await server.run('tag.setDoneMap', { tag: 'task', field: 'Status', checked: 'Done', unchecked: 'Todo' })
  const { ids } = await pageWith(server, app, ['Order seed trays'])
  await server.run('tag.add', { node: ids[0], tag: 'task' })
  await app.locator(`.row.node[data-id="${ids[0]}"] .checkbox`).click()
  await expect.poll(() => fieldOf(server, ids[0], 'Status')).toBe('Done')
  await app.locator(`.row.node[data-id="${ids[0]}"] .checkbox`).click()
  await expect.poll(() => fieldOf(server, ids[0], 'Status')).toBe('Todo')
})

test('UC-02.07 an Open tasks search gathers unchecked tasks from everywhere', works('UC-02.07'), async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['Sharpen the shears'])
  await server.run('tag.add', { node: ids[0], tag: 'task' })
  const s = await idOf(server, 'Open tasks')
  await server.run('nav.open', { node: s })
  await expect(app.locator('.row.node', { hasText: 'Sharpen the shears' })).toBeVisible()
  await expect(app.locator('.row.node', { hasText: 'Order cedar boards' })).toHaveCount(0)
})

test('UC-02.08 overdue tasks: unchecked tasks with a due date before today', works('UC-02.08'), async ({ app, server }) => {
  const late = await idOf(server, 'Fill with compost')
  const later = await idOf(server, 'Build four beds')
  await server.run('field.set', { node: late, field: 'Due', value: '2026-10-02' })
  await server.run('field.set', { node: later, field: 'Due', value: '2026-11-02' })
  const s = (await server.run('search.create', { query: '#task TODO Due<today', name: 'Overdue' })).result.id
  await server.run('nav.open', { node: s })
  await expect(app.locator('.row.node', { hasText: 'Fill with compost' })).toBeVisible()
  await expect(app.locator('.row.node', { hasText: 'Build four beds' })).toHaveCount(0)
})

test('UC-02.09 a board grouped by Status; dragging a card changes its Status', works('UC-02.09'), async ({ app, server }) => {
  const s = await idOf(server, 'Open tasks')
  await server.run('view.set', { node: s, kind: 'cards' })
  await server.run('view.group', { node: s, by: 'Status' })
  await server.run('nav.open', { node: s })
  await app.locator('.card', { hasText: 'Fill with compost' }).dragTo(app.locator('.board-col', { hasText: 'In progress' }))
  const id = await idOf(server, 'Fill with compost')
  await expect.poll(() => fieldOf(server, id, 'Status')).toBe('In progress')
})

test('UC-02.10 a project page lists its own tasks (a search with Project = PARENT)', works('UC-02.10'), async ({ app, server }) => {
  await server.run('field.create', { name: 'Project', type: 'options_from_tag' })
  await server.run('field.setSourceTag', { field: 'Project', tag: 'project' })
  const garden = await idOf(server, 'Community garden')
  const { ids } = await pageWith(server, app, ['Paint the shed', 'Plan the party'])
  for (const t of ids) await server.run('tag.add', { node: t, tag: 'task' })
  await server.run('field.add', { node: ids[0], field: 'Project', value: 'Community garden' })
  await server.run('search.create', { query: '#task Project:PARENT', name: 'Tasks for this project', parent: garden })
  await server.run('nav.open', { node: garden })
  const results = app.locator('.row.node', { hasText: 'Tasks for this project' })
  await expect(results).toBeVisible()
  await server.run('node.expand', { node: await idOf(server, 'Tasks for this project') })
  await expect(app.locator('.row.node', { hasText: 'Paint the shed' })).toBeVisible()
  await expect(app.locator('.row.node', { hasText: 'Plan the party' })).toHaveCount(0)
})

test('UC-02.11 gather loose checkboxes under a page (TODO inside the parent); no Find todos within command', partial('UC-02.11'), async ({ app, server }) => {
  const { page, ids } = await pageWith(server, app, ['Notes from the walk'])
  const c = (await server.run('node.add', { name: 'Ask about the hose', parent: ids[0] })).result.id
  await server.run('node.setDone', { node: c, done: false })
  const s = (await server.run('search.create', { query: 'TODO SCOPE:PARENT', name: 'Todos here', parent: page })).result
  expect(s.results).toBe(1)
  await server.run('node.expand', { node: s.id })
  await expect(app.locator('.row.node', { hasText: 'Ask about the hose' }).nth(0)).toBeVisible()
})

test('UC-02.12 Move to: send a line to another page from the selection toolbar', works('UC-02.12'), async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['Borrow a wheelbarrow'])
  await focus(server, app, ids[0], 0)
  await app.keyboard.press('Escape')
  await app.locator('.sel-toolbar button', { hasText: 'Move' }).click()
  await app.keyboard.type('Ideas')
  await expect(app.locator('.menu.picker .menu-item.sel')).toContainText('Ideas')
  await app.keyboard.press('Enter')
  const ideas = await idOf(server, 'Ideas')
  await expect.poll(async () => (await tree(server, ideas)).children.map((c: any) => c.name)).toContain('Borrow a wheelbarrow')
})

test('UC-02.13 a calendar of tasks by due date; dragging an event changes the date', works('UC-02.13'), async ({ app, server }) => {
  const s = await idOf(server, 'Open tasks')
  await server.run('view.set', { node: s, kind: 'calendar' })
  await server.run('nav.open', { node: s })
  const ev = app.locator('.cal-event', { hasText: 'Build four beds' })
  await expect(ev).toBeVisible()
  await ev.dragTo(app.locator('.cal-day:not(.out)', { hasText: /^\s*20\s*$/ }).first())
  const id = await idOf(server, 'Build four beds')
  await expect.poll(() => fieldOf(server, id, 'Due')).toBe('2026-10-20')
})

test('UC-02.14 subtasks under a project get #task by themselves (default child supertag)', works('UC-02.14'), async ({ app, server }) => {
  await server.run('tag.setChildTag', { tag: 'project', child: 'task' })
  const { ids } = await pageWith(server, app, ['Pond'])
  await server.run('tag.add', { node: ids[0], tag: 'project' })
  const c = (await server.run('node.add', { name: '', parent: ids[0] })).result.id
  await server.run('node.expand', { node: ids[0] })
  await focus(server, app, c, 0)
  await app.keyboard.type('Dig the hole')
  await app.keyboard.press('Enter')
  await app.keyboard.type('Line it')
  await expect.poll(async () => (await tree(server, ids[0])).children.map((k: any) => [k.name, k.tags])).toEqual([
    ['Dig the hole', expect.arrayContaining(['task'])],
    ['Line it', ['task']],
  ])
})

// Typing # or using # Tag with two lines selected tags only one of them.
test('UC-02.15 tag several selected lines at once', works('UC-02.15'), async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['Rake leaves', 'Turn compost', 'Mulch beds'])
  await focus(server, app, ids[0], 0)
  await app.keyboard.press('Shift+ArrowDown')
  await app.keyboard.press('Shift+ArrowDown')
  await expect(app.locator('.row.selected')).toHaveCount(2)
  await app.locator('.sel-toolbar button', { hasText: '# Tag' }).click()
  await app.keyboard.type('task')
  await expect(app.locator('.menu.picker .menu-item.sel')).toContainText('task')
  await app.keyboard.press('Enter')
  await expect.poll(async () => Promise.all(ids.map(async (id) => (await tree(server, id)).tags ?? []))).toEqual([['task'], ['task'], []])
  // `#` on the selection does the same (DIFFS N81), and types nothing into a line.
  await focus(server, app, ids[1], 0)
  await app.keyboard.press('Shift+ArrowDown')
  await app.keyboard.press('Shift+ArrowDown')
  await expect(app.locator('.row.selected')).toHaveCount(2)
  await app.keyboard.type('#')
  await expect(app.locator('.menu.picker')).toBeVisible()
  await app.keyboard.type('chore')
  await app.keyboard.press('Enter')
  await expect.poll(async () => Promise.all(ids.map(async (id) => (await tree(server, id)).tags ?? []))).toEqual([['task'], ['task', 'chore'], ['chore']])
  expect(await Promise.all(ids.map((id) => nameOf(server, id)))).toEqual(['Rake leaves', 'Turn compost', 'Mulch beds'])
})

test('UC-02.16 process an inbox into a move target (GTD style)', works('UC-02.16'), async ({ app, server }) => {
  const ideas = await idOf(server, 'Ideas')
  await server.run('node.setMoveTarget', { node: ideas })
  const { ids } = await pageWith(server, app, ['Seed library box'])
  await focus(server, app, ids[0], 0)
  await palette(app, 'Move to move target')
  await expect.poll(async () => (await tree(server, ideas)).children.map((c: any) => c.name)).toContain('Seed library box')
})

test('UC-02.17 a task table sums the hours column', works('UC-02.17'), async ({ app, server }) => {
  const s = await idOf(server, 'Open tasks')
  await server.run('view.set', { node: s, kind: 'table' })
  await server.run('view.calculate', { node: s, field: 'Effort (hours)', calc: 'sum' })
  await server.run('nav.open', { node: s })
  await expect(app.locator('.table')).toBeVisible()
  // Effort 6 + 3 under the column.
  await expect(app.locator('.table .t-foot')).toContainText('9')
})

test('UC-02.18 lanes: a project\'s sections side by side', works('UC-02.18'), async ({ app, server }) => {
  const garden = await idOf(server, 'Community garden')
  await server.run('view.set', { node: garden, kind: 'lanes' })
  await server.run('nav.open', { node: garden })
  for (const n of ['Raised beds', 'Seed list', 'Water rota']) await expect(app.getByText(n, { exact: true }).first()).toBeVisible()
})

test('UC-02.04 a pinned project shows its progress ring in the sidebar', works('UC-02.04'), async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['Shed rebuild'])
  const kids: number[] = []
  for (const n of ['Order wood', 'Pour base']) {
    const c = (await server.run('node.add', { name: n, parent: ids[0] })).result.id
    await server.run('node.setDone', { node: c, done: false })
    kids.push(c)
  }
  await server.run('node.togglePin', { node: ids[0] })
  const ring = app.locator('.sidebar .side-row', { hasText: 'Shed rebuild' }).locator('.progress-ring')
  await expect(ring).toHaveAttribute('aria-label', '0 of 2 done')
  await server.run('node.setDone', { node: kids[0], done: true })
  await expect(ring).toHaveAttribute('aria-label', '1 of 2 done')
})
test('UC-02.19 Cmd+Shift+M opens Move to from the keyboard', works('UC-02.19'), async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['Return the ladder'])
  await focus(server, app, ids[0], 3)
  await app.keyboard.press('Meta+Shift+M')
  await expect(app.locator('.menu.picker')).toBeVisible()
  await app.keyboard.type('Ideas')
  await expect(app.locator('.menu.picker .menu-item.sel')).toContainText('Ideas')
  await app.keyboard.press('Enter')
  const ideas = await idOf(server, 'Ideas')
  await expect.poll(async () => (await tree(server, ideas)).children.map((c: any) => c.name)).toContain('Return the ladder')
})
test('UC-02.20 commands on events: checking a task stamps a field; new children of an Inbox become tasks', works('UC-02.20'), async ({ app, server }) => {
  await server.run('commandNode.create', { name: 'Stamp done', steps: ['field.add {"field": "Finished", "value": "yes"}'] })
  await server.run('commandNode.create', { name: 'Make task', steps: ['tag.add {"tag": "task"}'] })
  const { ids } = await pageWith(server, app, ['Inbox'])
  // On new children: from the command line on the Inbox line.
  await focus(server, app, ids[0], 0)
  await palette(app, 'On new children run Make task')
  await expect.poll(async () => (await tree(server, ids[0])).onChild).toEqual(['Make task'])
  const item = (await server.run('node.add', { name: 'Call the plumber', parent: ids[0] })).result.id
  await expect.poll(async () => (await tree(server, item)).tags).toEqual(['task'])
  // When checked: Configure supertag, Commands.
  const task = (await server.run('graph.tags')).result.find((t: any) => t.name === 'task').id
  await server.run('nav.open', { node: task })
  // The test page has an Options button too: wait for the tag page before clicking.
  await expect(app.locator('.panel .title-input')).toHaveValue('task')
  await app.locator('.head-actions .btn[title="Options"]').click()
  await app.locator('.node-menu .menu-item', { hasText: 'Configure supertag' }).click()
  await app.locator('.cfg-card-head', { hasText: 'Command nodes that run' }).click()
  await app.locator('select[aria-label="When checked done"]').selectOption({ label: 'Stamp done' })
  await expect.poll(async () => (await server.rpc('schema', { node: task })).onDone).toBeTruthy()
  await server.run('node.setDone', { node: item, done: true })
  await expect.poll(async () => (await tree(server, item)).fields?.Finished).toBeTruthy()
})
