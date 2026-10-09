import '@fontsource/inter/400.css'
import '@fontsource/inter/400-italic.css'
import '@fontsource/inter/500.css'
import '@fontsource/inter/600.css'
import '@fontsource/inter/700.css'
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
