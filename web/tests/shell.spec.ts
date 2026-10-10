// The shell (spec 07, 09): the mini sidebar's flyouts, tabs, custom keybindings.
import { test, expect } from './fixtures'

test('the mini rail opens Pinned and Workspaces flyouts; an item opens and the flyout closes', async ({ app }) => {
  await app.keyboard.press('Meta+\\')
  await expect(app.locator('.sidebar.mini')).toBeVisible()
  await app.locator('.sidebar.mini > .side-row[title="Pinned"]').click()
  const fly = app.locator('.flyout[aria-label="Pinned"]')
  await expect(fly).toBeVisible()
  await expect(fly.locator('.side-row', { hasText: 'Community garden' })).toBeVisible()
  // Escape closes it.
  await app.keyboard.press('Escape')
  await expect(fly).toHaveCount(0)
  await app.locator('.sidebar.mini > .side-row[title="Pinned"]').click()
  await fly.locator('.side-row', { hasText: 'Community garden' }).click()
  await expect(fly).toHaveCount(0)
  await expect(app.locator('.panel .title-input')).toHaveValue('Community garden')
  // Workspaces: the workspace's entries and Manage workspaces; clicking elsewhere closes it.
  await app.locator('.sidebar.mini > .side-row[title="Workspaces"]').click()
  const ws = app.locator('.flyout[aria-label="Workspaces"]')
  await expect(ws.locator('.side-row', { hasText: 'Library' })).toBeVisible()
  await expect(ws.locator('.side-foot')).toHaveText('Manage workspaces')
  await app.locator('.panel').click({ position: { x: 400, y: 600 } })
  await expect(ws).toHaveCount(0)
})
