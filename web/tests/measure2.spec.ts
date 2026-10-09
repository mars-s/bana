import { test } from './fixtures'
test('measure text widths at optical sizes', async ({ app }) => {
  const r = await app.evaluate(async () => {
    await document.fonts.ready
    const out: Record<string, number[]> = {}
    for (const t of ['Alpha node', 'A reminder to install the Tana']) {
      out[t] = []
      for (const o of [14, 15, 16, 18, 20]) {
        const s = document.createElement('span')
        s.textContent = t
        s.style.cssText = `font: 375 15px 'Inter Variable'; font-variation-settings: 'opsz' ${o}; position:absolute; white-space:nowrap`
        document.body.appendChild(s)
        out[t].push(Math.round(s.getBoundingClientRect().width * 10) / 10)
        s.remove()
      }
    }
    return out
  })
  console.log(JSON.stringify(r))
})
