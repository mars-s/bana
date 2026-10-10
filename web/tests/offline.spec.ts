// The engine goes away mid-typing (DIFFS N73): the UI says so, keeps every edit in order (Enter
// still makes a new line), and sends them as soon as the engine is back, without a reload.
import { test, expect, pageWith, clickLine, lines, settle } from './fixtures'

test('killing and restarting the engine mid-typing keeps the lines as typed (N73)', async ({ app, server }) => {
  const { page } = await pageWith(server, app, ['kill line one'], 'Offline page')
  await clickLine(app, 'kill line one')
  await app.keyboard.type(' more')
  await settle(app)
  // Autosave has written everything so far.
  await app.waitForTimeout(1500)
  await server.kill('SIGKILL')
  await app.keyboard.press('Enter')
  await app.keyboard.type('typed while down')
  await expect(app.locator('.toast.offline')).toBeVisible()
  await app.waitForTimeout(800)
  await server.restart()
  await expect(app.locator('.toast.offline')).toBeHidden({ timeout: 10_000 })
  // No reload: the queued edits went in on their own, on the page that was open.
  await expect(app.locator('.panel .title-input')).toHaveValue('Offline page')
  await expect.poll(() => lines(app)).toEqual(['kill line one more', 'typed while down'])
  await app.keyboard.type('!')
  const names = async () => ((await server.run('graph.inspect', { node: page })).result.children ?? []).map((c: { name: string }) => c.name)
  await expect.poll(names).toEqual(['kill line one more', 'typed while down!'])
})

test('the offline notice shows when the engine goes away with nothing typed', async ({ app, server }) => {
  await server.kill('SIGKILL')
  await expect(app.locator('.toast.offline')).toBeVisible({ timeout: 5000 })
  await server.restart()
  await expect(app.locator('.toast.offline')).toBeHidden({ timeout: 10_000 })
})
