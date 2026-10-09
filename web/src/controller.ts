// The keyboard: every key press goes through `handleKey`, which runs engine commands the way the
// spec says (02 and 09, DIFFS). Ported rule by rule from legacy/egui/ui-egui/src/keys.rs.
//
// Data safety (DIFFS N15, N16, N26, N27, N32): typed text goes to the engine at once; while a
// command that moves the caret is on its way, keys wait in a buffer and are replayed into
// whichever line holds the caret afterwards, so no key is lost or lands in the wrong line.

import { type Json, type NodeRow, type Page, call } from './engine'
import { type KeyInfo, UI_COMMANDS, isTextKey, matches, shortcut } from './keys'
import { closeTab, getState, newTab, openInNewPanel, refresh, setState, showToast, switchTab, visiblePanels } from './store'
import { charLen } from './format'

/** What the mounted line editor exposes. */
export interface EditorHandle {
  panel: string
  node: number
  /** Caret and anchor in chars. */
  sel(): [number, number]
  value(): string
  insert(text: string): void
  setSel(anchor: number, caret: number): void
  /** Plain editing keys replayed from the buffer (Backspace, arrows). */
  native(k: KeyInfo): void
}

export interface TitleHandle {
  panel: string
  focus(atEnd?: boolean): void
  insert(text: string): void
}

let editor: EditorHandle | null = null
const titles = new Map<string, TitleHandle>()

export function registerEditor(h: EditorHandle | null) {
  editor = h
  if (h) drainSoon()
}

export function currentEditor() {
  return editor
}

export function registerTitle(panel: string, h: TitleHandle | null) {
  if (h) titles.set(panel, h)
  else titles.delete(panel)
}

// ---- running commands with a barrier ---------------------------------------------------------------

let barrier = 0
const buffer: KeyInfo[] = []
/** The caret last told to the engine, per node. */
let synced: { node: number; caret: number } | null = null

export function noteSynced(node: number, caret: number) {
  synced = { node, caret }
}

/** Runs a command that may move the caret; keys typed meanwhile wait and are replayed after. */
export async function exec(command: string, params: Json = {}, panel = getState().activePanel): Promise<Json> {
  barrier++
  try {
    // The engine learns where the caret is first (arrow keys and clicks move it without a command).
    if (editor && editor.panel === panel) {
      const [, caret] = editor.sel()
      if (!synced || synced.node !== editor.node || synced.caret !== caret) {
        await call('run', { command: 'focus.set', params: { node: editor.node, caret }, panel })
        synced = { node: editor.node, caret }
      }
    }
    const r = await call('run', { command, params, panel })
    if (r?.error) showToast(r.error)
    synced = null
    await refresh()
    return r
  } finally {
    barrier--
    drainSoon()
  }
}

export function inBarrier() {
  return barrier > 0
}

function drainSoon() {
  setTimeout(drain, 0)
}

let draining = false
function drain() {
  if (draining) return
  draining = true
  try {
    while (buffer.length && barrier === 0) {
      const k = buffer.shift()!
      if (!handleKey(k)) {
        const title = [...titles.values()].find((t) => document.activeElement?.closest(`[data-title-panel="${t.panel}"]`))
        if (editor && isTextKey(k)) editor.insert(k.key)
        else if (editor) editor.native(k)
        else if (title && isTextKey(k)) title.insert(k.key)
      }
    }
  } finally {
    draining = false
  }
}

/** Keeps a key for later (a command is still moving the caret). */
export function bufferKey(k: KeyInfo) {
  buffer.push(k)
}

const KEPT = 'tanacraft.keys'

/** The window is going away with keys still waiting: keep them for the next load. */
export function keepBufferedKeys() {
  if (!buffer.length) return
  try {
    sessionStorage.setItem(KEPT, JSON.stringify(buffer))
  } catch {
    /* storage off: nothing more can be done in an unloading page */
  }
}

/** Keys kept by an unloading page are typed again (the engine still has the caret there). */
export function restoreKeptKeys() {
  try {
    const kept = JSON.parse(sessionStorage.getItem(KEPT) ?? '[]') as KeyInfo[]
    sessionStorage.removeItem(KEPT)
    buffer.unshift(...kept)
  } catch {
    /* nothing kept */
  }
  drainSoon()
}

/** Puts a key back in front: it came before everything still waiting. */
function requeue(k: KeyInfo) {
  buffer.unshift(k)
}

/** Holds keys back while `fn` runs (several commands, then a focus change). */
export async function withBarrier<T>(fn: () => Promise<T>): Promise<T> {
  barrier++
  try {
    return await fn()
  } finally {
    barrier--
    drainSoon()
  }
}

const nextFrame = () => new Promise((r) => requestAnimationFrame(() => r(null)))

// ---- helpers -----------------------------------------------------------------------------------

function page(): Page | undefined {
  const s = getState()
  return s.pages[s.activePanel]
}

function nodeRows(p: Page): NodeRow[] {
  return p.rows.items.filter((r): r is NodeRow => r.k === 'node')
}

function neighbour(p: Page, id: number, step: number): NodeRow | undefined {
  const rows = nodeRows(p)
  const i = rows.findIndex((r) => r.id === id)
  return i < 0 ? undefined : rows[i + step]
}

/** The line typing goes to when nothing holds a caret (DIFFS N27, N34). */
const ghosts = new Map<string, { node: number; atStart: boolean }>()

export function setGhost(panel: string, node: number, atStart: boolean) {
  ghosts.set(panel, { node, atStart })
}

function runUi(id: string): boolean {
  const s = getState()
  const p = page()
  switch (id) {
    case 'ui.toggleSidebar':
      setState({ sidebarMode: ((s.sidebarMode + 1) % 3) as 0 | 1 | 2 })
      return true
    case 'ui.newTab':
      void newTab()
      return true
    case 'ui.closeTab':
      void closeTab()
      return true
    case 'ui.nextTab':
      void switchTab((s.tab + 1) % s.tabs.length)
      return true
    case 'ui.prevTab':
      void switchTab((s.tab + s.tabs.length - 1) % s.tabs.length)
      return true
    case 'ui.openInPanel': {
      const node = p?.focus ?? p?.sel[0] ?? p?.zoom
      if (node != null) void openInNewPanel(node)
      return true
    }
    case 'ui.shortcuts':
      setState({ shortcuts: !s.shortcuts })
      return true
    case 'ui.toggleTheme':
      setState({ theme: s.theme === 'dark' ? 'light' : 'dark' })
      return true
    case 'ui.commandLine':
      setState({ overlay: { kind: 'palette', query: '' } })
      return true
    case 'ui.searchOpen':
      setState({ overlay: { kind: 'search', query: '' } })
      return true
    case 'ui.quickAdd':
      setState({ overlay: { kind: 'quickadd', lines: [''] } })
      return true
  }
  return false
}

export function runUiCommand(id: string) {
  return runUi(id)
}

const TEXT_TRIGGERS = new Set(['#', '>', '@'])
/** Commands that need a line with a caret or a selection. */
const NEEDS_LINE = new Set(['node.indent', 'node.outdent', 'node.enter', 'node.backspace', 'node.insert'])

function registryCommand(sc: string): string | undefined {
  return getState().commands.find((c) => c.shortcut && !TEXT_TRIGGERS.has(c.shortcut) && matches(c.shortcut, sc))?.id
}

// ---- the key handler -------------------------------------------------------------------------------

/** Handles a key press; true when it was used (the caller then stops the browser's own action). */
export function handleKey(k: KeyInfo): boolean {
  const s = getState()
  if (s.overlay) return false // overlays read their own keys
  const sc = shortcut(k)
  // Mod+1..8 go to tab N, Mod+9 to the last tab (spec 09).
  const d = /^Mod\+(\d)$/.exec(sc)
  if (d && Number(d[1]) >= 1) {
    const n = Number(d[1])
    void switchTab(n === 9 ? s.tabs.length - 1 : n - 1)
    return true
  }
  for (const [id, , keys] of UI_COMMANDS) {
    if (keys && matches(keys, sc)) return runUi(id)
  }
  const p = page()
  if (!p) return false
  if (editor && editor.panel === s.activePanel) return editorKey(k, sc, p, editor)
  if (p.sel.length) return selectionKey(k, sc, p)
  // Nothing holds a caret: typed text goes into the last line edited, or the first line (N27).
  if (isTextKey(k) && k.key !== ' ' && !document.activeElement?.closest('input, textarea, [contenteditable]')) {
    const rows = nodeRows(p)
    const g = ghosts.get(s.activePanel)
    const target = (g && rows.find((r) => r.id === g.node)) || rows[0]
    if (!target) return false
    const atStart = g?.node === target.id && g.atStart
    requeue(k)
    void exec('focus.set', { node: target.id, caret: atStart ? 0 : charLen(target.name) })
    return true
  }
  const cmd = registryCommand(sc)
  if (cmd && !NEEDS_LINE.has(cmd) && !cmd.startsWith('format.')) {
    void exec(cmd)
    return true
  }
  return false
}

function selectionKey(k: KeyInfo, sc: string, p: Page): boolean {
  const panel = getState().activePanel
  const head = p.sel[p.sel.length - 1]
  const first = p.sel[0]
  switch (sc) {
    case 'Escape':
      // The selection goes, but typing still lands in its last line, at its start (N27, N34).
      setGhost(panel, head, true)
      void exec('select.clear')
      return true
    case 'Enter': {
      void exec('node.insert', { node: head })
      return true
    }
    case 'Backspace':
    case 'Delete': {
      const rows = nodeRows(p)
      const at = rows.findIndex((r) => r.id === first)
      void exec('node.delete').then(async (r) => {
        if (r?.result?.confirm) return
        const after = getState().pages[panel]
        const rest = after ? nodeRows(after) : []
        const pick = rest[at] ?? rest[rest.length - 1]
        if (pick) await exec('select.set', { from: pick.id })
      })
      return true
    }
    case 'Up':
    case 'Down': {
      const up = sc === 'Up'
      const rows = nodeRows(p)
      const idx = p.sel.map((id) => rows.findIndex((r) => r.id === id)).filter((i) => i >= 0)
      const at = up ? Math.min(...idx) - 1 : Math.max(...idx) + 1
      const r = rows[at] ?? rows[up ? Math.min(...idx) : Math.max(...idx)]
      if (r) void exec('focus.set', { node: r.id, caret: up ? 1e9 : 0 })
      return true
    }
  }
  if (isTextKey(k)) {
    // Typing on a selected line goes into it (N27): at its end.
    const row = nodeRows(p).find((r) => r.id === head)
    requeue(k)
    void exec('focus.set', { node: head, caret: row ? charLen(row.name) : 1e9 })
    return true
  }
  const cmd = registryCommand(sc)
  if (cmd && !cmd.startsWith('format.')) {
    void exec(cmd)
    return true
  }
  return false
}

function editorKey(k: KeyInfo, sc: string, p: Page, ed: EditorHandle): boolean {
  const [anchor, caret] = ed.sel()
  const empty = anchor === caret
  const len = charLen(ed.value())
  const panel = ed.panel
  switch (sc) {
    case 'Escape':
      // Leaves text editing and selects the line; Escape again clears.
      void exec('select.set', { from: ed.node })
      return true
    case 'Mod+.':
      void withBarrier(async () => {
        await exec('nav.zoomIn', { node: ed.node })
        const after = getState().pages[panel]
        if (after?.titleEditable) {
          await nextFrame()
          titles.get(panel)?.focus(true)
        } else {
          const first = after && nodeRows(after)[0]
          if (first) await exec('focus.set', { node: first.id, caret: 0 })
        }
      })
      return true
    case 'Delete':
      if (caret >= len && empty) {
        void exec('node.joinNext', { node: ed.node })
        return true
      }
      return false
    case 'Left':
    case 'Right': {
      if (!empty) return false
      const left = sc === 'Left'
      if ((left && caret > 0) || (!left && caret < len)) return false
      const n = neighbour(p, ed.node, left ? -1 : 1)
      if (!n) return false
      void exec('focus.set', { node: n.id, caret: left ? 1e9 : 0 })
      return true
    }
    case 'Home':
    case 'End':
    case 'Shift+Home':
    case 'Shift+End': {
      const to = sc.endsWith('Home') ? 0 : len
      ed.setSel(sc.startsWith('Shift+') ? anchor : to, to)
      return true
    }
    case 'Mod+A': {
      // The text first, then (all text already selected) every line.
      const all = Math.min(anchor, caret) === 0 && Math.max(anchor, caret) === len
      if (!all) return false
      void exec('select.all')
      return true
    }
    case 'Enter':
      void exec('node.enter', { node: ed.node, caret })
      return true
    case 'Backspace':
      if (caret === 0 && empty) {
        void exec('node.backspace', { node: ed.node })
        return true
      }
      return false
    case 'Up': {
      const first = nodeRows(p)[0]
      if (first?.id === ed.node && p.rows.start === 0 && p.titleEditable) {
        void withBarrier(async () => {
          await exec('focus.clear')
          await nextFrame()
          titles.get(panel)?.focus(true)
        })
        return true
      }
      void exec('focus.up')
      return true
    }
    case 'Down':
      void exec('focus.down')
      return true
  }
  if (isTextKey(k)) return false
  const cmd = registryCommand(sc)
  if (!cmd) return false
  if (cmd.startsWith('format.')) {
    void exec(cmd, { node: ed.node, start: Math.min(anchor, caret), end: Math.max(anchor, caret) })
    return true
  }
  void exec(cmd, NEEDS_LINE.has(cmd) || cmd.startsWith('node.') ? { node: ed.node } : {})
  return true
}

/** A click on a line's text: the caret goes there at once, then the engine follows. */
export function focusLine(panel: string, node: number, caret: number) {
  void exec('focus.set', { node, caret }, panel)
}

export function panelsShown() {
  return visiblePanels()
}
