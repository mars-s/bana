// Shared bits for the use-case scenarios (docs/use-cases/). Each scenario is tagged with its
// use-case id and status: @works and @partial must pass; @missing and @out-of-scope are listed with
// test.fixme so `playwright test --list` shows every gap next to what works.
import { test, expect, type Server } from '../fixtures'
import type { Page } from '@playwright/test'

export { test, expect }

/** A use case that Bana does not have yet: listed, skipped. `ai` marks it out of scope until
 * the AI decision is made. */
export function missing(id: string, title: string, status: 'missing' | 'out-of-scope' | 'ai' = 'missing') {
  const tags = status === 'ai' ? ['@out-of-scope', '@ai'] : [`@${status}`]
  test.fixme(`${id} ${title}`, { tag: [`@${id}`, ...tags] }, async () => {})
}

/** Tags for a scenario that must pass. */
export const works = (id: string) => ({ tag: [`@${id}`, '@works'] })
export const partial = (id: string) => ({ tag: [`@${id}`, '@partial'] })

/** The node as exported JSON (name, fields, children, done...). */
export async function tree(server: Server, id: number): Promise<any> {
  return (await server.run('export.json', { node: id })).result
}

export async function fieldOf(server: Server, node: number, field: string): Promise<any> {
  return (await tree(server, node))?.fields?.[field]
}

/** The id of the first node whose text is exactly `name`. */
export async function idOf(server: Server, name: string): Promise<number> {
  const hits = await server.rpc('find', { query: name, limit: 20 })
  const hit = hits.find((h: any) => h.name === name)
  if (!hit) throw new Error(`no node named ${name}`)
  return hit.id
}

/** The Library of the main workspace. */
export async function library(server: Server): Promise<number> {
  return (await server.rpc('sidebar')).workspaces[0].library
}

/** Runs a command by typing its name into the command line (Cmd+K) and pressing Enter. */
export async function palette(app: Page, text: string) {
  await app.keyboard.press('Meta+k')
  await expect(app.locator('.dialog.palette')).toBeVisible()
  await app.keyboard.type(text)
  await app.keyboard.press('Enter')
}

/** The open page as the engine sees it. */
export async function page(server: Server): Promise<any> {
  return server.rpc('page', {})
}

/** A line of the open page as the UI draws it (heading, icon, description, done...). */
export async function rowOf(server: Server, id: number): Promise<any> {
  return ((await page(server)).rows?.items ?? []).find((r: any) => r.id === id && r.k === 'node')
}
