// Prints the boxes of key elements (1x points) to compare with ../tanacraft-spec/12-screens-visual.md.
// Run: pnpm exec playwright test --project=shots measure
import { test } from './fixtures'

test('measure the day page', async ({ app }) => {
  const sel: Record<string, string> = {
    sidebarBtn: '.topbar-left .icon-btn',
    tab: '.tab.active',
    navToday: '.sidebar .side-row',
    create: '.side-actions .btn.create',
    pinnedHead: '.side-head',
    pinnedRow: '.side-row.pin',
    panel: '.panel',
    crumbBack: '.crumb-back',
    wsAvatar: '.ws-avatar',
    crumb: '.crumb',
    title: '.panel .title',
    chip: '.tag-row .chip',
    headBtn: '.head-actions .btn',
    dayNav: '.day-nav',
    firstRow: '.row.node',
    firstBullet: '.row.node .bullet',
    firstText: '.row.node .text',
    sideCard: '.side-card',
    field: '.row.field .row-inner',
  }
  const out: Record<string, string> = {}
  for (const [k, s] of Object.entries(sel)) {
    const b = await app.locator(s).first().boundingBox()
    out[k] = b ? `x ${b.x.toFixed(1)} y ${b.y.toFixed(1)} w ${b.width.toFixed(1)} h ${b.height.toFixed(1)}` : 'none'
  }
  console.log(JSON.stringify(out, null, 1))
})
