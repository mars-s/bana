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
