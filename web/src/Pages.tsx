// Pages that are not plain outlines: the Supertags overview (spec 03 §1), the header of a tag page
// (03 §2), and the Settings, Recents and Help pages (07 §8, 09).

import { useEffect, useState } from 'react'
import { call, type Json } from './engine'
import { Icon } from './icons'
import { tagStyle } from './Outline'
import { exec } from './controller'
import { UI_COMMANDS, keyInfo, shortcut } from './keys'
import { refresh, setState, useStore } from './store'
import { SyncSettings } from './Sync'

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
  const configuring = useStore((s) => !!s.configure[panel])
  // A view tab switches the view in place; the tag page stays open (N36).
  const showTab = (id: number | undefined) => {
    setState((s) => ({ tagView: { ...s.tagView, [panel]: id } }))
    void refresh()
  }
  return (
    <>
      <div className="tag-glow" style={tagStyle(color)} />
      {/* The configure panel shows the tag's settings only, not its views (spec 03 §5); its
          panel's close button closes it (spec 12 has no Done). */}
      {!configuring && (
        <div className="tag-tabs">
          <button className={`view-tab${active === tag ? ' active' : ''}`} onClick={() => showTab(undefined)}>
            Everything tagged
          </button>
          {tabs.map((t) => (
            <button key={t.id} className={`view-tab${active === t.id ? ' active' : ''}`} onClick={() => showTab(t.id)}>
              {t.name}
            </button>
          ))}
          <button className="ghost-btn" onClick={() => void exec('tag.addView', { tag, kind: 'table' }, panel)}>
            + Add view
          </button>
        </div>
      )}
    </>
  )
}

// ---- settings ----------------------------------------------------------------------------------

// The sections in Tana's order (DIFFS N23, N55), then Keybindings (ours).
const SECTIONS = ['Preferences', 'Notifications seen', 'Progress stats', 'Date and time', 'Move targets', 'Journaling', 'API keys', 'Sync', 'Keybindings']
/** Under each section heading (N67). */
const SUBTITLE: Record<string, string> = {
  Preferences: 'Your user preferences',
  'Date and time': 'How dates and times read',
  Keybindings: 'Your own keys for commands',
  Sync: 'Your notes on your other devices, end-to-end encrypted',
}
const EMPTY_NOTE: Record<string, string> = {
  'Notifications seen': 'No notifications yet.',
  'Progress stats': 'Nothing to count yet.',
  'Move targets': 'Nodes you move often will show here.',
  Journaling: 'Daily notes are on. Nothing else to set yet.',
  'API keys': 'No keys. Agents use the local command line or MCP server instead.',
}

/** A setting's switch: an 18 px checkbox on its own line (12: Settings). */
function Check({ on, onChange, label, disabled }: { on: boolean; onChange: (v: boolean) => void; label: string; disabled?: boolean }) {
  return (
    <button className={`checkbox setting-check${on ? ' on' : ''}`} role="switch" aria-checked={on} aria-label={label} disabled={disabled} onClick={() => onChange(!on)}>
      {on && <Icon name="check" size={12} />}
    </button>
  )
}

export function SettingsPage({ panel }: { panel: string }) {
  const [s, setS] = useState<Json>(null)
  const wanted = useStore((st) => st.settingsTab)
  const [tab, setTab] = useState(0)
  // The sync button opens this page at Sync.
  useEffect(() => {
    if (wanted && SECTIONS.includes(wanted)) {
      setTab(SECTIONS.indexOf(wanted))
      setState({ settingsTab: null })
    }
  }, [wanted])
  const theme = useStore((st) => st.theme)
  const followSystem = useStore((st) => st.followSystem)
  const load = () => void call('run', { command: 'settings.get', params: {}, panel }).then((r) => setS(r?.result))
  useEffect(load, [panel])
  const set = (key: string, value: unknown) => void exec('settings.set', { key, value }, panel).then(load)
  if (!s) return null
  const row = (label: string, help: string, control: React.ReactNode) => (
    <div className="setting">
      <span className="setting-icon">
        <Icon name="fieldCheck" size={12} />
      </span>
      <div className="setting-label">{label}</div>
      <div className="setting-help">{help}</div>
      <div className="setting-control">{control}</div>
    </div>
  )
  const name = SECTIONS[tab]
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
        <h2 className="section-title">{name}</h2>
        {SUBTITLE[name] && <p className="settings-sub">{SUBTITLE[name]}</p>}
        {EMPTY_NOTE[name] && <p className="muted">{EMPTY_NOTE[name]}</p>}
        {name === 'Preferences' && (
          <>
            {row('Use system theme', 'Follow the light or dark appearance of the system.', <Check label="Use system theme" on={followSystem} onChange={(v) => setState({ followSystem: v })} />)}
            {row('Dark theme', 'Switch between the dark and the light look.', <Check label="Dark theme" disabled={followSystem} on={theme === 'dark'} onChange={(v) => setState({ theme: v ? 'dark' : 'light' })} />)}
            {row('Warn before editing a referenced node', 'The first edit of a node used in other places says so.', <Check label="Warn" on={!!s.warn_referenced} onChange={(v) => set('warn_referenced', v)} />)}
            {row('Link previews', 'A line that is just a link shows a card under it.', <Check label="Link previews" on={!!s.url_embeds} onChange={(v) => set('url_embeds', v)} />)}
            {row('Reference counter', 'A small number beside lines that are used in other places.', <Check label="Reference counter" on={!!s.ref_counter} onChange={(v) => set('ref_counter', v)} />)}
            {[
              ['show_daily_notes', 'Daily notes in the sidebar'],
              ['show_supertags', 'Supertags in the sidebar'],
              ['show_recents', 'Recents in the sidebar'],
              ['show_help', 'Help button'],
              ['show_shortcuts', 'Keyboard shortcuts button'],
            ].map(([k, l]) => (
              <div key={k}>{row(l, 'Shown in the sidebar or top bar.', <Check label={l} on={!!s[k]} onChange={(v) => set(k, v)} />)}</div>
            ))}
          </>
        )}
        {name === 'Date and time' && (
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
        {name === 'Sync' && <SyncSettings />}
        {name === 'Keybindings' && (
          <>
            <CustomKeys panel={panel} />
            <ShortcutList />
          </>
        )}
      </div>
    </div>
  )
}

/** Custom keybindings (spec 09): press a shortcut, pick a command; saved with the graph. */
function CustomKeys({ panel }: { panel: string }) {
  const bound = useStore((s) => s.sidebar?.keybindings ?? NONE)
  const commands = useStore((s) => s.commands)
  const [key, setKey] = useState('')
  const [cmd, setCmd] = useState('')
  const all = [...UI_COMMANDS.map(([id, label]) => ({ id, label })), ...commands]
  const label = (id: string) => all.find((c) => c.id === id)?.label ?? id
  const add = () => {
    const id = all.find((c) => c.id === cmd || c.label.toLowerCase() === cmd.trim().toLowerCase())?.id
    if (!key || !id) return
    void exec('settings.setKeybinding', { shortcut: key, command: id }, panel).then(() => (setKey(''), setCmd('')))
  }
  return (
    <div className="custom-keys">
      <div className="setting-label">Your shortcuts</div>
      <div className="setting-help">Checked before the defaults below.</div>
      {Object.entries(bound).map(([k, id]) => (
        <div key={k} className="shortcut-row">
          <kbd>{k}</kbd>
          <span className="grow">{label(id)}</span>
          <button className="ghost-btn" aria-label={`Remove ${k}`} onClick={() => void exec('settings.setKeybinding', { shortcut: k }, panel)}>
            Remove
          </button>
        </div>
      ))}
      <div className="shortcut-row add">
        <input
          className="cfg-input key-capture"
          placeholder="Press a shortcut"
          value={key}
          readOnly
          onKeyDown={(e) => {
            if (['Shift', 'Meta', 'Control', 'Alt', 'Tab'].includes(e.key)) return
            e.preventDefault()
            setKey(e.key === 'Backspace' && !e.metaKey && !e.ctrlKey ? '' : shortcut(keyInfo(e)))
          }}
        />
        <input className="cfg-input grow" placeholder="Command" list="tc-commands" value={cmd} onChange={(e) => setCmd(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && add()} />
        <datalist id="tc-commands">
          {all.map((c) => (
            <option key={c.id} value={c.id}>
              {c.label}
            </option>
          ))}
        </datalist>
        <button className="btn regular" onClick={add}>
          Add
        </button>
      </div>
    </div>
  )
}

const NONE: Record<string, string> = {}

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
