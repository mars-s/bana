import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// The dev server proxies the JSON API to `bana serve` (default port 8787).
const api = process.env.BANA_API ?? 'http://127.0.0.1:8787'

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 5173, strictPort: true, proxy: { '/rpc': api, '/events': api } },
  build: { target: 'es2022', outDir: 'dist', emptyOutDir: true },
})
