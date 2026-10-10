import { defineConfig } from 'vitest/config'
import { cloudflareTest } from '@cloudflare/vitest-pool-workers'

// The relay's tests run inside workerd (Miniflare) with the real Durable Object.
export default defineConfig({
  // The relying party is the test host (the config names the deployed one).
  plugins: [cloudflareTest({ wrangler: { configPath: './wrangler.jsonc' }, miniflare: { bindings: { RP_ID: 'relay.test' } } })],
})
