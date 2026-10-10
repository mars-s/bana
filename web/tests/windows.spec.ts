// Two windows on one graph (DIFFS N60): each has its own page and caret; typing in both at once
// never mixes keys into the wrong line, and both windows' words in one line are kept.
import { test, expect, pageWith, nameOf, clickLine } from './fixtures'
import type { Browser, Page } from '@playwright/test'

async function second(browser: Browser, url: string): Promise<Page> {
  const b = await browser.newPage({ viewport: { width: 1512, height: 949 } })
  await b.goto(url)
  await expect(b.locator('.panel .title-input, .panel .title').first()).toBeVisible()
  return b
}

test('two windows type in two lines at once, each in its own line', async ({ app, server, browser }) => {
  const { ids } = await pageWith(server, app, ['Volunteer meetup', 'Seed swap'], 'Two')
  const b = await second(browser, server.url)
  await expect(b.locator('.panel .title-input')).toHaveValue('Two')
  await clickLine(app, 'Volunteer meetup')
  await clickLine(b, 'Seed swap')
  await Promise.all([app.keyboard.type(' AAA', { delay: 40 }), b.keyboard.type(' BBB', { delay: 40 })])
  await expect.poll(() => nameOf(server, ids[0])).toBe('Volunteer meetup AAA')
  await expect.poll(() => nameOf(server, ids[1])).toBe('Seed swap BBB')
  // Neither caret moved to the other window's line.
  await expect(app.locator(`.editor-input[data-node="${ids[0]}"]`)).toBeFocused()
  await expect(b.locator(`.editor-input[data-node="${ids[1]}"]`)).toBeFocused()
  await b.close()
})

test('two windows typing in the same line keep both words', async ({ app, server, browser }) => {
  const { ids } = await pageWith(server, app, ['Volunteer meetup'], 'One')
  const b = await second(browser, server.url)
  await clickLine(app, 'Volunteer meetup')
  await clickLine(b, 'Volunteer meetup')
  await b.keyboard.press('Home')
  await Promise.all([app.keyboard.type(' AAA', { delay: 40 }), b.keyboard.type('BBB ', { delay: 40 })])
  await expect.poll(() => nameOf(server, ids[0])).toBe('BBB Volunteer meetup AAA')
  // Each window shows the merged line, its caret after its own words.
  await expect(app.locator(`.editor-input[data-node="${ids[0]}"]`)).toHaveValue('BBB Volunteer meetup AAA')
  await expect(b.locator(`.editor-input[data-node="${ids[0]}"]`)).toHaveValue('BBB Volunteer meetup AAA')
  await app.keyboard.type('!')
  await b.keyboard.type('?')
  await expect.poll(() => nameOf(server, ids[0])).toBe('BBB ?Volunteer meetup AAA!')
  await b.close()
})

test('keys typed at the end of a line another window splits land in the second half (N74)', async ({ app, server, browser }) => {
  const { page } = await pageWith(server, app, ['Pick a date for the build day'], 'Split')
  const b = await second(browser, server.url)
  await expect(b.locator('.panel .title-input')).toHaveValue('Split')
  await clickLine(b, 'Pick a date for the build day')
  await b.keyboard.press('End')
  await clickLine(app, 'Pick a date for the build day')
  await app.keyboard.press('Home')
  for (let i = 0; i < 4; i++) await app.keyboard.press('ArrowRight')
  await Promise.all([app.keyboard.press('Enter'), b.keyboard.type('123', { delay: 40 })])
  await b.waitForTimeout(300)
  await b.keyboard.type('456')
  const kids = async () => ((await server.run('graph.inspect', { node: page })).result.children ?? []).map((c: { name: string }) => c.name)
  await expect.poll(kids).toEqual(['Pick', ' a date for the build day123456'])
  await b.close()
})
