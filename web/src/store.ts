// App state: what the engine last showed (pages, sidebar) plus view-only state (tabs, panels,
// menus, theme). Nothing here is note data: notes live in the engine and come back as snapshots.

import { useSyncExternalStore } from 'react'
import { type CommandInfo, type Json, type Page, type Sidebar, call, flushOnUnload, idle, transport } from './engine'

export interface Tab {
  id: string
  panels: string[]
  active: number
}

export interface Overlay {
  kind: 'palette' | 'search' | 'picker' | 'quickadd' | 'confirm'
  [k: string]: Json
}

export interface State {
  tabs: Tab[]
  tab: number
  pages: Record<string, Page>
  /** The window of rows each panel shows (start, count). */
  windows: Record<string, { start: number; count: number }>
  sidebar: Sidebar | null
  sidebarMode: 0 | 1 | 2
  theme: 'dark' | 'light'
  shortcuts: boolean
  overlay: Overlay | null
  commands: CommandInfo[]
  toast: string | null
  /** Bumped when the caret should be put where the engine says (after a command, not typing). */
  caretEpoch: number
  /** Bumped to scroll the focused line back into view (a key came while it was scrolled away). */
  focusScroll: number
  /** The panel that has the keyboard. */
  activePanel: string
}

const saved = (() => {
  try {
    return JSON.parse(localStorage.getItem('tanacraft.view') ?? '{}')
  } catch {
    return {}
  }
})()

let state: State = {
  tabs: [{ id: 't1', panels: ['main'], active: 0 }],
  tab: 0,
  pages: {},
  windows: {},
  sidebar: null,
  sidebarMode: saved.sidebarMode ?? 0,
  theme: new URLSearchParams(location.search).get('theme') === 'light' ? 'light' : (saved.theme ?? 'dark'),
  shortcuts: false,
  overlay: null,
  commands: [],
  toast: null,
  caretEpoch: 0,
  focusScroll: 0,
  activePanel: 'main',
}

const subs = new Set<() => void>()

export function getState() {
  return state
}

export function setState(patch: Partial<State> | ((s: State) => Partial<State>)) {
  const p = typeof patch === 'function' ? patch(state) : patch
  state = { ...state, ...p }
  if ('theme' in p || 'sidebarMode' in p) {
    try {
      localStorage.setItem('tanacraft.view', JSON.stringify({ theme: state.theme, sidebarMode: state.sidebarMode }))
    } catch {
      /* private mode: view prefs are not kept */
    }
  }
  subs.forEach((f) => f())
}

export function useStore<T>(pick: (s: State) => T): T {
  return useSyncExternalStore(
    (f) => {
      subs.add(f)
      return () => subs.delete(f)
    },
    () => pick(state),
  )
}

export function visiblePanels(s: State = state): string[] {
  return s.tabs[s.tab]?.panels ?? ['main']
}

// ---- talking to the engine -------------------------------------------------------------------------

/** Commands that only type text: they never move the caret, so the editor keeps its own. */
const TYPING = new Set(['node.setName', 'field.set', 'node.setDescription'])

let refreshQueued = false
let caretWanted = false
let seenVersion = 0

/** Runs a command in `panel` (default: the active one), then refreshes what is shown. */
export async function run(command: string, params: Json = {}, panel = state.activePanel): Promise<Json> {
  if (!TYPING.has(command)) caretWanted = true
  const r = await call('run', { command, params, panel })
  if (r?.error) showToast(r.error)
  scheduleRefresh()
  return r
}

export function scheduleRefresh() {
  if (refreshQueued) return
  refreshQueued = true
  void idle().then(refresh)
}

/** Reads the visible panels and the sidebar. Queued behind every pending edit. */
export async function refresh() {
  refreshQueued = false
  const panels = visiblePanels()
  const wantCaret = caretWanted
  caretWanted = false
  const pages: Record<string, Page> = {}
  await Promise.all(
    panels.map(async (p) => {
      const w = state.windows[p] ?? { start: 0, count: 300 }
      pages[p] = await call('page', { panel: p, start: w.start, count: w.count })
    }),
  )
  const sidebar = await call('sidebar', {})
  for (const p of Object.values(pages)) seenVersion = Math.max(seenVersion, p.version ?? 0)
  setState((s) => ({ pages: { ...s.pages, ...pages }, sidebar, caretEpoch: wantCaret ? s.caretEpoch + 1 : s.caretEpoch }))
}

let toastTimer = 0
export function showToast(msg: string) {
  setState({ toast: msg })
  clearTimeout(toastTimer)
  toastTimer = window.setTimeout(() => setState({ toast: null }), 3500)
}

export async function start() {
  const commands = await call('commands', {})
  setState({ commands: Array.isArray(commands) ? commands : [] })
  await refresh()
  // Changes made elsewhere (CLI, MCP, another window) show up here.
  transport.watch((v) => {
    if (v > seenVersion) scheduleRefresh()
  })
  // Leaving the page: the engine saves now (it also autosaves within a second).
  let flushed = false
  const flush = () => {
    if (flushed) return
    flushed = true
    flushOnUnload()
  }
  window.addEventListener('beforeunload', flush)
  window.addEventListener('pagehide', flush)
}

// ---- tabs and panels ---------------------------------------------------------------------------

let panelSeq = 0

export async function openInNewPanel(node: number) {
  const name = `p${Date.now().toString(36)}${panelSeq++}`
  await call('panel.open', { name, node })
  setState((s) => {
    const tabs = s.tabs.map((t, i) => (i === s.tab ? { ...t, panels: [...t.panels, name], active: t.panels.length } : t))
    return { tabs, activePanel: name }
  })
  await refresh()
}

export async function closePanel(name: string) {
  const s = state
  const tab = s.tabs[s.tab]
  if (!tab || tab.panels.length < 2) return
  const panels = tab.panels.filter((p) => p !== name)
  const active = Math.min(tab.active, panels.length - 1)
  setState({ tabs: s.tabs.map((t, i) => (i === s.tab ? { ...t, panels, active } : t)), activePanel: panels[active] })
  await call('panel.close', { name })
  await refresh()
}

export async function newTab(node?: number) {
  const name = `p${Date.now().toString(36)}${panelSeq++}`
  await call('panel.open', { name, node: node ?? state.pages[state.activePanel]?.zoom })
  setState((s) => ({ tabs: [...s.tabs, { id: name, panels: [name], active: 0 }], tab: s.tabs.length, activePanel: name }))
  await refresh()
}

export async function switchTab(i: number) {
  const s = state
  const t = s.tabs[i]
  if (!t) return
  setState({ tab: i, activePanel: t.panels[t.active] ?? t.panels[0] })
  await refresh()
}

export async function closeTab(i = state.tab) {
  const s = state
  if (s.tabs.length < 2) return
  const gone = s.tabs[i]
  const tabs = s.tabs.filter((_, j) => j !== i)
  const tab = Math.min(s.tab, tabs.length - 1)
  setState({ tabs, tab, activePanel: tabs[tab].panels[tabs[tab].active] })
  for (const p of gone?.panels ?? []) if (p !== 'main') await call('panel.close', { name: p })
  await refresh()
}

export function setActivePanel(p: string) {
  if (state.activePanel === p) return
  setState((s) => ({
    activePanel: p,
    tabs: s.tabs.map((t, i) => (i === s.tab ? { ...t, active: Math.max(0, t.panels.indexOf(p)) } : t)),
  }))
}
