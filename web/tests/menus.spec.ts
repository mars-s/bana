// Node menu, selection toolbar, descriptions and drag and drop (spec 02 §8, §9, §13).
import { test, expect, pageWith, focus, nameOf, type Server } from './fixtures'

async function tree(server: Server, id: number): Promise<any> {
  return (await server.run('export.json', { node: id })).result
}

test('right-clicking a bullet opens the node menu; Add supertag tags the node', async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['Seeds'])
  await app.locator(`.row.node[data-id="${ids[0]}"] .bullet`).click({ button: 'right' })
  await expect(app.locator('.node-menu')).toBeVisible()
  await app.locator('.node-menu .menu-item', { hasText: 'Add supertag' }).click()
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
  // Over the far side of the line: after it (just right of its bullet would nest it).
  await src.dragTo(dst, { targetPosition: { x: 300, y: 12 } })
  await expect.poll(async () => ((await tree(server, page)).children ?? []).map((c: any) => c.name)).toEqual(['two', 'three', 'one'])
})

test('dropping just right of a bullet nests the dragged line under it', async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['one', 'two'])
  const src = app.locator(`.row.node[data-id="${ids[1]}"] .bullet`)
  const dst = app.locator(`.row.node[data-id="${ids[0]}"]`)
  await src.dragTo(dst, { targetPosition: { x: 50, y: 12 } })
  await expect.poll(async () => ((await tree(server, ids[0])).children ?? []).map((c: any) => c.name)).toEqual(['two'])
})
