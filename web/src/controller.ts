// The keyboard: every key press goes through `handleKey`, which runs engine commands the way the
// spec says (02 and 09, DIFFS). Ported rule by rule from legacy/egui/ui-egui/src/keys.rs.
//
// Data safety (DIFFS N15, N16, N26, N27, N32): typed text goes to the engine at once; while a
// command that moves the caret is on its way, keys wait in a buffer and are replayed into
// whichever line holds the caret afterwards, so no key is lost or lands in the wrong line.

import { type Json, type NodeRow, type Page, call, onResume } from './engine'
import { type KeyInfo, UI_COMMANDS, isTextKey, matches, shortcut } from './keys'
import { closePanel, closeTab, getState, moveTab, newTab, openInNewPanel, refresh, setState, showToast, switchTab, visiblePanels } from './store'
import { charLen } from './format'
import { fieldKey, fieldRowKey, openPicker, pickerKey } from './picker'
import { toggleQuery } from './Views'

/** What the mounted line editor exposes. */
export interface EditorHandle {
  /** A line's text, or a field value (`field`: the node is the field's owner). */
  kind: 'line' | 'field'
  panel: string
  node: number
  /** Caret and anchor in chars. */
  sel(): [number, number]
  value(): string
  insert(text: string): void
  setSel(anchor: number, caret: number): void
  /** Plain editing keys replayed from the buffer (Backspace, arrows). */
  native(k: KeyInfo): void
  /** Takes the keyboard back (after a menu or dialog closed). */
  focus(): void
  /** Takes the engine's text and caret after a command (unless own edits are on their way). */
  adopt?(text: string, anchor: number, caret: number): void
}

export interface TitleHandle {
  panel: string
  focus(atEnd?: boolean): void
  insert(text: string): void
  /** A key that waited in the buffer while the title had the keyboard. */
  key(k: KeyInfo): void
}

let editor: EditorHandle | null = null
const titles = new Map<string, TitleHandle>()

export function registerEditor(h: EditorHandle) {
  editor = h
  // The line that had the caret last is where typing with nothing focused puts it back (N61).
  if (h.kind === 'line') lastEdited = h.node
  swallowing = false
  // A caret somewhere else means the field row is no longer selected.
  if (getState().fieldSel) setState({ fieldSel: null })
  if (waitingEditor) {
    waitingEditor = false
    clearTimeout(waitTimer)
    barrier--
  }
  drainSoon()
}

let waitingEditor = false
let waitTimer = 0

/** Keys wait until the next editor is drawn (a new value line, a new field editor). */
export function holdKeysForEditor() {
  if (waitingEditor) return
  waitingEditor = true
  barrier++
  waitTimer = window.setTimeout(() => {
    if (!waitingEditor) return
    waitingEditor = false
    barrier--
    drainSoon()
  }, 1000)
}

/** An editor going away lets go of the keyboard only if it still holds it. */
export function unregisterEditor(h: EditorHandle) {
  if (editor === h) editor = null
}

export function currentEditor() {
  return editor
}

/** The title a command key was pressed in: closing a menu without running anything gives it the
 * keyboard back (N40). */
let titleReturn: string | null = null

export function keyFromTitle(panel: string, k: KeyInfo) {
  titleReturn = panel
  if (!handleKey(k)) titleReturn = null
}

/** After an overlay closed: the title that opened it takes the keyboard back. */
export function returnKeyboard() {
  const p = titleReturn
  titleReturn = null
  if (p != null && !editor) titles.get(p)?.focus(true)
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

/** The line this window typed in last (N61). */
let lastEdited: number | null = null

export function noteSynced(node: number, caret: number) {
  synced = { node, caret }
  lastEdited = node
}

// The engine is back after an outage (N73). If it restarted, it forgot each panel's page and the
// caret: they are put back before the edits queued meanwhile go in (they carry their own lines).
onResume(async (direct) => {
  for (const panel of visiblePanels()) {
    const before = getState().pages[panel]
    if (!before) continue
    const now = await direct('page', { panel, start: 0, count: 1 })
    const moved = now?.zoom !== before.zoom
    if (moved) await direct('run', { command: 'nav.open', params: { node: before.zoom }, panel })
    const ed = editor && editor.kind === 'line' && editor.panel === panel ? editor : null
    if (ed && (moved || now?.focus !== ed.node)) {
      const r = await direct('run', { command: 'focus.set', params: { node: ed.node, caret: ed.sel()[1] }, panel })
      // The line is gone: no caret, so nothing queued can land in another line (N77).
      if (r?.error) await direct('run', { command: 'focus.clear', params: {}, panel })
    }
  }
  synced = null
})

/** Runs a command that may move the caret; keys typed meanwhile wait and are replayed after. */
export async function exec(command: string, params: Json = {}, panel = getState().activePanel): Promise<Json> {
  if (!getState().overlay) titleReturn = null
  // Deleting a node that is used in other places asks first (DIFFS N20).
  if (command === 'node.delete' && !params.force) {
    const nodes = params.node != null ? [params.node] : undefined
    const used = await call('usedElsewhere', { panel, ...(nodes ? { nodes } : {}) })
    if (used?.count > 0) {
      setState({ overlay: { kind: 'confirm', panel, params: { ...params, nodes: used.nodes }, name: used.name, count: used.count } })
      return { result: { confirm: true } }
    }
  }
  barrier++
  // Going anywhere leaves a Settings, Recents or Help page.
  if (command.startsWith('nav.') && getState().special[panel]) setState((s) => ({ special: { ...s.special, [panel]: undefined } }))
  try {
    // The engine learns where the caret is first (arrow keys and clicks move it without a command).
    if (editor && editor.kind === 'line' && editor.panel === panel) {
      const [, caret] = editor.sel()
      // `editor` can change while awaiting (the line scrolls away and unmounts): keep the node.
      const node = editor.node
      if (!synced || synced.node !== node || synced.caret !== caret) {
        // Not awaited: the queue keeps the order, and the command is logged in this same task.
        void call('run', { command: 'focus.set', params: { node, caret }, panel })
        synced = { node, caret }
      }
    }
    const r = await call('run', { command, params, panel })
    if (r?.error) showToast(r.error)
    // A command that reports something to the person (a count, UC-06.36).
    else if (typeof r?.result?.message === 'string') showToast(r.result.message)
    synced = null
    await refresh()
    // A line that just took the caret is drawn on the next frame: keys wait for its editor.
    const after = getState().pages[panel]
    // The same line keeps the caret but the engine moved it (format marks): keys wait until the
    // editor has put it there.
    if (after?.focus != null && editor?.kind === 'line' && editor.node === after.focus) {
      // The engine moved the caret in the same line (format marks, a join): keys typed meanwhile
      // go in there, so the editor takes it now rather than on its next render (N41, N42, N58).
      const row = nodeRows(after).find((x) => x.id === after.focus)
      if (row) editor.adopt?.(row.name, after.anchor, after.caret)
    }
    if (after?.focus != null && !getState().fieldEdit && (!editor || editor.kind !== 'line' || editor.node !== after.focus)) holdKeysForEditor()
    return r
  } finally {
    barrier--
    drainSoon()
  }
}

/** Keys must wait: a command is moving the caret, or earlier keys are still waiting their turn. */
export function inBarrier() {
  return barrier > 0 || buffer.length > 0
}

function drainSoon() {
  setTimeout(drain, 0)
}

let draining = false
function drain() {
  if (draining) return
  draining = true
  try {
    // A key leaves the buffer and becomes a logged edit in the same task (see keepBufferedKeys).
    while (buffer.length && barrier === 0) {
      const k = buffer.shift()!
      if (k.click) {
        void exec('focus.set', { node: k.click.node, caret: k.click.caret }, k.click.panel)
        continue
      }
      // A page title with the keyboard (and no line caret) takes the key as if typed there.
      // A title with the keyboard takes it even while a line editor is still registered (a line
      // that had the caret before the click on the title): the fuzz test sent words to that line.
      const title = [...titles.values()].find((t) => document.activeElement?.closest(`[data-title-panel="${t.panel}"]`))
      if (title && !getState().overlay) title.key(k)
      else if (!handleKey(k)) {
        if (editor && isTextKey(k)) editor.insert(k.key)
        else if (editor) editor.native(k)
      }
    }
  } finally {
    draining = false
    keepBufferedKeys()
  }
}

/** The caret is being put back after typing with nothing focused (N61): text keys are dropped. */
let swallowing = false

/** Keeps a key for later (a command is still moving the caret). */
export function bufferKey(k: KeyInfo) {
  if (swallowing && isTextKey(k)) return
  buffer.push(k)
  keepBufferedKeys()
}

const KEPT = 'bana.keys'

/** Keys still waiting are written down whenever the buffer changes (in the task of the key press),
 * so a reload or a crash of the page keeps them; the next load types them again. */
export function keepBufferedKeys() {
  // What the waiting keys will type, for the offline preview (N78).
  let text = ''
  for (const k of buffer) {
    if (k.click) text = ''
    else if (isTextKey(k)) text += k.key
    else if (k.key === 'Enter' && !k.metaKey && !k.ctrlKey) text += '\n'
    else if (k.key === 'Backspace') text = text.slice(0, -1)
  }
  if (text !== getState().waitingText) setState({ waitingText: text })
  try {
    if (!buffer.length) return sessionStorage.removeItem(KEPT)
    // Where typing was going: a page title takes keys the engine knows nothing about.
    const title = [...titles.values()].find((t) => document.activeElement?.closest(`[data-title-panel="${t.panel}"]`))?.panel ?? null
    sessionStorage.setItem(KEPT, JSON.stringify({ keys: buffer, title }))
  } catch {
    /* storage off: the keys still run while the page lives */
  }
}

/** Keys kept by the last page are typed again, after its logged edits are in (the engine has the
 * caret there; a title that had the keyboard takes it back first). */
export function restoreKeptKeys() {
  let kept: { keys: KeyInfo[]; title: string | null } = { keys: [], title: null }
  try {
    kept = JSON.parse(sessionStorage.getItem(KEPT) ?? 'null') ?? kept
  } catch {
    /* nothing kept */
  }
  if (!Array.isArray(kept.keys) || !kept.keys.length) return drainSoon()
  // Back in the buffer at once (and so still written down), then typed on the next frame.
  kept.keys.forEach((k) => staleKeys.add(k))
  buffer.unshift(...kept.keys)
  keepBufferedKeys()
  barrier++
  requestAnimationFrame(() => {
    if (kept.title && !editor) titles.get(kept.title)?.focus(true)
    barrier--
    drainSoon()
  })
}

/** Puts a key back in front: it came before everything still waiting. */
function requeue(k: KeyInfo) {
  buffer.unshift(k)
  keepBufferedKeys()
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

/** Keys pressed before a click elsewhere or a reload: an Escape among them still clears the
 * selection, but no longer leaves typing going on in that line (N61). */
const staleKeys = new WeakSet<KeyInfo>()

/** A click: keys still waiting were pressed before it, so an Escape among them leaves no line for
 * typing to go on in. Outside the outline, typing no longer goes on in the last line either (N61). */
export function noteClick(outsideRows: boolean) {
  buffer.forEach((k) => staleKeys.add(k))
  if (outsideRows) ghosts.clear()
}

/** Whether keys with nowhere else to go may land in the panel's outline: its rows are drawn and
 * the keyboard sits on the page itself, not on an input, a button or another control. */
function outlineHasKeyboard(panel: string): boolean {
  const a = document.activeElement
  if (a && a !== document.body && a.closest('input, textarea, select, button, [contenteditable], [role=button], a')) return false
  return outlineVisible(panel)
}

/** Whether the panel shows its outline rows (a table, board or calendar view, a configure panel or
 * a field page hide them): a caret or selection in hidden rows must never take keys (N35). */
export function outlineVisible(panel: string): boolean {
  const rows = document.querySelector<HTMLElement>(`[data-panel="${panel}"] .rows`)
  return !!rows && getComputedStyle(rows).display !== 'none'
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
    case 'ui.moveTabLeft':
      moveTab(s.tab, s.tab - 1)
      return true
    case 'ui.moveTabRight':
      moveTab(s.tab, s.tab + 1)
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
    // Move to from the keyboard (UC-02.19): the picker opens under the line.
    case 'ui.moveTo': {
      const node = p?.focus ?? p?.sel[0]
      if (node == null) return true
      const r = document.querySelector(`[data-panel="${s.activePanel}"] .row.node[data-id="${node}"]`)?.getBoundingClientRect()
      openPicker({ mode: 'moveTo', panel: s.activePanel, node, at: -1, x: r?.left ?? 200, y: r?.bottom ?? 200 })
      return true
    }
    case 'ui.editDescription': {
      const node = p?.focus ?? p?.sel[0]
      if (node != null && p) {
        setState({ descEdit: { panel: s.activePanel, node } })
        void exec('focus.clear', {}, s.activePanel)
      }
      return true
    }
    case 'ui.settings': {
      // Settings open in a second panel (N55); one already open takes the keyboard.
      const open = visiblePanels(s).find((x) => s.special[x] === 'settings')
      if (open) setState({ activePanel: open })
      else
        void openInNewPanel(p?.zoom ?? 0).then((name) => setState((st) => ({ special: { ...st.special, [name]: 'settings' } })))
      return true
    }
    case 'ui.recents':
    case 'ui.help':
      setState({ special: { ...s.special, [s.activePanel]: id.slice(3) as 'settings' | 'recents' | 'help' } })
      return true
    case 'ui.searchOptions': {
      // Only on a search page (DIFFS N13); elsewhere the key does nothing.
      const v = s.views[s.activePanel]
      if (v?.query && p) void toggleQuery(s.activePanel, v.node, s.queryOpen[s.activePanel] !== v.node)
      return true
    }
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
  if (s.picker && pickerKey(k)) return true
  if (s.fieldEdit) return fieldKey(k, editor?.value() ?? '')
  if (s.fieldSel && s.fieldSel.panel === s.activePanel && fieldRowKey(k)) return true
  const sc = shortcut(k)
  // Custom keybindings come first (spec 09, Customize keybindings).
  const custom = Object.entries(s.sidebar?.keybindings ?? {}).find(([key]) => matches(key, sc))?.[1]
  if (custom) {
    if (custom.startsWith('ui.')) return runUi(custom)
    void exec(custom)
    return true
  }
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
  // Escape closes a Settings panel (N55).
  if (sc === 'Escape' && s.special[s.activePanel] === 'settings') {
    if (visiblePanels(s).length > 1) void closePanel(s.activePanel)
    else setState({ special: { ...s.special, [s.activePanel]: undefined } })
    return true
  }
  if (editor && editor.kind === 'line' && editor.panel === s.activePanel) return editorKey(k, sc, p, editor)
  // A caret or selection left in rows that are not shown takes no keys (N35).
  if ((p.focus != null || p.sel.length) && !outlineVisible(s.activePanel)) {
    const cmd = registryCommand(sc)
    if (cmd && !NEEDS_LINE.has(cmd) && !cmd.startsWith('format.') && !cmd.startsWith('node.') && !cmd.startsWith('select.') && !cmd.startsWith('focus.')) {
      void exec(cmd)
      return true
    }
    return false
  }
  // A line holds the caret but its editor is not drawn yet (or is scrolled away): the key waits.
  if (p.focus != null) {
    requeue(k)
    holdKeysForEditor()
    setState((st) => ({ focusScroll: st.focusScroll + 1 }))
    return true
  }
  if (p.sel.length) return selectionKey(k, sc, p)
  // Nothing holds a caret: typed text goes into the last line edited, or the first line (N27),
  // but only while the outline is what the panel shows and the keyboard is not on a control
  // (a button in the query builder or a toolbar keeps it, N35).
  if (isTextKey(k) && k.key !== ' ' && outlineHasKeyboard(s.activePanel)) {
    const rows = nodeRows(p)
    const g = ghosts.get(s.activePanel)
    const target = g && rows.find((r) => r.id === g.node)
    if (target) {
      requeue(k)
      void exec('focus.set', { node: target.id, caret: g.atStart ? 0 : charLen(target.name) })
      return true
    }
    // After a click elsewhere or a reload: the caret goes back to the line edited last, and the
    // key itself is not typed, so it never lands in a line the person did not pick (N61).
    // (After a reload, the line the engine saw edited last; edit times are whole seconds.)
    const last = rows.find((r) => r.id === lastEdited) ?? rows.filter((r) => !r.ref).reduce<NodeRow | undefined>((a, r) => (!a || r.edited >= a.edited ? r : a), undefined)
    if (last) {
      // Text keys pressed until that line's editor is drawn (or already waiting) are dropped too.
      for (let i = buffer.length - 1; i >= 0; i--) if (isTextKey(buffer[i])) buffer.splice(i, 1)
      keepBufferedKeys()
      swallowing = true
      window.setTimeout(() => (swallowing = false), 1000)
      void exec('focus.set', { node: last.id, caret: charLen(last.name) })
    }
    return true
  }
  const cmd = registryCommand(sc)
  if (cmd && !NEEDS_LINE.has(cmd) && !cmd.startsWith('format.')) {
    void exec(cmd)
    return true
  }
  return false
}

// ---- copy and paste (spec 02 §12; DIFFS N12, N21) ---------------------------------------------------

/** The nodes copied last and the text that went to the clipboard: pasting that text back makes
 * references to the same nodes. */
let lastCopy: { text: string; ids: number[] } | null = null

/** Copies the selection (or the nodes in `params`) as HTML and text; pasting it back makes
 * references. */
export async function copySelection(panel: string, params: Json = {}) {
  const r = await call('run', { command: 'export.clipboard', params, panel })
  const out = r?.result
  if (!out) return
  lastCopy = { text: out.text, ids: out.nodes }
  try {
    const item = new ClipboardItem({ 'text/plain': new Blob([out.text], { type: 'text/plain' }), 'text/html': new Blob([out.html], { type: 'text/html' }) })
    await navigator.clipboard.write([item])
  } catch {
    try {
      await navigator.clipboard.writeText(out.text)
    } catch {
      showToast('Could not reach the clipboard')
    }
  }
}

/** Copy node link (UC-07.10): a link that opens the node in the engine's web UI from any app. */
export async function copyNodeLink(node: number, panel: string) {
  const r = await call('run', { command: 'node.link', params: { node }, panel })
  const text: string = r?.result?.url ?? r?.result?.token ?? `[[id:${node}]]`
  await navigator.clipboard?.writeText(text).then(
    () => showToast('Node link copied'),
    () => showToast('Could not reach the clipboard'),
  )
}

/** Text pasted into a line: copied nodes come back as references; several lines become nodes. */
export function pasteInto(panel: string, node: number, lineText: string, text: string): boolean {
  if (lastCopy && text.trim() === lastCopy.text.trim()) {
    const ids = lastCopy.ids
    void withBarrier(async () => {
      const page = getState().pages[panel]
      const row = page?.rows.items.find((r) => r.k === 'node' && r.id === node) as NodeRow | undefined
      if (!row || row.parent == null) return
      let index = (row.index ?? 0) + 1
      // Pasting into an empty line: the first reference takes that line's place (DIFFS N12).
      let replace: number | null = lineText === '' ? node : null
      for (const target of ids) {
        const params: Json = { target, parent: row.parent, index }
        if (replace != null) {
          params.replace = replace
          replace = null
        } else index++
        await exec('node.reference', params, panel)
      }
    })
    return true
  }
  // Tana Paste (UC-08.11): tagged nodes, fields, dates and searches under the line's parent.
  if (text.trimStart().startsWith('%%tana%%')) {
    const page = getState().pages[panel]
    const row = page?.rows.items.find((r) => r.k === 'node' && r.id === node) as NodeRow | undefined
    // At the caret (N80): after the line, or in its place when it is empty.
    const index = row?.index != null ? row.index + (lineText === '' ? 0 : 1) : undefined
    void withBarrier(async () => {
      await exec('import.tanaPaste', { text, parent: row?.parent ?? page?.zoom, ...(index != null ? { index } : {}) }, panel)
      if (lineText === '') await exec('node.delete', { node, force: true }, panel)
    })
    return true
  }
  if (text.includes('\n')) {
    void exec('node.paste', { node, text }, panel)
    return true
  }
  return false
}

function selectionKey(k: KeyInfo, sc: string, p: Page): boolean {
  const panel = getState().activePanel
  const head = p.sel[p.sel.length - 1]
  const first = p.sel[0]
  switch (sc) {
    case 'Mod+C':
      void copySelection(panel)
      return true
    case 'Escape':
      // The selection goes, but typing still lands in its last line, at its start (N27, N34).
      if (!staleKeys.has(k)) setGhost(panel, head, true)
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
      void withBarrier(async () => {
        const r = await exec('node.delete')
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
  // `#` on several selected lines tags them all (N81); on one line it is typed, as text is.
  if (k.key === '#' && p.sel.length > 1) {
    const rows = nodeRows(p).filter((r) => p.sel.includes(r.id))
    const el = document.querySelector(`[data-panel="${panel}"] .row.node[data-id="${head}"]`)?.getBoundingClientRect()
    openPicker({ mode: 'tag', panel, node: rows[rows.length - 1]?.target ?? head, nodes: rows.map((r) => r.target), at: -1, x: el?.left ?? 200, y: el?.bottom ?? 200 })
    return true
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
    case 'Down': {
      // Down from a line with a description goes into the description (spec 02 §13).
      const row = nodeRows(p).find((r) => r.id === ed.node)
      if (row?.description) {
        setState({ descEdit: { panel, node: ed.node } })
        void exec('focus.clear', {}, panel)
        return true
      }
      void exec('focus.down')
      return true
    }
  }
  if (isTextKey(k)) return false
  const cmd = registryCommand(sc)
  if (!cmd) return false
  if (cmd.startsWith('format.')) {
    // With no selection the engine opens an empty mark at the caret, so only what is typed next
    // is formatted (N41); a selection is formatted as it is.
    void exec(cmd, anchor === caret ? { node: ed.node } : { node: ed.node, start: Math.min(anchor, caret), end: Math.max(anchor, caret) })
    return true
  }
  // Enter names its line and offset: sent again after an outage it splits that line there, never
  // the line another caret is on (N77).
  void exec(cmd, cmd === 'node.enter' ? { node: ed.node, caret } : NEEDS_LINE.has(cmd) || cmd.startsWith('node.') ? { node: ed.node } : {})
  return true
}

/** A click on a line's text: the caret goes there at once, then the engine follows. */
export function focusLine(panel: string, node: number, caret: number) {
  // Keys pressed before the click (an Escape) still go first: the click waits its turn.
  if (inBarrier()) return bufferKey({ key: '', code: '', metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, click: { panel, node, caret } })
  void exec('focus.set', { node, caret }, panel)
}

export function panelsShown() {
  return visiblePanels()
}
