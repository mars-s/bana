// Overlays: the command line (Mod+K), Search and open (Mod+S), quick add (Mod+E) and the
// keyboard shortcut sheet.

import { useEffect, useRef, useState } from 'react'
import { call, type Json, type NodeRow } from './engine'
import { Icon } from './icons'
import { exec, runUiCommand } from './controller'
import { applyPick, openPicker } from './picker'
import { tagStyle } from './Outline'
import { UI_COMMANDS, keyInfo, shortcut } from './keys'
import { getState, newTab, openInNewPanel, refresh, setState, showToast, useStore } from './store'

function close() {
  setState({ overlay: null })
}

function prettyKey(sc: string) {
  return sc
    .split(' / ')[0]
    .replace(/Mod\+/g, '⌘')
    .replace(/Shift\+/g, '⇧')
    .replace(/Alt\+/g, '⌥')
    .replace(/Ctrl\+/g, '⌃')
    .replace('Up', '↑')
    .replace('Down', '↓')
    .replace('Backspace', '⌫')
    .replace('Enter', '↵')
}

/** Arrow keys move the highlight, Enter picks, Escape closes. */
function useListKeys(n: number, pick: (i: number) => void) {
  const [sel, setSel] = useState(0)
  useEffect(() => setSel((s) => Math.min(s, Math.max(0, n - 1))), [n])
  const onKeyDown = (e: React.KeyboardEvent) => {
    const sc = shortcut(keyInfo(e))
    if (sc === 'Down' || sc === 'Ctrl+N') {
      e.preventDefault()
      setSel((s) => (n ? (s + 1) % n : 0))
    } else if (sc === 'Up' || sc === 'Ctrl+P') {
      e.preventDefault()
      setSel((s) => (n ? (s + n - 1) % n : 0))
    } else if (sc === 'Enter' || sc === 'Mod+Enter') {
      e.preventDefault()
      if (n) pick(sel)
    } else if (sc === 'Escape') {
      e.preventDefault()
      close()
    }
    e.stopPropagation()
  }
  return { sel, setSel, onKeyDown }
}

/** The command line (Cmd+K): steps for the focused or selected line first (DIFFS P2), then every
 * command with its shortcut. */
function Palette() {
  const commands = useStore((s) => s.commands)
  const activePanel = useStore((s) => s.activePanel)
  const page = useStore((s) => s.pages[s.activePanel])
  const focusId = page?.focus ?? page?.sel[0]
  const focusRow = page?.rows.items.find((r) => r.k === 'node' && r.id === focusId) as NodeRow | undefined
  const ctx = focusRow ? { row: focusRow, panel: activePanel } : null
  const [q, setQ] = useState('')
  const words = q.toLowerCase().split(/\s+/).filter(Boolean)
  type Entry = { id: string; label: string; shortcut: string; run: () => void }
  const steps: Entry[] = []
  if (ctx) {
    const { row, panel } = ctx
    const at = { panel, at: -1, x: window.innerWidth - 520, y: 180 }
    steps.push({ id: 'step.tag', label: 'Add supertag', shortcut: '', run: () => openPicker({ ...at, mode: 'tag', node: row.target }) })
    for (const t of row.tags) steps.push({ id: `step.untag.${t.id}`, label: `Remove #${t.name}`, shortcut: '', run: () => void exec('tag.remove', { node: row.target, tag: t.id }, panel) })
    steps.push({ id: 'step.field', label: 'Add field', shortcut: '', run: () => openPicker({ ...at, mode: 'field', node: row.target, owner: row.target }) })
    steps.push({ id: 'step.done', label: row.done === null ? 'Add checkbox' : 'Toggle done', shortcut: 'Mod+Enter', run: () => void exec('node.toggleDone', { node: row.id }, panel) })
    steps.push({ id: 'step.move', label: 'Move to…', shortcut: '', run: () => openPicker({ ...at, mode: 'moveTo', node: row.id }) })
    for (const k of ['outline', 'table', 'cards', 'calendar']) steps.push({ id: `step.view.${k}`, label: `View as ${k}`, shortcut: '', run: () => void exec('view.set', { node: row.id, kind: k }, panel) })
  }
  const all: Entry[] = [
    ...steps,
    ...UI_COMMANDS.map(([id, label, sc]) => ({ id, label, shortcut: sc, run: () => runUiCommand(id) })),
    ...commands.map((c) => ({ id: c.id, label: c.label, shortcut: c.shortcut, run: () => void exec(c.id, {}) })),
  ]
  const list = all.filter((c) => words.every((w) => `${c.label} ${c.id}`.toLowerCase().includes(w))).slice(0, 60)
  const pick = (i: number) => {
    const c = list[i]
    close()
    c?.run()
  }
  const { sel, setSel, onKeyDown } = useListKeys(list.length, pick)
  return (
    <div className="dialog palette" role="dialog" aria-label="Command line">
      <div className="dialog-input">
        <input autoFocus value={q} placeholder="Enter command…" onChange={(e) => setQ(e.target.value)} onKeyDown={onKeyDown} />
      </div>
      <div className="dialog-list">
        {list.map((c, i) => (
          <div key={c.id} className={`dialog-item${i === sel ? ' sel' : ''}${c.id.startsWith('step.') ? ' step' : ''}`} onMouseEnter={() => setSel(i)} onMouseDown={(e) => (e.preventDefault(), pick(i))}>
            <span className="grow">{c.label}</span>
            {c.shortcut && <kbd>{prettyKey(c.shortcut)}</kbd>}
          </div>
        ))}
        {list.length === 0 && <div className="dialog-empty">No command matches</div>}
      </div>
    </div>
  )
}

function SearchOpen() {
  const [q, setQ] = useState('')
  const [hits, setHits] = useState<Json[]>([])
  useEffect(() => {
    let live = true
    void call('find', { query: q, limit: 12 }).then((r) => live && setHits(Array.isArray(r) ? r : []))
    return () => {
      live = false
    }
  }, [q])
  const pick = (i: number, tab = false) => {
    const h = hits[i]
    close()
    if (!h) return
    if (tab) void newTab(h.id)
    else void exec('nav.open', { node: h.id })
  }
  const { sel, setSel, onKeyDown } = useListKeys(hits.length, (i) => pick(i))
  return (
    <div className="dialog search" role="dialog" aria-label="Search and open">
      <div className="dialog-input">
        <Icon name="search" />
        <input
          autoFocus
          value={q}
          placeholder="Search and open"
          onChange={(e) => setQ(e.target.value)}
          onKeyDown={(e) => {
            if (shortcut(keyInfo(e)) === 'Mod+Enter') {
              e.preventDefault()
              pick(sel, true)
              return
            }
            onKeyDown(e)
          }}
        />
      </div>
      <div className="dialog-list">
        {!q && <div className="dialog-head">Recently viewed</div>}
        {hits.map((h, i) => (
          <div key={h.id} className={`dialog-item${i === sel ? ' sel' : ''}`} onMouseEnter={() => setSel(i)} onMouseDown={(e) => (e.preventDefault(), pick(i))}>
            <span className="grow">
              {h.title || 'Untitled'}
              {h.path && <span className="dialog-path">{h.path}</span>}
            </span>
          </div>
        ))}
        {q && hits.length === 0 && <div className="dialog-empty">Nothing found</div>}
      </div>
    </div>
  )
}

function QuickAdd() {
  const [text, setText] = useState('')
  // The node being typed: made on the first letter, then renamed as typed (the text is never held
  // only here).
  const node = useRef<Promise<number | null> | null>(null)
  const send = (v: string) => {
    const panel = getState().activePanel
    if (!node.current) {
      if (!v.trim()) return
      node.current = call('run', { command: 'node.quickAdd', params: { name: v }, panel }).then((r) => r?.result?.id ?? null)
      return
    }
    void node.current.then((id) => id != null && call('run', { command: 'node.setName', params: { node: id, name: v }, panel }))
  }
  const next = () => {
    node.current = null
    setText('')
    void refresh()
  }
  return (
    <div className="dialog quick" role="dialog" aria-label="Quick add">
      <div className="dialog-head">Quick add</div>
      <div className="dialog-input">
        <span className="dot" />
        <input
          autoFocus
          value={text}
          placeholder="Add a node to today"
          onChange={(e) => {
            setText(e.target.value)
            send(e.target.value)
          }}
          onKeyDown={(e) => {
            e.stopPropagation()
            if (e.key === 'Enter') {
              e.preventDefault()
              next()
            } else if (e.key === 'Escape') {
              e.preventDefault()
              next()
              close()
            }
          }}
        />
      </div>
      <div className="quick-help">
        Each line becomes a node on today's page as you type.
        <br />
        Enter for the next line, Escape to close. Open quick add with ⌘E.
      </div>
    </div>
  )
}

function Shortcuts() {
  const commands = useStore((s) => s.commands)
  const on = useStore((s) => s.shortcuts)
  if (!on) return null
  const rows = [...UI_COMMANDS.map(([id, label, sc]) => ({ id, label, shortcut: sc })), ...commands].filter((c) => c.shortcut)
  return (
    <aside className="shortcuts" aria-label="Keyboard shortcuts">
      <div className="shortcuts-head">
        <span>Keyboard shortcuts</span>
        <button className="icon-btn" onClick={() => setState({ shortcuts: false })} title="Close">
          <Icon name="close" size={14} />
        </button>
      </div>
      {rows.map((c) => (
        <div key={c.id} className="shortcut-row">
          <span className="grow">{c.label}</span>
          <kbd>{prettyKey(c.shortcut)}</kbd>
        </div>
      ))}
    </aside>
  )
}

const PICK_TITLES: Record<string, string> = {
  tag: 'Supertags',
  field: 'Fields',
  reference: 'Reference a node',
  inlineRef: 'Reference a node',
  slash: 'Insert',
  choice: 'Options',
  searchTag: 'List of supertag',
  moveTo: 'Move to',
  column: 'Add a column',
}

function Picker() {
  const p = useStore((s) => s.picker)
  if (!p) return null
  const left = Math.min(p.x, window.innerWidth - 260)
  return (
    <div className="menu picker" style={{ left, top: p.y + 4 }} role="listbox" aria-label={PICK_TITLES[p.mode]}>
      <div className="menu-label">{PICK_TITLES[p.mode]}</div>
      {p.items.length === 0 && <div className="menu-empty">{p.query ? 'Nothing matches' : 'Type to search'}</div>}
      {p.items.map((it, i) => {
        const label = it.create
          ? p.mode === 'tag'
            ? `Create and apply #${it.create}`
            : `Create "${it.create}"`
          : (it.label ?? '')
        return (
          <div
            key={i}
            role="option"
            aria-selected={i === p.sel}
            className={`menu-item${i === p.sel ? ' sel' : ''}`}
            onMouseEnter={() => setState({ picker: { ...p, sel: i } })}
            onMouseDown={(e) => {
              e.preventDefault()
              void applyPick(it, 'enter')
            }}
          >
            <span className="menu-icon">
              {p.mode === 'tag' && !it.create ? (
                <span className="side-hash" style={tagStyle(it.color ?? 'grey')}>
                  #
                </span>
              ) : it.create ? (
                <Icon name="plus" size={12} />
              ) : p.mode === 'field' ? (
                <Icon name="field" size={12} />
              ) : null}
            </span>
            <span className="grow">{label}</span>
          </div>
        )
      })}
    </div>
  )
}

// ---- the node menu ------------------------------------------------------------------------------

function Sub({ label, children }: { label: string; children: React.ReactNode }) {
  const [open, setOpen] = useState(false)
  return (
    <div className="menu-item has-sub" onMouseEnter={() => setOpen(true)} onMouseLeave={() => setOpen(false)}>
      <span className="grow">{label}</span>
      <Icon name="chevronRight" size={12} />
      {open && <div className="menu submenu">{children}</div>}
    </div>
  )
}

function NodeMenu() {
  const m = useStore((s) => s.nodeMenu)
  const row = useStore((s) => {
    const nm = s.nodeMenu
    const page = nm ? s.pages[nm.panel] : undefined
    return page?.rows.items.find((r) => r.k === 'node' && r.id === nm?.node) as NodeRow | undefined
  })
  useEffect(() => {
    if (!m) return
    const off = (e: MouseEvent) => {
      if (!(e.target as HTMLElement).closest?.('.node-menu')) setState({ nodeMenu: null })
    }
    const esc = (e: KeyboardEvent) => e.key === 'Escape' && setState({ nodeMenu: null })
    window.addEventListener('mousedown', off)
    window.addEventListener('keydown', esc, true)
    return () => {
      window.removeEventListener('mousedown', off)
      window.removeEventListener('keydown', esc, true)
    }
  }, [m])
  if (!m) return null
  const { panel, node } = m
  const target = row?.target ?? node
  const close = () => setState({ nodeMenu: null })
  const run = (cmd: string, p: Record<string, unknown> = {}) => () => {
    close()
    void exec(cmd, { node, ...p }, panel)
  }
  const pick = (mode: 'tag' | 'field' | 'moveTo') => () => {
    close()
    openPicker({ mode, panel, node: mode === 'moveTo' ? node : target, owner: mode === 'field' ? target : undefined, at: -1, x: m.x, y: m.y })
  }
  const copy = (text: string, note: string) => () => {
    close()
    void navigator.clipboard?.writeText(text).then(() => showToast(note), () => showToast('Could not reach the clipboard'))
  }
  const It = ({ label, on, sc, danger }: { label: string; on: () => void; sc?: string; danger?: boolean }) => (
    <div className={`menu-item${danger ? ' danger' : ''}`} onMouseDown={(e) => (e.preventDefault(), on())}>
      <span className="grow">{label}</span>
      {sc && <kbd>{prettyKey(sc)}</kbd>}
    </div>
  )
  return (
    <div className="menu node-menu" style={{ left: Math.min(m.x, window.innerWidth - 230), top: Math.min(m.y, window.innerHeight - 420) }} role="menu">
      <It label="Open" on={run('nav.zoomIn')} sc="Mod+." />
      <It label="Open in new panel" on={() => (close(), void openInNewPanel(target))} sc="Mod+M" />
      <It label={row?.expanded ? 'Collapse' : 'Expand'} on={run('node.toggleExpand')} />
      <div className="menu-sep" />
      <It label="Add supertag" on={pick('tag')} />
      <It label="Add field" on={pick('field')} />
      <It label={row?.done != null ? 'Toggle done' : 'Add checkbox'} on={run('node.toggleDone')} sc="Mod+Enter" />
      <It label="Add description" on={() => (close(), setState({ descEdit: { panel, node } }))} sc="Ctrl+I" />
      <It label="Toggle heading" on={run('node.toggleHeading')} />
      <It label="Show progress bar" on={run('node.toggleProgress')} />
      <It label={row?.pinned ? 'Unpin from sidebar' : 'Pin to sidebar'} on={run('node.togglePin')} />
      <div className="menu-sep" />
      <Sub label="View as">
        {['outline', 'table', 'cards', 'list', 'side_menu', 'tabs', 'calendar'].map((k) => (
          <It key={k} label={k[0].toUpperCase() + k.slice(1).replace('_', ' ')} on={run('view.set', { kind: k })} />
        ))}
      </Sub>
      <Sub label="Sort by">
        {[['name', 'Name'], ['created', 'Created'], ['edited', 'Last edited'], ['done', 'Done'], ['none', 'None']].map(([by, l]) => (
          <It key={by} label={l} on={run('view.sort', { by })} />
        ))}
      </Sub>
      <Sub label="Group by">
        {[['tags', 'Supertags'], ['done', 'Done'], ['owner', 'Owner'], ['created', 'Created day'], ['none', 'None']].map(([by, l]) => (
          <It key={by} label={l} on={run('view.group', { by })} />
        ))}
      </Sub>
      <div className="menu-sep" />
      <It label="Duplicate" on={run('node.duplicate')} sc="Mod+Shift+D" />
      <It label="Copy node link" on={copy(`[[id:${target}]]`, 'Node link copied')} />
      <It label="Move to…" on={pick('moveTo')} />
      <It label="Move to top" on={run('node.moveToTop')} />
      <It label="Move to bottom" on={run('node.moveToBottom')} />
      <It label="Move up" on={run('node.moveUp')} sc="Mod+Shift+Up" />
      <It label="Move down" on={run('node.moveDown')} sc="Mod+Shift+Down" />
      <It label="Indent" on={run('node.indent')} sc="Tab" />
      <It label="Outdent" on={run('node.outdent')} sc="Shift+Tab" />
      <div className="menu-sep" />
      {row?.kind === 'search' && <It label="Convert search node to plain list" on={run('search.toPlainList')} />}
      {row && !row.ref && row.kind === 'plain' && <It label="Convert to supertag" on={run('node.convertToSupertag')} />}
      <It label="More commands" on={() => (close(), runUiCommand('ui.commandLine'))} sc="Mod+K" />
      {row?.inTrash ? (
        <It label="Restore" on={run('node.restore')} />
      ) : row?.ref ? (
        <It label="Remove reference" on={run('node.delete')} />
      ) : (
        <>
          <It label="Delete" on={run('node.delete')} sc="Mod+Shift+Backspace" danger />
          <It label="Hard delete including references" on={run('node.hardDelete')} danger />
        </>
      )}
    </div>
  )
}

export function Overlays() {
  const o = useStore((s) => s.overlay)
  return (
    <>
      <Picker />
      <NodeMenu />
      <Shortcuts />
      {o && (
        <div className={`scrim${o.kind === 'palette' ? '' : ' dim'}`} onMouseDown={(e) => e.target === e.currentTarget && close()}>
          {o.kind === 'palette' && <Palette />}
          {o.kind === 'search' && <SearchOpen />}
          {o.kind === 'quickadd' && <QuickAdd />}
        </div>
      )}
    </>
  )
}
