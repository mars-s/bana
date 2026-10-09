// Pages that are not plain outlines: the Supertags overview (spec 03 §1), the header of a tag page
// (03 §2), and the Settings, Recents and Help pages (07 §8, 09).

import { useEffect, useState } from 'react'
import { call, type Json } from './engine'
import { Icon } from './icons'
import { tagStyle } from './Outline'
import { exec } from './controller'
import { UI_COMMANDS } from './keys'
import { setState, useStore } from './store'

interface TagInfo {
  id: number
  name: string
  color: string
  pinned: boolean
  workspace: number
  count: number
}

export function SupertagsPage({ panel }: { panel: string }) {
  const [data, setData] = useState<{ tags: TagInfo[]; workspaces: { id: number; name: string }[] } | null>(null)
  const [q, setQ] = useState('')
  const version = useStore((s) => s.pages[panel]?.version)
  useEffect(() => {
    void call('tags', {}).then(setData)
  }, [version])
  if (!data) return null
  const match = (t: TagInfo) => t.name.toLowerCase().includes(q.trim().toLowerCase())
  const pinned = data.tags.filter((t) => t.pinned && match(t))
  return (
    <div className="supertags">
      <input className="filter-input" autoFocus placeholder="Filter tags" value={q} onChange={(e) => setQ(e.target.value)} />
      {pinned.length > 0 && (
        <div className="tag-tiles">
          {pinned.map((t) => (
            <button key={t.id} className="tag-tile" style={tagStyle(t.color)} onClick={() => void exec('nav.open', { node: t.id }, panel)}>
              <span className="tag-tile-name">{t.name}</span>
              <span className="tag-square big">#</span>
            </button>
          ))}
        </div>
      )}
      {data.workspaces.map((w) => {
        const tags = data.tags.filter((t) => t.workspace === w.id && match(t))
        if (!tags.length) return null
        return (
          <section key={w.id}>
            <h2 className="section-title">{w.name}</h2>
            <div className="tag-rows">
              {tags.map((t) => (
                <button key={t.id} className="tag-row-tile" style={tagStyle(t.color)} onClick={() => void exec('nav.open', { node: t.id }, panel)}>
                  <span className="tag-square">#</span>
                  <span className="tag-row-name">{t.name}</span>
                </button>
              ))}
            </div>
          </section>
        )
      })}
    </div>
  )
}

/** The tag page's extra header: glow, "+ Create new", and its view tabs. */
export function TagHeader({ panel, tag, color, tabs, active }: { panel: string; tag: number; color: string; tabs: { id: number; name: string }[]; active: number }) {
  return (
    <>
      <div className="tag-glow" style={tagStyle(color)} />
      <div className="tag-tabs">
        <button className={`view-tab${active === tag ? ' active' : ''}`} onClick={() => void exec('nav.open', { node: tag }, panel)}>
          Everything tagged
        </button>
        {tabs.map((t) => (
          <button key={t.id} className={`view-tab${active === t.id ? ' active' : ''}`} onClick={() => void exec('nav.open', { node: t.id }, panel)}>
            {t.name}
          </button>
        ))}
        <button className="ghost-btn" onClick={() => void exec('tag.addView', { tag, kind: 'table' }, panel)}>
          + Add view
        </button>
        <span className="spacer" />
        <button className="btn regular" onClick={() => void exec('view.addItem', { node: tag, name: '' }, panel)}>
          <Icon name="plus" size={12} /> Create new
        </button>
      </div>
    </>
  )
}

// ---- settings ----------------------------------------------------------------------------------

const SECTIONS = ['Preferences', 'Date and time', 'Sidebar', 'Keyboard shortcuts']

function Toggle({ on, onChange, label }: { on: boolean; onChange: (v: boolean) => void; label: string }) {
  return (
    <button className={`toggle${on ? ' on' : ''}`} role="switch" aria-checked={on} aria-label={label} onClick={() => onChange(!on)}>
      <span className="knob" />
    </button>
  )
}

export function SettingsPage({ panel }: { panel: string }) {
  const [s, setS] = useState<Json>(null)
  const [tab, setTab] = useState(0)
  const theme = useStore((st) => st.theme)
  const load = () => void call('run', { command: 'settings.get', params: {}, panel }).then((r) => setS(r?.result))
  useEffect(load, [panel])
  const set = (key: string, value: unknown) => void exec('settings.set', { key, value }, panel).then(load)
  if (!s) return null
  const row = (label: string, help: string, control: React.ReactNode) => (
    <div className="setting">
      <div className="grow">
        <div className="setting-label">{label}</div>
        <div className="setting-help">{help}</div>
      </div>
      {control}
    </div>
  )
  return (
    <div className="settings">
      <nav className="settings-nav">
        {SECTIONS.map((x, i) => (
          <button key={x} className={`view-tab${tab === i ? ' active' : ''}`} onClick={() => setTab(i)}>
            {x}
          </button>
        ))}
      </nav>
      <div className="settings-body">
        <h2 className="section-title">{SECTIONS[tab]}</h2>
        {tab === 0 && (
          <>
            {row('Dark theme', 'Switch between the dark and the light look.', <Toggle label="Dark theme" on={theme === 'dark'} onChange={(v) => setState({ theme: v ? 'dark' : 'light' })} />)}
            {row('Warn before editing a referenced node', 'The first edit of a node used in other places asks first.', <Toggle label="Warn" on={!!s.warn_referenced} onChange={(v) => set('warn_referenced', v)} />)}
            {row('Link previews', 'A line that is just a link shows a card under it.', <Toggle label="Link previews" on={!!s.url_embeds} onChange={(v) => set('url_embeds', v)} />)}
          </>
        )}
        {tab === 1 && (
          <>
            {row('Time format', 'How times read across the app.', (
              <span className="segmented">
                {['24h', '12h'].map((v) => (
                  <button key={v} className={`seg${s.time_format === v ? ' on' : ''}`} onClick={() => set('time_format', v)}>
                    {v}
                  </button>
                ))}
              </span>
            ))}
            {row('First day of the week', 'Calendars and week pages start here.', (
              <span className="segmented">
                {['monday', 'sunday'].map((v) => (
                  <button key={v} className={`seg${s.week_start === v ? ' on' : ''}`} onClick={() => set('week_start', v)}>
                    {v[0].toUpperCase() + v.slice(1)}
                  </button>
                ))}
              </span>
            ))}
          </>
        )}
        {tab === 2 &&
          [
            ['show_daily_notes', 'Daily notes'],
            ['show_supertags', 'Supertags'],
            ['show_recents', 'Recents'],
            ['show_help', 'Help button'],
            ['show_shortcuts', 'Keyboard shortcuts button'],
          ].map(([k, l]) => <div key={k}>{row(l, 'Shown in the sidebar or top bar.', <Toggle label={l} on={!!s[k]} onChange={(v) => set(k, v)} />)}</div>)}
        {tab === 3 && <ShortcutList />}
      </div>
    </div>
  )
}

function ShortcutList() {
  const commands = useStore((s) => s.commands)
  const rows = [...UI_COMMANDS.map(([id, label, shortcut]) => ({ id, label, shortcut })), ...commands].filter((c) => c.shortcut)
  return (
    <div className="shortcut-table">
      {rows.map((c) => (
        <div key={c.id} className="shortcut-row">
          <span className="grow">{c.label}</span>
          <kbd>{c.shortcut}</kbd>
        </div>
      ))}
    </div>
  )
}

export function RecentsPage({ panel }: { panel: string }) {
  const recents = useStore((s) => s.sidebar?.recents ?? [])
  return (
    <div className="recents">
      {recents.length === 0 && <p className="empty-note">Pages you open show up here.</p>}
      {recents.map((r) => (
        <div key={r.id} className="nav-item" onClick={() => void exec('nav.open', { node: r.id }, panel)}>
          <span className="bullet">
            <span className="dot" />
          </span>
          <span className="item-name">{r.name || 'Untitled'}</span>
        </div>
      ))}
    </div>
  )
}

export function HelpPage() {
  return (
    <div className="help-page">
      <p>Every action is a command. Press ⌘K for the command line, ⌘S to search and open, ⌘E for quick add.</p>
      <ShortcutList />
    </div>
  )
}
