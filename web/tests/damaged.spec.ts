// A damaged graph file (owners in a loop, children and links that are not there, views pointing
// at missing fields) opens and draws in every view without an error in the page (ported from
// legacy/egui ui_fuzz.rs damaged_files_draw_in_every_view; the engine side is
// crates/commands/tests/fuzz.rs).
import { writeFileSync } from 'node:fs'
import { test, expect } from './fixtures'

function damagedGraph() {
  const node = (id: number, name: string, parent: number | null, extra: Record<string, unknown> = {}) => ({ id, name, parent, created: 1790000000, ...extra })
  return {
    format: 'tanacraft-graph',
    version: 1,
    next_id: 100,
    system: { root: 1, library: 2, journal: 3, schema: 4, trash: 5, day_tag: 6 },
    nodes: [
      node(1, 'Workspace', null, { children: [2, 3, 4, 5] }),
      node(2, 'Library', 1, { children: [20, 22, 24, 999] }),
      node(3, 'Daily notes', 1),
      node(4, 'Schema', 1, { children: [6, 10, 11] }),
      node(5, 'Trash', 1),
      node(6, 'day', 4, { kind: { type: 'tag', color: 'grey' } }),
      node(10, 'task', 4, { kind: { type: 'tag', color: 'pink', fields: [11, 998] } }),
      node(11, 'Due', 4, { kind: { type: 'field', field_type: 'date' } }),
      // Views pointing at fields that are gone, sorted by a missing field.
      node(20, 'Broken views', 2, {
        children: [21, 997],
        view: { kind: 'calendar', columns: [996], date_field: 21, sort: [{ by: { by: 'field', field: 995 } }] },
      }),
      // Tags that are not tags, field entries pointing at nothing.
      node(21, 'Odd item', 20, { tags: [20, 994, 10], fields: [{ field: 20, value: { type: 'node', value: 993 } }, { field: 992, value: null }] }),
      // A reference to nothing.
      node(22, 'Dangling', 2, { kind: { type: 'reference', target: 991 } }),
      // Owners in a loop.
      node(24, 'Loop A', 25, { children: [25] }),
      node(25, 'Loop B', 24, { children: [24] }),
    ],
  }
}

test('a damaged file draws in every view (outline, table, cards, list, side menu, tabs, calendar)', async ({ app, server }) => {
  await server.kill('SIGKILL')
  writeFileSync(server.file, JSON.stringify(damagedGraph()))
  await server.restart()
  await app.reload()
  await expect(app.locator('.panel .title, .panel .title-input').first()).toBeVisible()
  const lib = (await server.rpc('sidebar')).workspaces[0].library
  for (const node of [lib, 20, 21, 22, 24]) {
    const n = await server.run('graph.inspect', { node })
    if (n.error) continue
    await server.run('nav.open', { node })
    for (const kind of ['outline', 'table', 'cards', 'list', 'side_menu', 'tabs', 'calendar']) {
      await server.run('view.set', { node, kind })
      await app.waitForTimeout(80)
      await expect(app.locator('.panel').first()).toBeVisible()
    }
    await server.run('view.set', { node, kind: 'outline' })
  }
  // Typing still works on the repaired graph.
  await server.run('nav.open', { node: lib })
  const r = await server.run('node.add', { parent: lib, name: 'still here' })
  expect(r.result?.id).toBeGreaterThan(0)
})
