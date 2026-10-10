// Screenshots of the web UI for comparing with the spec's (docs/images/web/, 2x).
import { test } from './fixtures'
import { join, resolve } from 'node:path'

const out = resolve(import.meta.dirname, '../../docs/images/web')
const shot = (name: string) => ({ path: join(out, `${name}.png`) })

test('today dark', async ({ app }) => {
  await app.mouse.move(0, 400)
  await app.screenshot(shot('today-dark'))
})

test('today light', async ({ app, server }) => {
  await app.goto(`${server.url}?theme=light`)
  await app.locator('.panel .title').waitFor()
  await app.mouse.move(0, 400)
  await app.screenshot(shot('today-light'))
})

async function openTasks(server: any) {
  const hits = await server.rpc('find', { query: 'Open tasks', limit: 5 })
  return hits.find((h: any) => h.name === 'Open tasks').id
}

test('views: table, board, calendar', async ({ app, server }) => {
  const id = await openTasks(server)
  await server.run('nav.open', { node: id })
  await server.run('view.set', { node: id, kind: 'table' })
  await app.locator('.table').waitFor()
  await app.mouse.move(0, 400)
  await app.screenshot(shot('table'))
  await server.run('view.set', { node: id, kind: 'cards' })
  await server.run('view.group', { node: id, by: 'Status' })
  await app.locator('.board').waitFor()
  await app.screenshot(shot('board'))
  await server.run('view.group', { node: id, by: 'none' })
  await server.run('view.set', { node: id, kind: 'calendar' })
  await app.locator('.calendar').waitFor()
  await app.screenshot(shot('calendar'))
})

test('supertags, tag page, settings', async ({ app, server }) => {
  await server.run('nav.supertags')
  await app.locator('.supertags').waitFor()
  await app.mouse.move(0, 400)
  await app.screenshot(shot('supertags'))
  const tags = (await server.rpc('tags')).tags
  await server.run('nav.open', { node: tags.find((t: any) => t.name === 'task').id })
  await app.locator('.tag-tabs').waitFor()
  await app.screenshot(shot('tag-page'))
  await app.locator('.topbar-right .icon-btn[title="Settings"]').click()
  await app.locator('.settings').waitFor()
  await app.screenshot(shot('settings'))
})

test('command line, query builder, quick add, two panels', async ({ app, server }) => {
  const hits = await server.rpc('find', { query: 'Community garden', limit: 5 })
  const garden = hits.find((h: any) => h.name === 'Community garden').id
  await server.run('nav.open', { node: garden })
  await app.locator('.panel .title-input').waitFor()
  await app.keyboard.press('Meta+k')
  await app.locator('.dialog.palette').waitFor()
  await app.screenshot(shot('command-line'))
  await app.keyboard.press('Escape')
  const id = await openTasks(server)
  await server.run('nav.open', { node: id })
  await server.run('view.set', { node: id, kind: 'table' })
  await app.locator('.table').waitFor()
  await app.locator('.view-toolbar .tool[title^="Search conditions"]').click()
  await app.locator('.query-builder').waitFor()
  await app.screenshot(shot('query-builder'))
  await app.locator('.q-foot .btn', { hasText: 'Done' }).click()
  await app.keyboard.press('Meta+e')
  await app.locator('.dialog.quick').waitFor()
  await app.screenshot(shot('quick-add'))
  await app.keyboard.press('Escape')
  await server.run('nav.open', { node: garden })
  await app.locator('.panel .title-input').waitFor()
  await app.locator(`.row.node .bullet`).first().click({ button: 'right' })
  await app.locator('.node-menu .menu-item', { hasText: 'Open in new panel' }).click()
  await app.locator('.panel').nth(1).waitFor()
  await app.mouse.move(0, 400)
  await app.screenshot(shot('two-panels'))
})

test('page with a banner picture and an icon', async ({ app, server }) => {
  const hits = await server.rpc('find', { query: 'Community garden', limit: 5 })
  const garden = hits.find((h: any) => h.name === 'Community garden').id
  await server.run('node.setBanner', { node: garden, color: 'picture:orbit' })
  await server.run('node.setIcon', { node: garden, icon: '🌱' })
  await server.run('nav.open', { node: garden })
  await app.locator('.page-banner').waitFor()
  await app.waitForTimeout(200)
  await app.mouse.move(0, 400)
  await app.screenshot(shot('banner'))
})

test('mini sidebar rail', async ({ app }) => {
  await app.keyboard.press('Meta+Backslash')
  await app.locator('.sidebar.mini').waitFor()
  await app.waitForTimeout(300)
  await app.mouse.move(700, 400)
  await app.screenshot(shot('mini-rail'))
})
