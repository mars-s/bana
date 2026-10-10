import { test } from './fixtures'
test('dbg', async ({ app }) => {
  const chip = app.locator('.row.node .chip').first()
  await app.screenshot({ path: '/private/tmp/claude-501/-Users-avi-Documents-ChatGPT-bana/3d2b6b1d-4b62-48f1-bbaf-b56df187d9ba/scratchpad/chip1.png', clip: { x: 250, y: 220, width: 500, height: 300 } })
  await chip.hover()
  await app.waitForTimeout(200)
  await app.screenshot({ path: '/private/tmp/claude-501/-Users-avi-Documents-ChatGPT-bana/3d2b6b1d-4b62-48f1-bbaf-b56df187d9ba/scratchpad/chip2.png', clip: { x: 250, y: 220, width: 500, height: 300 } })
})
