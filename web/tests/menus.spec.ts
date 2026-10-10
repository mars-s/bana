// Node menu, selection toolbar, descriptions and drag and drop (spec 02 §8, §9, §13).
import { test, expect, pageWith, focus, nameOf, type Server } from './fixtures'

async function tree(server: Server, id: number): Promise<any> {
  return (await server.run('export.json', { node: id })).result
}

test('right-clicking a bullet opens the node menu; Add tag tags the node', async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['Seeds'])
  await app.locator(`.row.node[data-id="${ids[0]}"] .bullet`).click({ button: 'right' })
  await expect(app.locator('.node-menu')).toBeVisible()
  await app.locator('.node-menu .menu-item', { hasText: 'Add tag' }).click()
  await expect(app.locator('.menu.picker')).toBeVisible()
  await app.locator('.menu.picker .menu-item', { hasText: 'task' }).first().click()
  await expect(app.locator(`.row.node[data-id="${ids[0]}"] .chip`)).toContainText('task')
  await expect.poll(async () => (await tree(server, ids[0])).tags).toContain('task')
})

test('Escape on a line shows the selection toolbar; its H makes a heading', async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['Plan'])
  await focus(server, app, ids[0], 4)
  await app.keyboard.press('Escape')
  await expect(app.locator('.sel-toolbar')).toBeVisible()
  await app.locator('.sel-toolbar .sel-btn', { hasText: 'H' }).click()
  await expect(app.locator(`.row.node.heading[data-id="${ids[0]}"]`)).toBeVisible()
})

test('Ctrl+I adds a description, saved as typed; Escape goes back to the line', async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['Seeds'])
  await focus(server, app, ids[0], 5)
  await app.keyboard.press('Control+i')
  await expect(app.locator('.desc-input')).toBeFocused()
  await app.keyboard.type('bring spares')
  await expect.poll(async () => (await tree(server, ids[0])).description).toBe('bring spares')
  await app.keyboard.press('Escape')
  await app.keyboard.type('!')
  await expect.poll(() => nameOf(server, ids[0])).toBe('Seeds!')
})

test('dragging a bullet onto another line moves it below that line', async ({ app, server }) => {
  const { page, ids } = await pageWith(server, app, ['one', 'two', 'three'])
  const src = app.locator(`.row.node[data-id="${ids[0]}"] .bullet`)
  const dst = app.locator(`.row.node[data-id="${ids[2]}"]`)
  // The lower half of the line: below it.
  await src.dragTo(dst, { targetPosition: { x: 300, y: 22 } })
  await expect.poll(async () => ((await tree(server, page)).children ?? []).map((c: any) => c.name)).toEqual(['two', 'three', 'one'])
})

test('the sideways offset never nests; a drop under an open row makes a first child (N44)', async ({ app, server }) => {
  const { page, ids } = await pageWith(server, app, ['one', 'two', 'three'])
  // Lower half of a collapsed leaf, just right of its bullet: a sibling below it, never a child.
  await app.locator(`.row.node[data-id="${ids[2]}"] .bullet`).dragTo(app.locator(`.row.node[data-id="${ids[0]}"]`), { targetPosition: { x: 50, y: 22 } })
  await expect.poll(async () => ((await tree(server, page)).children ?? []).map((c: any) => c.name)).toEqual(['one', 'three', 'two'])
  // Upper half: above.
  await app.locator(`.row.node[data-id="${ids[1]}"] .bullet`).dragTo(app.locator(`.row.node[data-id="${ids[0]}"]`), { targetPosition: { x: 300, y: 4 } })
  await expect.poll(async () => ((await tree(server, page)).children ?? []).map((c: any) => c.name)).toEqual(['two', 'one', 'three'])
  // Lower half of an expanded row with children: its first child.
  await server.run('node.add', { parent: ids[0], name: 'kid' })
  await server.run('node.expand', { node: ids[0] })
  await expect(app.locator('.row.node', { hasText: 'kid' })).toBeVisible()
  await app.locator(`.row.node[data-id="${ids[2]}"] .bullet`).dragTo(app.locator(`.row.node[data-id="${ids[0]}"]`), { targetPosition: { x: 300, y: 22 } })
  await expect.poll(async () => ((await tree(server, ids[0])).children ?? []).map((c: any) => c.name)).toEqual(['three', 'kid'])
})

test('pasting several lines makes nodes; copied lines paste back as references', async ({ app, server, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write'])
  const { page, ids } = await pageWith(server, app, ['first', ''])
  await focus(server, app, ids[1], 0)
  await app.evaluate(() => navigator.clipboard.writeText('alpha\nbeta\ngamma'))
  await app.keyboard.press('Meta+v')
  await expect.poll(async () => ((await tree(server, page)).children ?? []).map((c: any) => c.name)).toEqual(['first', 'alpha', 'beta', 'gamma'])
  // Copy "first" as a node, then paste it into a new empty line: a reference.
  await focus(server, app, ids[0], 5)
  await app.keyboard.press('Escape')
  await app.keyboard.press('Meta+c')
  await app.waitForTimeout(100)
  const last = (await tree(server, page)).children.at(-1).id
  await focus(server, app, last, 5)
  await app.keyboard.press('Enter')
  await app.keyboard.press('Meta+v')
  await expect.poll(async () => (await tree(server, page)).children.at(-1).ref).toBe(ids[0])
})

test('deleting a node used elsewhere asks first (N20)', async ({ app, server }) => {
  const { page, ids } = await pageWith(server, app, ['shared', 'holder'])
  await server.run('node.reference', { target: ids[0], parent: ids[1] })
  await focus(server, app, ids[0], 0)
  await app.keyboard.press('Escape')
  await app.keyboard.press('Backspace')
  await expect(app.locator('.dialog.confirm')).toBeVisible()
  await app.keyboard.press('Escape')
  await expect(app.locator('.dialog.confirm')).toHaveCount(0)
  expect(((await tree(server, page)).children ?? []).map((c: any) => c.name)).toContain('shared')
  await app.keyboard.press('Backspace')
  await app.locator('.dialog.confirm .btn', { hasText: 'Delete anyway' }).click()
  await expect.poll(async () => ((await tree(server, page)).children ?? []).map((c: any) => c.name)).not.toContain('shared')
})

test('manage workspaces creates a second workspace, shown folded in the sidebar (N24)', async ({ app }) => {
  await app.locator('.side-foot', { hasText: 'Manage workspaces' }).click()
  await app.locator('.workspaces input[placeholder="Workspace name"]').fill('Lab')
  await app.locator('.workspaces input[placeholder="Workspace name"]').press('Enter')
  await app.keyboard.press('Escape')
  await expect(app.locator('.side-ws-head', { hasText: 'Lab' })).toBeVisible()
  await expect(app.locator('.side-head', { hasText: 'Workspaces' })).toBeVisible()
  // Folded until opened.
  await expect(app.locator('.side-ws').nth(1).locator('.side-ws-items')).toHaveCount(0)
  await app.locator('.side-ws-head', { hasText: 'Lab' }).click()
  await expect(app.locator('.side-ws').nth(1).locator('.side-row', { hasText: 'Library' })).toBeVisible()
})

test('a selected line keeps its text readable above the selection fill (N39)', async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['Seed swap'])
  await focus(server, app, ids[0], 2)
  await app.keyboard.press('Escape')
  const text = app.locator(`.row.node.selected[data-id="${ids[0]}"] .text`)
  await expect(text).toBeVisible()
  const box = (await text.boundingBox())!
  const top = await app.evaluate(([x, y]) => document.elementFromPoint(x, y)?.closest('.text') !== null, [box.x + 5, box.y + box.height / 2])
  expect(top).toBe(true)
})

test('the bullet menu fits the window; the page Options menu opens under its button (N37, N38)', async ({ app }) => {
  // Today's first line, near the middle of a short window.
  await app.setViewportSize({ width: 1512, height: 700 })
  const bullet = app.locator('.row.node .bullet').first()
  await bullet.click({ button: 'right' })
  const menu = app.locator('.node-menu')
  await expect(menu).toBeVisible()
  const box = (await menu.boundingBox())!
  expect(box.width).toBeCloseTo(210, 0)
  expect(box.y + box.height).toBeLessThanOrEqual(700)
  await expect(menu.locator('.menu-item', { hasText: /^Delete/ })).toBeInViewport()
  await app.keyboard.press('Escape')
  await expect(menu).toHaveCount(0)
  await app.setViewportSize({ width: 1512, height: 949 })
  const opts = app.locator('.head-actions .btn[title="Options"]').first()
  await opts.click()
  await expect(menu).toBeVisible()
  const b = (await opts.boundingBox())!
  const mb = (await menu.boundingBox())!
  // Its left edge at the button (round 9, d79).
  expect(Math.abs(mb.x - b.x)).toBeLessThan(2)
  expect(Math.round(mb.y - (b.y + b.height))).toBe(7)
})

test('dragging a selection moves both lines in order and they stay selected (N45)', async ({ app, server }) => {
  const { page, ids } = await pageWith(server, app, ['one', 'two', 'three'])
  await focus(server, app, ids[0], 0)
  // The first Shift+Down selects the line, the second takes in the next.
  await app.keyboard.press('Shift+ArrowDown')
  await app.keyboard.press('Shift+ArrowDown')
  await expect(app.locator('.row.node.selected')).toHaveCount(2)
  await app.locator(`.row.node[data-id="${ids[0]}"] .bullet`).dragTo(app.locator(`.row.node[data-id="${ids[2]}"]`), { targetPosition: { x: 300, y: 22 } })
  await expect.poll(async () => ((await tree(server, page)).children ?? []).map((c: any) => c.name)).toEqual(['three', 'one', 'two'])
  await expect(app.locator('.row.node.selected')).toHaveCount(2)
})

test('hovering a tag chip shows a remove part; clicking it removes the tag (N53)', async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['Seeds'])
  await server.run('tag.add', { node: ids[0], tag: 'chores' })
  const chip = app.locator(`.row.node[data-id="${ids[0]}"] .chip`)
  await expect(chip).toContainText('chores')
  await expect(chip.locator('.chip-x')).toBeHidden()
  await chip.hover()
  await expect(chip.locator('.chip-x')).toBeVisible()
  await chip.locator('.chip-x').click()
  await expect(app.locator(`.row.node[data-id="${ids[0]}"] .chip`)).toHaveCount(0)
})

test('Cmd+Down on a leaf shows an empty child line; a node dropped on it goes inside (N45)', async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['parent', 'mover'])
  await focus(server, app, ids[0], 6)
  await app.keyboard.press('Meta+ArrowDown')
  await expect(app.locator('.row.empty-child')).toHaveCount(1)
  await app.keyboard.press('Escape')
  await app.locator(`.row.node[data-id="${ids[1]}"] .bullet`).dragTo(app.locator('.row.empty-child'))
  await expect.poll(async () => ((await tree(server, ids[0])).children ?? []).map((c: any) => c.name)).toEqual(['mover'])
})

test('page and supertag "…" menus: Tana\'s sets, left edge at the button (N65)', async ({ app, server }) => {
  const btn = app.locator('.head-actions [title="Options"]').first()
  await btn.click()
  const menu = app.locator('.node-menu')
  await expect(menu).toBeVisible()
  const b = (await btn.boundingBox())!
  expect(Math.abs((await menu.boundingBox())!.x - b.x)).toBeLessThan(2)
  for (const label of ['Copy', 'Show edit log', 'Delete', 'More commands']) await expect(menu.locator('.menu-item', { hasText: new RegExp(`^${label}`) }).first()).toBeVisible()
  for (const label of ['Open in new panel', 'Move to…', 'Toggle heading']) await expect(menu.locator('.menu-item', { hasText: label })).toHaveCount(0)
  await menu.locator('.menu-item', { hasText: 'Show edit log' }).click()
  await expect(app.locator('.toast')).toContainText('created')
  const tags = (await server.run('graph.tags', {})).result
  await server.run('nav.open', { node: tags.find((t: any) => t.name === 'task').id })
  await expect(app.locator('.tag-tabs')).toBeVisible()
  await app.locator('.head-actions [title="Options"]').first().click()
  await expect(menu.locator('.menu-item .grow')).toHaveText(['Configure supertag', 'Unpin tag', 'Copy node link', 'Copy', 'Duplicate', 'Move', 'Add tag', 'Add description', 'Show edit log', 'Delete', 'More commands'])
})
