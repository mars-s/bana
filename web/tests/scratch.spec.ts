import { test, pageWith, focus, lines } from './fixtures'
test('dbg', async ({ app, server }) => {
  for (let round = 0; round < 15; round++) {
    const log: string[] = []
    app.removeAllListeners('request')
    app.on('request', (r) => {
      const d = r.postData()
      if (d && (d.includes('setName') || d.includes('"run"'))) log.push(d.slice(0, 160))
    })
    const { ids } = await pageWith(server, app, [''], `T${round}`)
    await app.evaluate(() => ((window as any).__log = []))
    await focus(server, app, ids[0], 0)
    const k = app.keyboard
    await k.type('Groceries')
    await k.press('Enter')
    await k.type('Milk')
    await k.press('Tab')
    await k.press('Enter')
    await k.type('Bread')
    await k.press('Enter')
    await k.type('Call mum')
    await app.waitForTimeout(400)
    const l = await lines(app)
    if (l[1] !== 'Milk') {
      console.log(JSON.stringify(l))
      console.log(log.join('\n'))
      console.log((await app.evaluate(() => (window as any).__log)).join(' | '))
      break
    }
  }
})
