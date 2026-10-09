import { test, pageWith, focus } from './fixtures'
test('dbg', async ({ app, server }) => {
  app.on('console', (m) => console.log('LOG', m.text()))
  app.on('pageerror', (m) => console.log('ERR', m.message))
  const { ids } = await pageWith(server, app, ['one', 'two'])
  await focus(server, app, ids[0], 3)
  await app.keyboard.press('Meta+k')
  await app.waitForTimeout(300)
  console.log(await app.locator('.scrim').count(), await app.evaluate(() => document.activeElement?.className))
})
