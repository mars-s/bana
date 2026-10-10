// `#`, `>`, `@` and field values (spec 02 §11, 03; DIFFS N15, N16, N17), ported from
// legacy/egui/ui-egui/tests/typing.rs.
import { test, expect, pageWith, focus, nameOf, type Server } from './fixtures'
import type { Page } from '@playwright/test'

async function tree(server: Server, id: number): Promise<any> {
  return (await server.run('export.json', { node: id })).result
}

async function valuesOf(server: Server, node: number, field: string): Promise<string[]> {
  const t = await tree(server, node)
  const v = t?.fields?.[field]
  return v == null ? [] : Array.isArray(v) ? v : [v]
}

async function childNames(server: Server, node: number): Promise<string[]> {
  return ((await tree(server, node))?.children ?? []).map((c: any) => c.name)
}

/** A page with line "x", then `>` + name + Tab on a new line below: the caret is in the value. */
async function fieldAfterX(server: Server, app: Page, name: string) {
  const { page, ids } = await pageWith(server, app, ['x'], 'Page')
  await focus(server, app, ids[0], 1)
  await app.keyboard.press('Enter')
  await app.keyboard.type('>')
  await expect(app.locator('.menu.picker')).toBeVisible()
  await app.keyboard.type(name)
  await app.keyboard.press('Tab')
  await expect(app.locator('.field-input')).toBeFocused()
  return { page, x: ids[0] }
}

test('typing #task then Enter applies a new supertag', async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['Bread'])
  await focus(server, app, ids[0], 5)
  await app.keyboard.type(' #')
  await expect(app.locator('.menu.picker')).toBeVisible()
  await app.keyboard.type('chores')
  await expect(app.locator('.menu-item.sel')).toContainText('chores')
  await app.keyboard.press('Enter')
  await expect(app.locator('.menu.picker')).toHaveCount(0)
  await expect.poll(() => nameOf(server, ids[0])).toBe('Bread')
  await expect(app.locator(`.row.node[data-id="${ids[0]}"] .chip`)).toContainText('chores')
  // Typing goes on in the line.
  await app.keyboard.type('!')
  await expect.poll(() => nameOf(server, ids[0])).toBe('Bread!')
})

test('field values are saved as typed; Escape keeps them (N15)', async ({ app, server }) => {
  const { page, x } = await fieldAfterX(server, app, 'Prio')
  await app.keyboard.type('High')
  await expect.poll(() => valuesOf(server, page, 'Prio')).toEqual(['High'])
  await app.keyboard.press('Escape')
  await expect(app.locator(`.row.node.selected[data-id="${x}"]`)).toBeVisible()
  await expect.poll(() => valuesOf(server, page, 'Prio')).toEqual(['High'])
})

test('Up from a field value keeps it and edits the line above (N15)', async ({ app, server }) => {
  const { page, x } = await fieldAfterX(server, app, 'Lead')
  await app.keyboard.type('Ann')
  await app.keyboard.press('ArrowUp')
  await app.keyboard.type('Z')
  await expect.poll(() => valuesOf(server, page, 'Lead')).toEqual(['Ann'])
  await expect.poll(() => nameOf(server, x)).toBe('xZ')
})

test('Enter in a value adds another; Escape keeps both', async ({ app, server }) => {
  const { page } = await fieldAfterX(server, app, 'Ppl')
  await app.keyboard.type('Ann')
  await app.keyboard.press('Enter')
  await app.keyboard.type('Bob')
  await app.keyboard.press('Escape')
  await expect.poll(() => valuesOf(server, page, 'Ppl')).toEqual(['Ann', 'Bob'])
})

test('Shift+Tab moves a value out to a line below the field (N16, N17)', async ({ app, server }) => {
  const { page } = await fieldAfterX(server, app, 'Pq')
  await app.keyboard.type('Ann')
  await app.keyboard.press('Enter')
  await app.keyboard.type('Bob')
  await app.keyboard.press('Shift+Tab')
  await app.keyboard.type('!')
  await expect.poll(() => valuesOf(server, page, 'Pq')).toEqual(['Ann'])
  await expect.poll(() => childNames(server, page)).toEqual(['x', 'Bob!'])
})

test('Shift+Tab on an empty value line starts a line below and typing goes on there', async ({ app, server }) => {
  const { page } = await fieldAfterX(server, app, 'Lead')
  await app.keyboard.type('Robin')
  await app.keyboard.press('Enter')
  await app.keyboard.press('Shift+Tab')
  await app.keyboard.type('Timeline2')
  await app.keyboard.press('Enter')
  await app.keyboard.type('Week2')
  await expect.poll(() => valuesOf(server, page, 'Lead')).toEqual(['Robin'])
  await expect.poll(() => childNames(server, page)).toEqual(['x', 'Timeline2', 'Week2'])
})

test('a field name typed after > stays with Escape', async ({ app, server }) => {
  const { page, ids } = await pageWith(server, app, ['x'], 'Page')
  await focus(server, app, ids[0], 1)
  await app.keyboard.press('Enter')
  await app.keyboard.type('>Qq')
  await app.keyboard.press('Escape')
  await expect(app.locator('.menu.picker')).toHaveCount(0)
  await expect.poll(() => childNames(server, page)).toEqual(['x'])
  await expect.poll(async () => Object.keys((await tree(server, page)).fields ?? {})).toContain('Qq')
})

test('@ on an empty line references a node; @ mid-text links one inline', async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['', 'see '], 'Refs')
  await focus(server, app, ids[0], 0)
  await app.keyboard.type('@Robin')
  await expect(app.locator('.menu-item.sel')).toContainText('Robin Ash')
  await app.keyboard.press('Enter')
  await expect(app.locator('.row.node .bullet .dot.ref')).toHaveCount(1)
  await focus(server, app, ids[1], 4)
  await app.keyboard.type('@Robin')
  await expect(app.locator('.menu-item.sel')).toContainText('Robin Ash')
  await app.keyboard.press('Enter')
  await expect.poll(() => nameOf(server, ids[1])).toMatch(/^see \[\[id:\d+\]\]$/)
  // (While the line is being typed in, the editor shows the raw text.)
  await app.keyboard.press('Escape')
  await expect(app.locator('.inline-ref')).toContainText('Robin Ash')
})

test('! on an empty line makes a heading', async ({ app, server }) => {
  const { ids } = await pageWith(server, app, [''], 'Heads')
  await focus(server, app, ids[0], 0)
  await app.keyboard.type('!Plan')
  await expect.poll(() => nameOf(server, ids[0])).toBe('Plan')
  await expect(app.locator(`.row.node.heading[data-id="${ids[0]}"]`)).toBeVisible()
})

test('a date field takes a typed date and shows it as a day', async ({ app, server }) => {
  await server.run('field.create', { name: 'When', type: 'date' })
  const { page } = await fieldAfterX(server, app, 'When')
  await expect(app.locator('.date-pick')).toBeVisible()
  await app.keyboard.type('2026-10-20')
  await expect.poll(() => valuesOf(server, page, 'When')).toEqual(['2026-10-20'])
  await app.keyboard.press('Escape')
  await expect(app.locator('.row.field', { hasText: 'When' })).toContainText('Tue, 20 Oct')
})

/** A page with lines x and y; `>` + name on a new line after x, then Escape: the field row is selected. */
async function selectedFieldRow(server: Server, app: Page, name: string) {
  const { page, ids } = await pageWith(server, app, ['x', 'y'], 'Page')
  await focus(server, app, ids[0], 1)
  await app.keyboard.press('Enter')
  await app.keyboard.type('>')
  await expect(app.locator('.menu.picker')).toBeVisible()
  await app.keyboard.type(name)
  await app.keyboard.press('Escape')
  await expect(app.locator('.row.field.selected')).toContainText(name)
  return { page, x: ids[0], y: ids[1] }
}

test('Escape in a new field name selects the field row; Tab edits its value (N28)', async ({ app, server }) => {
  const { page } = await selectedFieldRow(server, app, 'Ppl')
  await expect(app.locator('.row.node.selected')).toHaveCount(0)
  await app.keyboard.press('Tab')
  await app.keyboard.type('Ann')
  await expect.poll(() => valuesOf(server, page, 'Ppl')).toEqual(['Ann'])
  await expect(app.locator('.row.field.selected')).toHaveCount(0)
})

test('Enter on a selected field row adds a line below it; letters extend its name (N33)', async ({ app, server }) => {
  const { page, x, y } = await selectedFieldRow(server, app, 'Qq')
  await app.keyboard.press('Enter')
  await app.keyboard.type('N')
  await expect.poll(() => childNames(server, page)).toEqual(['x', 'N', 'y'])
  expect(await valuesOf(server, page, 'Qq')).toEqual([])
  void x
  void y
  // Fast typing on a selected row: every letter goes on the name.
  await selectedFieldRow(server, app, 'Qq8')
  await app.keyboard.type('Nm')
  const fields = async () => Object.keys((await tree(server, (await server.rpc('page', { panel: 'main' })).zoom))?.fields ?? {})
  await expect.poll(fields).toEqual(['Qq8Nm'])
})

test('a checkbox field after > and Tab stays selected; Space does nothing (N31)', async ({ app, server }) => {
  await server.run('field.create', { name: 'Outdoors', type: 'checkbox' })
  const { page, ids } = await pageWith(server, app, ['x'], 'Page')
  await focus(server, app, ids[0], 1)
  await app.keyboard.press('Enter')
  await app.keyboard.type('>Outdoors')
  await expect(app.locator('.menu-item.sel')).toContainText('Outdoors')
  await app.keyboard.press('Tab')
  await expect(app.locator('.row.field.selected')).toContainText('Outdoors')
  await app.keyboard.press('Space')
  await app.keyboard.press('Shift+Tab')
  await app.waitForTimeout(200)
  expect(await childNames(server, page)).toEqual(['x'])
  expect(await valuesOf(server, page, 'Outdoors')).toEqual([])
})

for (const gap of [0, 40, 120]) {
  test(`Escape then Enter on a new field name adds a line below at any key speed (N57, ${gap} ms)`, async ({ app, server }) => {
    const { page, ids } = await pageWith(server, app, ['x', 'y'], 'Page')
    await focus(server, app, ids[0], 1)
    for (const key of ['Enter', '>', 'Q', 'q', 'Escape', 'Enter', 'N']) {
      if (key.length === 1) await app.keyboard.type(key)
      else await app.keyboard.press(key)
      if (gap) await app.waitForTimeout(gap)
    }
    await expect.poll(() => childNames(server, page)).toEqual(['x', 'N', 'y'])
    expect(await valuesOf(server, page, 'Qq')).toEqual([])
  })
}
