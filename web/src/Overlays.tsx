// Overlays: the command line (Mod+K), Search and open (Mod+S), quick add (Mod+E) and the
// keyboard shortcut sheet.

import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { call, type Json, type NodeRow } from './engine'
import { Icon, type IconName } from './icons'
import { copySelection, exec, returnKeyboard, runUiCommand } from './controller'
import { applyPick, openPicker, pickerKey, setPickerQuery } from './picker'
import { tagStyle } from './Outline'
import { SubMenu } from './Views'
import { UI_COMMANDS, keyInfo, shortcut } from './keys'
import { getState, newTab, openConfigure, openInNewPanel, refresh, setState, showToast, useStore } from './store'

function close() {
  setState({ overlay: null })
  returnKeyboard()
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
  // Command nodes are listed by name and run on the line (UC-06.33).
  const cmdNodes = useStore((s) => s.sidebar?.commandNodes) ?? []
  // With a line focused the box opens under that line (N63); else top right (spec 12).
  const [under] = useState(() => {
    const el = focusId == null ? null : document.querySelector(`[data-panel="${activePanel}"] [data-node="${focusId}"]`)
    const r = el?.getBoundingClientRect()
    if (!r) return null
    const top = Math.min(r.bottom + 6, window.innerHeight - 380)
    return { top, left: Math.min(Math.max(8, r.left), window.innerWidth - 510), right: 'auto' }
  })
  const words = q.toLowerCase().split(/\s+/).filter(Boolean)
  type Entry = { id: string; label: string; shortcut: string; run: () => void }
  const steps: Entry[] = []
  if (ctx) {
    const { row, panel } = ctx
    const at = { panel, at: -1, x: window.innerWidth - 520, y: 180 }
    // Tana's order with a line focused (N63): Find nodes, Add tag, Move to, Move and leave
    // reference to, then the views.
    steps.push({ id: 'step.find', label: 'Find nodes →', shortcut: '', run: () => runUiCommand('ui.searchOpen') })
    steps.push({ id: 'step.tag', label: 'Add tag', shortcut: '#', run: () => openPicker({ ...at, mode: 'tag', node: row.target }) })
    steps.push({ id: 'step.move', label: 'Move to', shortcut: '', run: () => openPicker({ ...at, mode: 'moveTo', node: row.id }) })
    steps.push({ id: 'step.moveRef', label: 'Move and leave reference to', shortcut: '', run: () => openPicker({ ...at, mode: 'moveTo', node: row.id, leaveRef: true }) })
    for (const k of ['table', 'cards', 'calendar', 'outline']) steps.push({ id: `step.view.${k}`, label: `View as ${k}`, shortcut: '', run: () => void exec('view.set', { node: row.id, kind: k }, panel) })
    for (const t of row.tags) steps.push({ id: `step.untag.${t.id}`, label: `Remove #${t.name}`, shortcut: '', run: () => void exec('tag.remove', { node: row.target, tag: t.id }, panel) })
    steps.push({ id: 'step.field', label: 'Add field', shortcut: '', run: () => openPicker({ ...at, mode: 'field', node: row.target, owner: row.target }) })
    steps.push({ id: 'step.done', label: row.done === null ? 'Add checkbox' : 'Toggle done', shortcut: 'Mod+Enter', run: () => void exec('node.toggleDone', { node: row.id }, panel) })
  }
  const all: Entry[] = [
    ...steps,
    ...cmdNodes.map((c) => ({ id: `cmdnode.${c.id}`, label: `Run ${c.name}`, shortcut: '', run: () => void exec('commandNode.run', { command: c.id, ...(ctx ? { node: ctx.row.target } : {}) }) })),
    // On a line: run a command node on each new child of it (UC-02.20).
    ...(ctx ? cmdNodes.map((c) => ({ id: `cmdchild.${c.id}`, label: `On new children run ${c.name}`, shortcut: '', run: () => void exec('commandNode.on', { command: c.id, event: 'child', node: ctx.row.target }) })) : []),
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
    <div className="dialog palette" role="dialog" aria-label="Command line" style={under ?? undefined}>
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
  // Tab on a result searches inside it (UC-06.42); Backspace in an empty box leaves it.
  const [scope, setScope] = useState<{ id: number; title: string } | null>(null)
  useEffect(() => {
    let live = true
    void call('find', { query: q, limit: 12, ...(scope ? { under: scope.id } : {}) }).then((r) => live && setHits(Array.isArray(r) ? r : []))
    return () => {
      live = false
    }
  }, [q, scope])
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
      {/* Header: a small label over the search box, then a rule (spec 12: Search and open). */}
      <div className="search-head">Search and open</div>
      <div className="dialog-input">
        <Icon name="search" />
        {scope && (
          <span className="search-scope" title="Searching inside this node; Backspace to leave">
            {scope.title || 'Untitled'}
          </span>
        )}
        <input
          autoFocus
          value={q}
          placeholder={scope ? 'Search inside…' : 'Search...'}
          onChange={(e) => setQ(e.target.value)}
          onKeyDown={(e) => {
            if (shortcut(keyInfo(e)) === 'Mod+Enter') {
              e.preventDefault()
              pick(sel, true)
              return
            }
            if (e.key === 'Tab' && !e.shiftKey && hits[sel]) {
              e.preventDefault()
              setScope({ id: hits[sel].id, title: hits[sel].title })
              setQ('')
              setSel(0)
              return
            }
            if (e.key === 'Backspace' && !q && scope) {
              e.preventDefault()
              setScope(null)
              return
            }
            onKeyDown(e)
          }}
        />
      </div>
      <div className="dialog-list">
        {!q && !scope && <div className="dialog-head">Recently viewed</div>}
        {hits.map((h, i) => (
          <div key={h.id} className={`dialog-item${i === sel ? ' sel' : ''}`} onMouseEnter={() => setSel(i)} onMouseDown={(e) => (e.preventDefault(), pick(i))}>
            <span className="grow">
              {h.title || 'Untitled'}
              {h.path && <span className="dialog-path">{h.path}</span>}
            </span>
          </div>
        ))}
        {(q || scope) && hits.length === 0 && <div className="dialog-empty">Nothing found</div>}
      </div>
    </div>
  )
}

/** Quick add: each line becomes a node on today's page as it is typed. `onDone` is the capture
 * window's way out (UC-08.14); without it Escape closes the overlay. */
export function QuickAdd({ onDone }: { onDone?: () => void } = {}) {
  const [text, setText] = useState('')
  const input = useRef<HTMLInputElement>(null)
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
    if (!onDone) void refresh()
  }
  // The capture window is kept between uses: each time it shows, it starts empty with the caret in.
  useEffect(() => {
    if (!onDone) return
    const again = () => {
      next()
      input.current?.focus()
    }
    window.addEventListener('focus', again)
    return () => window.removeEventListener('focus', again)
  })
  return (
    <div className={`dialog quick${onDone ? ' standalone' : ''}`} role="dialog" aria-label="Quick add">
      {/* The capture window has no title bar: its title moves it. */}
      <div className="dialog-head" data-tauri-drag-region={onDone ? true : undefined}>
        Quick add
      </div>
      <div className="dialog-input">
        <span className="dot" />
        <input
          ref={input}
          autoFocus
          aria-label="New line"
          value={text}
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
              if (onDone) onDone()
              else close()
            }
          }}
        />
      </div>
      <div className="quick-help">
        Each line becomes a node on today's page as you type.
        <br />
        {onDone ? 'Enter for the next line, Escape to close. Open it from any app with ⌘⇧E.' : 'Enter for the next line, Escape to close. Open quick add with ⌘E.'}
      </div>
    </div>
  )
}

/** "Used elsewhere" question before a delete (DIFFS N20). */
function ConfirmDelete({ o }: { o: Json }) {
  const del = () => {
    close()
    void exec('node.delete', { ...o.params, force: true }, o.panel)
  }
  const show = () => {
    close()
    const first = o.params.nodes?.[0]
    if (first != null) void exec('nav.open', { node: first }, o.panel)
  }
  return (
    <div
      className="dialog confirm"
      role="alertdialog"
      aria-label="Delete a node used elsewhere"
      tabIndex={-1}
      ref={(el) => el?.focus()}
      onKeyDown={(e) => {
        e.stopPropagation()
        if (e.key === 'Enter') del()
        else if (e.key === 'Escape') close()
      }}
    >
      <p className="confirm-text">
        <b>{o.name || 'This node'}</b> is used in {o.count} {o.count === 1 ? 'other place' : 'other places'}. Deleting it removes it there too.
      </p>
      <div className="confirm-actions">
        <button className="btn regular" onClick={close}>
          Cancel
        </button>
        <button className="btn regular" onClick={show}>
          Show where it is used
        </button>
        <button className="btn regular danger" onClick={del}>
          Delete anyway
        </button>
      </div>
    </div>
  )
}

/** Manage workspaces (spec 07 §3): create one, rename them. Each is a part of the one graph file. */
function ManageWorkspaces() {
  const sb = useStore((s) => s.sidebar)
  const [name, setName] = useState('')
  const create = () => {
    if (!name.trim()) return
    void exec('workspace.create', { name: name.trim() })
    setName('')
  }
  return (
    <div className="dialog quick workspaces" role="dialog" aria-label="Manage workspaces">
      <div className="dialog-head">Manage workspaces</div>
      <p className="confirm-text">Workspaces keep separate Libraries, supertags and trash in the same file. They all stay open in the sidebar.</p>
      {sb?.workspaces.map((w) => (
        <div key={w.id} className="ws-row">
          <input
            className="cfg-input"
            aria-label={`Rename ${w.name}`}
            defaultValue={w.name}
            onBlur={(e) => e.target.value.trim() && e.target.value !== w.name && void exec('workspace.rename', { workspace: w.id, name: e.target.value.trim() })}
            onKeyDown={(e) => {
              e.stopPropagation()
              if (e.key === 'Enter') (e.target as HTMLInputElement).blur()
              if (e.key === 'Escape') close()
            }}
          />
        </div>
      ))}
      <div className="ws-row">
        <input
          className="cfg-input"
          placeholder="Workspace name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => {
            e.stopPropagation()
            if (e.key === 'Enter') create()
            if (e.key === 'Escape') close()
          }}
        />
        <button className="btn regular" onClick={create}>
          Create workspace
        </button>
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
  templateField: 'Add a field to the template',
  sourceTag: 'Options from supertag',
  source: 'Source of options',
  mergeInto: 'Merge into',
}

/** "/" items: icon and shortcut (spec 02 §11; DIFFS N62). */
const SLASH: Record<string, { icon: IconName | string; key?: string }> = {
  Paste: { icon: 'copy', key: '⌘V' },
  'Search node': { icon: 'search' },
  Field: { icon: 'field' },
  Reference: { icon: '@', key: '@' },
  Heading: { icon: 'H', key: '!' },
  Checkbox: { icon: 'checkCircle', key: '⌘⏎' },
  Checklist: { icon: 'fieldCheck' },
  'More commands': { icon: 'more', key: '⌘K' },
}
const FIELD_TYPES: [string, string][] = [['plain', 'Plain'], ['options', 'Options'], ['options_from_tag', 'Options from supertag'], ['date', 'Date'], ['number', 'Number'], ['user', 'User'], ['url', 'Url'], ['email', 'E-Mail'], ['checkbox', 'Checkbox']]

function SlashIcon({ icon }: { icon: string }) {
  return icon.length <= 1 ? <span className="menu-glyph">{icon}</span> : <Icon name={icon as IconName} size={12} />
}

function Picker() {
  const p = useStore((s) => s.picker)
  const ref = useRef<HTMLDivElement>(null)
  const [above, setAbove] = useState(false)
  const [fieldSub, setFieldSub] = useState(false)
  // Near the bottom of the window the menu opens above the line (spec 02 §11).
  useLayoutEffect(() => {
    const r = ref.current?.getBoundingClientRect()
    if (!r || !p) return setAbove(false)
    if (!above && r.bottom > window.innerHeight - 8 && p.y - r.height > 8) setAbove(true)
  })
  useEffect(() => {
    setAbove(false)
    setFieldSub(false)
  }, [p?.mode, p?.node])
  if (!p) return null
  const width = p.mode === 'tag' ? 500 : p.mode === 'slash' ? 210 : 260
  const left = Math.min(p.x, window.innerWidth - width - 8)
  const nodes = p.mode === 'reference' || p.mode === 'inlineRef'
  const head = p.mode === 'tag' ? 'Add supertag' : nodes && !p.query ? 'Recently opened' : null
  const top = above ? p.y - (ref.current?.getBoundingClientRect().height ?? 0) - 26 : p.y + 4
  return (
    <div ref={ref} className={`menu picker mode-${p.mode}`} style={{ left, top }} role="listbox" aria-label={PICK_TITLES[p.mode]}>
      {head ? <div className="menu-caps">{head}</div> : p.mode !== 'slash' && <div className="menu-label">{PICK_TITLES[p.mode]}</div>}
      {p.at < 0 && p.mode !== 'choice' && (
        // Opened from a menu or button: the query is typed here.
        <input
          className="filter-input small"
          autoFocus
          placeholder="Type to search"
          value={p.query}
          onChange={(e) => setPickerQuery(e.target.value)}
          onKeyDown={(e) => {
            e.stopPropagation()
            if (pickerKey(keyInfo(e))) e.preventDefault()
          }}
        />
      )}
      {p.items.length === 0 && <div className="menu-empty">{p.query ? 'Nothing matches' : 'Type to search'}</div>}
      {p.items.map((it, i) => {
        const label = it.create
          ? p.mode === 'tag'
            ? `Create and apply #${it.create}`
            : `Create "${it.create}"`
          : (it.label ?? '')
        const slash = p.mode === 'slash' ? SLASH[label] : undefined
        return (
          <div
            key={i}
            role="option"
            aria-selected={i === p.sel}
            className={`menu-item${i === p.sel ? ' sel' : ''}${it.description || it.path ? ' two-line' : ''}`}
            onMouseEnter={() => {
              setState({ picker: { ...p, sel: i } })
              setFieldSub(label === 'Field' && p.mode === 'slash')
            }}
            onMouseDown={(e) => {
              e.preventDefault()
              void applyPick(it, 'enter')
            }}
          >
            <span className="menu-icon">
              {slash ? (
                <SlashIcon icon={slash.icon} />
              ) : p.mode === 'tag' && !it.create ? (
                <span className="side-hash" style={tagStyle(it.color ?? 'grey')}>
                  #
                </span>
              ) : it.create ? (
                <Icon name="plus" size={12} />
              ) : it.date ? (
                <Icon name="calendar" size={12} />
              ) : p.mode === 'field' ? (
                <Icon name="field" size={12} />
              ) : null}
            </span>
            <span className="grow">
              {it.path && <span className="pick-path">{it.path}</span>}
              {label}
              {it.description && <span className="pick-desc">{it.description}</span>}
            </span>
            {it.workspace && <span className="pick-ws">{it.workspace}</span>}
            {slash?.key && <kbd>{slash.key}</kbd>}
            {slash && label === 'Field' && <Icon name="chevronRight" size={10} />}
            {slash && label === 'Field' && fieldSub && (
              <div className="menu submenu picker-sub">
                {FIELD_TYPES.map(([type, name]) => (
                  <div
                    key={type}
                    className="menu-item"
                    onMouseDown={(e) => {
                      e.preventDefault()
                      e.stopPropagation()
                      void applyPick({ ...it, fieldType: type }, 'enter')
                    }}
                  >
                    {name}
                  </div>
                ))}
              </div>
            )}
          </div>
        )
      })}
    </div>
  )
}

// ---- the node menu ------------------------------------------------------------------------------

/** The node menu (spec 02 §16): right-click a bullet (top-left at the pointer) or a page's "…"
 * (under the button, right-aligned, 7 px below). It always fits the window (N37, N38). */
function NodeMenu() {
  const m = useStore((s) => s.nodeMenu)
  const row = useStore((s) => {
    const nm = s.nodeMenu
    const page = nm ? s.pages[nm.panel] : undefined
    return page?.rows.items.find((r) => r.k === 'node' && r.id === nm?.node) as NodeRow | undefined
  })
  const page = useStore((s) => (s.nodeMenu ? s.pages[s.nodeMenu.panel] : undefined))
  const pinned = useStore((s) => s.sidebar?.pinned)
  const pinnedIds = (pinned ?? []).map((p) => p.id)
  const ref = useRef<HTMLDivElement>(null)
  const [pos, setPos] = useState<{ left: number; top: number; maxHeight?: number } | null>(null)
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
  useLayoutEffect(() => {
    const el = ref.current
    if (!m || !el) return setPos(null)
    const margin = 8
    const w = el.offsetWidth
    const h = el.scrollHeight
    let left = m.alignRight ? m.x - w : m.x
    left = Math.max(margin, Math.min(left, window.innerWidth - w - margin))
    let top = m.y
    // No room below: it moves up to fit, and scrolls only if the window is shorter than it.
    if (top + h > window.innerHeight - margin) top = Math.max(margin, window.innerHeight - h - margin)
    const maxHeight = h > window.innerHeight - 2 * margin ? window.innerHeight - 2 * margin : undefined
    setPos({ left, top, maxHeight })
  }, [m])
  if (!m) return null
  const { panel, node } = m
  const target = row?.target ?? (page?.zoom === node ? page.target : node)
  const kind = row?.kind ?? page?.kind
  const isRef = row?.ref ?? false
  const close = () => setState({ nodeMenu: null })
  const run = (cmd: string, p: Record<string, unknown> = {}) => () => {
    close()
    void exec(cmd, { node, ...p }, panel)
  }
  const pick = (mode: 'tag' | 'field' | 'moveTo' | 'mergeInto') => () => {
    close()
    // On a line in a selection, Add tag tags every selected line (UC-02.15).
    const sel = page?.sel ?? []
    const nodes =
      mode === 'tag' && sel.length > 1 && sel.includes(node)
        ? (page?.rows.items ?? []).filter((x): x is NodeRow => x.k === 'node' && sel.includes(x.id)).map((x) => x.target)
        : undefined
    openPicker({ mode, panel, node: mode === 'moveTo' ? node : target, nodes, owner: mode === 'field' ? target : undefined, at: -1, x: m.x, y: m.y })
  }
  const copy = (text: string, note: string) => () => {
    close()
    void navigator.clipboard?.writeText(text).then(() => showToast(note), () => showToast('Could not reach the clipboard'))
  }
  const It = ({ label, on, sc, danger, icon }: { label: string; on: () => void; sc?: string; danger?: boolean; icon?: IconName }) => (
    <div className={`menu-item${danger ? ' danger' : ''}`} onMouseDown={(e) => (e.preventDefault(), on())}>
      <span className="menu-icon">{icon && <Icon name={icon} size={12} />}</span>
      <span className="grow">{label}</span>
      {sc && <kbd>{prettyKey(sc)}</kbd>}
    </div>
  )
  const done = row ? row.done : null
  // Opened from a page's "…" (N65): Tana's page menu, or its supertag menu on a tag page.
  const fromPage = !row && page?.zoom === node
  const editLog = () => {
    close()
    void exec('node.editLog', { node: target }, panel).then((r) => {
      const log = (r?.result?.log ?? []) as { event: string; at: string }[]
      showToast(log.map((e) => `${e.event} ${e.at}`).join(' · ') || 'No edits yet')
    })
  }
  const copyNode = () => (close(), void copySelection(panel, { node: target }))
  if (fromPage && kind === 'tag')
    return (
      <div ref={ref} className="menu node-menu" style={pos ? { left: pos.left, top: pos.top } : { left: m.x, top: m.y, visibility: 'hidden' }} role="menu">
        <It label="Configure supertag" icon="gear" on={() => (close(), void openConfigure(target))} />
        <It label={pinnedIds.includes(target) ? 'Unpin tag' : 'Pin tag'} icon="pin" on={run('node.togglePin')} />
        <div className="menu-sep" />
        <It label="Copy node link" icon="link" on={copy(`[[id:${target}]]`, 'Node link copied')} />
        <It label="Copy" icon="copy" on={copyNode} sc="Mod+C" />
        <It label="Duplicate" icon="duplicate" on={run('node.duplicate')} sc="Mod+Shift+D" />
        <It label="Move" icon="forward" on={pick('moveTo')} />
        <div className="menu-sep" />
        <It label="Add tag" icon="hash" on={pick('tag')} />
        <It label="Add description" on={() => (close(), setState({ descEdit: { panel, node } }))} sc="Ctrl+I" />
        <It label="Show edit log" on={editLog} />
        <div className="menu-sep" />
        <It label="Delete" icon="trash" on={run('node.delete')} sc="Mod+Shift+Backspace" danger />
        <It label="More commands" icon="search" on={() => (close(), runUiCommand('ui.commandLine'))} sc="Mod+K" />
      </div>
    )
  return (
    <div
      ref={ref}
      className="menu node-menu"
      style={pos ? { left: pos.left, top: pos.top, maxHeight: pos.maxHeight, overflowY: pos.maxHeight ? 'auto' : 'visible' } : { left: m.x, top: m.y, visibility: 'hidden' }}
      role="menu"
    >
      <SubMenu label="View as">
        {['outline', 'table', 'cards', 'list', 'side_menu', 'tabs', 'calendar'].map((k) => (
          <It key={k} label={k[0].toUpperCase() + k.slice(1).replace('_', ' ')} on={run('view.set', { kind: k })} />
        ))}
      </SubMenu>
      <It label="Show view toolbar" on={run('view.toggleToolbar')} />
      <SubMenu label="Filter by">
        <It label="Not done" on={run('view.filter', { query: 'NOT DONE' })} />
        <It label="Done" on={run('view.filter', { query: 'DONE' })} />
        <It label="No filter" on={run('view.filter', { query: '' })} />
      </SubMenu>
      <SubMenu label="Sort by">
        {[['name', 'Name'], ['created', 'Created'], ['edited', 'Last edited'], ['done', 'Done'], ['none', 'None']].map(([by, l]) => (
          <It key={by} label={l} on={run('view.sort', { by })} />
        ))}
      </SubMenu>
      <SubMenu label="Group by">
        {[['tags', 'Supertags'], ['done', 'Done'], ['owner', 'Owner'], ['created', 'Created day'], ['none', 'None']].map(([by, l]) => (
          <It key={by} label={l} on={run('view.group', { by })} />
        ))}
      </SubMenu>
      <SubMenu label="Display">
        {['Created time', 'Last edited time', 'Done time'].map((f) => (
          <It key={f} label={f} on={run('view.toggleSystemField', { field: f })} />
        ))}
      </SubMenu>
      {/* Icon ▸ and Banner ▸ as on Tana's page menu (N65). */}
      <SubMenu label="Icon">
        {['📄', '🌱', '⭐', '📌'].map((e) => (
          <It key={e} label={e} on={() => (close(), void exec('node.setIcon', { node: target, icon: e }, panel))} />
        ))}
        <It label="No icon" on={() => (close(), void exec('node.setIcon', { node: target, icon: '' }, panel))} />
      </SubMenu>
      <SubMenu label="Banner">
        {[['picture:orbit', 'Orbit picture'], ['blue', 'Blue'], ['green', 'Green'], ['', 'No banner']].map(([v, l]) => (
          <It key={l} label={l} on={() => (close(), void exec('node.setBanner', { node: target, color: v }, panel))} />
        ))}
      </SubMenu>
      <div className="menu-sep" />
      {!fromPage && <It label="Open in new panel" icon="sideMenu" on={() => (close(), void openInNewPanel(target))} sc="Mod+M" />}
      <It label="Copy node link" icon="link" on={copy(`[[id:${target}]]`, 'Node link copied')} />
      {fromPage && <It label="Copy" icon="copy" on={copyNode} sc="Mod+C" />}
      <It label="Duplicate" icon="duplicate" on={run('node.duplicate')} sc="Mod+Shift+D" />
      {!fromPage && <It label="Move to…" icon="forward" on={pick('moveTo')} />}
      {!isRef && <It label="Merge into…" on={pick('mergeInto')} />}
      {/* Remind me (UC-01.12): a reference on that day's page. */}
      <SubMenu label="Add to day">
        {[['today', 'Today'], ['tomorrow', 'Tomorrow'], ['next monday', 'Next Monday'], ['next week', 'In a week']].map(([d, l]) => (
          <It
            key={d}
            label={l}
            on={() => {
              close()
              void exec('node.addToDay', { node: target, date: d }, panel).then((r) => r?.result && showToast(`Added to ${r.result.date}`))
            }}
          />
        ))}
      </SubMenu>
      <div className="menu-sep" />
      {kind === 'tag' && <It label="Configure supertag" icon="gear" on={() => (close(), void openConfigure(target))} />}
      {kind === 'tag' && <It label={row?.pinned ? 'Unpin tag' : 'Pin tag'} icon="pin" on={run('node.togglePin')} />}
      <It label="Add tag" icon="hash" on={pick('tag')} />
      {!fromPage && <It label="Toggle heading" on={run('node.toggleHeading')} />}
      <It label="Add description" on={() => (close(), setState({ descEdit: { panel, node } }))} sc="Ctrl+I" />
      <It label={done != null ? 'Toggle done' : 'Add checkbox'} on={run('node.toggleDone')} sc="Mod+Enter" />
      <It label="Show progress bar" on={run('node.toggleProgress')} />
      {fromPage && <It label="Show edit log" on={editLog} />}
      {isRef && <It label="Notes and fields for this context" on={run('node.contextNote')} sc="Mod+Alt+Down" />}
      {kind === 'search' && <It label="Convert search node to plain list" on={run('search.toPlainList')} />}
      <div className="menu-sep" />
      {row?.inTrash ? (
        <It label="Restore" on={run('node.restore')} />
      ) : isRef ? (
        <It label="Remove reference" icon="trash" on={run('node.delete')} />
      ) : (
        <It label="Delete" icon="trash" on={run('node.delete')} sc="Mod+Shift+Backspace" danger />
      )}
      <It label="More commands" icon="search" on={() => (close(), runUiCommand('ui.commandLine'))} sc="Mod+K" />
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
          {o.kind === 'confirm' && <ConfirmDelete o={o} />}
          {o.kind === 'workspaces' && <ManageWorkspaces />}
        </div>
      )}
    </>
  )
}
