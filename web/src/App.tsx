// The window: top bar (sidebar button, history, tabs, right cluster), sidebar and panels.

import { Fragment, useEffect, useRef, useState } from 'react'
import { Icon, TodayIcon, type IconName } from './icons'
import { DragHint, Panel, tagStyle } from './Outline'
import { closeTab, getState, moveTab, newTab, setState, showToast, switchTab, useStore } from './store'
import { bufferKey, noteClick, currentEditor, exec, handleKey, holdKeysForEditor, inBarrier, runUiCommand } from './controller'
import { isTextKey, keyInfo } from './keys'
import { Overlays } from './Overlays'
import { SyncButton } from './Sync'
import { SubMenu } from './Views'
import type { Sidebar as SidebarData, SidebarItem } from './engine'

/** What the sidebar button does next, as its accessible name (N75): full, mini, hidden, full. */
const SIDEBAR_NEXT = ['Show mini sidebar', 'Hide sidebar', 'Show sidebar']

/** Which importer reads a file (UC-08.10, UC-08.11): OPML, Roam JSON, Tana Paste, else Markdown. */
function importCommand(name: string, text: string): string {
  const n = name.toLowerCase()
  if (n.endsWith('.opml')) return 'import.opml'
  if (n.endsWith('.json')) return 'import.roam'
  if (text.trimStart().startsWith('%%tana%%')) return 'import.tanaPaste'
  return 'import.markdown'
}

function TopBar() {
  const tabs = useStore((s) => s.tabs)
  const tab = useStore((s) => s.tab)
  const pages = useStore((s) => s.pages)
  const special = useStore((s) => s.special)
  const active = useStore((s) => s.activePanel)
  const page = pages[active]
  const SPECIAL = { settings: 'Settings', recents: 'Recents', help: 'Help' } as const
  const tabTitle = (p: string) => {
    const sp = special[p]
    if (sp) return SPECIAL[sp]
    return pages[p]?.kind === 'schema' ? 'Tag overview' : (pages[p]?.title ?? '')
  }
  const sidebarMode = useStore((s) => s.sidebarMode)
  return (
    <header className="topbar" data-tauri-drag-region>
      <div className="topbar-left">
        <button className="icon-btn" title="Sidebar (full, mini, hidden)" aria-label={SIDEBAR_NEXT[sidebarMode]} onClick={() => runUiCommand('ui.toggleSidebar')}>
          <Icon name="sidebar" size={14} />
        </button>
        <button className="icon-btn" title="Back" disabled={!page?.canBack} onClick={() => void exec('nav.back')}>
          <Icon name="back" size={14} />
        </button>
        <button className="icon-btn" title="Forward" disabled={!page?.canForward} onClick={() => void exec('nav.forward')}>
          <Icon name="forward" size={14} />
        </button>
      </div>
      {/* Each panel by its own page: Settings, Recents and Help by name (N70), the Supertags page as
          Tag overview (N59.38). */}
      <nav className="tabs" data-tauri-drag-region>
        {tabs.map((t, i) => (
          <div
            key={t.id}
            className={`tab${i === tab ? ' active' : ''}`}
            onMouseDown={() => void switchTab(i)}
            draggable
            onDragStart={(e) => e.dataTransfer.setData('text/x-bana-tab', String(i))}
            onDragOver={(e) => e.dataTransfer.types.includes('text/x-bana-tab') && e.preventDefault()}
            onDrop={(e) => {
              const from = Number(e.dataTransfer.getData('text/x-bana-tab'))
              if (Number.isFinite(from)) moveTab(from, i)
            }}
            title={t.panels.map(tabTitle).join(' | ')}
          >
            {t.panels.map((p, j) => (
              <span key={p} className="tab-title">
                {j > 0 && <span className="tab-div" />}
                {tabTitle(p)}
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
        <SyncButton />
        <button className="icon-btn" title="Notifications (none yet)">
          <Icon name="bell" size={14} />
        </button>
        <button className="icon-btn" title="Help" onClick={() => runUiCommand('ui.help')}>
          <Icon name="help" size={14} />
        </button>
        <button className="icon-btn" title="Keyboard shortcuts" onClick={() => runUiCommand('ui.shortcuts')}>
          <Icon name="keyboard" size={14} />
        </button>
        <AccountMenu />
      </div>
    </header>
  )
}

/** Downloads the workspace as a ZIP of Markdown files (UC-08.13). */
async function exportZip(shape: string) {
  const r = await exec('export.zip', { shape })
  const z = r?.result
  if (!z?.data) return
  const bytes = Uint8Array.from(atob(z.data), (c) => c.charCodeAt(0))
  const a = document.createElement('a')
  a.href = URL.createObjectURL(new Blob([bytes], { type: 'application/zip' }))
  a.download = z.name
  a.click()
  setTimeout(() => URL.revokeObjectURL(a.href), 10_000)
  showToast(`Exported ${z.files} ${z.files === 1 ? 'page' : 'pages'} as Markdown`)
}

/** The account menu under the person icon (N66): what a local app has of Tana's list. */
function AccountMenu() {
  const [open, setOpen] = useState(false)
  const ref = useRef<HTMLSpanElement>(null)
  const file = useRef<HTMLInputElement>(null)
  const theme = useStore((s) => s.theme)
  useEffect(() => {
    if (!open) return
    const off = (e: MouseEvent) => !ref.current?.contains(e.target as Node) && setOpen(false)
    window.addEventListener('mousedown', off)
    return () => window.removeEventListener('mousedown', off)
  }, [open])
  const item = (label: string, run: () => void) => (
    <div className="menu-item" role="menuitem" onMouseDown={(e) => (e.preventDefault(), setOpen(false), run())}>
      {label}
    </div>
  )
  return (
    <span className="menu-anchor" ref={ref}>
      <button className={`icon-btn${open ? ' on' : ''}`} title="Account" aria-label="Settings" onClick={() => setOpen((o) => !o)}>
        <Icon name="user" size={14} />
      </button>
      {/* A Markdown, OPML, Roam JSON or Tana Paste file read here and imported by the engine (spec 07). */}
      <input
        ref={file}
        type="file"
        accept=".md,.markdown,.txt,.opml,.json"
        hidden
        onChange={(e) => {
          const f = e.target.files?.[0]
          e.target.value = ''
          if (f) void f.text().then((text) => exec(importCommand(f.name, text), { text }))
        }}
      />
      {open && (
        <div className="menu pop account-menu" role="menu">
          {item('Workspaces', () => setState({ overlay: { kind: 'workspaces' } }))}
          {item('Settings', () => runUiCommand('ui.settings'))}
          {item('Import content…', () => file.current?.click())}
          {/* Everything as Markdown files in a ZIP (UC-08.13). */}
          <SubMenu label="Export everything as Markdown" icon={false}>
            {item('By page', () => void exportZip('outline'))}
            {item('By supertag', () => void exportZip('tags'))}
            {item('As an Obsidian vault', () => void exportZip('vault'))}
          </SubMenu>
          {item('Browse templates', () => setState({ overlay: { kind: 'templates' } }))}
          <div className="menu-sep" />
          {item(theme === 'dark' ? 'Light theme' : 'Dark theme', () => runUiCommand('ui.toggleTheme'))}
          {item('Keyboard shortcuts', () => runUiCommand('ui.shortcuts'))}
          {item('Help', () => runUiCommand('ui.help'))}
        </div>
      )}
    </span>
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

/** A pinned project's progress ring (UC-02.04): done of total checkboxes below it. */
function ProgressRing({ done, total }: { done: number; total: number }) {
  const r = 5
  const c = 2 * Math.PI * r
  return (
    <svg className="progress-ring" width="14" height="14" viewBox="0 0 14 14" role="img" aria-label={`${done} of ${total} done`}>
      <title>{`${done} of ${total} done`}</title>
      <circle cx="7" cy="7" r={r} className="ring-track" />
      <circle cx="7" cy="7" r={r} className="ring-fill" strokeDasharray={`${(c * done) / Math.max(1, total)} ${c}`} transform="rotate(-90 7 7)" />
    </svg>
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
  const ref = useRef<HTMLDivElement>(null)
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
    <div className="rail" ref={ref}>
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
              sb.pinned.map((it) => (
                <SideRow key={it.id} pin icon={pinIcon(it)} label={it.name} active={zoom === it.id} onClick={() => void exec('nav.open', { node: it.id })}>
                  {it.progress && <ProgressRing done={it.progress[0]} total={it.progress[1]} />}
                </SideRow>
              ))
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
    </div>
  )
}

/** The 4 px handle on the sidebar's right edge: drag to resize (N51, motion M14). */
function SidebarGrip() {
  return (
    <span
      className="side-grip"
      title="Drag to resize"
      onPointerDown={(e) => {
        e.preventDefault()
        const aside = e.currentTarget.parentElement as HTMLElement
        const x0 = e.clientX
        const w0 = aside.getBoundingClientRect().width
        aside.classList.add('resizing')
        const move = (ev: PointerEvent) => setState({ sidebarW: Math.max(180, Math.min(420, Math.round(w0 + ev.clientX - x0))) })
        const up = () => {
          aside.classList.remove('resizing')
          window.removeEventListener('pointermove', move)
          window.removeEventListener('pointerup', up)
        }
        window.addEventListener('pointermove', move)
        window.addEventListener('pointerup', up)
      }}
    />
  )
}

function Sidebar() {
  const width = useStore((st) => st.sidebarW)
  const sb = useStore((s) => s.sidebar)
  const mode = useStore((s) => s.sidebarMode)
  const active = useStore((s) => s.activePanel)
  const page = useStore((s) => s.pages[s.activePanel])
  if (!sb) return null
  const zoom = page?.zoom
  const shows = (k: string) => !sb.hidden.includes(k)
  // One element for every mode, so its width animates between them (motion M1 to M3).
  if (mode === 1)
    return (
      <aside className="sidebar mini">
        <MiniRail sb={sb} zoom={zoom} shows={shows} />
      </aside>
    )
  return (
    <aside className={`sidebar${mode === 2 ? ' hidden' : ''}`} data-panel-for={active} style={width && mode === 0 ? { width } : undefined} aria-hidden={mode === 2}>
      {mode === 0 && <SidebarGrip />}
      <SideRow icon={<TodayIcon day={sb.today} />} label="Today" onClick={() => void exec('nav.today')} />
      {shows('supertags') && <SideRow icon={<Icon name="hash" size={14} />} label="Supertags" active={false} onClick={() => void exec('nav.supertags')} />}
      {shows('recents') && <SideRow icon={<Icon name="clock" size={14} />} label="Recents" onClick={() => runUiCommand('ui.recents')} />}
      {/* Where Tana has AI chats (out of scope): the space is kept so everything below sits right. */}
      <div className="side-spacer" aria-hidden />
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
        <SideRow key={it.id} pin icon={pinIcon(it)} label={it.name} onClick={() => void exec('nav.open', { node: it.id })}>
          {it.progress && <ProgressRing done={it.progress[0]} total={it.progress[1]} />}
        </SideRow>
      ))}
      {/* Workspaces: a heading and one folding row each (N59.3). */}
      <div className="side-head">Workspaces</div>
      {sb.workspaces.map((ws) => (
        <WorkspaceRows key={ws.id} ws={ws} several zoom={zoom} journal={sb.journal} showDaily={shows('daily_notes')} />
      ))}
      <div className="side-foot" onClick={() => setState({ overlay: { kind: 'workspaces' } })} role="button">
        Manage workspaces
      </div>
    </aside>
  )
}

/** The gap between two panels: drag it to share the width differently (N52, motion M14). */
function PanelGrip({ left, right }: { left: string; right: string }) {
  return (
    <span
      className="panel-grip"
      title="Drag to resize"
      onPointerDown={(e) => {
        e.preventDefault()
        const grip = e.currentTarget
        const l = grip.previousElementSibling as HTMLElement | null
        const r = grip.nextElementSibling as HTMLElement | null
        if (!l || !r) return
        const lw = l.getBoundingClientRect().width
        const rw = r.getBoundingClientRect().width
        const s0 = getState().panelGrow
        const total = (s0[left] ?? 1) + (s0[right] ?? 1)
        const x0 = e.clientX
        grip.classList.add('active')
        const move = (ev: PointerEvent) => {
          const nl = Math.max(240, Math.min(lw + rw - 240, lw + ev.clientX - x0))
          const gl = (total * nl) / (lw + rw)
          setState((st) => ({ panelGrow: { ...st.panelGrow, [left]: gl, [right]: total - gl } }))
        }
        const up = () => {
          grip.classList.remove('active')
          window.removeEventListener('pointermove', move)
          window.removeEventListener('pointerup', up)
        }
        window.addEventListener('pointermove', move)
        window.addEventListener('pointerup', up)
      }}
    />
  )
}

function Toast() {
  const t = useStore((s) => s.toast)
  const leaving = useStore((s) => s.toastLeaving)
  return t ? (
    <div className={`toast${leaving ? ' leaving' : ''}`} role="status">
      <Icon name="info" size={20} className="toast-icon" />
      <span>{t}</span>
    </div>
  ) : null
}

/** The engine is out of reach (DIFFS N73): edits wait in the log and go as soon as it is back. */
function OfflineBanner() {
  const off = useStore((s) => s.offline)
  return off ? (
    <div className="toast offline" role="alert">
      <Icon name="info" size={20} className="toast-icon" />
      <span>Offline: can't reach the engine. Your edits are kept and will be saved when it is back.</span>
    </div>
  ) : null
}

/** The page follows the theme setting, or the system's appearance (N56). */
export function useTheme() {
  const theme = useStore((s) => s.theme)
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
  // A theme picked in another window of this app (the capture window follows the main one).
  useEffect(() => {
    const on = (e: StorageEvent) => {
      if (e.key !== 'bana.view' || !e.newValue) return
      try {
        const v = JSON.parse(e.newValue)
        if (v.theme === 'light' || v.theme === 'dark') setState({ theme: v.theme, followSystem: !!v.followSystem })
      } catch {
        /* not ours */
      }
    }
    window.addEventListener('storage', on)
    return () => window.removeEventListener('storage', on)
  }, [])
}

export function App() {
  const tabs = useStore((s) => s.tabs)
  const tab = useStore((s) => s.tab)
  const panels = tabs[tab]?.panels ?? []
  useTheme()
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (['Shift', 'Meta', 'Control', 'Alt', 'CapsLock'].includes(e.key) || e.isComposing) return
      const s = getState()
      if (s.overlay) return
      const target = e.target as HTMLElement | null
      const inOtherInput = target?.closest?.('input, textarea') && !target.closest('.editor-input') && !target.closest('.title-input')
      if (inOtherInput) return
      const k = keyInfo(e)
      // The title handles its own keys, but not ahead of keys still waiting: those come first.
      if (target?.closest?.('.title-input')) {
        if (!inBarrier()) return
        e.preventDefault()
        e.stopPropagation()
        bufferKey(k)
        return
      }
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
    const onDown = (e: MouseEvent) => {
      if (getState().fieldSel) setState({ fieldSel: null })
      noteClick(!(e.target as HTMLElement | null)?.closest?.('.row'))
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
          {panels.map((p, i) => (
            <Fragment key={p}>
              {i > 0 && <PanelGrip left={panels[i - 1]} right={p} />}
              <Panel panel={p} many={panels.length > 1} />
            </Fragment>
          ))}
        </main>
      </div>
      <Overlays />
      <DragHint />
      <Toast />
      <OfflineBanner />
    </div>
  )
}
