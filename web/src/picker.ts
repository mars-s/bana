// The `#`, `@`, `>` and `/` menus, and field values. Ported from legacy/egui/ui-egui/src/overlays.rs
// (picker_items, apply_choice, picker_key) and keys.rs (the field editor keys).
//
// The text typed after a trigger stays in the line, so it is in the engine like any other text;
// picking removes it again and runs the commands.

import { call } from './engine'
import { type KeyInfo, shortcut } from './keys'
import { type PickItem, type PickerState, getState, setState } from './store'
import { exec, holdKeysForEditor, withBarrier, runUiCommand } from './controller'
import { charLen } from './format'

let fetchSeq = 0
let latest: Promise<void> = Promise.resolve()

function fetchChoices(p: PickerState) {
  latest = doFetch(p)
  return latest
}

async function doFetch(p: PickerState) {
  const seq = ++fetchSeq
  const params: Record<string, unknown> = { mode: p.mode, node: p.owner ?? p.node, query: p.query }
  if (p.field != null) params.field = p.field
  const r = await call('choices', params)
  if (seq !== fetchSeq) return
  setState((s) => (s.picker ? { picker: { ...s.picker, items: r?.items ?? [], nextColor: r?.nextColor ?? 'grey', sel: Math.min(s.picker.sel, Math.max(0, (r?.items?.length ?? 1) - 1)) } } : {}))
}

export function openPicker(p: Omit<PickerState, 'items' | 'sel' | 'nextColor' | 'query'> & { query?: string }) {
  const picker: PickerState = { query: '', ...p, items: [], sel: 0, nextColor: 'grey' }
  setState({ picker })
  void fetchChoices(picker)
}

export function closePicker() {
  if (getState().picker) setState({ picker: null })
}

export function setPickerQuery(query: string) {
  const p = getState().picker
  if (!p || p.query === query) return
  const next = { ...p, query, sel: 0 }
  setState({ picker: next })
  void fetchChoices(next)
}

/** Called by the line editor after every change: opens, follows or closes a menu. Returns the text
 * the line should hold instead (for `!`), or null. */
export function onLineText(row: { id: number; parent: number | null }, panel: string, value: string, caret: number, at: (i: number) => { x: number; y: number }): string | null {
  const chars = Array.from(value)
  const open = getState().picker
  if (open && open.node === row.id && open.mode !== 'choice') {
    const trigger = chars[open.at]
    const want = { tag: '#', inlineRef: '@', reference: '@', field: '>', slash: '/' }[open.mode as string]
    if (trigger !== want || caret <= open.at) closePicker()
    else setPickerQuery(chars.slice(open.at + 1, caret).join(''))
    return null
  }
  const typed = chars[caret - 1]
  const pos = at(caret - 1)
  const base = { panel, node: row.id, at: caret - 1, x: pos.x, y: pos.y }
  if (typed === '#') openPicker({ ...base, mode: 'tag' })
  else if (typed === '@' && value === '@') openPicker({ ...base, mode: 'reference' })
  else if (typed === '@') openPicker({ ...base, mode: 'inlineRef' })
  else if (typed === '>' && value === '>' && row.parent != null) openPicker({ ...base, mode: 'field', owner: row.parent })
  else if (typed === '/' && value === '/') openPicker({ ...base, mode: 'slash' })
  else if (typed === '!' && value === '!') {
    // `!` on an empty line makes it a heading (spec 02 §10).
    void withBarrier(async () => {
      await call('run', { command: 'node.setName', params: { node: row.id, name: '', caret: 0 }, panel })
      await exec('node.toggleHeading', { node: row.id }, panel)
    })
    return ''
  }
  return null
}

/** Keys while a menu is open. Typing goes on into the line (the query); these keys pick. */
export function pickerKey(k: KeyInfo): boolean {
  const p = getState().picker
  if (!p) return false
  const sc = shortcut(k)
  const n = p.items.length
  switch (sc) {
    case 'Up':
      setState({ picker: { ...p, sel: n ? (p.sel + n - 1) % n : 0 } })
      return true
    case 'Down':
      setState({ picker: { ...p, sel: n ? (p.sel + 1) % n : 0 } })
      return true
    case 'Escape':
      // An options menu under a field value: Escape closes it and leaves the value (one key).
      if (p.mode === 'choice') {
        closePicker()
        return false
      }
      // A field name typed after `>` is kept (DIFFS N15, N28).
      if (p.mode === 'field' && p.query.trim()) {
        const exact = p.items.find((i) => i.label?.toLowerCase() === p.query.trim().toLowerCase())
        void applyPick(exact ?? { create: p.query.trim() }, 'escape')
      } else closePicker()
      return true
    case 'Mod+Enter': {
      // Creates from the typed text instead of picking a match (spec 02 §2, §11).
      const q = p.query.trim().replace(/^#/, '')
      if (q) void applyPick({ create: q }, 'enter')
      return true
    }
    case 'Enter':
    case 'Tab': {
      // The menu may still be catching up with the last letters typed: pick from the answer to
      // the whole query (keys typed meanwhile wait).
      const how = sc === 'Tab' ? 'tab' : 'enter'
      void withBarrier(async () => {
        await latest
        const q = getState().picker
        if (!q) return
        const item = q.items[Math.min(q.sel, q.items.length - 1)]
        if (item) await applyPick(item, how)
        else closePicker()
      })
      return true
    }
  }
  return false
}

/** The line with the trigger and the query taken out; caret where they were. */
async function cutQuery(p: PickerState, replacement = '', trimBefore = false) {
  const name = await lineName(p.node)
  const chars = Array.from(name)
  const end = Math.min(chars.length, p.at + 1 + charLen(p.query))
  let head = chars.slice(0, p.at).join('')
  if (trimBefore) head = head.replace(/\s+$/, '')
  const next = head + replacement + chars.slice(end).join('')
  const caret = charLen(head + replacement)
  await call('run', { command: 'node.setName', params: { node: p.node, name: next, caret }, panel: p.panel })
  return caret
}

async function lineName(id: number): Promise<string> {
  const r = await call('run', { command: 'graph.inspect', params: { node: id } })
  return r?.result?.name ?? ''
}

export async function applyPick(item: PickItem, how: 'enter' | 'tab' | 'escape' | 'create') {
  const p = getState().picker
  if (!p) return
  setState({ picker: null })
  const panel = p.panel
  await withBarrier(async () => {
    switch (p.mode) {
      case 'tag': {
        // From the line (`#`): the typed text leaves the line. From a menu: nothing to cut.
        const caret = p.at >= 0 ? await cutQuery(p, '', true) : null
        if (item.create) await exec('tag.add', { node: p.node, tag: item.create, color: p.nextColor }, panel)
        else await exec('tag.add', { node: p.node, tag: item.id }, panel)
        if (caret != null) await exec('focus.set', { node: p.node, caret }, panel)
        break
      }
      case 'inlineRef': {
        let target = item.id
        if (item.create) {
          const lib = getState().sidebar?.workspaces[0]?.library
          const r = await exec('node.add', { parent: lib, name: item.create }, panel)
          target = r?.result?.id
        }
        if (target == null) break
        const caret = await cutQuery(p, `[[id:${target}]]`)
        await exec('focus.set', { node: p.node, caret }, panel)
        break
      }
      case 'reference': {
        let target = item.id
        if (item.create) {
          const lib = getState().sidebar?.workspaces[0]?.library
          const r = await exec('node.add', { parent: lib, name: item.create }, panel)
          target = r?.result?.id
        }
        // The line must be empty to be replaced by the reference.
        await cutQuery(p)
        if (target != null) await exec('node.reference', { target, replace: p.node }, panel)
        break
      }
      case 'field': {
        const owner = p.owner!
        if (p.at < 0) {
          // From a menu: the field goes on the node, and its value is typed next.
          const r = await exec('field.add', { node: owner, field: item.create ?? item.id }, panel)
          const fid: number | null = r?.result?.field ?? null
          if (fid != null) await startFieldEdit(panel, owner, fid, 0)
          break
        }
        const field = item.create ?? item.id
        // The row takes the place of the line `>` was typed on: after the sibling above it.
        const page = getState().pages[panel]
        const rows = page?.rows.items.filter((r) => r.k === 'node' && (r as { parent: number | null }).parent === owner) ?? []
        const i = rows.findIndex((r) => r.k === 'node' && r.id === p.node)
        const params: Record<string, unknown> = { node: owner, field }
        const prev = i > 0 ? rows[i - 1] : undefined
        if (prev && prev.k === 'node') params.after = prev.id
        const r = await exec('field.add', params, panel)
        const fid: number | null = r?.result?.field ?? null
        if (how === 'enter') {
          // The empty line stays, just below the new field row (spec 02 §3).
          await call('run', { command: 'node.setName', params: { node: p.node, name: '', caret: 0 }, panel })
          await exec('focus.set', { node: p.node, caret: 0 }, panel)
        } else {
          await exec('node.delete', { node: p.node }, panel)
          if (fid != null && how === 'tab') await startFieldEdit(panel, owner, fid, 0)
          else if (fid != null) await exec('focus.clear', {}, panel)
        }
        break
      }
      case 'slash': {
        await cutQuery(p)
        const label = item.label
        const pos = { panel, node: p.node, at: 0, x: p.x, y: p.y }
        if (label === 'Field') {
          await call('run', { command: 'node.setName', params: { node: p.node, name: '>', caret: 1 }, panel })
          const page = getState().pages[panel]
          const row = page?.rows.items.find((r) => r.k === 'node' && r.id === p.node) as { parent: number | null } | undefined
          if (row?.parent != null) openPicker({ ...pos, mode: 'field', owner: row.parent })
        } else if (label === 'Reference') {
          await call('run', { command: 'node.setName', params: { node: p.node, name: '@', caret: 1 }, panel })
          openPicker({ ...pos, mode: 'reference' })
        } else if (label === 'Search node') {
          openPicker({ ...pos, mode: 'searchTag', at: -1 })
        } else if (label === 'Heading') await exec('node.toggleHeading', { node: p.node }, panel)
        else if (label === 'Checkbox') await exec('node.toggleDone', { node: p.node }, panel)
        else if (label === 'Checklist') await exec('node.insertChecklist', { node: p.node }, panel)
        else runUiCommand('ui.commandLine')
        await exec('focus.set', { node: p.node, caret: 1e9 }, panel)
        break
      }
      case 'searchTag': {
        if (item.id == null) break
        const name = item.label ?? ''
        await exec('search.create', { query: `#"${name}"`, name: `List of ${name}`, replace: p.node }, panel)
        break
      }
      case 'choice': {
        const value = item.create ?? item.label ?? ''
        await call('run', { command: 'field.set', params: { node: p.node, field: p.field, index: p.index ?? 0, value }, panel })
        setState({ fieldEdit: null })
        await exec('focus.clear', {}, panel)
        break
      }
      case 'column': {
        await exec('view.addColumn', { node: p.node, field: item.create ?? item.id }, panel)
        break
      }
      case 'moveTo': {
        if (item.id != null) await exec('node.move', { node: p.node, parent: item.id }, panel)
        break
      }
    }
  })
}

// ---- field values -------------------------------------------------------------------------------

export async function startFieldEdit(panel: string, owner: number, field: number, index: number) {
  holdKeysForEditor()
  setState({ fieldEdit: { panel, owner, field, index } })
  await exec('focus.clear', {}, panel)
}

export function stopFieldEdit() {
  if (getState().fieldEdit) setState({ fieldEdit: null })
}

/** Keys in a field value (DIFFS N15, N16, N17): the value is saved as typed. */
export function fieldKey(k: KeyInfo, text: string): boolean {
  const fe = getState().fieldEdit
  if (!fe) return false
  const sc = shortcut(k)
  const panel = fe.panel
  switch (sc) {
    case 'Escape':
    case 'Up':
    case 'Down': {
      // The value is kept (it is saved as typed). Escape selects the line above the field row; Up
      // and Down put the caret in the line above or below (DIFFS N15).
      stopFieldEdit()
      const page = getState().pages[panel]
      const rows = page?.rows.items ?? []
      const at = rows.findIndex((r) => r.k === 'field' && r.owner === fe.owner && r.field === fe.field)
      const above = rows.slice(0, Math.max(0, at)).reverse().find((r) => r.k === 'node')
      const below = rows.slice(at + 1).find((r) => r.k === 'node')
      if (sc === 'Escape' && above?.k === 'node') void exec('select.set', { from: above.id }, panel)
      else if (sc === 'Up' && above?.k === 'node') void exec('focus.set', { node: above.id, caret: 1e9 }, panel)
      else if (sc === 'Down' && below?.k === 'node') void exec('focus.set', { node: below.id, caret: 0 }, panel)
      return true
    }
    case 'Enter':
    case 'Shift+Tab':
      if (sc === 'Enter' && text.trim()) {
        // Keeps the value and opens another value line in the same field (spec 02 §3).
        holdKeysForEditor()
        setState({ fieldEdit: { ...fe, index: fe.index + 1 } })
        return true
      }
      // Enter on an empty value line moves out like Shift+Tab.
      // The value becomes a line under the field row (DIFFS N16, N17).
      stopFieldEdit()
      void exec('field.valueToNode', { node: fe.owner, field: fe.field, index: fe.index, text }, panel)
      return true
  }
  return false
}
