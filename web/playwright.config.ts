import { defineConfig } from '@playwright/test'

// Each test gets its own `bana serve` (see tests/fixtures.ts), so tests never share data.
// The window matches the spec's screenshots: 1512x949.
export default defineConfig({
  testDir: 'tests',
  timeout: 60_000,
  workers: 3,
  reporter: [['list']],
  use: { viewport: { width: 1512, height: 949 }, deviceScaleFactor: 1, colorScheme: 'dark' },
  projects: [
    { name: 'tests', testIgnore: /[/\\](shots|measure2?|scratch|sync|bana-id)\.spec\.ts$/ },
    // Two engines syncing through the relay on local workerd (`wrangler dev`): `cargo xtask sync`.
    { name: 'sync', testMatch: /[/\\](sync|bana-id)\.spec\.ts$/ },
    { name: 'shots', testMatch: /shots\.spec\.ts/, use: { deviceScaleFactor: 2 } },
    { name: 'measure', testMatch: /[/\\](measure2?|scratch)\.spec\.ts$/ },
  ],
})
