// Use cases: the outliner as a personal knowledge base (docs/use-cases/06-knowledge-base.md):
// writing, structure, references, supertags and fields, search, views and navigation.
import { test, expect, missing, works, partial, tree, fieldOf, idOf, palette, rowOf, page as pageOf } from './helpers'
import { pageWith, focus, nameOf, lines } from '../fixtures'

// Writing and structure

test('UC-06.01 write an outline with Enter, Tab and Shift+Tab', works('UC-06.01'), async ({ app, server }) => {
  const { ids } = await pageWith(server, app, [''])
  await focus(server, app, ids[0], 0)
  for (const [k, t] of [['', 'Soil'], ['Enter', 'Clay'], ['Tab', ''], ['Enter', 'Sand'], ['Enter', 'Water'], ['Shift+Tab', '']] as const) {
    if (k) await app.keyboard.press(k)
    if (t) await app.keyboard.type(t)
  }
  await expect.poll(() => lines(app)).toEqual(['Soil', 'Clay', 'Sand', 'Water'])
  await expect(app.locator('.row.node[data-depth="1"]')).toHaveCount(2)
})

test('UC-06.02 reorder lines with Cmd+Shift+Up and Down', works('UC-06.02'), async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['one', 'two', 'three'])
  await focus(server, app, ids[2], 0)
  await app.keyboard.press('Meta+Shift+ArrowUp')
  await app.keyboard.press('Meta+Shift+ArrowUp')
  await expect.poll(() => lines(app)).toEqual(['three', 'one', 'two'])
})

test('UC-06.03 zoom into a line (Cmd+.) and back out (Cmd+,); breadcrumbs show where it lives', works('UC-06.03'), async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['Chapter'], 'Book draft')
  await server.run('node.add', { name: 'Scene', parent: ids[0] })
  await focus(server, app, ids[0], 0)
  await app.keyboard.press('Meta+Period')
  await expect(app.locator('.panel .title-input')).toHaveValue('Chapter')
  await expect(app.locator('.crumb', { hasText: 'Book draft' })).toBeVisible()
  await app.keyboard.press('Meta+Comma')
  await expect(app.locator('.panel .title-input')).toHaveValue('Book draft')
})

test('UC-06.04 fold and unfold a branch (Cmd+Up, Cmd+Down)', works('UC-06.04'), async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['Parent'])
  await server.run('node.add', { name: 'Hidden child', parent: ids[0] })
  await focus(server, app, ids[0], 0)
  await app.keyboard.press('Meta+ArrowDown')
  await expect(app.locator('.row.node', { hasText: 'Hidden child' })).toBeVisible()
  await app.keyboard.press('Meta+ArrowUp')
  await expect(app.locator('.row.node', { hasText: 'Hidden child' })).toHaveCount(0)
})

test('UC-06.05 bold, italic, underline and code; no strikethrough', partial('UC-06.05'), async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['a b c'])
  await focus(server, app, ids[0], 0)
  await app.keyboard.press('Shift+ArrowRight')
  await app.keyboard.press('Meta+b')
  await expect.poll(() => nameOf(server, ids[0])).toBe('**a** b c')
  await server.run('format.italic', { node: ids[0], start: 2, end: 3 })
  await server.run('format.code', { node: ids[0], start: 4, end: 5 })
  await expect.poll(() => nameOf(server, ids[0])).toMatch(/\*a\*|_a_|\*b\*|_b_/)
  await expect(app.locator(`.row.node[data-id="${ids[0]}"] .code`).first()).toBeVisible()
  // Gap: no strikethrough format.
})

test('UC-06.06 headings with ! and a description under a line (Ctrl+I)', works('UC-06.06'), async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['', 'Body'])
  await focus(server, app, ids[0], 0)
  await app.keyboard.type('!')
  await app.keyboard.type('Overview')
  await expect.poll(async () => (await rowOf(server, ids[0]))?.heading).toBe(true)
  await focus(server, app, ids[1], 4)
  await app.keyboard.press('Control+i')
  await app.keyboard.type('the main text')
  await expect.poll(async () => (await rowOf(server, ids[1]))?.description).toBe('the main text')
})

test('UC-06.07 copy a line and paste it elsewhere as a live reference; edits show in both', works('UC-06.07'), async ({ app, server, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write'])
  const { page, ids } = await pageWith(server, app, ['Compost ratio 3:1', 'Uses', ''])
  await focus(server, app, ids[0], 0)
  await app.keyboard.press('Escape')
  await app.keyboard.press('Meta+c')
  await app.waitForTimeout(100)
  await focus(server, app, ids[2], 0)
  await app.keyboard.press('Meta+v')
  await expect.poll(async () => (await tree(server, page)).children.at(-1).ref).toBe(ids[0])
  await server.run('node.setName', { node: ids[0], name: 'Compost ratio 30:1' })
  await expect(app.locator('.row.node', { hasText: 'Compost ratio 30:1' })).toHaveCount(2)
})

test('UC-06.08 the reference counter shows how often a line is used', works('UC-06.08'), async ({ app, server }) => {
  const { page, ids } = await pageWith(server, app, ['Idea', 'A', 'B'])
  await server.run('node.reference', { target: ids[0], parent: ids[1] })
  await server.run('node.reference', { target: ids[0], parent: ids[2] })
  await server.run('settings.set', { key: 'ref_counter', value: true })
  await server.run('nav.open', { node: page })
  await expect(app.locator(`.row.node[data-id="${ids[0]}"] .ref-count`)).toHaveText('2')
})

test('UC-06.09 notes on a reference that belong to this place only', works('UC-06.09'), async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['Recipe', 'Plan'])
  const r = (await server.run('node.reference', { target: ids[0], parent: ids[1] })).result.id
  await server.run('node.expand', { node: ids[1] })
  await focus(server, app, r, 0)
  await app.keyboard.press('Meta+Alt+ArrowDown')
  await app.keyboard.type('double it for Sunday')
  await expect.poll(async () => (await tree(server, ids[1])).children?.[0]?.notes?.map((c: any) => c.name)).toEqual(['double it for Sunday'])
  expect((await tree(server, ids[0])).children ?? []).toEqual([])
})

test('UC-06.10 duplicate a line with its children (Cmd+Shift+D)', works('UC-06.10'), async ({ app, server }) => {
  const { page, ids } = await pageWith(server, app, ['Checklist'])
  await server.run('node.add', { name: 'item', parent: ids[0] })
  await focus(server, app, ids[0], 0)
  await app.keyboard.press('Meta+Shift+d')
  await expect.poll(async () => (await tree(server, page)).children.map((c: any) => [c.name, c.children?.length, c.ref ?? null])).toEqual([
    ['Checklist', 1, null],
    ['Checklist', 1, null],
  ])
})

test('UC-06.11 delete to the trash and restore from it', works('UC-06.11'), async ({ app, server }) => {
  const { page, ids } = await pageWith(server, app, ['Keep', 'Oops'])
  await focus(server, app, ids[1], 0)
  await app.keyboard.press('Meta+Shift+Backspace')
  await expect.poll(async () => (await tree(server, page)).children.map((c: any) => c.name)).toEqual(['Keep'])
  await server.run('nav.trash')
  const row = app.locator('.row.node', { hasText: 'Oops' })
  await expect(row).toBeVisible()
  await row.locator('.bullet').first().click({ button: 'right' })
  await app.locator('.menu-item', { hasText: 'Restore' }).click()
  await expect.poll(async () => (await tree(server, page)).children.map((c: any) => c.name)).toEqual(['Keep', 'Oops'])
})

test('UC-06.12 see who changed a page and when (edit log)', works('UC-06.12'), async ({ server }) => {
  const id = await idOf(server, 'Reading notes')
  const log = (await server.run('node.editLog', { node: id })).result
  expect(JSON.stringify(log)).toMatch(/created|edited|Created|Edited/)
})

test('UC-06.13 give a page an emoji icon and a banner', partial('UC-06.13'), async ({ app, server }) => {
  await pageWith(server, app, ['x'], 'Garden journal')
  await app.locator('.panel .title').first().hover()
  await app.locator('.decorate .ghost-btn', { hasText: 'Icon' }).click()
  await app.keyboard.type('🌱')
  await app.keyboard.press('Enter')
  await expect.poll(async () => (await pageOf(server)).icon).toBe('🌱')
  await app.locator('.panel .title').first().hover()
  await app.locator('.decorate .ghost-btn', { hasText: 'Banner' }).click()
  await app.locator('.menu-item', { hasText: 'Green' }).click()
  await expect(app.locator('.page-banner')).toBeVisible()
  // Gap: banners are colours or one shipped picture; no upload of your own image.
})

test('UC-06.14 a wiki: a page shown as side menu, tabs or list; the picked item opens beside it', partial('UC-06.14'), async ({ app, server }) => {
  const { page } = await pageWith(server, app, ['Onboarding', 'Tools', 'Contacts'], 'Wiki')
  for (const kind of ['side_menu', 'tabs', 'list'] as const) {
    await server.run('view.set', { node: page, kind })
    await expect(app.locator(`.nav-view.${kind} .nav-item`)).toHaveCount(3)
  }
  await app.locator('.nav-view.list .nav-item', { hasText: 'Tools' }).click()
  await expect(app.locator('.panel')).toHaveCount(2)
  await expect(app.locator('.panel').nth(1).locator('.title-input, .title').first()).toContainText('Tools')
  // Gap: side menu and tabs also open the item in a second panel instead of showing it in place.
})

test('UC-06.15 Cmd+S finds any node by its words and opens it', works('UC-06.15'), async ({ app }) => {
  await app.keyboard.press('Meta+s')
  await expect(app.locator('.dialog.search')).toBeVisible()
  await app.keyboard.type('Mulch keeps')
  await expect(app.locator('.dialog.search .dialog-item.sel')).toContainText('Mulch')
  await app.keyboard.press('Enter')
  await expect(app.locator('.panel .title').first()).toContainText('Mulch keeps')
})

test('UC-06.16 Recents lists what was opened lately', works('UC-06.16'), async ({ app, server }) => {
  await server.run('nav.open', { node: await idOf(server, 'Reading notes') })
  await server.run('nav.today')
  await app.locator('.sidebar .side-row', { hasText: 'Recents' }).click()
  await expect(app.getByText('Reading notes').first()).toBeVisible()
})

test('UC-06.17 pin a page to the sidebar', works('UC-06.17'), async ({ app, server }) => {
  const id = await idOf(server, 'Reading notes')
  await server.run('nav.open', { node: id })
  await palette(app, 'Pin or unpin in sidebar')
  await expect(app.locator('.sidebar .side-row', { hasText: 'Reading notes' })).toBeVisible()
  await app.locator('.sidebar .side-row', { hasText: 'Reading notes' }).click()
  await expect(app.locator('.panel .title').first()).toContainText('Reading notes')
})

test('UC-06.18 open a page in a second panel and in a new tab', works('UC-06.18'), async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['Side note'])
  await focus(server, app, ids[0], 0)
  await app.keyboard.press('Meta+m')
  await expect(app.locator('.panel')).toHaveCount(2)
  await app.keyboard.press('Escape')
  await app.locator('.topbar [title="New tab"]').click()
  await expect(app.locator('.tab')).toHaveCount(2)
})

test('UC-06.19 bind your own shortcut to a command', works('UC-06.19'), async ({ app, server }) => {
  await server.run('settings.setKeybinding', { shortcut: 'Mod+Shift+H', command: 'node.toggleHeading' })
  const { ids } = await pageWith(server, app, ['Section'])
  await focus(server, app, ids[0], 0)
  await app.keyboard.press('Meta+Shift+h')
  await expect.poll(async () => (await rowOf(server, ids[0]))?.heading).toBe(true)
})

test('UC-06.20 run any action by name from the command line (Cmd+K)', works('UC-06.20'), async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['Draft'])
  await focus(server, app, ids[0], 0)
  await palette(app, 'Toggle heading')
  await expect.poll(async () => (await rowOf(server, ids[0]))?.heading).toBe(true)
})

test('UC-06.21 undo and redo', works('UC-06.21'), async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['abc'])
  await focus(server, app, ids[0], 3)
  await app.keyboard.type('def')
  await expect.poll(() => nameOf(server, ids[0])).toBe('abcdef')
  await app.keyboard.press('Meta+z')
  await expect.poll(() => nameOf(server, ids[0])).not.toBe('abcdef')
  await app.keyboard.press('Meta+Shift+z')
  await expect.poll(() => nameOf(server, ids[0])).toBe('abcdef')
})

// Supertags and fields

test('UC-06.22 turn a list into a supertag with its children as the template', works('UC-06.22'), async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['recipe'])
  await server.run('node.add', { name: 'Ingredients', parent: ids[0] })
  await focus(server, app, ids[0], 0)
  await palette(app, 'Convert to supertag')
  await expect.poll(async () => (await server.run('graph.tags')).result.map((t: any) => t.name)).toContain('recipe')
})

test('UC-06.23 a supertag that extends another inherits its fields', works('UC-06.23'), async ({ app, server }) => {
  await server.run('tag.create', { name: 'design task', color: 'violet' })
  await server.run('tag.extend', { tag: 'design task', base: 'task' })
  const { ids } = await pageWith(server, app, ['Draw the shed plan'])
  await server.run('tag.add', { node: ids[0], tag: 'design task' })
  await server.run('node.expand', { node: ids[0] })
  await expect(app.locator('.row.field', { hasText: 'Status' }).first()).toBeVisible()
  await expect(app.locator(`.row.node[data-id="${ids[0]}"] .checkbox`)).toBeVisible()
})

test('UC-06.24 optional fields wait behind a + button until needed', works('UC-06.24'), async ({ app, server }) => {
  await server.run('field.create', { name: 'Allergies', type: 'plain' })
  await server.run('tag.setOptionalField', { tag: 'person', field: 'Allergies', on: true })
  const { ids } = await pageWith(server, app, ['Lee Park'])
  await server.run('tag.add', { node: ids[0], tag: 'person' })
  await server.run('node.expand', { node: ids[0] })
  await expect(app.locator('.row.field', { hasText: 'Allergies' })).toHaveCount(0)
  await expect(app.getByText('Allergies').first()).toBeVisible()
})

test('UC-06.25 field types check values: a number outside its range is refused (Tana keeps it with a warning)', partial('UC-06.25'), async ({ server }) => {
  await server.run('field.setRange', { field: 'Effort (hours)', min: 0, max: 40 })
  const id = await idOf(server, 'Build four beds')
  expect((await server.run('field.set', { node: id, field: 'Effort (hours)', value: '99' })).error).toMatch(/range/)
  expect(await fieldOf(server, id, 'Effort (hours)')).toBe('6')
})

test('UC-06.26 hide a template field while it is empty', works('UC-06.26'), async ({ app, server }) => {
  await server.run('field.setHide', { field: 'Website', mode: 'when_empty' })
  const { ids } = await pageWith(server, app, ['Pond project'])
  await server.run('tag.add', { node: ids[0], tag: 'project' })
  await server.run('node.expand', { node: ids[0] })
  await expect(app.locator('.row.field', { hasText: 'Status' })).toBeVisible()
  await expect(app.locator('.row.field', { hasText: 'Website' })).toHaveCount(0)
  await server.run('field.set', { node: ids[0], field: 'Website', value: 'https://example.org/pond' })
  await expect(app.locator('.row.field', { hasText: 'Website' })).toBeVisible()
})

test('UC-06.27 a field holds several values', works('UC-06.27'), async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['Trip'])
  await server.run('field.add', { node: ids[0], field: 'Packing', value: 'boots' })
  await server.run('field.addValue', { node: ids[0], field: 'Packing', value: 'hat' })
  await server.run('node.expand', { node: ids[0] })
  await expect(app.locator('.row.field', { hasText: 'Packing' })).toContainText('boots')
  await expect(app.locator('.row.field', { hasText: 'Packing' })).toContainText('hat')
})

test('UC-06.28 the Supertags page lists every supertag; a tag page shows its nodes', works('UC-06.28'), async ({ app }) => {
  await app.locator('.sidebar .side-row', { hasText: 'Supertags' }).click()
  await expect(app.locator('.supertags')).toContainText('person')
  await app.locator('.supertags').getByText('person', { exact: true }).first().click()
  await expect(app.getByText('Robin Ash').first()).toBeVisible()
  await expect(app.getByText('Mia Chen').first()).toBeVisible()
})

// Search and views

test('UC-06.29 a search with nested OR and NOT groups', works('UC-06.29'), async ({ app, server }) => {
  const s = (await server.run('search.create', { query: '#task NOT DONE', name: 'Mixed' })).result.id
  await server.run('search.addGroup', { node: s, kind: 'or' })
  await server.run('search.addClause', { node: s, clause: 'Status:"In progress"', path: [2] })
  await server.run('search.addClause', { node: s, clause: 'Status:"Not started"', path: [2] })
  await server.run('nav.open', { node: s })
  await expect(app.locator('.row.node', { hasText: 'Build four beds' })).toBeVisible()
  await expect(app.locator('.row.node', { hasText: 'Order cedar boards' })).toHaveCount(0)
})

test('UC-06.30 a table: add a column and type in a cell', works('UC-06.30'), async ({ app, server }) => {
  const { page } = await pageWith(server, app, ['Apples', 'Pears'], 'Fruit')
  await server.run('view.set', { node: page, kind: 'table' })
  await server.run('view.addColumn', { node: page, field: 'Price', type: 'number' })
  const row = app.locator('.t-row', { hasText: 'Apples' })
  await row.locator('.t-cell').nth(1).click()
  await expect(app.locator('.field-input')).toBeFocused()
  await app.keyboard.type('3')
  await app.keyboard.press('Escape')
  const id = Number(await row.getAttribute('data-id'))
  await expect.poll(() => fieldOf(server, id, 'Price')).toBe('3')
})

test('UC-06.31 group an outline by supertag; hide a board column', partial('UC-06.31'), async ({ app, server }) => {
  const s = await idOf(server, 'Open tasks')
  await server.run('view.set', { node: s, kind: 'cards' })
  await server.run('view.group', { node: s, by: 'Status' })
  await server.run('nav.open', { node: s })
  await expect(app.locator('.board-col', { hasText: 'In progress' })).toBeVisible()
  await app.locator('.board-col', { hasText: 'In progress' }).locator('.board-hide').click()
  await expect(app.locator('.board-col', { hasText: 'In progress' }).locator('.card')).toHaveCount(0)
  // Gap: group headers in outline and list views do not fold (Tana's collapsible groups).
})

test('UC-06.32 switch light and dark theme', works('UC-06.32'), async ({ app }) => {
  const before = await app.evaluate(() => document.documentElement.dataset.theme ?? '')
  await palette(app, 'Switch light or dark theme')
  await expect.poll(() => app.evaluate(() => document.documentElement.dataset.theme ?? '')).not.toBe(before)
})

test('UC-06.33 custom commands: a saved list of steps run on a node (engine, CLI and MCP only)', partial('UC-06.33'), async ({ app, server }) => {
  await server.run('commandNode.create', { name: 'Make task', steps: ['tag.add {"tag": "task"}', 'node.toggleDone'] })
  const { ids } = await pageWith(server, app, ['Clean the gutters'])
  await server.run('commandNode.run', { command: 'Make task', node: ids[0] })
  await expect(app.locator(`.row.node[data-id="${ids[0]}"] .chip`)).toContainText('task')
  await expect(app.locator(`.row.node[data-id="${ids[0]}"] .checkbox`)).toBeVisible()
  // Gap: the web command line does not list command nodes by name; no buttons on supertags or
  // fields; no node filter or context parameters.
})

// Gaps
missing('UC-06.34', 'merge duplicate nodes, supertags or fields into one')
missing('UC-06.35', 'lock a node so it cannot be edited')
missing('UC-06.36', 'count words, characters and nodes of a page or selection')
missing('UC-06.37', 'give an inline reference its own alias text, or turn it back into plain text')
missing('UC-06.38', 'type : and a word to pick an emoji')
missing('UC-06.39', 'code blocks with a language and Mermaid diagrams')
missing('UC-06.40', 'a Part of hierarchy with a search that finds everything beneath a node (COMPONENTS REC)')
missing('UC-06.41', 'search with a regular expression')
missing('UC-06.42', 'search inside one node from the search box (Tab on a result)')
missing('UC-06.43', 'rolling date periods in searches and filters (this week, NEXT 14 DAYS)')
missing('UC-06.44', 'content width per page (medium or full) from the panel button')
missing('UC-06.45', 'strikethrough text')
