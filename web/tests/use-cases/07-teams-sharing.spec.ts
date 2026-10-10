// Use cases: workspaces, collaboration and publishing (docs/use-cases/07-teams-sharing.md).
// TanaCraft is a local app with one user: members, notifications and publishing are gaps.
import { test, expect, missing, works, partial, tree } from './helpers'
import { pageWith, focus, nameOf, clickLine } from '../fixtures'

test('UC-07.01 keep work and personal apart: a second workspace with its own Library', works('UC-07.01'), async ({ app, server }) => {
  await app.locator('.side-foot', { hasText: 'Manage workspaces' }).click()
  await app.locator('.workspaces input[placeholder="Workspace name"]').fill('Allotment club')
  await app.locator('.workspaces input[placeholder="Workspace name"]').press('Enter')
  await app.keyboard.press('Escape')
  const ws = (await server.rpc('sidebar')).workspaces
  expect(ws.map((w: any) => w.name)).toEqual(['Workspace', 'Allotment club'])
  expect(ws[1].library).not.toBe(ws[0].library)
  await app.locator('.side-ws-head', { hasText: 'Allotment club' }).click()
  await app.locator('.side-ws').nth(1).locator('.side-row', { hasText: 'Library' }).click()
  await expect(app.locator('.crumbs')).toContainText('Allotment club')
})

test('UC-07.02 rename a workspace', works('UC-07.02'), async ({ app, server }) => {
  await server.run('workspace.rename', { name: 'Home garden' })
  await expect(app.locator('.sidebar').getByText('Home garden')).toBeVisible()
})

test('UC-07.03 move a draft from the private workspace into another one (engine only; Move to cannot find other workspaces)', partial('UC-07.03'), async ({ app, server }) => {
  await server.run('workspace.create', { name: 'Club' })
  const club = (await server.rpc('sidebar')).workspaces[1]
  const { ids } = await pageWith(server, app, ['Rules for the shed'])
  await focus(server, app, ids[0], 0)
  await app.keyboard.press('Escape')
  await app.locator('.sel-toolbar button', { hasText: 'Move' }).click()
  await app.keyboard.type('Club')
  // Gap: the Move to menu lists nothing from the other workspace.
  await expect(app.locator('.menu.picker')).toContainText('Nothing matches')
  await app.keyboard.press('Escape')
  await server.run('node.move', { node: ids[0], parent: club.library })
  await expect.poll(async () => (await tree(server, club.library)).children?.map((c: any) => c.name)).toContain('Rules for the shed')
})

test('UC-07.04 two windows on one graph type into the same line and keep both words', partial('UC-07.04'), async ({ app, server, browser }) => {
  const { ids } = await pageWith(server, app, ['Agenda:'], 'Shared notes')
  const b = await browser.newPage({ viewport: { width: 1512, height: 949 } })
  await b.goto(server.url)
  await expect(b.locator('.panel .title-input')).toHaveValue('Shared notes')
  await clickLine(app, 'Agenda:')
  await clickLine(b, 'Agenda:')
  await app.keyboard.type(' tools')
  await expect.poll(() => nameOf(server, ids[0])).toBe('Agenda: tools')
  await b.keyboard.press('End')
  await b.keyboard.type(' seeds')
  await expect.poll(() => nameOf(server, ids[0])).toContain('seeds')
  expect(await nameOf(server, ids[0])).toContain('tools')
  // Gap: both windows talk to one engine on one machine; no sync between people or devices.
  await b.close()
})

missing('UC-07.05', 'invite people to a workspace and work on it together', 'out-of-scope')
missing('UC-07.06', 'sync a workspace between computers (planned: iroh)')
missing('UC-07.07', 'notify a member about a node (Notify @user) and see notifications')
missing('UC-07.08', 'publish a node as a read-only web page (outline or article), with optional password')
missing('UC-07.09', 'choose which other workspaces a workspace may search and @-mention (Allow content from)')
missing('UC-07.10', 'copy a link to a node that opens it from another app')
missing('UC-07.11', 'save a view as the default for everyone in a shared workspace', 'out-of-scope')
