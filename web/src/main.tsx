import '@fontsource-variable/inter/opsz.css'
import '@fontsource-variable/inter/opsz-italic.css'
import './tokens.css'
import './app.css'
import { createRoot } from 'react-dom/client'
import { App, useTheme } from './App'
import { QuickAdd } from './Overlays'
import { start } from './store'
import { call, replayWal } from './engine'
import { exec, restoreKeptKeys, runUiCommand } from './controller'

const root = document.getElementById('root')
// The capture window (UC-08.14): the desktop app opens this page as `#capture` in a small window of
// its own when Cmd+Shift+E is pressed in any app. Only Quick add; Escape closes the window.
if (root && location.hash === '#capture') {
  document.documentElement.classList.add('capture')
  const done = () => {
    if (window.__TAURI_INTERNALS__) void import('@tauri-apps/api/core').then(({ invoke }) => invoke('capture_done'))
    else window.close()
  }
  const Capture = () => (useTheme(), <QuickAdd onDone={done} />)
  createRoot(root).render(<Capture />)
} else if (root) {
  createRoot(root).render(<App />)
  // Edits the last page logged but the engine never answered go in first; then the keys it kept
  // are typed again (they go on from that text).
  void replayWal()
    // A field name this window was typing after `>` becomes the field (N26; once: its line goes).
    .then(() => call('fieldDraft', { commit: true }))
    .then(start)
    .then(restoreKeptKeys)
}

// Inside the desktop app, the native menu bar sends command ids (spec 00 §7).
if (window.__TAURI_INTERNALS__ && location.hash !== '#capture') {
  void import('@tauri-apps/api/event').then(({ listen }) =>
    listen<string>('menu', (e) => {
      if (e.payload.startsWith('ui.')) runUiCommand(e.payload)
      else void exec(e.payload)
    }),
  )
}
