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
  // What is typed shows at once, as the line it will be (N78).
  await expect(app.locator('.waiting-text')).toHaveText('typed while down')
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

// DIFFS N77: the engine killed 0 to 800 ms after the last key (some runs with the page 4 times
// slower): the line just typed is kept, and what is typed during the outage lands in its own new
// line, never glued onto another one.
const rng = (seed: number) => () => ((seed = (seed * 1103515245 + 12345) % 2 ** 31) / 2 ** 31)
for (let seed = 1; seed <= Number(process.env.KILL_SEEDS ?? 6); seed++) {
  test(`a kill right after typing loses nothing and mixes no lines (N77, seed ${seed})`, async ({ app, server }) => {
    if (seed % 2) await (await app.context().newCDPSession(app)).send('Emulation.setCPUThrottlingRate', { rate: 4 })
    const pick = rng(seed)
    const { page } = await pageWith(server, app, ['Agenda', 'Notes'], 'Kill page')
    await clickLine(app, 'Agenda')
    await app.keyboard.press('End')
    await app.keyboard.press('Enter')
    await app.keyboard.type(`kill line ${seed}`)
    await app.waitForTimeout(Math.floor(pick() * 800))
    await server.kill('SIGKILL')
    await app.keyboard.press('Enter')
    await app.keyboard.type('down A')
    await app.waitForTimeout(300)
    await server.restart()
    await expect(app.locator('.toast.offline')).toBeHidden({ timeout: 15_000 })
    const want = ['Agenda', `kill line ${seed}`, 'down A', 'Notes']
    await expect.poll(() => lines(app), { timeout: 10_000 }).toEqual(want)
    const names = async () => ((await server.run('graph.inspect', { node: page })).result.children ?? []).map((c: { name: string }) => c.name)
    await expect.poll(names).toEqual(want)
    // And on disk: a second crash now, then the file as the next start reads it.
    await server.kill('SIGKILL')
    await server.restart()
    expect(await names()).toEqual(want)
  })
}
