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
