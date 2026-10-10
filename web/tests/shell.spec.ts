// The shell (spec 07, 09): the mini sidebar's flyouts, tabs, custom keybindings.
import { test, expect } from './fixtures'

test('the mini rail opens Pinned and Workspaces flyouts; an item opens and the flyout closes', async ({ app }) => {
  await app.keyboard.press('Meta+\\')
  await expect(app.locator('.sidebar.mini')).toBeVisible()
  await app.locator('.sidebar.mini .rail > .side-row[title="Pinned"]').click()
  const fly = app.locator('.flyout[aria-label="Pinned"]')
  await expect(fly).toBeVisible()
  await expect(fly.locator('.side-row', { hasText: 'Community garden' })).toBeVisible()
  // Escape closes it.
  await app.keyboard.press('Escape')
  await expect(fly).toHaveCount(0)
  await app.locator('.sidebar.mini .rail > .side-row[title="Pinned"]').click()
  await fly.locator('.side-row', { hasText: 'Community garden' }).click()
  await expect(fly).toHaveCount(0)
  await expect(app.locator('.panel .title-input')).toHaveValue('Community garden')
  // Workspaces: the workspace's entries and Manage workspaces; clicking elsewhere closes it.
  await app.locator('.sidebar.mini .rail > .side-row[title="Workspaces"]').click()
  const ws = app.locator('.flyout[aria-label="Workspaces"]')
  await ws.locator('.side-ws-head').click()
  await expect(ws.locator('.side-row', { hasText: 'Library' })).toBeVisible()
  await expect(ws.locator('.side-foot')).toHaveText('Manage workspaces')
  await app.locator('.panel').click({ position: { x: 400, y: 600 } })
  await expect(ws).toHaveCount(0)
})

test('Ctrl+Shift+PageUp/PageDown and dragging move tabs', async ({ app }) => {
  await app.keyboard.press('Meta+t')
  await expect(app.locator('.tab')).toHaveCount(2)
  const titles = () => app.locator('.tab').allTextContents()
  await app.keyboard.press('Meta+1')
  await app.locator('.sidebar .side-ws-head').first().click()
  await app.locator('.sidebar .side-row', { hasText: 'Library' }).click()
  await expect(app.locator('.tab').first()).toContainText('Library')
  await app.keyboard.press('Control+Shift+PageDown')
  await expect(app.locator('.tab').last()).toContainText('Library')
  await expect(app.locator('.tab.active')).toContainText('Library')
  await app.keyboard.press('Control+Shift+PageUp')
  await expect(app.locator('.tab').first()).toContainText('Library')
  const before = await titles()
  await app.locator('.tab').first().dragTo(app.locator('.tab').last())
  await expect.poll(titles).toEqual([before[1], before[0]])
})

test('a custom keybinding runs its command before the default', async ({ app, server }) => {
  await server.run('settings.setKeybinding', { shortcut: 'Mod+Shift+H', command: 'ui.quickAdd' })
  await app.reload()
  await expect(app.locator('.panel .title, .panel .title-input').first()).toBeVisible()
  await app.keyboard.press('Meta+Shift+h')
  await expect(app.locator('[role=dialog][aria-label="Quick add"]')).toBeVisible()
  await app.keyboard.press('Escape')
  // Set from Settings > Keybindings.
  await app.locator('.topbar [title="Account"]').click()
  await app.locator('.account-menu .menu-item', { hasText: 'Settings' }).click()
  await app.locator('.settings-nav .view-tab', { hasText: 'Keybindings' }).click()
  await expect(app.locator('.custom-keys kbd')).toHaveText('Mod+Shift+H')
  await app.locator('.key-capture').press('Meta+Shift+KeyJ')
  await expect(app.locator('.key-capture')).toHaveValue('Mod+Shift+J')
  await app.locator('.custom-keys input[list]').fill('ui.recents')
  await app.locator('.custom-keys .btn', { hasText: 'Add' }).click()
  await expect.poll(async () => (await server.run('settings.get')).result.keybindings).toEqual({ 'Mod+Shift+H': 'ui.quickAdd', 'Mod+Shift+J': 'ui.recents' })
  await app.locator('.custom-keys button[aria-label="Remove Mod+Shift+H"]').click()
  await expect.poll(async () => Object.keys((await server.run('settings.get')).result.keybindings)).toEqual(['Mod+Shift+J'])
})

test('Settings open in a second panel with the sections in order; Escape closes them (N55)', async ({ app }) => {
  await app.locator('.topbar [title="Account"]').click()
  await app.locator('.account-menu .menu-item', { hasText: 'Settings' }).click()
  await expect(app.locator('.panel')).toHaveCount(2)
  const settings = app.locator('.panel.settings-panel')
  await expect(settings.locator('.crumbs')).toContainText('User settings')
  await expect(settings.locator('.settings-nav .view-tab')).toHaveText(['Preferences', 'Notifications seen', 'Progress stats', 'Date and time', 'Move targets', 'Journaling', 'API keys', 'Sync', 'Keybindings'])
  await expect(settings.locator('button[aria-label="Use system theme"]')).toBeVisible()
  await app.keyboard.press('Escape')
  await expect(app.locator('.panel')).toHaveCount(1)
})

test('the theme follows the system appearance until a theme is picked (N56)', async ({ app }) => {
  await app.emulateMedia({ colorScheme: 'light' })
  await expect(app.locator('html')).toHaveAttribute('data-theme', 'light')
  await app.emulateMedia({ colorScheme: 'dark' })
  await expect(app.locator('html')).toHaveAttribute('data-theme', 'dark')
  await app.locator('.topbar [title="Account"]').click()
  await app.locator('.account-menu .menu-item', { hasText: 'Settings' }).click()
  await app.locator('button[aria-label="Use system theme"]').click()
  await app.locator('button[aria-label="Dark theme"]').click()
  await expect(app.locator('html')).toHaveAttribute('data-theme', 'light')
  await app.emulateMedia({ colorScheme: 'dark' })
  await expect(app.locator('html')).toHaveAttribute('data-theme', 'light')
})

test('the sidebar resizes by its edge and slides between modes (N51)', async ({ app }) => {
  const side = app.locator('aside.sidebar')
  const w0 = (await side.boundingBox())!.width
  const grip = (await app.locator('.side-grip').boundingBox())!
  await app.mouse.move(grip.x + 2, grip.y + 300)
  await app.mouse.down()
  await app.mouse.move(grip.x + 62, grip.y + 300, { steps: 4 })
  await app.mouse.up()
  await expect.poll(async () => Math.round((await side.boundingBox())!.width - w0)).toBeGreaterThan(50)
  // Full to mini to hidden: the width animates (it is not there at once).
  await app.keyboard.press('Meta+\\')
  await app.keyboard.press('Meta+\\')
  const mid = (await side.boundingBox())?.width ?? 0
  await expect.poll(async () => (await side.boundingBox())?.width ?? 0).toBe(0)
  expect(mid).toBeGreaterThan(0)
})

test('the gap between panels resizes them; a half panel puts the side card under the outline (N52)', async ({ app }) => {
  // Today in two panels side by side.
  await app.keyboard.press('Meta+m')
  await expect(app.locator('.panel')).toHaveCount(2)
  const first = app.locator('.panel').first()
  const side = first.locator('.page-side')
  await expect(side).toBeVisible()
  const main = (await first.locator('.page-main').boundingBox())!
  const card = (await side.boundingBox())!
  expect(card.y).toBeGreaterThan(main.y + 50)
  const w0 = (await first.boundingBox())!.width
  const grip = (await app.locator('.panel-grip').boundingBox())!
  await app.mouse.move(grip.x + 3, grip.y + 300)
  await app.mouse.down()
  await app.mouse.move(grip.x + 103, grip.y + 300, { steps: 5 })
  await app.mouse.up()
  await expect.poll(async () => Math.round((await first.boundingBox())!.width - w0)).toBeGreaterThan(80)
})

test('the day toolbar goes to a picked date, and calendar sync explains itself', async ({ app }) => {
  await app.locator('.day-nav [title="Go to date"]').click()
  await app.locator('.date-picker .dp-day:not(.out)', { hasText: /^15$/ }).click()
  await app.locator('.date-picker .btn', { hasText: 'OK' }).click()
  await expect(app.locator('.panel .title').first()).toContainText('15 Oct')
  await app.locator('.day-nav [title="Set up calendar sync"]').click()
  await expect(app.locator('.toast')).toContainText('Calendar sync')
})

test('dialogs and submenus sit where spec 12 puts them', async ({ app }) => {
  // Measured in Tana (d48, d56; spec 12): Search and open at y 180 with a header label, Quick add
  // at y 368, submenus overlapping their menu by about 6.6.
  await app.locator('.day-nav').waitFor()
  await app.keyboard.press('Meta+s')
  await expect(app.locator('.dialog.search .search-head')).toHaveText('Search and open')
  await app.waitForTimeout(300)
  expect(Math.round((await app.locator('.dialog.search').boundingBox())!.y)).toBe(180)
  await app.keyboard.press('Escape')
  await app.keyboard.press('Meta+e')
  await expect(app.locator('.dialog.quick')).toBeVisible()
  await app.waitForTimeout(300)
  expect(Math.round((await app.locator('.dialog.quick').boundingBox())!.y)).toBe(368)
})

test('the day title uses Inter at its automatic optical size (N59.7)', async ({ app }) => {
  // Tana's "Today, Sat, 10 Oct" at 29 px 550 is 232.6 wide.
  const w = (await app.locator('.panel .title').first().boundingBox())!.width
  const text = await app.locator('.panel .title').first().evaluate((e) => {
    const r = document.createRange()
    r.selectNodeContents(e)
    return r.getBoundingClientRect().width
  })
  expect(w).toBeGreaterThan(0)
  expect(Math.abs(text - 232.6)).toBeLessThan(4)
})

test('Supertags page: no sidebar fill, a 600 title, Browse templates; quick add starts with a bare line (N69, N71, N59.38)', async ({ app }) => {
  await app.locator('.sidebar .side-row', { hasText: 'Supertags' }).click()
  await expect(app.locator('.supertags')).toBeVisible()
  await expect(app.locator('.sidebar .side-row.active', { hasText: 'Supertags' })).toHaveCount(0)
  expect(await app.locator('.panel .title').first().evaluate((e) => getComputedStyle(e).fontWeight)).toBe('600')
  await expect(app.locator('.head-actions .btn', { hasText: 'Browse templates' })).toBeVisible()
  await expect(app.locator('.tab.active .tab-title')).toHaveText('Tag overview')
  await app.locator('.filter-input').blur()
  await app.keyboard.press('Meta+e')
  const input = app.locator('.dialog.quick input')
  await expect(input).toBeVisible()
  expect(await input.getAttribute('placeholder')).toBeNull()
})

test('Settings layout as spec 12: title y 100, nav box from x 898.5, options from x 1116 with an icon, a subtitle (N67)', async ({ app }) => {
  await app.locator('.topbar [title="Account"]').click()
  await app.locator('.account-menu .menu-item', { hasText: 'Settings' }).click()
  const p = app.locator('.settings-panel')
  await expect(p.locator('.settings-sub')).toHaveText('Your user preferences')
  const r = async (sel: string) => (await p.locator(sel).first().boundingBox())!
  expect(Math.abs((await r('.title')).y - 100)).toBeLessThan(1.5)
  expect(Math.abs((await r('.settings-nav')).x - 898.5)).toBeLessThan(1.5)
  expect(Math.abs((await r('.settings-body')).x - 1116)).toBeLessThan(1.5)
  expect(Math.abs((await r('.setting-label')).x - 1139.3)).toBeLessThan(1.5)
  await expect(p.locator('.setting').first().locator('.setting-icon')).toBeVisible()
})

test('the sidebar button is named for what it does next; the account button reads Settings (N75)', async ({ app }) => {
  await expect(app.getByRole('button', { name: 'Show mini sidebar' })).toBeVisible()
  await app.getByRole('button', { name: 'Show mini sidebar' }).click()
  await expect(app.getByRole('button', { name: 'Hide sidebar' })).toBeVisible()
  await app.getByRole('button', { name: 'Hide sidebar' }).click()
  await app.getByRole('button', { name: 'Show sidebar' }).click()
  await expect(app.getByRole('button', { name: 'Show mini sidebar' })).toBeVisible()
  await expect(app.locator('.topbar').getByRole('button', { name: 'Settings' })).toBeVisible()
})

test('the day toolbar ends with Workspaces ▾ at the column edge; it opens a workspace (N59.9)', async ({ app, server }) => {
  const btn = app.locator('.day-nav .ws-btn')
  const b = (await btn.boundingBox())!
  expect(Math.round(b.width)).toBe(98)
  expect(Math.round(b.x + b.width)).toBe(1020)
  await btn.click()
  const lib = (await server.rpc('sidebar')).workspaces[0]
  await app.locator('.ws-list .menu-item').first().click()
  await expect.poll(async () => (await server.rpc('page', { panel: 'main', start: 0, count: 1 })).zoom).toBe(lib.library)
})
