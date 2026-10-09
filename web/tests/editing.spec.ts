// Outline editing by keyboard (spec 02, DIFFS), ported from legacy/egui/ui-egui/tests/typing.rs.
import { test, expect, pageWith, focus, nameOf, lines, clickLine } from './fixtures'

test('typing an outline with the keyboard', async ({ app, server }) => {
  const { ids } = await pageWith(server, app, [''])
  await focus(server, app, ids[0], 0)
  const k = app.keyboard
  await k.type('Groceries')
  await k.press('Enter')
  await k.type('Milk')
  await k.press('Tab')
  await k.press('Enter')
  await k.type('Bread')
  await k.press('Enter')
  await k.type('Call mum')
  await k.press('Shift+Tab')
  await expect.poll(() => lines(app)).toEqual(['Groceries', 'Milk', 'Bread', 'Call mum'])
  await expect(app.locator('.row.node[data-depth="1"]')).toHaveCount(2)
  // Backspace at the start merges into the previous sibling (spec 02 §4).
  await k.press('Home')
  await k.press('Backspace')
  await expect.poll(() => lines(app)).toEqual(['GroceriesCall mum', 'Milk', 'Bread'])
  // Undo puts it back.
  await k.press('Meta+z')
  await expect.poll(() => lines(app)).toEqual(['Groceries', 'Milk', 'Bread', 'Call mum'])
})

test('enter splits at the caret and an empty line goes above at the start', async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['hello world'])
  await focus(server, app, ids[0], 5)
  await app.keyboard.press('Enter')
  await expect.poll(() => lines(app)).toEqual(['hello', ' world'])
  await app.keyboard.type('X')
  await expect.poll(() => lines(app)).toEqual(['hello', 'X world'])
})

test('clicking text puts the caret there', async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['alpha', 'beta'])
  await clickLine(app, 'beta')
  await app.keyboard.type('!')
  await expect.poll(() => nameOf(server, ids[1])).toBe('beta!')
})

test('escape then down or up edits the next or previous line (N3)', async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['one', 'two', 'three'])
  await focus(server, app, ids[1], 1)
  await app.keyboard.press('Escape')
  await app.keyboard.press('ArrowDown')
  await app.keyboard.type('X')
  await expect.poll(() => nameOf(server, ids[2])).toBe('Xthree')
  await app.keyboard.press('Escape')
  await app.keyboard.press('ArrowUp')
  await app.keyboard.type('Y')
  await expect.poll(() => nameOf(server, ids[1])).toBe('twoY')
})

test('typing on a selected line goes into it (N27, N34)', async ({ app, server }) => {
  let { ids } = await pageWith(server, app, ['one', 'two', 'three'])
  await focus(server, app, ids[1], 3)
  await app.keyboard.press('Escape')
  await expect(app.locator('.row.selected')).toHaveCount(1)
  await app.keyboard.type('Zy')
  await expect.poll(() => nameOf(server, ids[1])).toBe('twoZy')

  ;({ ids } = await pageWith(server, app, ['one', 'two', 'three'], 'Second'))
  await focus(server, app, ids[0], 0)
  await app.keyboard.press('Shift+ArrowDown')
  await app.keyboard.press('Shift+ArrowDown')
  await app.keyboard.press('Escape')
  await app.keyboard.type('QR')
  await expect.poll(() => nameOf(server, ids[1])).toBe('QRtwo')
})

test('cmd+period zooms in and puts the caret in the title (N4)', async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['P'])
  await server.run('node.add', { parent: ids[0], name: 'c' })
  await focus(server, app, ids[0], 1)
  await app.keyboard.press('Meta+Period')
  await expect(app.locator('.panel .title-input')).toHaveValue('P')
  await app.keyboard.type('X')
  await expect.poll(() => nameOf(server, ids[0])).toBe('PX')
})

test('left at the start and right at the end move to the next line', async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['ab', 'cd'])
  await focus(server, app, ids[0], 2)
  await app.keyboard.press('ArrowRight')
  await app.keyboard.type('1')
  await expect.poll(() => nameOf(server, ids[1])).toBe('1cd')
  await app.keyboard.press('ArrowLeft')
  await app.keyboard.press('ArrowLeft')
  await app.keyboard.type('2')
  await expect.poll(() => nameOf(server, ids[0])).toBe('ab2')
})

test('long lines wrap and typing never adds line breaks', async ({ app, server }) => {
  const { ids } = await pageWith(server, app, [''])
  await focus(server, app, ids[0], 0)
  const long = 'word '.repeat(60).trim()
  await app.keyboard.type(long)
  await expect.poll(() => nameOf(server, ids[0])).toBe(long)
  const h = await app.locator('.editor').boundingBox()
  expect(h!.height).toBeGreaterThan(40)
})

test('bold shortcut wraps the selection in marks', async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['make this bold'])
  await focus(server, app, ids[0], 14)
  await app.keyboard.press('Shift+Alt+ArrowLeft')
  await app.keyboard.press('Meta+b')
  await expect.poll(() => nameOf(server, ids[0])).toBe('make this **bold**')
})

test('Cmd+K then Escape: typing goes on in the line', async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['one', 'two'])
  await focus(server, app, ids[0], 3)
  await app.keyboard.press('Meta+k')
  await expect(app.locator('.dialog.palette')).toBeVisible()
  await app.keyboard.press('Escape')
  await app.keyboard.type('X')
  await expect.poll(() => nameOf(server, ids[0])).toBe('oneX')
})

test('the command line offers steps for the focused line first', async ({ app, server }) => {
  const { ids } = await pageWith(server, app, ['one'])
  await focus(server, app, ids[0], 3)
  await app.keyboard.press('Meta+k')
  await expect(app.locator('.dialog-item').first()).toHaveText(/Add supertag/)
  await app.keyboard.type('toggle done')
  await app.keyboard.press('Enter')
  await expect(app.locator(`.row.node[data-id="${ids[0]}"] .checkbox`)).toBeVisible()
})
