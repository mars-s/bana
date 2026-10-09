// Key presses as portable shortcut strings (`Mod+Shift+Up`), matched against the engine's
// registry shortcuts. `Mod` is Cmd on macOS and Ctrl elsewhere; `Ctrl` is the Control key on macOS.

export const MAC = /Mac|iPhone|iPad/.test(navigator.platform) || /Mac OS X/.test(navigator.userAgent)

export interface KeyInfo {
  key: string
  code: string
  metaKey: boolean
  ctrlKey: boolean
  altKey: boolean
  shiftKey: boolean
}

export function keyInfo(e: KeyboardEvent | React.KeyboardEvent): KeyInfo {
  return { key: e.key, code: e.code, metaKey: e.metaKey, ctrlKey: e.ctrlKey, altKey: e.altKey, shiftKey: e.shiftKey }
}

const NAMES: Record<string, string> = {
  ArrowUp: 'Up',
  ArrowDown: 'Down',
  ArrowLeft: 'Left',
  ArrowRight: 'Right',
  ' ': 'Space',
  Esc: 'Escape',
}

function keyName(k: KeyInfo): string {
  if (/^Key[A-Z]$/.test(k.code)) return k.code.slice(3)
  if (/^Digit\d$/.test(k.code)) return k.code.slice(5)
  if (NAMES[k.key]) return NAMES[k.key]
  if (k.code === 'Period') return '.'
  if (k.code === 'Comma') return ','
  if (k.code === 'BracketLeft') return '['
  if (k.code === 'BracketRight') return ']'
  if (k.code === 'Backslash') return '\\'
  if (k.code === 'Slash') return '/'
  if (k.code === 'Equal') return '='
  if (k.code === 'Minus') return '-'
  return k.key.length === 1 ? k.key.toUpperCase() : k.key
}

export function shortcut(k: KeyInfo): string {
  let s = ''
  if (MAC && k.ctrlKey) s += 'Ctrl+'
  if (MAC ? k.metaKey : k.ctrlKey) s += 'Mod+'
  if (k.altKey) s += 'Alt+'
  if (k.shiftKey) s += 'Shift+'
  return s + keyName(k)
}

/** Whether a registry shortcut (`A / B`) matches the pressed one. Off macOS, `Ctrl` means `Mod`. */
export function matches(spec: string, pressed: string): boolean {
  return spec
    .split(' / ')
    .filter(Boolean)
    .some((k) => {
      const kk = MAC ? k : k.replace('Ctrl+Mod+', 'Mod+').replace('Ctrl+', 'Mod+')
      return kk.toLowerCase() === pressed.toLowerCase()
    })
}

/** A key that types text (no Mod/Ctrl/Alt chord). */
export function isTextKey(k: KeyInfo): boolean {
  return k.key.length >= 1 && [...k.key].length === 1 && !k.metaKey && !k.ctrlKey && !(k.altKey && !MAC)
}

/** UI commands (no graph change), listed with the engine's in the command line and help. */
export const UI_COMMANDS: [string, string, string][] = [
  ['ui.commandLine', 'Command line', 'Mod+K'],
  ['ui.toggleSidebar', 'Toggle sidebar (full, mini, hidden)', 'Mod+\\'],
  ['ui.newTab', 'New tab', 'Mod+T'],
  ['ui.closeTab', 'Close tab', 'Mod+W'],
  ['ui.nextTab', 'Next tab', 'Ctrl+Tab'],
  ['ui.prevTab', 'Previous tab', 'Ctrl+Shift+Tab'],
  ['ui.openInPanel', 'Open in new panel', 'Mod+M'],
  ['ui.shortcuts', 'Keyboard shortcuts', 'Mod+/ / Mod+Shift+/ / Mod+Shift+?'],
  ['ui.toggleTheme', 'Switch light or dark theme', ''],
  ['ui.searchOpen', 'Search and open', 'Mod+S'],
  ['ui.quickAdd', 'Quick add', 'Mod+E'],
]
