// Views of a node's items (spec 04): table, cards, board, list, side menu, tabs and calendar, with
// the view toolbar. Outline views stay in the outline (Outline.tsx). Everything here reads the
// engine's `view` snapshot and runs commands; nothing is stored here.

import { useEffect, useRef, useState, type ReactNode } from 'react'
import type { FieldRow, Json, NodeRow } from './engine'
import { Icon, type IconName } from './icons'
import { RichText, TagChip, tagStyle, FieldEditorCell } from './Outline'
import { exec } from './controller'
import { openInNewPanel, setActivePanel, setState, useStore } from './store'
import { openPicker, startFieldEdit } from './picker'

export interface ViewItem extends NodeRow {
  cells: Record<string, string[]>
  line: string
  dates: string[]
  time: [number, number] | null
}

export interface ViewColumn {
  id: number
  name: string
  type: string
  width: number | null
  calc: string | null
  calcValue: string | null
}

export interface ViewSnap {
  node: number
  kind: string
  toolbar: boolean
  groups: { label: string; count: number; items: ViewItem[] }[]
  grouped: boolean
  columns: ViewColumn[]
  titleWidth: number | null
  sorted: boolean
  filtered: boolean
  filteredOut: number
  dateField: { id: number; name: string } | null
  calendar: { year: number; month: number; mode: 'month' | 'week' | 'day'; today: string; weekStart: string; weekdays: string[] }
  selected: number | null
  tabs: { id: number; name: string }[]
  query: { tree: QueryGroup; text: string; held: boolean } | null
  refs: Record<string, string>
}

export interface QueryGroup {
  any: boolean
  clauses: { not: boolean; text?: string; group?: QueryGroup }[]
}

const KINDS: { kind: string; label: string; icon: IconName }[] = [
  { kind: 'outline', label: 'Outline', icon: 'list' },
  { kind: 'table', label: 'Table', icon: 'table' },
  { kind: 'cards', label: 'Cards', icon: 'lanes' },
  { kind: 'list', label: 'List', icon: 'page' },
  { kind: 'side_menu', label: 'Side menu', icon: 'sideMenu' },
  { kind: 'tabs', label: 'Tabs', icon: 'tabsView' },
  { kind: 'calendar', label: 'Calendar', icon: 'calendarView' },
]

const FIELD_ICON: Record<string, IconName> = { date: 'fieldDate', checkbox: 'fieldCheck', url: 'fieldUrl', options: 'fieldOptions', options_from_tag: 'fieldInstance', number: 'fieldNumber', user: 'fieldUser', email: 'fieldEmail', plain: 'fieldText' }

// ---- a small popover menu ---------------------------------------------------------------------

export function MenuButton({ icon, label, title, children, active }: { icon?: IconName; label?: string; title: string; children: (close: () => void) => ReactNode; active?: boolean }) {
  const [open, setOpen] = useState(false)
  const ref = useRef<HTMLSpanElement>(null)
  useEffect(() => {
    if (!open) return
    const off = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false)
    }
    window.addEventListener('mousedown', off)
    return () => window.removeEventListener('mousedown', off)
  }, [open])
  return (
    <span className="menu-anchor" ref={ref}>
      <button className={`tool${active ? ' active' : ''}${label ? ' labelled' : ''}`} title={title} onClick={() => setOpen((o) => !o)}>
        {icon && <Icon name={icon} size={14} />}
        {label}
      </button>
      {open && <div className="menu pop">{children(() => setOpen(false))}</div>}
    </span>
  )
}

function Item({ children, onPick, checked, icon }: { children: ReactNode; onPick: () => void; checked?: boolean; icon?: IconName }) {
  return (
    <div className={`menu-item${checked ? ' checked' : ''}`} onMouseDown={(e) => (e.preventDefault(), onPick())}>
      <span className="menu-icon">{checked ? <Icon name="check" size={12} /> : icon ? <Icon name={icon} size={12} /> : null}</span>
      <span className="grow">{children}</span>
    </div>
  )
}

// ---- toolbar -----------------------------------------------------------------------------------

function Toolbar({ v, panel }: { v: ViewSnap; panel: string }) {
  const run = (c: string, p: Json) => void exec(c, { node: v.node, ...p }, panel)
  const qOpen = useStore((s) => !!s.queryOpen[panel])
  return (
    <div className="view-toolbar">
      {v.query && (
        <button className={`tool${qOpen ? ' active' : ''}`} title="Search conditions (query builder)" onClick={() => void toggleQuery(panel, v.node, !qOpen)}>
          <Icon name="search" size={14} />
        </button>
      )}
      <MenuButton icon={KINDS.find((k) => k.kind === v.kind)?.icon ?? 'list'} title="View as">
        {(close) =>
          KINDS.map((k) => (
            <Item key={k.kind} icon={k.icon} checked={k.kind === v.kind} onPick={() => (close(), run('view.set', { kind: k.kind }))}>
              {k.label}
            </Item>
          ))
        }
      </MenuButton>
      <MenuButton icon="sliders" title="Display fields">
        {(close) =>
          v.columns.length ? (
            v.columns.map((c) => (
              <Item key={c.id} checked onPick={() => (close(), run('view.toggleDisplay', { field: c.id }))}>
                {c.name}
              </Item>
            ))
          ) : (
            <div className="menu-empty">No fields on these items yet</div>
          )
        }
      </MenuButton>
      <MenuButton icon="sort" title="Sort by" active={v.sorted}>
        {(close) => (
          <>
            {[['name', 'Name'], ['created', 'Created time'], ['edited', 'Last edited time'], ['done', 'Done']].map(([by, label]) => (
              <Item key={by} onPick={() => (close(), run('view.sort', { by }))}>
                {label}
              </Item>
            ))}
            {v.columns.map((c) => (
              <Item key={c.id} onPick={() => (close(), run('view.sort', { by: String(c.id) }))}>
                {c.name}
              </Item>
            ))}
            <div className="menu-sep" />
            <Item onPick={() => (close(), run('view.sort', { by: 'none' }))}>No sorting</Item>
          </>
        )}
      </MenuButton>
      <MenuButton icon="group" title="Group by" active={v.grouped}>
        {(close) => (
          <>
            {[['tags', 'Tags'], ['done', 'Done'], ['owner', 'Owner node'], ['created', 'Created day']].map(([by, label]) => (
              <Item key={by} onPick={() => (close(), run('view.group', { by }))}>
                {label}
              </Item>
            ))}
            {v.columns.map((c) => (
              <Item key={c.id} onPick={() => (close(), run('view.group', { by: String(c.id) }))}>
                {c.name}
              </Item>
            ))}
            <div className="menu-sep" />
            <Item onPick={() => (close(), run('view.group', { by: 'none' }))}>No grouping</Item>
          </>
        )}
      </MenuButton>
      <MenuButton icon="funnel" title="Filter" active={v.filtered}>
        {(close) => (
          <>
            <Item onPick={() => (close(), run('view.filter', { query: 'NOT DONE' }))}>Not done</Item>
            <Item onPick={() => (close(), run('view.filter', { query: 'DONE' }))}>Done</Item>
            <div className="menu-sep" />
            <Item onPick={() => (close(), run('view.filter', { query: '' }))}>No filter</Item>
          </>
        )}
      </MenuButton>
      {v.filteredOut > 0 && <span className="tool-note">{v.filteredOut} filtered out</span>}
    </div>
  )
}

// ---- query builder (spec 05 §2) ------------------------------------------------------------------

/** Opens or closes the conditions of a search node. While open, results wait for "Run once". */
export async function toggleQuery(panel: string, node: number, open: boolean) {
  setState((s) => ({ queryOpen: { ...s.queryOpen, [panel]: open } }))
  await exec('search.hold', { node, on: open }, panel)
}

function GroupBox({ g, node, panel, path, not }: { g: QueryGroup; node: number; panel: string; path: number[]; not?: boolean }) {
  const [text, setText] = useState('')
  const add = () => {
    if (!text.trim()) return
    void exec('search.addClause', { node, clause: text.trim(), path }, panel)
    setText('')
  }
  return (
    <div className="q-group">
      <button className="q-rail" title="Match all or any" onClick={() => void exec('search.setMode', { node, any: !g.any, path }, panel)}>
        {not ? 'NOT' : g.any ? 'OR' : 'AND'}
      </button>
      <div className="q-body">
        {g.clauses.map((c, i) =>
          c.group ? (
            <div key={i} className="q-nested">
              <GroupBox g={c.group} node={node} panel={panel} path={[...path, i]} not={c.not} />
              <button className="q-x" title="Remove" onClick={() => void exec('search.removeClause', { node, index: i, path }, panel)}>
                <Icon name="close" size={10} />
              </button>
            </div>
          ) : (
            <div key={i} className="q-clause">
              <span className="bullet">
                <span className="dot" />
              </span>
              <span className={`q-text${c.not ? ' not' : ''}`}>{c.not ? `NOT ${c.text}` : c.text}</span>
              <button className="ghost-btn" title="Negate" onClick={() => void exec('search.toggleNot', { node, index: i, path }, panel)}>
                NOT
              </button>
              <button className="q-x" title="Remove" onClick={() => void exec('search.removeClause', { node, index: i, path }, panel)}>
                <Icon name="close" size={10} />
              </button>
            </div>
          ),
        )}
        <input
          className="q-input"
          placeholder="Add a condition: #tag, DONE, Field:value, text"
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            e.stopPropagation()
            if (e.key === 'Enter') add()
          }}
        />
        <div className="q-add">
          {(['and', 'or', 'not'] as const).map((k) => (
            <button key={k} className="btn" onClick={() => void exec('search.addGroup', { node, kind: k, path }, panel)}>
              + {k.toUpperCase()}
            </button>
          ))}
        </div>
      </div>
    </div>
  )
}

export function QueryBuilder({ v, panel }: { v: ViewSnap; panel: string }) {
  const open = useStore((s) => !!s.queryOpen[panel])
  if (!v.query || !open) return null
  return (
    <div className="query-builder">
      <div className="q-head">
        <span className="q-title">Query builder</span>
        <span className="muted">Results wait until you run the query.</span>
      </div>
      <GroupBox g={v.query.tree} node={v.node} panel={panel} path={[]} />
      <div className="q-foot">
        <span className="q-expr">{v.query.text || 'No conditions yet'}</span>
        <span className="spacer" />
        <button className="btn regular" onClick={() => void exec('search.hold', { node: v.node, on: true }, panel)}>
          <Icon name="play" size={12} /> Run once
        </button>
        <button className="btn regular confirm" onClick={() => void toggleQuery(panel, v.node, false)}>
          <Icon name="checkCircle" size={12} /> Done
        </button>
      </div>
    </div>
  )
}

// ---- pieces shared by the views ----------------------------------------------------------------

function ItemTitle({ it, panel, refs, chips = true, compact = false }: { it: ViewItem; panel: string; refs: Record<string, string>; chips?: boolean; compact?: boolean }) {
  return (
    <span className="item-title">
      {it.done !== null && (
        <span className={`checkbox${it.done ? ' on' : ''}${compact ? ' compact' : ''}`} onMouseDown={(e) => e.preventDefault()} onClick={() => void exec('node.toggleDone', { node: it.target }, panel)}>
          {it.done && <Icon name="check" size={11} />}
        </span>
      )}
      <span className={`item-name${it.done ? ' done' : ''}`} onClick={() => void exec('nav.open', { node: it.target }, panel)}>
        {it.name ? <RichText text={it.name} refs={refs} /> : <span className="faint">Untitled</span>}
      </span>
      {chips &&
        it.tags.map((t) =>
          compact ? (
            <span key={t.id} className="chip mini" style={tagStyle(t.color)}>
              #
            </span>
          ) : (
            <TagChip key={t.id} chip={t} />
          ),
        )}
    </span>
  )
}

function ItemBullet({ it, panel }: { it: ViewItem; panel: string }) {
  const tag = it.tags.find((t) => t.name !== 'day')
  return (
    <span className={`bullet${it.hasKids ? ' closed' : ''}${tag ? ' tagged' : ''}`} style={tag ? tagStyle(tag.color) : undefined} onClick={() => void exec('nav.open', { node: it.target }, panel)}>
      <span className="dot" />
    </span>
  )
}

function AddItem({ v, panel, preset, label = '+ Add' }: { v: ViewSnap; panel: string; preset?: { field: number; value: string }; label?: string }) {
  return (
    <button
      className="ghost-btn"
      onClick={() =>
        void exec('view.addItem', { node: v.node, name: '', stay: false, ...(preset ? { field: preset.field, value: preset.value } : {}) }, panel)
      }
    >
      {label}
    </button>
  )
}

// ---- table -------------------------------------------------------------------------------------

function Table({ v, panel }: { v: ViewSnap; panel: string }) {
  const fe = useStore((s) => s.fieldEdit)
  const tmpl = `minmax(${v.titleWidth ?? 260}px, 2fr) ${v.columns.map((c) => `${c.width ?? 160}px`).join(' ')} 80px`
  return (
    <div className="table" role="table">
      <div className="t-row t-head" style={{ gridTemplateColumns: tmpl }} role="row">
        <span className="t-cell" role="columnheader">
          Name
        </span>
        {v.columns.map((c) => (
          <span key={c.id} className="t-cell" role="columnheader" title={c.name}>
            <Icon name={FIELD_ICON[c.type] ?? 'fieldText'} size={12} />
            <span className="t-col-name" onClick={() => void exec('nav.open', { node: c.id }, panel)}>
              {c.name}
            </span>
          </span>
        ))}
        <span className="t-cell">
          <button
            className="ghost-btn"
            onClick={(e) => {
              const r = e.currentTarget.getBoundingClientRect()
              openPicker({ mode: 'column', panel, node: v.node, at: -1, x: r.left, y: r.bottom })
            }}
          >
            + Add
          </button>
        </span>
      </div>
      {v.groups.map((g) => (
        <div key={g.label} className="t-group">
          {v.grouped && <div className="group-head">{g.label || 'No value'}</div>}
          {g.items.map((it) => (
            <div key={it.id} className="t-row" style={{ gridTemplateColumns: tmpl }} role="row" data-id={it.id}>
              <span className="t-cell t-title">
                <ItemBullet it={it} panel={panel} />
                <ItemTitle it={it} panel={panel} refs={v.refs} compact />
              </span>
              {v.columns.map((c) => {
                const editing = fe && fe.owner === it.target && fe.field === c.id
                const vals = it.cells[String(c.id)] ?? []
                return (
                  <span
                    key={c.id}
                    className="t-cell"
                    onMouseDown={(e) => {
                      if (editing) return
                      e.preventDefault()
                      setActivePanel(panel)
                      void startFieldEdit(panel, it.target, c.id, 0)
                    }}
                  >
                    {editing ? (
                      <FieldEditorCell
                        row={{ k: 'field', key: '', owner: it.target, field: c.id, depth: 0, name: c.name, type: c.type, values: [], missing: false, invalid: false, boxed: false } as FieldRow}
                        panel={panel}
                        index={fe.index}
                        initial={vals[fe.index] ?? ''}
                      />
                    ) : vals.length ? (
                      vals.join(', ')
                    ) : (
                      <span className="faint-dot" />
                    )}
                  </span>
                )
              })}
              <span className="t-cell" />
            </div>
          ))}
        </div>
      ))}
      {v.columns.some((c) => c.calcValue) && (
        <div className="t-row t-foot" style={{ gridTemplateColumns: tmpl }}>
          <span className="t-cell" />
          {v.columns.map((c) => (
            <span key={c.id} className="t-cell">
              {c.calcValue ?? ''}
            </span>
          ))}
          <span className="t-cell" />
        </div>
      )}
      <div className="t-add">
        <AddItem v={v} panel={panel} label="+ New item" />
      </div>
    </div>
  )
}

// ---- cards and board ---------------------------------------------------------------------------

function Card({ it, panel, refs, onDragStart }: { it: ViewItem; panel: string; refs: Record<string, string>; onDragStart?: (e: React.DragEvent) => void }) {
  return (
    <div className="card" draggable={!!onDragStart} onDragStart={onDragStart} data-id={it.id}>
      <div className="card-head">
        <ItemBullet it={it} panel={panel} />
        {it.tags.map((t) => (
          <TagChip key={t.id} chip={t} />
        ))}
      </div>
      <div className="card-body">
        <ItemTitle it={it} panel={panel} refs={refs} chips={false} />
      </div>
      {it.line && <div className="card-foot">{it.line}</div>}
    </div>
  )
}

function Board({ v, panel }: { v: ViewSnap; panel: string }) {
  const [over, setOver] = useState<string | null>(null)
  const groupField = v.columns.find((c) => v.groups.some((g) => g.items.some((it) => (it.cells[String(c.id)] ?? []).includes(g.label))))
  if (!v.grouped) {
    return (
      <div className="cards">
        <div className="cards-add">
          <AddItem v={v} panel={panel} />
        </div>
        <div className="card-grid">
          {v.groups.flatMap((g) => g.items).map((it) => (
            <Card key={it.id} it={it} panel={panel} refs={v.refs} />
          ))}
        </div>
      </div>
    )
  }
  return (
    <div className="board">
      {v.groups.map((g) => (
        <div
          key={g.label}
          className={`board-col${over === g.label ? ' over' : ''}`}
          onDragOver={(e) => {
            e.preventDefault()
            setOver(g.label)
          }}
          onDragLeave={() => setOver(null)}
          onDrop={(e) => {
            e.preventDefault()
            setOver(null)
            const id = Number(e.dataTransfer.getData('text/x-node'))
            // Dropping a card on a column gives it that column's value (spec 04 §5).
            if (id && groupField) void exec('field.set', { node: id, field: groupField.id, value: g.label === 'Not defined' ? '' : g.label }, panel)
          }}
        >
          <div className="board-head">
            <span className="board-name">{g.label}</span>
            <AddItem v={v} panel={panel} preset={groupField ? { field: groupField.id, value: g.label } : undefined} />
          </div>
          {g.items.map((it) => (
            <Card key={it.id} it={it} panel={panel} refs={v.refs} onDragStart={(e) => e.dataTransfer.setData('text/x-node', String(it.target))} />
          ))}
        </div>
      ))}
    </div>
  )
}

// ---- list, side menu, tabs (navigation views) --------------------------------------------------

function Navigation({ v, panel }: { v: ViewSnap; panel: string }) {
  const items = v.groups.flatMap((g) => g.items)
  const sel = items.find((i) => i.target === v.selected) ?? items[0]
  return (
    <div className={`nav-view ${v.kind}`}>
      <div className="nav-items">
        {items.map((it) => (
          <div
            key={it.id}
            className={`nav-item${sel?.id === it.id ? ' active' : ''}`}
            onClick={() => void openInNewPanel(it.target)}
            title="Open beside"
          >
            <ItemBullet it={it} panel={panel} />
            <ItemTitle it={it} panel={panel} refs={v.refs} chips={v.kind !== 'tabs'} />
          </div>
        ))}
        <div className="nav-add">
          <AddItem v={v} panel={panel} />
        </div>
      </div>
    </div>
  )
}

// ---- calendar ----------------------------------------------------------------------------------

const MONTHS = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December']

function addDays(iso: string, n: number): string {
  const d = new Date(`${iso}T00:00:00Z`)
  d.setUTCDate(d.getUTCDate() + n)
  return d.toISOString().slice(0, 10)
}

function Calendar({ v, panel }: { v: ViewSnap; panel: string }) {
  const c = v.calendar
  const items = v.groups.flatMap((g) => g.items)
  const byDay = new Map<string, ViewItem[]>()
  for (const it of items) for (const d of it.dates) byDay.set(d, [...(byDay.get(d) ?? []), it])
  const undated = items.filter((it) => it.dates.length === 0)
  const first = `${c.year}-${String(c.month).padStart(2, '0')}-01`
  // The grid starts on the first day of the week that holds the 1st.
  const dow = (new Date(`${first}T00:00:00Z`).getUTCDay() + 6) % 7
  const sundayFirst = c.weekdays[0]?.startsWith('Sun')
  const start = addDays(first, -(sundayFirst ? (dow + 1) % 7 : dow))
  const weeks = c.mode === 'week' ? 1 : 6
  const gridStart = c.mode === 'week' ? c.weekStart : start
  const days = Array.from({ length: weeks * 7 }, (_, i) => addDays(gridStart, i))
  const title = c.mode === 'week' ? `Week of ${c.weekStart}` : `${MONTHS[c.month - 1]} ${c.year}`
  return (
    <div className="calendar">
      <div className="cal-head">
        <span className="cal-title">{title}</span>
        <span className="spacer" />
        <span className="segmented">
          {(['day', 'week', 'month'] as const).map((m) => (
            <button key={m} className={`seg${c.mode === m ? ' on' : ''}`} onClick={() => void exec('view.calendarMode', { mode: m }, panel)}>
              {m[0].toUpperCase() + m.slice(1)}
            </button>
          ))}
        </span>
        <button className="icon-btn" title="Previous" onClick={() => void exec('view.month', { step: -1 }, panel)}>
          <Icon name="chevronLeft" size={14} />
        </button>
        <button className="icon-btn" title="This month" onClick={() => void exec('view.month', {}, panel)}>
          <Icon name="calendar" size={14} />
        </button>
        <button className="icon-btn" title="Next" onClick={() => void exec('view.month', { step: 1 }, panel)}>
          <Icon name="chevronRight" size={14} />
        </button>
      </div>
      <div className="cal-grid">
        {c.weekdays.map((d) => (
          <div key={d} className="cal-dow">
            {d}
          </div>
        ))}
        {days.map((d, i) => {
          const day = Number(d.slice(8))
          const out = c.mode === 'month' && Number(d.slice(5, 7)) !== c.month
          return (
            <div key={d} className={`cal-cell${out ? ' out' : ''}`}>
              {i % 7 === 0 && <span className="cal-week">W{isoWeek(d)}</span>}
              <span className={`cal-day${d === c.today ? ' today' : ''}`} onClick={() => void exec('nav.day', { date: d }, panel)}>
                {day}
              </span>
              {(byDay.get(d) ?? []).slice(0, 4).map((it) => {
                const tag = it.tags.find((t) => t.name !== 'day')
                return (
                  <div key={it.id} className="cal-event" style={tag ? tagStyle(tag.color) : undefined} onClick={() => void exec('nav.open', { node: it.target }, panel)}>
                    {it.done !== null && <span className={`checkbox compact${it.done ? ' on' : ''}`}>{it.done && <Icon name="check" size={9} />}</span>}
                    <span className="cal-event-name">{it.name || 'Untitled'}</span>
                  </div>
                )
              })}
            </div>
          )
        })}
      </div>
      {undated.length > 0 && <div className="cal-undated">{undated.length} nodes with no dates ›</div>}
    </div>
  )
}

function isoWeek(iso: string): number {
  const d = new Date(`${iso}T00:00:00Z`)
  const day = (d.getUTCDay() + 6) % 7
  d.setUTCDate(d.getUTCDate() - day + 3)
  const firstThu = new Date(Date.UTC(d.getUTCFullYear(), 0, 4))
  return 1 + Math.round(((d.getTime() - firstThu.getTime()) / 86400000 - 3 + ((firstThu.getUTCDay() + 6) % 7)) / 7)
}

// ---- the view body -----------------------------------------------------------------------------

export function ViewBody({ v, panel, showToolbar = true }: { v: ViewSnap; panel: string; showToolbar?: boolean }) {
  return (
    <div className="view-body" data-view={v.kind}>
      <QueryBuilder v={v} panel={panel} />
      {showToolbar && v.toolbar && <Toolbar v={v} panel={panel} />}
      {v.kind === 'table' && <Table v={v} panel={panel} />}
      {(v.kind === 'cards' || v.kind === 'lanes') && <Board v={v} panel={panel} />}
      {(v.kind === 'list' || v.kind === 'side_menu' || v.kind === 'tabs') && <Navigation v={v} panel={panel} />}
      {v.kind === 'calendar' && <Calendar v={v} panel={panel} />}
    </div>
  )
}

export { Toolbar as ViewToolbar }
