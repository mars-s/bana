import '@fontsource-variable/inter/opsz.css'
import '@fontsource-variable/inter/opsz-italic.css'
import './tokens.css'
import './app.css'
import { createRoot } from 'react-dom/client'
import { App } from './App'
import { start } from './store'
import { keepBufferedKeys, restoreKeptKeys } from './controller'

const root = document.getElementById('root')
if (root) {
  createRoot(root).render(<App />)
  window.addEventListener('pagehide', keepBufferedKeys)
  void start().then(restoreKeptKeys)
}
