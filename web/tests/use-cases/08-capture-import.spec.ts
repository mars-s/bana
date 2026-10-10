// Use cases: capture, import, export and keeping data safe (docs/use-cases/08-capture-import.md).
import { execFileSync } from 'node:child_process'
import { writeFileSync, mkdtempSync, mkdirSync, readFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { test, expect, missing, works, partial, tree, idOf, library, palette } from './helpers'
import { pageWith, focus, nameOf } from '../fixtures'

const bin = join(process.env.CARGO_TARGET_DIR ?? resolve(import.meta.dirname, '../../../target'), 'debug', 'bana')

test('UC-08.01 capture to an Inbox from outside the app and find it there', works('UC-08.01'), async ({ app, server }) => {
  const r = await server.run('node.capture', { name: 'Idea from the bus: seed bombs' })
  expect(r.result?.id ?? r.result).toBeTruthy()
  await palette(app, 'Open capture inbox')
  await expect(app.locator('.row.node', { hasText: 'Idea from the bus: seed bombs' })).toBeVisible()
})

test('UC-08.02 paste several lines of text: each becomes a node', works('UC-08.02'), async ({ app, server, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write'])
  const { page, ids } = await pageWith(server, app, [''])
  await focus(server, app, ids[0], 0)
  await app.evaluate(() => navigator.clipboard.writeText('Tomatoes\nBasil\nGarlic'))
  await app.keyboard.press('Meta+v')
  await expect.poll(async () => (await tree(server, page)).children.map((c: any) => c.name)).toEqual(['Tomatoes', 'Basil', 'Garlic'])
})

test('UC-08.03 paste an indented Markdown list: the nesting is kept', works('UC-08.03'), async ({ app, server, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write'])
  const { page, ids } = await pageWith(server, app, [''])
  await focus(server, app, ids[0], 0)
  await app.evaluate(() => navigator.clipboard.writeText('- Beds\n  - North bed\n  - South bed\n- Paths'))
  await app.keyboard.press('Meta+v')
  await expect.poll(async () => (await tree(server, page)).children.map((c: any) => [c.name, (c.children ?? []).map((k: any) => k.name)])).toEqual([
    ['Beds', ['North bed', 'South bed']],
    ['Paths', []],
  ])
})

test('UC-08.04 import a Markdown or OPML file (Workflowy, other outliners) from the account menu', works('UC-08.04'), async ({ app, server }) => {
  await app.locator('.topbar [title="Account"]').click()
  const chooser = app.waitForEvent('filechooser')
  await app.locator('.account-menu .menu-item', { hasText: 'Import content' }).click()
  await (await chooser).setFiles({
    name: 'plan.opml',
    mimeType: 'text/x-opml',
    buffer: Buffer.from('<?xml version="1.0"?><opml version="2.0"><body><outline text="Spring plan"><outline text="Sow peas" /></outline></body></opml>'),
  })
  await expect.poll(async () => (await server.rpc('find', { query: 'Sow peas', limit: 5 })).map((h: any) => h.name)).toContain('Sow peas')
  const md = join(mkdtempSync(join(tmpdir(), 'uc-')), 'notes.md')
  writeFileSync(md, '# Autumn\n- [ ] Plant bulbs #task\n- Rake\n')
  await server.run('import.file', { path: md })
  await expect.poll(async () => (await server.rpc('find', { query: 'Plant bulbs', limit: 5 })).length).toBeGreaterThan(0)
  const bulbs = await idOf(server, 'Plant bulbs')
  expect((await tree(server, bulbs)).tags).toContain('task')
})

test('UC-08.05 export a page as Markdown, OPML or JSON with its fields', works('UC-08.05'), async ({ server }) => {
  const garden = await idOf(server, 'Community garden')
  const md = (await server.run('export.markdown', { node: garden })).result
  expect(JSON.stringify(md)).toContain('Raised beds')
  expect(JSON.stringify(md)).toMatch(/Status/)
  const opml = (await server.run('export.opml', { node: garden })).result
  expect(JSON.stringify(opml)).toContain('<outline')
  const json = (await server.run('export.json', { node: garden })).result
  expect(json.fields.Status).toBe('In progress')
})

test('UC-08.06 everything stays on your computer and survives a crash', works('UC-08.06'), async ({ app, server }) => {
  const { ids } = await pageWith(server, app, [''])
  await focus(server, app, ids[0], 0)
  await app.keyboard.type('Written offline')
  await expect.poll(() => nameOf(server, ids[0])).toBe('Written offline')
  await app.waitForTimeout(1500)
  await server.kill('SIGKILL')
  await server.restart()
  await app.reload()
  await expect.poll(() => nameOf(server, ids[0])).toBe('Written offline')
})

test('UC-08.07 add notes from a script or the terminal (CLI)', works('UC-08.07'), async () => {
  const file = join(mkdtempSync(join(tmpdir(), 'uc-')), 'g.bana')
  execFileSync(bin, ['sample', '--out', file])
  const id = execFileSync(bin, ['--file', file, 'add', 'Order manure', '--today', '--tag', 'task']).toString().trim()
  expect(Number(id)).toBeGreaterThan(0)
  execFileSync(bin, ['--file', file, 'capture', 'Call the council'])
  const found = execFileSync(bin, ['--file', file, 'search', 'manure']).toString()
  expect(found).toContain('Order manure')
  expect(execFileSync(bin, ['--file', file, 'export', 'today']).toString()).toContain('Order manure')
})

test('UC-08.08 paste code: kept as code, inline only (no code block)', partial('UC-08.08'), async ({ app, server }) => {
  const { ids } = await pageWith(server, app, [''])
  await server.run('edit.pasteAsCode', { node: ids[0], caret: 0, text: 'let x = 1' })
  await expect(app.locator(`.row.node[data-id="${ids[0]}"] .code`)).toHaveText('let x = 1')
})

test('UC-08.09 import into the Library by default', works('UC-08.09'), async ({ server }) => {
  await server.run('import.markdown', { text: '- Bean varieties\n  - Borlotti' })
  const lib = await library(server)
  expect((await tree(server, lib)).children.map((c: any) => c.name)).toContain('Bean varieties')
})

test('UC-08.10 import a Notion or Logseq export folder, or a Roam JSON export', works('UC-08.10'), async ({ server }) => {
  const dir = mkdtempSync(join(tmpdir(), 'notion-'))
  mkdirSync(join(dir, 'Garden 0123456789abcdef0123456789abcdef'))
  writeFileSync(join(dir, 'Garden 0123456789abcdef0123456789abcdef', 'Beds 0123456789abcdef0123456789abcdef.md'), '# Beds\n\n- North bed\n- South bed #task\n')
  await server.run('import.folder', { path: dir })
  const lib = await library(server)
  const garden = (await tree(server, lib)).children.find((c: any) => c.name === 'Garden')
  expect(garden.children[0].name).toBe('Beds')
  expect(garden.children[0].children.map((c: any) => c.name)).toEqual(['North bed', 'South bed'])
  await server.run('import.roam', { text: JSON.stringify([{ title: 'Roam page', children: [{ string: '{{[[DONE]]}} prune [[Apple tree]]' }] }]) })
  const roam = (await tree(server, lib)).children.find((c: any) => c.name === 'Roam page')
  expect(roam.children[0].done).toBe(true)
})

test('UC-08.11 paste Tana Paste text: tagged nodes, fields, dates and a search at once', works('UC-08.11'), async ({ app, server, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write'])
  const { page, ids } = await pageWith(server, app, [''], 'From an AI')
  await focus(server, app, ids[0], 0)
  const paste = '%%tana%%\n- Plant garlic #task\n  - Due:: [[date:2026-10-20]]\n- Open tasks %%search%%\n  - #task\n'
  await app.evaluate((t) => navigator.clipboard.writeText(t), paste)
  await app.keyboard.press('Meta+v')
  await expect.poll(async () => ((await tree(server, page)).children ?? []).map((c: any) => c.name)).toEqual(['Plant garlic', 'Open tasks'])
  const garlic = (await tree(server, page)).children[0]
  expect(garlic.tags).toEqual(['task'])
  expect(garlic.fields.Due).toBeTruthy()
  // Into an empty line between two others, it lands right there (DIFFS N80).
  await server.run('node.add', { name: 'Agenda', parent: page, index: 0 })
  const blank = (await server.run('node.add', { name: '', parent: page, index: 1 })).result.id
  await focus(server, app, blank, 0)
  await app.evaluate((t) => navigator.clipboard.writeText(t), '%%tana%%\n- TP one\n- TP two\n')
  await app.keyboard.press('Meta+v')
  await expect.poll(async () => ((await tree(server, page)).children ?? []).map((c: any) => c.name)).toEqual(['Agenda', 'TP one', 'TP two', 'Plant garlic', 'Open tasks'])
})
test('UC-08.12 paste spreadsheet cells as nodes with fields, shown as a table', works('UC-08.12'), async ({ app, server, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write'])
  const { page, ids } = await pageWith(server, app, [''], 'Seeds')
  await app.evaluate(() => navigator.clipboard.writeText('Plant\tSow\tSpacing\nBeans\tMay\t10 cm\nKale\tApril\t40 cm\n'))
  await focus(server, app, ids[0], 0)
  await app.keyboard.press('Meta+v')
  await expect.poll(async () => (await tree(server, page)).children.map((c: any) => c.name)).toEqual(['Beans', 'Kale'])
  const kale = (await tree(server, page)).children[1]
  expect(kale.fields).toEqual({ Sow: 'April', Spacing: '40 cm' })
  // As a table, the columns are the spreadsheet's.
  await server.run('view.set', { node: page, kind: 'table' })
  await server.run('view.addColumn', { node: page, field: 'Sow' })
  await expect(app.locator('.t-row', { hasText: 'Kale' })).toContainText('April')
})
test('UC-08.13 export a whole workspace as a ZIP of Markdown files (by page, by supertag, a vault)', works('UC-08.13'), async ({ app }) => {
  await app.locator('.topbar [title="Account"]').click()
  await app.locator('.account-menu .menu-item.has-sub', { hasText: 'Export everything as Markdown' }).hover()
  const [download] = await Promise.all([app.waitForEvent('download'), app.locator('.account-menu .submenu .menu-item', { hasText: 'As an Obsidian vault' }).click()])
  expect(download.suggestedFilename()).toMatch(/vault 2026-10-10\.zip$/)
  const zip = readFileSync(await download.path())
  // Stored entries: names and Markdown are readable in the archive itself.
  const text = zip.toString('utf8')
  expect(zip.subarray(0, 2).toString()).toBe('PK')
  expect(text).toContain('Library/Community garden.md')
  expect(text).toContain('Daily notes/2026-10-10.md')
  expect(text).toContain('tags: ["project"]')
  expect(text).toContain('[[Robin Ash]]')
})
// UC-08.14: Cmd+Shift+E in any app shows the desktop app's capture window, which loads this page
// at #capture (the shortcut and window are checked by Rust tests in apps/bana-desktop). Here:
// that page is Quick add alone, saves each line to today and closes on Escape.
test('UC-08.14 capture from any app: a small window of its own saves lines to today and closes', works('UC-08.14'), async ({ app, server }) => {
  const w = await app.context().newPage()
  await w.setViewportSize({ width: 630, height: 210 })
  await w.addInitScript(() => {
    window.close = () => document.documentElement.setAttribute('data-closed', '')
  })
  await w.goto(server.url + '#capture')
  // Only Quick add: no sidebar or outline, and it fits the window.
  await expect(w.getByRole('dialog', { name: 'Quick add' })).toBeVisible()
  await expect(w.locator('.sidebar, .panel')).toHaveCount(0)
  const box = await w.locator('.dialog.quick').boundingBox()
  expect(box?.height ?? 999).toBeLessThanOrEqual(210)
  await expect(w.getByLabel('New line')).toBeFocused()
  await w.keyboard.type('Call the vet')
  await w.keyboard.press('Enter')
  await w.keyboard.type('Buy bird seed')
  await w.keyboard.press('Escape')
  await expect(w.locator('html')).toHaveAttribute('data-closed', '')
  // Both lines are on today's page in the main window.
  await server.run('nav.today')
  await expect(app.locator('.row.node', { hasText: 'Call the vet' })).toBeVisible()
  await expect(app.locator('.row.node', { hasText: 'Buy bird seed' })).toBeVisible()
})
test('UC-08.15 browse and install templates from a gallery', works('UC-08.15'), async ({ app }) => {
  await app.locator('.topbar [title="Account"]').click()
  await app.locator('.account-menu .menu-item', { hasText: 'Browse templates' }).click()
  const dialog = app.getByRole('dialog', { name: 'Templates' })
  await expect(dialog.locator('.template-row')).toHaveCount(6)
  await dialog.locator('.template-row', { hasText: 'Reading list' }).getByRole('button', { name: 'Install' }).click()
  // The new page opens, with its search showing the books to read.
  await expect(app.locator('.panel .title-input')).toHaveValue('Reading list')
  await expect(app.locator('.row.node', { hasText: 'The book I keep meaning to start' }).first()).toBeVisible()
  await expect(app.locator('.row.node', { hasText: 'Up next' })).toBeVisible()
})
missing('UC-08.16', 'capture on the phone: app, home screen widgets, share sheet, Siri', 'out-of-scope')
missing('UC-08.17', 'voice memo on the phone, transcribed and tagged by AI', 'ai')
