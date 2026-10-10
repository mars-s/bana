import '@fontsource-variable/inter/opsz.css'
import '@fontsource-variable/inter/opsz-italic.css'
import './tokens.css'
import './app.css'
import { createRoot } from 'react-dom/client'
import { App } from './App'
import { start } from './store'
import { waitForUnloadBatch } from './engine'
import { exec, keepBufferedKeys, restoreKeptKeys, runUiCommand } from './controller'

const root = document.getElementById('root')
if (root) {
  createRoot(root).render(<App />)
  window.addEventListener('pagehide', keepBufferedKeys)
  // Kept keys are typed again only once the last page's unload batch is in (it carries the text
  // they go on from).
  void waitForUnloadBatch()
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
