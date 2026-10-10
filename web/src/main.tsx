import '@fontsource-variable/inter/opsz.css'
import '@fontsource-variable/inter/opsz-italic.css'
import './tokens.css'
import './app.css'
import { createRoot } from 'react-dom/client'
import { App } from './App'
import { start } from './store'
import { replayWal } from './engine'
import { exec, restoreKeptKeys, runUiCommand } from './controller'

const root = document.getElementById('root')
if (root) {
  createRoot(root).render(<App />)
  // Edits the last page logged but the engine never answered go in first; then the keys it kept
  // are typed again (they go on from that text).
  void replayWal()
    .then(start)
    .then(restoreKeptKeys)
}

// Inside the desktop app, the native menu bar sends command ids (spec 00 §7).
if (window.__TAURI_INTERNALS__) {
  void import('@tauri-apps/api/event').then(({ listen }) =>
    listen<string>('menu', (e) => {
      if (e.payload.startsWith('ui.')) runUiCommand(e.payload)
      else void exec(e.payload)
    }),
  )
}
