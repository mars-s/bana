// Onboarding on first launch (Profile.tsx): name and picture, light or dark, how to start
// (fresh, the sample, an import), optional sync; every step skippable, and it never shows again.
// The profile page: name and picture, and Bana ID signed out (no relay needed for that).

import { join, resolve } from 'node:path'
import { test, expect, nameOf } from './fixtures'

// SHOTS=1: pictures of each step for the docs.
const shot = (page: import('@playwright/test').Page, name: string) =>
  process.env.SHOTS ? page.screenshot({ path: join(resolve(import.meta.dirname, '../..'), `docs/images/web/onboarding-${name}.png`) }) : Promise.resolve()

test.use({ onboarded: false, serveArgs: [] })

test('first launch: name, look, the sample, then never again', async ({ app, server }) => {
  const dialog = app.getByRole('dialog', { name: 'Welcome to Bana' })
  await expect(dialog).toBeVisible()
  await expect(dialog).toHaveAttribute('data-step', 'welcome')
  await shot(app, '1-welcome')
  await dialog.getByRole('button', { name: 'Get started' }).click()

  await dialog.getByLabel('Your name').fill('Ada Lovelace')
  await expect(dialog.locator('.avatar.large')).toHaveText('AL')
  await shot(app, '2-you')
  await dialog.getByRole('button', { name: 'Continue' }).click()

  await expect(dialog).toHaveAttribute('data-step', 'look')
  await dialog.getByRole('radio', { name: 'Light' }).click()
  await expect(app.locator('html')).toHaveAttribute('data-theme', 'light')
  await dialog.getByRole('radio', { name: 'Dark' }).click()
  await expect(app.locator('html')).toHaveAttribute('data-theme', 'dark')
  await dialog.getByRole('button', { name: 'Continue' }).click()

  await expect(dialog).toHaveAttribute('data-step', 'start')
  await shot(app, '3-start')
  await dialog.getByRole('button', { name: /Open the sample/ }).click()
  await expect(dialog).toHaveAttribute('data-step', 'sync')
  await expect(dialog.getByRole('button', { name: 'Create Bana ID' })).toBeVisible()
  await expect(dialog.getByRole('button', { name: 'Pair with a code instead' })).toBeVisible()
  await shot(app, '4-sync')
  await dialog.getByRole('button', { name: 'Not now' }).click()
  await expect(dialog).toBeHidden()

  // Saved beside the graph, and the name is the graph's #person node.
  const p = await server.rpc('profile.get')
  expect(p).toMatchObject({ name: 'Ada Lovelace', onboarded: true })
  expect(await nameOf(server, p.me)).toBe('Ada Lovelace')
  // The sample is in the Library.
  const lib = (await server.rpc('sidebar')).workspaces[0].library
  expect((await server.run('graph.inspect', { node: lib })).result.children.length).toBeGreaterThan(2)
  // The account button shows the initials.
  await expect(app.locator('.topbar-right [aria-label="Settings"] .avatar')).toHaveText('AL')

  await app.reload()
  await expect(app.locator('.panel .title').first()).toBeVisible()
  await app.waitForTimeout(500)
  await expect(app.getByRole('dialog', { name: 'Welcome to Bana' })).toHaveCount(0)
})

test('skip setup on the first screen: it never shows again', async ({ app, server }) => {
  const dialog = app.getByRole('dialog', { name: 'Welcome to Bana' })
  await dialog.getByRole('button', { name: 'Skip setup' }).click()
  await expect(dialog).toBeHidden()
  expect((await server.rpc('profile.get')).onboarded).toBe(true)
  await app.reload()
  await expect(app.locator('.panel .title').first()).toBeVisible()
  await app.waitForTimeout(500)
  await expect(dialog).toHaveCount(0)
})

test('import a file while onboarding', async ({ app, server }) => {
  const dialog = app.getByRole('dialog', { name: 'Welcome to Bana' })
  await dialog.getByRole('button', { name: 'Get started' }).click()
  await dialog.getByRole('button', { name: 'Continue' }).click()
  await dialog.getByRole('button', { name: 'Continue' }).click()
  await dialog.getByLabel('Import file').setInputFiles({ name: 'garden.md', mimeType: 'text/markdown', buffer: Buffer.from('- Garden plan\n  - Tomatoes\n') })
  await expect(dialog).toHaveAttribute('data-step', 'sync')
  const found = await server.rpc('find', { query: 'Tomatoes' })
  expect(JSON.stringify(found)).toContain('Tomatoes')
})

test('the profile page: name, picture and a signed-out Bana ID', async ({ app, server }) => {
  await app.getByRole('dialog', { name: 'Welcome to Bana' }).getByRole('button', { name: 'Skip setup' }).click()
  await app.locator('.topbar-right [aria-label="Settings"]').click()
  await app.locator('.account-menu .menu-item', { hasText: 'Profile and Bana ID' }).click()
  const page = app.locator('.profile-settings')
  await expect(page).toBeVisible()
  await page.getByLabel('Your name').fill('Grace Hopper')
  await page.getByLabel('Your name').press('Enter')
  await expect.poll(async () => (await server.rpc('profile.get')).name).toBe('Grace Hopper')
  // A picture, then back to initials.
  const png = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==', 'base64')
  await page.getByLabel('Picture file').setInputFiles({ name: 'me.png', mimeType: 'image/png', buffer: png })
  await expect(page.locator('.avatar.large img')).toBeVisible()
  await expect.poll(async () => (await server.rpc('profile.get')).avatar).toMatch(/^data:image\/jpeg;base64,/)
  await page.getByRole('button', { name: 'Use initials' }).click()
  await expect(page.locator('.avatar.large')).toHaveText('GH')
  // Signed out: create, sign in, recover or pair; the relay can be changed.
  await expect(page.getByRole('button', { name: 'Create Bana ID' })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Sign in with a passkey' })).toBeVisible()
  await page.getByRole('button', { name: 'Use a recovery code' }).click()
  await page.getByLabel('Recovery code').fill('not a code')
  await page.getByRole('button', { name: 'Recover', exact: true }).click()
  await expect(page.locator('.sync-error')).toContainText('recovery code')
  await expect(page.locator('.relay-choice summary')).toContainText('bana-sync.lifeos-read-mirror-worker.workers.dev')
})
