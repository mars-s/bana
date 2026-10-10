// The window: top bar (sidebar button, history, tabs, right cluster), sidebar and panels.

import { useEffect, useRef, useState } from 'react'
import { Icon, TodayIcon, type IconName } from './icons'
import { DragHint, Panel, tagStyle } from './Outline'
import { closeTab, getState, moveTab, newTab, setState, switchTab, useStore } from './store'
import { bufferKey, currentEditor, exec, handleKey, holdKeysForEditor, inBarrier, runUiCommand } from './controller'
import { isTextKey, keyInfo } from './keys'
import { Overlays } from './Overlays'
import type { Sidebar as SidebarData, SidebarItem } from './engine'

function TopBar() {
  const tabs = useStore((s) => s.tabs)
  const tab = useStore((s) => s.tab)
  const pages = useStore((s) => s.pages)
  const active = useStore((s) => s.activePanel)
  const page = pages[active]
  return (
    <header className="topbar" data-tauri-drag-region>
      <div className="topbar-left">
        <button className="icon-btn" title="Sidebar (full, mini, hidden)" onClick={() => runUiCommand('ui.toggleSidebar')}>
          <Icon name="sidebar" size={14} />
        </button>
        <button className="icon-btn" title="Back" disabled={!page?.canBack} onClick={() => void exec('nav.back')}>
          <Icon name="back" size={14} />
        </button>
        <button className="icon-btn" title="Forward" disabled={!page?.canForward} onClick={() => void exec('nav.forward')}>
          <Icon name="forward" size={14} />
        </button>
      </div>
      <nav className="tabs" data-tauri-drag-region>
        {tabs.map((t, i) => (
          <div
            key={t.id}
            className={`tab${i === tab ? ' active' : ''}`}
            onMouseDown={() => void switchTab(i)}
            draggable
            onDragStart={(e) => e.dataTransfer.setData('text/x-tanacraft-tab', String(i))}
            onDragOver={(e) => e.dataTransfer.types.includes('text/x-tanacraft-tab') && e.preventDefault()}
            onDrop={(e) => {
              const from = Number(e.dataTransfer.getData('text/x-tanacraft-tab'))
              if (Number.isFinite(from)) moveTab(from, i)
            }}
            title={t.panels.map((p) => pages[p]?.title ?? '').join(' | ')}
          >
            {t.panels.map((p, j) => (
              <span key={p} className="tab-title">
                {j > 0 && <span className="tab-div" />}
                {pages[p]?.title ?? ''}
              </span>
            ))}
            {tabs.length > 1 && (
              <span
                className="tab-close"
                onMouseDown={(e) => {
                  e.stopPropagation()
                  void closeTab(i)
                }}
              >
                <Icon name="close" size={11} />
              </span>
            )}
          </div>
        ))}
        <button className="icon-btn tab-add" title="New tab" onClick={() => void newTab()}>
          <Icon name="plus" size={14} />
        </button>
      </nav>
      <div className="topbar-right">
        <button className="icon-btn" title="Notifications (none yet)">
          <Icon name="bell" size={14} />
        </button>
        <button className="icon-btn" title="Help" onClick={() => runUiCommand('ui.help')}>
          <Icon name="help" size={14} />
        </button>
        <button className="icon-btn" title="Keyboard shortcuts" onClick={() => runUiCommand('ui.shortcuts')}>
          <Icon name="keyboard" size={14} />
        </button>
        <button className="icon-btn" title="Settings" onClick={() => runUiCommand('ui.settings')}>
          <Icon name="user" size={14} />
        </button>
      </div>
    </header>
  )
}

function SideRow({ icon, label, onClick, active, pin, children }: { icon: React.ReactNode; label: string; onClick: (e: React.MouseEvent) => void; active?: boolean; pin?: boolean; children?: React.ReactNode }) {
  return (
    <div className={`side-row${active ? ' active' : ''}${pin ? ' pin' : ''}`} onClick={onClick} role="button" title={label}>
      <span className="side-icon">{icon}</span>
      <span className="side-label">{label}</span>
      {children}
    </div>
  )
}

function pinIcon(it: SidebarItem): React.ReactNode {
  if (it.icon) return <span className="emoji">{it.icon}</span>
  if (it.kind === 'tag')
    return (
      <span className="side-hash" style={tagStyle(it.color ?? 'grey')}>
        #
      </span>
    )
  const map: Record<string, IconName> = { search: 'search', day: 'daily', journal: 'daily', trash: 'trash', library: 'home', inbox: 'inbox' }
  return map[it.kind] ? <Icon name={map[it.kind]} /> : <span className="side-dot" />
}

function initials(name: string) {
  return name
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((w) => w[0]?.toUpperCase())
    .join('')
}

/** One workspace in the sidebar: its row (avatar, name, "+"), and its entries (spec 07 §3). With
 * several workspaces each row folds (they start folded, DIFFS N24). */
function WorkspaceRows({ ws, several, zoom, journal, showDaily }: { ws: SidebarData['workspaces'][number]; several: boolean; zoom?: number; journal: number; showDaily: boolean }) {
  const [open, setOpen] = useState(false)
  const unfolded = !several || open
  return (
    <div className="side-ws">
      <div className="side-ws-head" onClick={() => (several ? setOpen((o) => !o) : void exec('nav.open', { node: ws.library }))} role="button">
        {several && (
          <span className="ws-chev">
            <Icon name={open ? 'chevronDown' : 'chevronRight'} size={10} />
          </span>
        )}
        <span className="ws-dot">{initials(ws.name)}</span>
        <span className="side-label">{ws.name}</span>
        <span className="spacer" />
        <button
          className="icon-btn small"
          title={`Create new item in ${ws.name}`}
          onClick={(e) => {
            e.stopPropagation()
            void exec('node.add', { name: '', parent: ws.library }).then((r) => r?.result?.id != null && exec('nav.open', { node: r.result.id }))
          }}
        >
          <Icon name="plus" size={12} />
        </button>
      </div>
      {unfolded && (
        <div className="side-ws-items">
          {ws.main && showDaily && <SideRow pin icon={null} label="Daily notes" active={zoom === journal} onClick={() => void exec('nav.dailyNotes')} />}
          <SideRow pin icon={null} label="Library" active={zoom === ws.library} onClick={() => void exec('nav.open', { node: ws.library })} />
          {ws.entries.slice(0, 12).map((it) => (
            <SideRow key={it.id} pin icon={null} label={it.name || 'Untitled'} active={zoom === it.id} onClick={() => void exec('nav.open', { node: it.id })} />
          ))}
          <SideRow pin icon={null} label="Trash" active={zoom === ws.trash} onClick={() => void exec('nav.open', { node: ws.trash })} />
        </div>
      )}
    </div>
  )
}

/** The mini rail: one column of icon buttons (spec 07 §1, 12: Mini rail). Pinned and Workspaces
 * open a flyout to the right with the same lists the full sidebar shows. */
function MiniRail({ sb, zoom, shows }: { sb: SidebarData; zoom?: number; shows: (k: string) => boolean }) {
  const [fly, setFly] = useState<{ what: 'pinned' | 'workspaces'; top: number } | null>(null)
  const ref = useRef<HTMLElement>(null)
  useEffect(() => {
    if (!fly) return
    const off = (e: MouseEvent) => !ref.current?.contains(e.target as Node) && setFly(null)
    const esc = (e: KeyboardEvent) => e.key === 'Escape' && (e.stopPropagation(), setFly(null))
    window.addEventListener('mousedown', off)
    window.addEventListener('keydown', esc, true)
    return () => {
      window.removeEventListener('mousedown', off)
      window.removeEventListener('keydown', esc, true)
    }
  }, [fly])
  const toggle = (what: 'pinned' | 'workspaces') => (e: React.MouseEvent) => {
    const top = (e.currentTarget as HTMLElement).getBoundingClientRect().top
    setFly((f) => (f?.what === what ? null : { what, top }))
  }
  // Clicking an item inside the flyout opens it and closes the flyout.
  const closeAfter = () => setTimeout(() => setFly(null), 0)
  return (
    <aside className="sidebar mini" ref={ref}>
      <SideRow icon={<TodayIcon day={sb.today} />} label="Today" onClick={() => void exec('nav.today')} />
      {shows('supertags') && <SideRow icon={<Icon name="hash" size={14} />} label="Supertags" onClick={() => void exec('nav.supertags')} />}
      <SideRow icon={<Icon name="search" size={14} />} label="Search and open" onClick={() => runUiCommand('ui.searchOpen')} />
      {shows('recents') && <SideRow icon={<Icon name="clock" size={14} />} label="Recents" onClick={() => runUiCommand('ui.recents')} />}
      <SideRow icon={<Icon name="compose" size={14} />} label="Create new" onClick={() => void exec('node.createNew')} />
      <SideRow icon={<Icon name="pin" size={14} />} label="Pinned" active={fly?.what === 'pinned'} onClick={toggle('pinned')} />
      <SideRow icon={<Icon name="workspace" size={14} />} label="Workspaces" active={fly?.what === 'workspaces'} onClick={toggle('workspaces')} />
      {fly && (
        <div className="flyout" style={{ top: fly.top }} role="menu" aria-label={fly.what === 'pinned' ? 'Pinned' : 'Workspaces'} onClick={(e) => (e.target as HTMLElement).closest('.side-row') && closeAfter()}>
          {fly.what === 'pinned' ? (
            sb.pinned.length ? (
              sb.pinned.map((it) => <SideRow key={it.id} pin icon={pinIcon(it)} label={it.name} active={zoom === it.id} onClick={() => void exec('nav.open', { node: it.id })} />)
            ) : (
              <p className="flyout-empty">Nothing pinned yet</p>
            )
          ) : (
            <>
              {sb.workspaces.map((ws) => (
                <WorkspaceRows key={ws.id} ws={ws} several={sb.workspaces.length > 1} zoom={zoom} journal={sb.journal} showDaily={shows('daily_notes')} />
              ))}
              <div className="side-foot" onClick={() => (setFly(null), setState({ overlay: { kind: 'workspaces' } }))} role="button">
                Manage workspaces
              </div>
            </>
          )}
        </div>
      )}
    </aside>
  )
}

function Sidebar() {
  const sb = useStore((s) => s.sidebar)
  const mode = useStore((s) => s.sidebarMode)
  const active = useStore((s) => s.activePanel)
  const page = useStore((s) => s.pages[s.activePanel])
  if (!sb || mode === 2) return null
  const zoom = page?.zoom
  const shows = (k: string) => !sb.hidden.includes(k)
  if (mode === 1) return <MiniRail sb={sb} zoom={zoom} shows={shows} />
  return (
    <aside className="sidebar" data-panel-for={active}>
      <SideRow icon={<TodayIcon day={sb.today} />} label="Today" active={page?.kind === 'day' && page.title.startsWith('Today')} onClick={() => void exec('nav.today')} />
      {shows('supertags') && <SideRow icon={<Icon name="hash" size={14} />} label="Supertags" active={zoom === sb.schema} onClick={() => void exec('nav.supertags')} />}
      {shows('recents') && <SideRow icon={<Icon name="clock" size={14} />} label="Recents" onClick={() => runUiCommand('ui.recents')} />}
      <div className="side-actions">
        <button className="btn create" onClick={() => void exec('node.createNew')}>
          <Icon name="compose" size={14} />
          <span className="side-label">Create new</span>
        </button>
        <button className="btn icon" title="Search and open" onClick={() => runUiCommand('ui.searchOpen')}>
          <Icon name="search" size={14} />
        </button>
      </div>
      {sb.pinned.length > 0 && <div className="side-head">Pinned</div>}
      {sb.pinned.map((it) => (
        <SideRow key={it.id} pin icon={pinIcon(it)} label={it.name} active={zoom === it.id} onClick={() => void exec('nav.open', { node: it.id })} />
      ))}
      {sb.workspaces.length > 1 && <div className="side-head">Workspaces</div>}
      {sb.workspaces.map((ws) => (
        <WorkspaceRows key={ws.id} ws={ws} several={sb.workspaces.length > 1} zoom={zoom} journal={sb.journal} showDaily={shows('daily_notes')} />
      ))}
      <div className="side-foot" onClick={() => setState({ overlay: { kind: 'workspaces' } })} role="button">
        Manage workspaces
      </div>
    </aside>
  )
}

function Toast() {
  const t = useStore((s) => s.toast)
  return t ? (
    <div className="toast" role="status">
      <Icon name="info" size={20} className="toast-icon" />
      <span>{t}</span>
    </div>
  ) : null
}

export function App() {
  const theme = useStore((s) => s.theme)
  const tabs = useStore((s) => s.tabs)
  const tab = useStore((s) => s.tab)
  const panels = tabs[tab]?.panels ?? []
  useEffect(() => {
    document.documentElement.dataset.theme = theme
  }, [theme])
  // Following the system appearance (N56): a change there switches the theme here.
  const followSystem = useStore((s) => s.followSystem)
  useEffect(() => {
    if (!followSystem) return
    const mq = window.matchMedia?.('(prefers-color-scheme: light)')
    const on = () => setState({ theme: mq.matches ? 'light' : 'dark' })
    on()
    mq?.addEventListener('change', on)
    return () => mq?.removeEventListener('change', on)
  }, [followSystem])
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (['Shift', 'Meta', 'Control', 'Alt', 'CapsLock'].includes(e.key) || e.isComposing) return
      const s = getState()
      if (s.overlay) return
      const target = e.target as HTMLElement | null
      const inOtherInput = target?.closest?.('input, textarea') && !target.closest('.editor-input') && !target.closest('.title-input')
      if (inOtherInput || target?.closest?.('.title-input')) return
      const k = keyInfo(e)
      // A command is moving the caret: the key waits and is replayed where the caret lands.
      if (inBarrier()) {
        e.preventDefault()
        bufferKey(k)
        return
      }
      // A line holds the caret but the keyboard is elsewhere (a menu or dialog just closed): the
      // key goes into the line, and the line takes the keyboard back (DIFFS N16, N27).
      const holder = currentEditor()
      if (holder && !target?.closest?.('input, textarea, [contenteditable]')) {
        holder.focus()
        if (isTextKey(k)) {
          e.preventDefault()
          holder.insert(k.key)
          return
        }
      }
      // The key landed in a line editor that no longer holds the caret: it waits for the right one.
      const ed = currentEditor()
      const fromNode = target?.closest?.('.editor-input')?.getAttribute('data-node')
      if (fromNode && (!ed || ed.kind !== 'line' || String(ed.node) !== fromNode)) {
        e.preventDefault()
        bufferKey(k)
        holdKeysForEditor()
        return
      }
      // The focused line is scrolled away (its editor is not drawn): bring it back first.
      const page = s.pages[s.activePanel]
      if (page?.focus != null && !currentEditor()) {
        e.preventDefault()
        bufferKey(k)
        setState((st) => ({ focusScroll: st.focusScroll + 1 }))
        return
      }
      if (handleKey(k)) e.preventDefault()
    }
    // A click anywhere lets go of a selected field row.
    const onDown = () => {
      if (getState().fieldSel) setState({ fieldSel: null })
    }
    window.addEventListener('keydown', onKey, true)
    window.addEventListener('mousedown', onDown, true)
    return () => {
      window.removeEventListener('keydown', onKey, true)
      window.removeEventListener('mousedown', onDown, true)
    }
  }, [])
  const sidebarMode = useStore((s) => s.sidebarMode)
  return (
    <div className={`app${sidebarMode === 2 ? ' sidebar-hidden' : ''}`}>
      <TopBar />
      <div className="body">
        <Sidebar />
        <main className="panels">
          {panels.map((p) => (
            <Panel key={p} panel={p} many={panels.length > 1} />
          ))}
        </main>
      </div>
      <Overlays />
      <DragHint />
      <Toast />
    </div>
  )
}
