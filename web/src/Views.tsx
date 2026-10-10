// Views of a node's items (spec 04): table, cards, board, list, side menu, tabs and calendar, with
// the view toolbar. Outline views stay in the outline (Outline.tsx). Everything here reads the
// engine's `view` snapshot and runs commands; nothing is stored here.

import { useEffect, useRef, useState, type ReactNode } from 'react'
import { call, type FieldRow, type Json, type NodeRow } from './engine'
import { Icon, type IconName } from './icons'
import { RichText, TagChip, tagStyle, FieldEditorCell } from './Outline'
import { exec, withBarrier } from './controller'
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
  sorts?: { by: string; label: string; desc: boolean }[]
  filters?: string[]
  system?: string[]
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

/** The view toolbar (spec 04, 12: Table; N48): quick text filter, view, display, sort, group,
 * active filter chips, filter. */
function Toolbar({ v, panel }: { v: ViewSnap; panel: string }) {
  const run = (c: string, p: Json) => void exec(c, { node: v.node, ...p }, panel)
  const [quick, setQuick] = useState<string | null>(null)
  const [tags, setTags] = useState<TagInfo[]>([])
  useEffect(() => {
    void call('run', { command: 'graph.tags', params: {} }).then((r) => setTags(Array.isArray(r?.result) ? r.result : []))
  }, [v.node])
  const sortChoices: [string, string][] = [['name', 'Name'], ['created', 'Created time'], ['edited', 'Last edited time'], ['done', 'Done'], ...v.columns.map((c): [string, string] => [String(c.id), c.name])]
  return (
    <div className="view-toolbar">
      {quick === null ? (
        <button className="tool" title="Filter by text" onClick={() => setQuick('')}>
          <Icon name="search" size={14} />
        </button>
      ) : (
        <input
          className="tool-search"
          autoFocus
          placeholder="Filter by text"
          value={quick}
          onChange={(e) => setQuick(e.target.value)}
          onBlur={() => !quick && setQuick(null)}
          onKeyDown={(e) => {
            e.stopPropagation()
            if (e.key === 'Enter' && quick.trim()) {
              run('view.addFilter', { clause: quick.trim() })
              setQuick(null)
            } else if (e.key === 'Escape') setQuick(null)
          }}
        />
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
        {(close) => (
          <>
            {v.columns.map((c) => (
              <Item key={c.id} checked onPick={() => (close(), run('view.toggleDisplay', { field: c.id }))}>
                {c.name}
              </Item>
            ))}
            <div className="menu-caps">System fields</div>
            {['Created time', 'Last edited time', 'Done time', 'Owner node', 'Tags', 'Number of references'].map((f) => (
              <Item key={f} checked={(v.system ?? []).includes(f)} onPick={() => (close(), run('view.toggleSystemField', { field: f }))}>
                {f}
              </Item>
            ))}
          </>
        )}
      </MenuButton>
      <MenuButton icon="sort" title="Sort by" active={v.sorted}>
        {(close) => (
          <>
            {(v.sorts ?? []).map((r, i) => (
              <div key={i} className="menu-item sort-rule">
                <span className="menu-icon" />
                <span className="grow">{r.label}</span>
                <button className="ghost-btn" onMouseDown={(e) => (e.preventDefault(), close(), void exec('view.removeSort', { node: v.node, index: i }, panel).then(() => run('view.addSort', { by: r.by, desc: !r.desc })))}>
                  {r.desc ? 'Descending' : 'Ascending'}
                </button>
                <button className="q-x" title="Remove" onMouseDown={(e) => (e.preventDefault(), close(), run('view.removeSort', { index: i }))}>
                  <Icon name="close" size={10} />
                </button>
              </div>
            ))}
            <SubMenu label="+ Add sort">
              {sortChoices.map(([by, label]) => (
                <Item key={by} onPick={() => (close(), run('view.addSort', { by }))}>
                  {label}
                </Item>
              ))}
            </SubMenu>
            {v.sorted && <Item onPick={() => (close(), run('view.sort', { by: 'none' }))}>Reset</Item>}
          </>
        )}
      </MenuButton>
      {v.kind !== 'table' && (
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
      )}
      {(v.filters ?? []).map((f, i) => (
        <span key={i} className="filter-chip">
          {f.replace(/^Search for nodes /, '')}
          <button title="Remove filter" onClick={() => run('view.removeFilter', { index: i })}>
            <Icon name="close" size={10} />
          </button>
        </span>
      ))}
      <MenuButton icon="funnel" title="Filter" active={v.filtered}>
        {(close) => (
          <>
            <Item onPick={() => (close(), run('view.addFilter', { clause: 'NOT DONE' }))}>Not done</Item>
            <Item onPick={() => (close(), run('view.addFilter', { clause: 'DONE' }))}>Done</Item>
            <SubMenu label="Tags">
              {tags.map((t) => (
                <Item key={t.id} onPick={() => (close(), run('view.addFilter', { clause: `#"${t.name}"` }))}>
                  {`#${t.name}`}
                </Item>
              ))}
            </SubMenu>
            {v.columns.map((c) => (
              <SubMenu key={c.id} label={c.name}>
                <Item onPick={() => (close(), run('view.addFilter', { clause: `"${c.name}":*` }))}>Set</Item>
                <Item onPick={() => (close(), run('view.addFilter', { clause: `"${c.name}":NOTSET` }))}>Not set</Item>
              </SubMenu>
            ))}
            {v.filtered && (
              <>
                <div className="menu-sep" />
                <Item onPick={() => (close(), run('view.filter', { query: '' }))}>Clear filters</Item>
              </>
            )}
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
          data-path={path.join('.')}
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
            <button
              key={k}
              className="btn"
              onClick={() =>
                // The new group's first condition is typed next, in the group (N35).
                void exec('search.addGroup', { node, kind: k, path }, panel).then(() =>
                  requestAnimationFrame(() => document.querySelector<HTMLInputElement>(`.q-input[data-path="${[...path, g.clauses.length].join('.')}"]`)?.focus()),
                )
              }
            >
              + {k.toUpperCase()}
            </button>
          ))}
        </div>
      </div>
    </div>
  )
}

const OPERATORS: [string, string][] = [
  ['TODO', 'TODO'], ['DONE', 'DONE'], ['NOT DONE', 'NOT DONE'], ['DONE LAST 7 DAYS', 'DONE:7'], ['CREATED LAST 7 DAYS', 'CREATED:7'], ['EDITED LAST 7 DAYS', 'EDITED:7'],
  ['HAS TAG', 'HAS:TAG'], ['HAS FIELD', 'HAS:FIELD'], ['IS TAG', 'IS:TAG'], ['IS FIELD', 'IS:FIELD'], ['IS SEARCH NODE', 'IS:SEARCH'], ['IS CALENDAR NODE', 'IS:CALENDAR'],
  ['IS COMMAND', 'IS:COMMAND'], ['PARENTS DESCENDANTS', 'SCOPE:PARENT'], ['GRANDPARENTS DESCENDANTS', 'SCOPE:GRANDPARENT'], ['IN LIBRARY', 'IN:LIBRARY'], ['ON DAY NODE', 'ON:DAY'], ['FOR DATE today', 'DATE:today'],
]
const SYSTEM: [string, string][] = [['Created time: last 7 days', 'CREATED:7'], ['Last edited time: last 7 days', 'EDITED:7'], ['Done time: last 7 days', 'DONE:7'], ['Tags: any', 'HAS:TAG'], ['Date from calendar node: today', 'DATE:today']]
const VALUES: [string, string][] = [['PARENT', 'PARENT'], ['GRANDPARENT', 'GRANDPARENT'], ['FOR RELATIVE DATE: today', 'today'], ['FOR RELATIVE DATE: yesterday', 'yesterday'], ['FOR RELATIVE DATE: tomorrow', 'tomorrow'], ['Set', 'SET'], ['Not Set', 'NOTSET'], ['Defined', 'DEFINED'], ['Not Defined', 'UNDEFINED']]
const BASE_TYPES = ['task', 'meeting', 'person', 'organization', 'location', 'project', 'topic', 'event', 'article', 'memo', 'reflection', 'day', 'week']

interface TagInfo {
  id: number
  name: string
  fields: { id: number; name: string }[]
}

/** The add bar's menus (spec 05 §2): supertag, tag fields, field values, system fields, tag types,
 * search operators, and the workspace when there are several (N43). */
function QueryMenus({ v, panel }: { v: ViewSnap; panel: string }) {
  const [tags, setTags] = useState<TagInfo[]>([])
  const workspaces = useStore((s) => s.sidebar?.workspaces ?? [])
  useEffect(() => {
    void call('run', { command: 'graph.tags', params: {} }).then((r) => setTags(Array.isArray(r?.result) ? r.result : []))
  }, [v.query?.text])
  const add = (clause: string, not = false) => void exec('search.addClause', { node: v.node, clause, ...(not ? { not } : {}) }, panel)
  const text = v.query?.text.toLowerCase() ?? ''
  const inQuery = tags.filter((t) => text.includes(`#${t.name.toLowerCase()}`) || text.includes(`tagged:${t.name.toLowerCase()}`))
  const tagFields = inQuery.flatMap((t) => t.fields)
  const valueFields = tagFields.length ? tagFields : tags.flatMap((t) => t.fields)
  return (
    <div className="q-menus">
      <button
        className="btn"
        onClick={(e) => {
          const r = e.currentTarget.getBoundingClientRect()
          openPicker({ mode: 'searchTag', panel, node: v.node, at: -1, x: r.left, y: r.bottom + 2 })
        }}
      >
        + Supertag
      </button>
      <MenuButton label="Tag fields ▾" title="Tag fields">
        {(close) =>
          tagFields.length ? (
            tagFields.map((f) => (
              <SubMenu key={f.id} label={f.name}>
                <Item onPick={() => (close(), add(`"${f.name}":*`))}>Set</Item>
                <Item onPick={() => (close(), add(`"${f.name}":*`, true))}>Not set</Item>
              </SubMenu>
            ))
          ) : (
            <p className="muted menu-note">Add a supertag condition first</p>
          )
        }
      </MenuButton>
      <MenuButton label="Field values ▾" title="Field values">
        {(close) =>
          [...new Map(valueFields.map((f) => [f.name, f])).values()].map((f) => (
            <SubMenu key={f.id} label={f.name}>
              {VALUES.map(([l, x]) => (
                <Item key={x} onPick={() => (close(), add(`"${f.name}":${x}`))}>
                  {l}
                </Item>
              ))}
            </SubMenu>
          ))
        }
      </MenuButton>
      <MenuButton label="System fields ▾" title="System fields">
        {(close) => SYSTEM.map(([l, x]) => <Item key={l} onPick={() => (close(), add(x))}>{l}</Item>)}
      </MenuButton>
      <MenuButton label="Tag types ▾" title="Tag types">
        {(close) => BASE_TYPES.map((b) => <Item key={b} onPick={() => (close(), add(`TYPE:${b}`))}>{`${b} (base type)`}</Item>)}
      </MenuButton>
      <MenuButton label="Search operators ▾" title="Search operators">
        {(close) => OPERATORS.map(([l, x]) => <Item key={l} onPick={() => (close(), add(x))}>{l}</Item>)}
      </MenuButton>
      {workspaces.length > 1 && (
        <MenuButton label="Workspace ▾" title="Workspace">
          {(close) => workspaces.map((w) => <Item key={w.id} onPick={() => (close(), add(`WORKSPACE:${w.id}`))}>{w.name}</Item>)}
        </MenuButton>
      )}
    </div>
  )
}

export function QueryBuilder({ v, panel }: { v: ViewSnap; panel: string }) {
  const open = useStore((s) => !!s.queryOpen[panel])
  if (!v.query || !open) return null
  return (
    <div className="query-builder">
      <button className="q-close" title="Close the query builder" onClick={() => void toggleQuery(panel, v.node, false)}>
        <Icon name="close" size={9} />
      </button>
      <div className="q-head">
        <span className="q-title">Query builder</span>
        <span className="muted">Results wait until you run the query.</span>
        <a className="q-learn" href="https://outliner.tana.inc/learn" target="_blank" rel="noreferrer">
          Learn more
        </a>
      </div>
      <GroupBox g={v.query.tree} node={v.node} panel={panel} path={[]} />
      <QueryMenus v={v} panel={panel} />
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

/** A column header's name: a click renames the field in place (Enter keeps, Escape leaves it as it
 * was); the icon beside it opens the field (spec 04, table). */
function HeaderName({ id, name, panel }: { id: number; name: string; panel: string }) {
  const [editing, setEditing] = useState(false)
  if (!editing)
    return (
      <span className="t-col-name" onClick={() => setEditing(true)}>
        {name}
      </span>
    )
  const done = (text: string | null) => {
    setEditing(false)
    if (text != null && text.trim() && text !== name) void exec('node.setName', { node: id, name: text.trim() }, panel)
  }
  return (
    <input
      className="t-col-input"
      autoFocus
      defaultValue={name}
      aria-label="Field name"
      onFocus={(e) => e.currentTarget.select()}
      onKeyDown={(e) => {
        e.stopPropagation()
        if (e.key === 'Enter') done(e.currentTarget.value)
        else if (e.key === 'Escape') done(null)
      }}
      onBlur={(e) => done(e.currentTarget.value)}
    />
  )
}

const FIELD_TYPES: [string, string][] = [['plain', 'Plain'], ['options', 'Options'], ['options_from_tag', 'Options from supertag'], ['date', 'Date'], ['number', 'Number'], ['user', 'User'], ['url', 'Url'], ['email', 'E-Mail'], ['checkbox', 'Checkbox']]
const CALCS: [string, string][] = [['sum', 'Sum'], ['average', 'Average'], ['median', 'Median'], ['min', 'Min'], ['max', 'Max'], ['count_all', 'Count all'], ['count_values', 'Count values'], ['count_unique', 'Count unique'], ['none', 'None']]

/** Right-click on a column header (spec 04, table): type, calculation, sort, move, hide, delete. */
function HeaderMenu({ v, panel, col, x, y, close }: { v: ViewSnap; panel: string; col: ViewColumn; x: number; y: number; close: () => void }) {
  const ref = useRef<HTMLDivElement>(null)
  useEffect(() => {
    const off = (e: MouseEvent) => !ref.current?.contains(e.target as Node) && close()
    const esc = (e: KeyboardEvent) => e.key === 'Escape' && (e.stopPropagation(), close())
    window.addEventListener('mousedown', off)
    window.addEventListener('keydown', esc, true)
    return () => {
      window.removeEventListener('mousedown', off)
      window.removeEventListener('keydown', esc, true)
    }
  }, [close])
  const run = (c: string, p: Json = {}) => () => {
    close()
    void exec(c, { node: v.node, field: col.id, ...p }, panel)
  }
  return (
    <div ref={ref} className="menu header-menu" style={{ left: Math.min(x, window.innerWidth - 230), top: Math.min(y, window.innerHeight - 360) }} role="menu">
      <SubMenu label="Change field type">
        {FIELD_TYPES.map(([t, l]) => (
          <Item key={t} checked={col.type === t} onPick={() => (close(), void exec('field.setType', { field: col.id, type: t }, panel))}>
            {l}
          </Item>
        ))}
      </SubMenu>
      <Item icon="gear" onPick={() => (close(), void exec('nav.open', { node: col.id }, panel))}>
        Configure field…
      </Item>
      <div className="menu-sep" />
      <SubMenu label="Calculate">
        {CALCS.map(([k, l]) => (
          <Item key={k} checked={(col.calc ?? 'none') === k} onPick={run('view.calculate', { calc: k })}>
            {l}
          </Item>
        ))}
      </SubMenu>
      <div className="menu-sep" />
      <Item icon="sort" onPick={() => (close(), void exec('view.sort', { node: v.node, by: String(col.id), desc: false }, panel))}>
        Sort ascending
      </Item>
      <Item onPick={() => (close(), void exec('view.sort', { node: v.node, by: String(col.id), desc: true }, panel))}>Sort descending</Item>
      <div className="menu-sep" />
      <Item onPick={run('view.moveColumn', { direction: 'left' })}>Move column left</Item>
      <Item onPick={run('view.moveColumn', { direction: 'right' })}>Move column right</Item>
      <div className="menu-sep" />
      <Item onPick={run('view.removeColumn')}>Hide column</Item>
      <Item onPick={run('view.hideOtherColumns')}>Hide all other columns</Item>
      <div className="menu-item danger" onMouseDown={(e) => (e.preventDefault(), run('view.deleteColumn')())}>
        <span className="menu-icon">
          <Icon name="trash" size={12} />
        </span>
        <span className="grow">Delete column and values</span>
      </div>
    </div>
  )
}

/** A menu item with a submenu on hover; it opens to the left when there is no room on the right. */
export function SubMenu({ label, children, icon = true }: { label: string; children: ReactNode; icon?: boolean }) {
  const [open, setOpen] = useState<'right' | 'left' | null>(null)
  return (
    <div
      className="menu-item has-sub"
      onMouseEnter={(e) => setOpen(e.currentTarget.getBoundingClientRect().right + 240 > window.innerWidth ? 'left' : 'right')}
      onMouseLeave={() => setOpen(null)}
    >
      {icon && <span className="menu-icon" />}
      <span className="grow">{label}</span>
      <Icon name="chevronRight" size={12} />
      {open && <div className={`menu submenu${open === 'left' ? ' to-left' : ''}`}>{children}</div>}
    </div>
  )
}

let measureCtx: CanvasRenderingContext2D | null = null
/** The width the longest title needs, with its bullet, checkbox and chip (a guess from the font). */
function titleFit(v: ViewSnap): number {
  measureCtx ??= document.createElement('canvas').getContext('2d')
  if (!measureCtx) return 0
  measureCtx.font = `${getComputedStyle(document.documentElement).getPropertyValue('--fs-dense').trim() || '14px'} Inter Variable, Inter, system-ui, sans-serif`
  let max = 0
  for (const g of v.groups) for (const it of g.items) max = Math.max(max, measureCtx.measureText(it.name).width)
  return Math.ceil(max + 70)
}

function Table({ v, panel }: { v: ViewSnap; panel: string }) {
  const fe = useStore((s) => s.fieldEdit)
  const [menu, setMenu] = useState<{ col: ViewColumn; x: number; y: number } | null>(null)
  // A column being resized: its width follows the pointer, and is saved on release (N46).
  const [sizing, setSizing] = useState<{ col: number | 'title'; width: number } | null>(null)
  const w = (col: number | 'title', saved: number | null, token: string) => (sizing?.col === col ? `${sizing.width}px` : saved ? `${saved}px` : `var(${token})`)
  // The title column fits its titles up to a limit (150 wide for short ones, spec 12: Table).
  // Each row is its own grid, so the width is worked out once from the longest title.
  const title = sizing?.col === 'title' ? `${sizing.width}px` : v.titleWidth ? `${v.titleWidth}px` : `clamp(var(--table-title-w), ${titleFit(v)}px, var(--table-title-max))`
  const tmpl = `${title} ${v.columns.map((c) => w(c.id, c.width, '--table-col-w')).join(' ')} auto`
  const grip = (col: number | 'title') => (
    <span
      className="t-grip"
      title="Drag to resize"
      onPointerDown={(e) => {
        e.preventDefault()
        e.stopPropagation()
        const cell = (e.currentTarget.parentElement as HTMLElement).getBoundingClientRect()
        const x0 = e.clientX
        let width = cell.width
        const move = (ev: PointerEvent) => {
          width = Math.max(60, Math.round(cell.width + ev.clientX - x0))
          setSizing({ col, width })
        }
        const up = () => {
          window.removeEventListener('pointermove', move)
          window.removeEventListener('pointerup', up)
          void exec('view.setColumnWidth', { node: v.node, field: col === 'title' ? 'title' : col, width }, panel).then(() => setSizing(null))
        }
        window.addEventListener('pointermove', move)
        window.addEventListener('pointerup', up)
      }}
    />
  )
  // The first column is named after the supertag every item has (spec 12: Table, N59.17).
  const all = v.groups.flatMap((g) => g.items)
  const tag = all[0]?.tags.find((t) => t.name !== 'day' && all.every((it) => it.tags.some((x) => x.id === t.id)))
  return (
    <div className="table-scroll">
    <div className="table" role="table" style={tag ? tagStyle(tag.color) : undefined}>
      <div className="t-row t-head" style={{ gridTemplateColumns: tmpl }} role="row">
        <span className={`t-cell${tag ? ' t-tag' : ''}`} role="columnheader">
          {tag ? (
            <>
              <span className="tag-square small">#</span> {tag.name[0].toUpperCase() + tag.name.slice(1)}
            </>
          ) : (
            'Name'
          )}
          {grip('title')}
        </span>
        {v.columns.map((c) => (
          <span
            key={c.id}
            className="t-cell"
            role="columnheader"
            title={c.name}
            onContextMenu={(e) => {
              e.preventDefault()
              setMenu({ col: c, x: e.clientX, y: e.clientY })
            }}
          >
            <span className="t-col-icon" title="Open the field" onClick={() => void openInNewPanel(c.id)}>
              <Icon name={FIELD_ICON[c.type] ?? 'fieldText'} size={12} />
            </span>
            <HeaderName id={c.id} name={c.name} panel={panel} />
            {grip(c.id)}
          </span>
        ))}
        <span className="t-cell">
          <MenuButton label="+ Add" title="Add a column">
            {(close) => (
              <>
                <Item
                  icon="plus"
                  onPick={() => {
                    close()
                    const r = document.querySelector('.t-head .menu-anchor')?.getBoundingClientRect()
                    openPicker({ mode: 'column', panel, node: v.node, at: -1, x: r?.left ?? 0, y: (r?.bottom ?? 0) + 4, exclude: v.columns.map((c) => c.id) })
                  }}
                >
                  Add new field
                </Item>
                <SubMenu label="System fields">
                  {['Created time', 'Last edited time', 'Done time', 'Owner node', 'Tags'].map((f) => (
                    <Item key={f} onPick={() => (close(), void exec('view.toggleSystemField', { node: v.node, field: f }, panel))}>
                      {f}
                    </Item>
                  ))}
                </SubMenu>
              </>
            )}
          </MenuButton>
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
      {menu && <HeaderMenu v={v} panel={panel} col={menu.col} x={menu.x} y={menu.y} close={() => setMenu(null)} />}
    </div>
    </div>
  )
}

// ---- cards and board ---------------------------------------------------------------------------

function Card({ it, panel, refs, onDragStart, onDragEnd, dragged, shift }: { it: ViewItem; panel: string; refs: Record<string, string>; onDragStart?: (e: React.DragEvent) => void; onDragEnd?: () => void; dragged?: boolean; shift?: number }) {
  return (
    <div
      className={`card${dragged ? ' dragged' : ''}`}
      draggable={!!onDragStart}
      onDragStart={onDragStart}
      onDragEnd={onDragEnd}
      data-id={it.id}
      style={shift ? { transform: `translateY(${shift}px)` } : undefined}
    >
      <div className="card-head">
        <ItemBullet it={it} panel={panel} />
        {it.tags.map((t) => (
          <TagChip key={t.id} chip={t} />
        ))}
      </div>
      <div className="card-body">
        <ItemTitle it={it} panel={panel} refs={refs} chips={false} />
      </div>
      {(it.line || it.dates[0]) && <div className="card-foot">{it.line || shortWhen(it.dates[0], it.time)}</div>}
    </div>
  )
}

/** A new card's name, typed in place (N49): every key goes to the engine at once. */
function CardNameEditor({ id, panel, done }: { id: number; panel: string; done: () => void }) {
  const [text, setText] = useState('')
  return (
    <input
      className="card-name-input"
      autoFocus
      placeholder="Untitled"
      value={text}
      onChange={(e) => {
        setText(e.target.value)
        void call('run', { command: 'node.setName', params: { node: id, name: e.target.value }, panel })
      }}
      onKeyDown={(e) => {
        e.stopPropagation()
        if (e.key === 'Enter' || e.key === 'Escape') done()
      }}
      onBlur={done}
    />
  )
}

/** Where a dragged card would land in a column: the number of other cards above the pointer.
 * (offsetTop ignores the transforms of cards already sliding aside.) */
function slotAt(col: HTMLElement, clientY: number): number {
  const y = clientY - col.getBoundingClientRect().top
  let i = 0
  for (const c of col.querySelectorAll<HTMLElement>('.card:not(.dragged)')) if (c.offsetTop + c.offsetHeight / 2 < y) i++
  return i
}

/** How far each card of `col` slides (px, by view item id) with the dragged card at `slot`: in its
 * own column the dimmed card itself moves there as the placeholder and the cards it passes make
 * room; in another column a gap of its height opens (14 M19). */
function slideFor(col: HTMLElement, dragId: number, slot: number, h: number): Record<number, number> {
  const cards = [...col.querySelectorAll<HTMLElement>('.card')]
  const gap = parseFloat(getComputedStyle(col).rowGap) || 0
  const own = cards.find((c) => Number(c.dataset.id) === dragId)
  const order: (HTMLElement | number)[] = cards.filter((c) => c !== own)
  order.splice(slot, 0, own ?? h)
  let y = cards[0]?.offsetTop ?? 0
  const out: Record<number, number> = {}
  for (const c of order) {
    if (typeof c === 'number') {
      y += c
      continue
    }
    out[Number(c.dataset.id)] = y - c.offsetTop
    y += c.offsetHeight + gap
  }
  return out
}

function Board({ v, panel }: { v: ViewSnap; panel: string }) {
  // The card being dragged (its view item, column and height with the gap below it) and the slot
  // it would land in: cards from that slot down slide aside to make room (14 M19).
  const [drag, setDrag] = useState<{ id: number; target: number; col: string; h: number } | null>(null)
  const [slot, setSlot] = useState<{ col: string; index: number; slide: Record<number, number> } | null>(null)
  const [fresh, setFresh] = useState<number | null>(null)
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
  // "+ Add" in a column: a card at the top of that column, its name typed in place (spec 04 §5).
  const addTo = async (label: string) => {
    const r = await exec('view.addItem', { node: v.node, name: '', stay: true, ...(groupField ? { field: groupField.id, value: label === 'Not defined' ? '' : label } : {}) }, panel)
    const id = r?.result?.id
    if (id != null) setFresh(id)
  }
  const endDrag = () => {
    setDrag(null)
    setSlot(null)
  }
  return (
    <div className="board">
      {v.groups.map((g) => {
        const items = fresh != null ? [...g.items.filter((it) => it.target === fresh), ...g.items.filter((it) => it.target !== fresh)] : g.items
        const label = g.label === 'Not defined' && groupField ? `${groupField.name} not set` : g.label
        const others = items.filter((it) => it.id !== drag?.id)
        const open = drag && slot?.col === g.label ? slot.index : null
        return (
          <div
            key={g.label}
            className={`board-col${open != null ? ' over' : ''}`}
            onDragOver={(e) => {
              e.preventDefault()
              const index = slotAt(e.currentTarget, e.clientY)
              if (drag && (slot?.col !== g.label || slot.index !== index)) setSlot({ col: g.label, index, slide: slideFor(e.currentTarget, drag.id, index, drag.h) })
            }}
            onDragLeave={(e) => {
              if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setSlot(null)
            }}
            onDrop={(e) => {
              e.preventDefault()
              const d = drag
              const index = slotAt(e.currentTarget, e.clientY)
              endDrag()
              if (!d) return
              // Dropping a card on a column gives it that column's value (spec 04 §5), and it goes
              // where it was dropped among that column's cards.
              const anchor = others[index]
              const last = others[others.length - 1]
              const place = anchor ? { before: anchor.id } : last ? { after: last.id } : {}
              void withBarrier(async () => {
                if (d.col !== g.label && groupField) await exec('field.set', { node: d.target, field: groupField.id, value: g.label === 'Not defined' ? '' : g.label }, panel)
                await exec('view.moveItem', { node: v.node, item: d.id, ...place }, panel)
              })
            }}
          >
            <div className="board-head">
              <span className="board-name">{label}</span>
              <button className="ghost-btn board-hide" title="Hide column" onClick={() => void exec('view.toggleGroupHidden', { node: v.node, group: g.label }, panel)}>
                Hide
              </button>
              <button className="ghost-btn" onClick={() => void addTo(g.label)}>
                + Add
              </button>
            </div>
            {items.map((it) =>
              it.target === fresh ? (
                <div key={it.id} className="card" data-id={it.id}>
                  <div className="card-body">
                    <CardNameEditor id={it.target} panel={panel} done={() => setFresh(null)} />
                  </div>
                </div>
              ) : (
                <Card
                  key={it.id}
                  it={it}
                  panel={panel}
                  refs={v.refs}
                  dragged={drag?.id === it.id}
                  shift={open != null ? slot?.slide[it.id] : 0}
                  onDragStart={(e) => {
                    e.dataTransfer.setData('text/x-node', String(it.target))
                    const el = e.currentTarget as HTMLElement
                    const gap = parseFloat(getComputedStyle(el.parentElement ?? el).rowGap) || 0
                    setDrag({ id: it.id, target: it.target, col: g.label, h: el.offsetHeight + gap })
                  }}
                  onDragEnd={endDrag}
                />
              ),
            )}
          </div>
        )
      })}
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
  const [offset, setOffset] = useState(0)
  const items = v.groups.flatMap((g) => g.items)
  const byDay = new Map<string, ViewItem[]>()
  for (const it of items) for (const d of it.dates) byDay.set(d, [...(byDay.get(d) ?? []), it])
  const undated = items.filter((it) => it.dates.length === 0)
  const first = `${c.year}-${String(c.month).padStart(2, '0')}-01`
  // The grid starts on the first day of the week that holds the 1st.
  const dow = (new Date(`${first}T00:00:00Z`).getUTCDay() + 6) % 7
  const sundayFirst = c.weekdays[0]?.startsWith('Sun')
  const start = addDays(first, -(sundayFirst ? (dow + 1) % 7 : dow))
  // As many weeks as the month needs (five for October 2026; N59.27).
  const inMonth = new Date(Date.UTC(c.year, c.month, 0)).getUTCDate()
  const lead = sundayFirst ? (dow + 1) % 7 : dow
  const days = Array.from({ length: Math.ceil((lead + inMonth) / 7) * 7 }, (_, i) => addDays(start, i))
  // Week and day views step on their own (the engine keeps the month).
  const span = c.mode === 'day' ? [addDays(c.today, offset)] : Array.from({ length: 7 }, (_, i) => addDays(c.weekStart, offset + i))
  const title = c.mode === 'month' ? `${MONTHS[c.month - 1]} ${c.year}` : c.mode === 'week' ? `Week ${isoWeek(span[0])}, ${span[0].slice(0, 4)}` : longDay(span[0])
  const step = (dir: number) => {
    if (c.mode === 'month') void exec('view.month', { step: dir }, panel)
    else setOffset((o) => o + dir * (c.mode === 'week' ? 7 : 1))
  }
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
        <button className="icon-btn" title="Previous" onClick={() => step(-1)}>
          <Icon name="chevronLeft" size={14} />
        </button>
        <button className="icon-btn" title="Today" onClick={() => (setOffset(0), void exec('view.month', {}, panel))}>
          <Icon name="calendar" size={14} />
        </button>
        <button className="icon-btn" title="Next" onClick={() => step(1)}>
          <Icon name="chevronRight" size={14} />
        </button>
      </div>
      {c.mode !== 'month' && <TimeGrid v={v} days={span} byDay={byDay} today={c.today} panel={panel} />}
      {c.mode === 'month' && <div className="cal-grid">
        {c.weekdays.map((d) => (
          <div key={d} className="cal-dow">
            {d}
          </div>
        ))}
        {days.map((d, i) => {
          const day = Number(d.slice(8))
          const out = c.mode === 'month' && Number(d.slice(5, 7)) !== c.month
          return (
            <div
              key={d}
              className={`cal-cell${out ? ' out' : ''}${isWeekend(d) ? ' weekend' : ''}`}
              onDragOver={(e) => e.dataTransfer.types.includes('text/x-cal') && e.preventDefault()}
              onDrop={(e) => {
                // An event dropped on another day moves there, keeping its time (spec 04 §7).
                const it = dropped(e)
                if (it) setWhen(v, panel, it.node, d, it.time)
              }}
            >
              {i % 7 === 0 && <span className="cal-week">W{isoWeek(d)}</span>}
              <span className={`cal-day${d === c.today ? ' today' : ''}`} onClick={() => void exec('nav.day', { date: d }, panel)}>
                {day}
              </span>
              {(byDay.get(d) ?? []).slice(0, 4).map((it) => {
                const tag = it.tags.find((t) => t.name !== 'day')
                return (
                  <div key={it.id} className="cal-event" draggable onDragStart={(e) => dragItem(e, it)} style={tag ? tagStyle(tag.color) : undefined} onClick={() => void exec('nav.open', { node: it.target }, panel)}>
                    {it.done !== null && <span className={`checkbox compact${it.done ? ' on' : ''}`}>{it.done && <Icon name="check" size={9} />}</span>}
                    <span className="cal-event-name">{it.name || 'Untitled'}</span>
                  </div>
                )
              })}
            </div>
          )
        })}
      </div>}
      {undated.length > 0 && <div className="cal-undated">{undated.length} nodes with no dates ›</div>}
    </div>
  )
}

/** Week and day: an all-day strip over an hour grid (spec 04 §10; 12: Calendar week). */
const WEEKDAY = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat']
const weekdayOf = (d: string) => WEEKDAY[new Date(`${d}T00:00:00Z`).getUTCDay()]
const isWeekend = (d: string) => [0, 6].includes(new Date(`${d}T00:00:00Z`).getUTCDay())

/** "Sat, 17 Oct, 10:30 → 13:00": a card's date line (N59.26). */
function shortWhen(d: string, time: [number, number] | null): string {
  const day = `${weekdayOf(d)}, ${Number(d.slice(8))} ${MONTHS[Number(d.slice(5, 7)) - 1].slice(0, 3)}`
  return time ? `${day}, ${hm(time[0])} → ${hm(time[1])}` : day
}

/** "October 10th, 2026" (the day view's title, N50). */
function longDay(d: string): string {
  const n = Number(d.slice(8))
  const th = n % 10 === 1 && n !== 11 ? 'st' : n % 10 === 2 && n !== 12 ? 'nd' : n % 10 === 3 && n !== 13 ? 'rd' : 'th'
  return `${MONTHS[Number(d.slice(5, 7)) - 1]} ${n}${th}, ${d.slice(0, 4)}`
}

/** Minutes from the top of an hour column, to a quarter hour. */
function minuteAt(e: { clientY: number }, col: HTMLElement): number {
  const y = e.clientY - col.getBoundingClientRect().top
  const hourH = parseFloat(getComputedStyle(col).getPropertyValue('--tg-hour-h')) || 45
  return Math.max(0, Math.min(24 * 60, Math.round(((y / hourH) * 60) / 15) * 15))
}

function TimeGrid({ v, days, byDay, today, panel }: { v: ViewSnap; days: string[]; byDay: Map<string, ViewItem[]>; today: string; panel: string }) {
  const grid = useRef<HTMLDivElement>(null)
  // Dragging on empty grid makes a new event (a span, then its name); dragging an event's lower
  // edge changes its end (spec 04 §7, N50).
  const [span, setSpan] = useState<{ day: string; a: number; b: number; naming: boolean } | null>(null)
  const [resize, setResize] = useState<{ node: number; day: string; a: number; b: number } | null>(null)
  const [name, setName] = useState('')
  useEffect(() => {
    // Start the view at 08:00.
    if (grid.current) grid.current.scrollTop = 8 * (parseFloat(getComputedStyle(grid.current).getPropertyValue('--tg-hour-h')) || 45)
  }, [])
  const hours = Array.from({ length: 24 }, (_, h) => h)
  const cols = `var(--tg-gutter) repeat(${days.length}, minmax(0, 1fr))`
  const create = () => {
    if (!span || !name.trim()) return setSpan(null)
    const value = `${span.day} ${hm(Math.min(span.a, span.b))}-${hm(Math.max(span.a, span.b))}`
    void exec('view.addItem', { node: v.node, name: name.trim(), stay: true, ...(v.dateField ? { field: v.dateField.id, value } : {}) }, panel)
    setSpan(null)
    setName('')
  }
  return (
    <div className="tg">
      <div className="tg-head" style={{ gridTemplateColumns: cols }}>
        <span className="tg-week">W{isoWeek(days[0])}</span>
        {days.map((d) => (
          <span key={d} className={`tg-day${d === today ? ' today' : ''}`}>
            {weekdayOf(d)} {d.slice(8)}/{d.slice(5, 7)}
          </span>
        ))}
      </div>
      <div className="tg-allday" style={{ gridTemplateColumns: cols }}>
        <span />
        {days.map((d) => (
          <span
            key={d}
            className="tg-cell"
            onDragOver={(e) => e.dataTransfer.types.includes('text/x-cal') && e.preventDefault()}
            onDrop={(e) => {
              const it = dropped(e)
              if (it) setWhen(v, panel, it.node, d, null)
            }}
          >
            {(byDay.get(d) ?? [])
              .filter((it) => !it.time)
              .map((it) => (
                <EventChip key={it.id} it={it} panel={panel} />
              ))}
          </span>
        ))}
      </div>
      <div className="tg-body" ref={grid}>
        <div className="tg-hours" style={{ gridTemplateColumns: cols }}>
          <div className="tg-gutter">
            {hours.map((h) => (
              <span key={h} className="tg-hour">
                {String(h).padStart(2, '0')}:00
              </span>
            ))}
          </div>
          {days.map((d) => (
            <div
              key={d}
              className={`tg-col${isWeekend(d) ? ' weekend' : ''}`}
              data-day={d}
              onDragOver={(e) => e.dataTransfer.types.includes('text/x-cal') && e.preventDefault()}
              onDrop={(e) => {
                // Dropped on an hour: starts there, keeping its length (an hour for all-day items).
                const it = dropped(e)
                if (!it) return
                const a = minuteAt(e, e.currentTarget)
                const len = it.time ? it.time[1] - it.time[0] : 60
                setWhen(v, panel, it.node, d, [a, a + len])
              }}
              onPointerDown={(e) => {
                if (e.target !== e.currentTarget || e.button !== 0) return
                e.preventDefault()
                const col = e.currentTarget
                const a = minuteAt(e, col)
                setSpan({ day: d, a, b: a + 30, naming: false })
                const move = (ev: PointerEvent) => setSpan((sp) => (sp ? { ...sp, b: Math.max(minuteAt(ev, col), sp.a + 15) } : sp))
                const up = () => {
                  window.removeEventListener('pointermove', move)
                  window.removeEventListener('pointerup', up)
                  setSpan((sp) => (sp ? { ...sp, naming: true } : sp))
                }
                window.addEventListener('pointermove', move)
                window.addEventListener('pointerup', up)
              }}
            >
              {(byDay.get(d) ?? [])
                .filter((it) => it.time)
                .map((it) => {
                  const [a, b0] = it.time!
                  const b = resize?.node === it.target && resize.day === d ? resize.b : b0
                  return (
                    <div key={it.id} className="tg-event-wrap" style={{ top: `calc(${a / 60} * var(--tg-hour-h))`, height: `max(var(--tg-event-min), calc(${(b - a) / 60} * var(--tg-hour-h)))` }}>
                      <EventChip it={it} panel={panel} time={`${hm(a)} → ${hm(b)}`} />
                      <span
                        className="tg-resize"
                        title="Drag to change the end"
                        onPointerDown={(e) => {
                          e.preventDefault()
                          e.stopPropagation()
                          const col = (e.currentTarget.closest('.tg-col') as HTMLElement) ?? e.currentTarget
                          let end = b0
                          const move = (ev: PointerEvent) => {
                            end = Math.max(a + 15, minuteAt(ev, col))
                            setResize({ node: it.target, day: d, a, b: end })
                          }
                          const up = () => {
                            window.removeEventListener('pointermove', move)
                            window.removeEventListener('pointerup', up)
                            setResize(null)
                            if (end !== b0) setWhen(v, panel, it.target, d, [a, end])
                          }
                          window.addEventListener('pointermove', move)
                          window.addEventListener('pointerup', up)
                        }}
                      />
                    </div>
                  )
                })}
              {span?.day === d && (
                <div className="tg-new" style={{ top: `calc(${Math.min(span.a, span.b) / 60} * var(--tg-hour-h))`, height: `calc(${Math.abs(span.b - span.a) / 60} * var(--tg-hour-h))` }}>
                  {span.naming ? (
                    <input
                      className="tg-new-input"
                      autoFocus
                      placeholder="New event"
                      value={name}
                      onChange={(e) => setName(e.target.value)}
                      onKeyDown={(e) => {
                        e.stopPropagation()
                        if (e.key === 'Enter') create()
                        else if (e.key === 'Escape') (setSpan(null), setName(''))
                      }}
                      onBlur={create}
                    />
                  ) : (
                    <span className="tg-new-time">
                      {hm(Math.min(span.a, span.b))} → {hm(Math.max(span.a, span.b))}
                    </span>
                  )}
                </div>
              )}
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}

const hm = (m: number) => `${String(Math.floor(m / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`

/** Puts an item on a day (and a time span, in minutes) through the calendar's date field. */
function setWhen(v: ViewSnap, panel: string, node: number, day: string, time: [number, number] | null) {
  if (!v.dateField) return
  const value = time ? `${day} ${hm(time[0])}-${hm(Math.min(time[1], 24 * 60 - 1))}` : day
  void exec('field.set', { node, field: v.dateField.id, value }, panel)
}

function dragItem(e: React.DragEvent, it: ViewItem) {
  e.dataTransfer.setData('text/x-cal', JSON.stringify({ node: it.target, time: it.time }))
  e.dataTransfer.effectAllowed = 'move'
}

function dropped(e: React.DragEvent): { node: number; time: [number, number] | null } | null {
  try {
    return JSON.parse(e.dataTransfer.getData('text/x-cal'))
  } catch {
    return null
  }
}

function EventChip({ it, panel, time }: { it: ViewItem; panel: string; time?: string }) {
  const tag = it.tags.find((t) => t.name !== 'day')
  return (
    <div className="cal-event" draggable onDragStart={(e) => dragItem(e, it)} style={tag ? tagStyle(tag.color) : undefined} onClick={() => void exec('nav.open', { node: it.target }, panel)}>
      <span className="cal-event-name">{it.name || 'Untitled'}</span>
      {time && <span className="cal-event-time">{time}</span>}
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
