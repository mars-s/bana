// Overlays: the command line (Mod+K), Search and open (Mod+S), quick add (Mod+E) and the
// keyboard shortcut sheet.

import { useEffect, useRef, useState } from 'react'
import { call, type Json } from './engine'
import { Icon } from './icons'
import { exec, runUiCommand } from './controller'
import { applyPick } from './picker'
import { tagStyle } from './Outline'
import { UI_COMMANDS, keyInfo, shortcut } from './keys'
import { getState, newTab, refresh, setState, useStore } from './store'

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

function Palette() {
  const commands = useStore((s) => s.commands)
  const [q, setQ] = useState('')
  const words = q.toLowerCase().split(/\s+/).filter(Boolean)
  const all: { id: string; label: string; shortcut: string; ui: boolean }[] = [
    ...UI_COMMANDS.map(([id, label, sc]) => ({ id, label, shortcut: sc, ui: true })),
    ...commands.map((c) => ({ id: c.id, label: c.label, shortcut: c.shortcut, ui: false })),
  ]
  const list = all.filter((c) => words.every((w) => `${c.label} ${c.id}`.toLowerCase().includes(w))).slice(0, 60)
  const pick = (i: number) => {
    const c = list[i]
    close()
    if (!c) return
    if (c.ui) runUiCommand(c.id)
    else void exec(c.id, {})
  }
  const { sel, setSel, onKeyDown } = useListKeys(list.length, pick)
  return (
    <div className="dialog palette" role="dialog" aria-label="Command line">
      <div className="dialog-input">
        
        <input autoFocus value={q} placeholder="Enter command…" onChange={(e) => setQ(e.target.value)} onKeyDown={onKeyDown} />
      </div>
      <div className="dialog-list">
        {list.map((c, i) => (
          <div key={c.id} className={`dialog-item${i === sel ? ' sel' : ''}`} onMouseEnter={() => setSel(i)} onMouseDown={(e) => (e.preventDefault(), pick(i))}>
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

export function Overlays() {
  const o = useStore((s) => s.overlay)
  return (
    <>
      <Picker />
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
