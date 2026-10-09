// A panel: breadcrumbs, the page header and the outline (virtualized: only rows near the
// viewport are in the DOM; a row never holds unsaved text, every keystroke is already sent).

import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type MouseEvent as RME } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import type { Chip, FieldRow, MoreRow, NodeRow, Page, Row } from './engine'
import { call } from './engine'
import { Icon, type IconName } from './icons'
import { type Span, charLen, rawSpans, toChars, toUtf16 } from './format'
import { getState, openInNewPanel, refresh, setActivePanel, setState, closePanel, useStore } from './store'
import { type EditorHandle, exec, focusLine, pasteInto, noteSynced, registerEditor, registerTitle, setGhost, unregisterEditor, withBarrier } from './controller'
import { onLineText, openPicker, setPickerQuery, startFieldEdit } from './picker'
import { QueryBuilder, ViewBody, ViewToolbar } from './Views'
import { HelpPage, RecentsPage, SettingsPage, SupertagsPage, TagHeader } from './Pages'
import { keyInfo } from './keys'

const OVERSCAN_ROWS = 150
const WINDOW = 400

const TAG_NAMES = new Set(['grey', 'pink', 'red', 'orange', 'amber', 'yellow', 'lime', 'green', 'teal', 'cyan', 'blue', 'indigo', 'violet', 'magenta'])

/** The four tones of a supertag colour (10 §1.5) as CSS variables: --tag (fg), --tag-t1, --tag-t2. */
export function tagStyle(color: string): CSSProperties {
  if (/^#[0-9a-f]{6}$/i.test(color)) {
    return {
      '--tag': color,
      '--tag-t1': `color-mix(in srgb, ${color} 22%, var(--c-panel))`,
      '--tag-t2': `color-mix(in srgb, ${color} 32%, var(--c-panel))`,
    } as CSSProperties
  }
  const name = TAG_NAMES.has(color) ? color : 'grey'
  return { '--tag': `var(--tag-${name}-fg)`, '--tag-t1': `var(--tag-${name}-t1)`, '--tag-t2': `var(--tag-${name}-t2)` } as CSSProperties
}

export function TagChip({ chip, onOpen }: { chip: Chip; onOpen?: () => void }) {
  return (
    <span className="chip" style={tagStyle(chip.color)} onMouseDown={(e) => e.preventDefault()} onClick={onOpen} data-tag={chip.name}>
      <span className="chip-hash">#</span>
      {chip.name}
    </span>
  )
}

/** Styled text of a line (marks hidden), with inline references and links. */
export function RichText({ text, refs, onRef }: { text: string; refs: Record<string, string>; onRef?: (id: number) => void }) {
  const spans = rawSpans(text)
  const linkOnly = spans.filter((s) => !s.marker && s.text.trim()).length === 1 && spans.some((s) => s.url)
  return (
    <>
      {spans.map((s, i) => {
        if (s.marker) return <span key={i} data-at={s.at} data-marker="1" className="mark-hidden" />
        if (s.node != null) {
          const name = refs[String(s.node)]
          return (
            <span
              key={i}
              className="inline-ref"
              data-at={s.at}
              onMouseDown={(e) => e.preventDefault()}
              onClick={(e) => {
                e.stopPropagation()
                onRef?.(s.node!)
              }}
            >
              {name === undefined ? '(missing node)' : name || 'Untitled'}
            </span>
          )
        }
        if (s.url) {
          return (
            <a
              key={i}
              className="link"
              href={s.url}
              target="_blank"
              rel="noreferrer"
              data-at={s.at}
              onMouseDown={(e) => e.preventDefault()}
              onClick={(e) => e.stopPropagation()}
            >
              {!linkOnly && <Icon name="external" size={12} className="link-icon" />}
              {s.text}
            </a>
          )
        }
        return (
          <span key={i} className={spanClass(s)} data-at={s.at}>
            {s.text}
          </span>
        )
      })}
    </>
  )
}

function spanClass(s: Span) {
  return [s.bold && 'b', s.italic && 'i', s.underline && 'u', s.highlight && 'hl', s.code && 'code'].filter(Boolean).join(' ')
}

/** The raw text while editing: marks shown faint, the text between them formatted. */
function EditMirror({ text }: { text: string }) {
  return (
    <>
      {rawSpans(text).map((s, i) =>
        s.marker ? (
          <span key={i} className="mark">
            {s.text}
          </span>
        ) : s.url ? (
          <span key={i} className="link">
            {s.text}
          </span>
        ) : (
          <span key={i} className={spanClass(s)}>
            {s.text}
          </span>
        ),
      )}
      {'​'}
    </>
  )
}

/** Where in the raw text a click landed (chars), from the span under the pointer. */
function caretFromPoint(e: RME, raw: string): number {
  const doc = document as Document & { caretRangeFromPoint?: (x: number, y: number) => Range | null }
  const r = doc.caretRangeFromPoint?.(e.clientX, e.clientY)
  const el = r?.startContainer.parentElement?.closest('[data-at]') as HTMLElement | null
  if (!r || !el || el.dataset.marker || el.classList.contains('inline-ref')) return charLen(raw)
  const at = Number(el.dataset.at) + r.startOffset
  return toChars(raw, Math.min(at, raw.length))
}

// ---- the line editor ---------------------------------------------------------------------------

function LineEditor({ row, panel, page }: { row: NodeRow; panel: string; page: Page }) {
  const [value, setValue] = useState(row.name)
  const ta = useRef<HTMLTextAreaElement>(null)
  const pending = useRef(0)
  const valueRef = useRef(value)
  valueRef.current = value
  const caretEpoch = useStore((s) => s.caretEpoch)

  // The engine's text wins once every edit sent from here has arrived.
  useEffect(() => {
    if (pending.current === 0 && row.name !== valueRef.current) setValue(row.name)
  }, [row.name])

  const sendName = (name: string, caret: number) => {
    pending.current++
    noteSynced(row.id, caret)
    void call('run', { command: 'node.setName', params: { node: row.id, name, caret }, panel }).then((r) => {
      pending.current--
      if (r?.error) setState({ toast: r.error })
      if (pending.current === 0) void scheduleQuietRefresh()
    })
  }

  /** New text in the line (typed, pasted or replayed): to the engine, and menus follow it. */
  const textChanged = (raw: string, caretUtf16: number) => {
    // Keys never insert line breaks (Enter is a command).
    const v = raw.replace(/\n/g, ' ')
    valueRef.current = v
    setValue(v)
    const caret = toChars(v, caretUtf16)
    const el = ta.current
    const instead = onLineText(row, panel, v, caret, (i) => (el ? caretPoint(el, v, i) : { x: 0, y: 0 }))
    if (instead !== null) {
      valueRef.current = instead
      setValue(instead)
      return
    }
    sendName(v, caret)
  }

  // Put the caret where the engine says: on mount and after commands (not after typing).
  useLayoutEffect(() => {
    const el = ta.current
    if (!el) return
    el.focus({ preventScroll: true })
    const v = valueRef.current
    const c = toUtf16(v, page.caret)
    const a = toUtf16(v, page.anchor)
    el.setSelectionRange(Math.min(a, c), Math.max(a, c), a > c ? 'backward' : 'forward')
    el.scrollIntoView({ block: 'nearest' })
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [row.id, caretEpoch])

  useEffect(() => {
    const h: EditorHandle = {
      kind: 'line',
      panel,
      node: row.id,
      sel(): [number, number] {
        const el = ta.current
        const v = valueRef.current
        if (!el) return [charLen(v), charLen(v)]
        const fwd = el.selectionDirection !== 'backward'
        const a = toChars(v, fwd ? el.selectionStart : el.selectionEnd)
        const c = toChars(v, fwd ? el.selectionEnd : el.selectionStart)
        return [a, c]
      },
      value: () => valueRef.current,
      insert(text: string) {
        const el = ta.current
        const v = valueRef.current
        const s0 = el ? el.selectionStart : v.length
        const s1 = el ? el.selectionEnd : v.length
        const next = v.slice(0, s0) + text + v.slice(s1)
        const at = s0 + text.length
        if (el) {
          el.value = next
          el.setSelectionRange(at, at)
        }
        textChanged(next, at)
      },
      focus() {
        ta.current?.focus({ preventScroll: true })
      },
      setSel(a: number, c: number) {
        const el = ta.current
        if (!el) return
        const v = valueRef.current
        const ua = toUtf16(v, a)
        const uc = toUtf16(v, c)
        el.setSelectionRange(Math.min(ua, uc), Math.max(ua, uc), ua > uc ? 'backward' : 'forward')
      },
      native(k: ReturnType<typeof keyInfo>) {
        const el = ta.current
        if (!el) return
        const v = valueRef.current
        if ((k.metaKey || k.ctrlKey) && k.code === 'KeyA') {
          el.setSelectionRange(0, v.length)
          return
        }
        // A paste that waited behind a command: read the clipboard now.
        if ((k.metaKey || k.ctrlKey) && k.code === 'KeyV') {
          void navigator.clipboard?.readText().then((text) => {
            if (!pasteInto(panel, row.id, valueRef.current, text)) h.insert(text)
          })
          return
        }
        let s0 = el.selectionStart
        let s1 = el.selectionEnd
        if (k.key === 'Backspace' || k.key === 'Delete') {
          if (s0 === s1) {
            if (k.key === 'Backspace') s0 = Math.max(0, s0 - 1)
            else s1 = Math.min(v.length, s1 + 1)
          }
          const next = v.slice(0, s0) + v.slice(s1)
          valueRef.current = next
          setValue(next)
          el.value = next
          el.setSelectionRange(s0, s0)
          sendName(next, toChars(next, s0))
        } else if (k.key === 'ArrowLeft') el.setSelectionRange(Math.max(0, s0 - 1), Math.max(0, s0 - 1))
        else if (k.key === 'ArrowRight') el.setSelectionRange(Math.min(v.length, s1 + 1), Math.min(v.length, s1 + 1))
      },
    }
    registerEditor(h)
    return () => unregisterEditor(h)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [row.id, panel])

  return (
    <span className={`editor${row.heading ? ' heading' : ''}`}>
      <span className="editor-mirror" aria-hidden="true">
        <EditMirror text={value} />
      </span>
      <textarea
        ref={ta}
        className="editor-input"
        data-node={row.id}
        value={value}
        rows={1}
        spellCheck={false}
        placeholder=""
        onChange={(e) => textChanged(e.target.value, e.target.selectionEnd)}
        onPaste={(e) => {
          const text = e.clipboardData.getData('text/plain')
          if (pasteInto(panel, row.id, valueRef.current, text)) e.preventDefault()
        }}
      />
    </span>
  )
}

/** Where char `i` of a single-line editor sits on screen (for menus opened from the text). */
function caretPoint(el: HTMLTextAreaElement, text: string, i: number): { x: number; y: number } {
  const r = el.getBoundingClientRect()
  const cs = getComputedStyle(el)
  const ctx = document.createElement('canvas').getContext('2d')
  let w = 0
  if (ctx) {
    ctx.font = `${cs.fontWeight} ${cs.fontSize} ${cs.fontFamily}`
    w = ctx.measureText(Array.from(text).slice(0, i).join('')).width
  }
  return { x: r.left + Math.min(w, r.width), y: r.top + parseFloat(cs.lineHeight || '21') }
}

let quietTimer = 0
/** After typing settles, read the page again (title suffixes, references elsewhere). */
function scheduleQuietRefresh() {
  clearTimeout(quietTimer)
  quietTimer = window.setTimeout(() => void refresh(), 120)
}

// ---- rows --------------------------------------------------------------------------------------

function Guides({ depth }: { depth: number }) {
  return (
    <>
      {Array.from({ length: depth }, (_, i) => (
        <span key={i} className="guide" style={{ left: `calc(${i} * var(--indent) + var(--bullet) / 2)` }} />
      ))}
    </>
  )
}

function Bullet({ row, onClick, panel }: { row: NodeRow; onClick: () => void; panel?: string }) {
  const tag = row.tags.find((t) => t.name !== 'day')
  const style = tag ? tagStyle(tag.color) : undefined
  const closed = (row.hasKids && !row.expanded) || row.kind === 'search'
  let glyph: JSX.Element
  if (row.kind === 'search') glyph = <Icon name="search" size={9} className="glyph" />
  else if (row.ref) glyph = <span className={`dot ref${row.notes ? ' notes' : ''}`} />
  else if (row.link) glyph = <Icon name="link" size={11} className="glyph" />
  else glyph = <span className="dot" />
  return (
    <span
      className={`bullet${closed ? ' closed' : ''}${tag ? ' tagged' : ''}`}
      style={style}
      onMouseDown={(e) => e.button === 0 && !panel && e.preventDefault()}
      onClick={onClick}
      title={`Edited ${ago(row.edited)}, created ${ago(row.created)}`}
      draggable={!!panel}
      onDragStart={(e) => {
        if (!panel) return
        const page = getState().pages[panel]
        const ids = page?.sel.includes(row.id) ? page.sel : [row.id]
        e.dataTransfer.setData('text/x-nodes', JSON.stringify(ids))
        e.dataTransfer.effectAllowed = 'move'
        dragging = ids
      }}
      onDragEnd={() => {
        dragging = []
        setDrop(null)
      }}
      onContextMenu={(e) => {
        if (!panel) return
        e.preventDefault()
        setState({ nodeMenu: { panel, node: row.id, x: e.clientX, y: e.clientY } })
      }}
    >
      {glyph}
    </span>
  )
}

// ---- selection toolbar (spec 02 §8; 11 §8) ---------------------------------------------------------

function SelectionToolbar({ row, panel }: { row: NodeRow; panel: string }) {
  const b = (label: string | JSX.Element, title: string, on: (e: React.MouseEvent) => void) => (
    <button className="sel-btn" title={title} onMouseDown={(e) => e.preventDefault()} onClick={on}>
      {label}
    </button>
  )
  return (
    <div className="sel-toolbar" role="toolbar" aria-label="Selection">
      {b(<Icon name="chevronLeft" size={12} />, 'Outdent', () => void exec('node.outdent', {}, panel))}
      {b(<Icon name="chevronRight" size={12} />, 'Indent', () => void exec('node.indent', {}, panel))}
      <span className="sel-div" />
      {b('# Tag', 'Add supertag', (e) => {
        const r = (e.currentTarget as HTMLElement).getBoundingClientRect()
        openPicker({ mode: 'tag', panel, node: row.target, at: -1, x: r.left, y: r.bottom })
      })}
      {b('Move', 'Move to', (e) => {
        const r = (e.currentTarget as HTMLElement).getBoundingClientRect()
        openPicker({ mode: 'moveTo', panel, node: row.id, at: -1, x: r.left, y: r.bottom })
      })}
      <span className="sel-div" />
      {b('H', 'Toggle heading', () => void exec('node.toggleHeading', { node: row.id }, panel))}
      <span className="sel-div" />
      {b(<Icon name="link" size={12} />, 'Copy node link', () => void navigator.clipboard?.writeText(`[[id:${row.target}]]`))}
      {b(<Icon name="sidebar" size={12} />, 'Open in new panel', () => void openInNewPanel(row.target))}
      {b(<Icon name="more" size={12} />, 'Node menu', (e) => {
        const r = (e.currentTarget as HTMLElement).getBoundingClientRect()
        setState({ nodeMenu: { panel, node: row.id, x: r.left, y: r.bottom + 4 } })
      })}
    </div>
  )
}

// ---- descriptions (spec 02 §13) --------------------------------------------------------------------

function DescEditor({ row, panel }: { row: NodeRow; panel: string }) {
  const [value, setValue] = useState(row.description)
  const ref = useRef<HTMLTextAreaElement>(null)
  useLayoutEffect(() => {
    const el = ref.current
    if (!el) return
    el.focus()
    el.setSelectionRange(el.value.length, el.value.length)
  }, [])
  useLayoutEffect(() => {
    const el = ref.current
    if (el) {
      el.style.height = '0px'
      el.style.height = `${el.scrollHeight}px`
    }
  })
  const leave = (toLine: boolean) => {
    setState({ descEdit: null })
    if (toLine) void exec('focus.set', { node: row.id, caret: 1e9 }, panel)
  }
  return (
    <textarea
      ref={ref}
      className="desc-input"
      value={value}
      rows={1}
      placeholder="Add description"
      onChange={(e) => {
        const v = e.target.value.replace(/\n/g, ' ')
        setValue(v)
        // Saved as typed, like a line.
        void call('run', { command: 'node.setDescription', params: { node: row.id, text: v }, panel }).then(() => scheduleQuietRefresh())
      }}
      onKeyDown={(e) => {
        e.stopPropagation()
        if (e.key === 'Escape' || e.key === 'ArrowUp' || e.key === 'Enter') {
          e.preventDefault()
          leave(true)
        }
      }}
      onBlur={() => setTimeout(() => getState().descEdit?.node === row.id && document.activeElement !== ref.current && setState({ descEdit: null }), 0)}
    />
  )
}

// ---- drag and drop (spec 02 §9; 11 §10) -----------------------------------------------------------

let dragging: number[] = []
let dropState: { id: number; nest: boolean } | null = null
const dropSubs = new Set<() => void>()

function setDrop(d: { id: number; nest: boolean } | null) {
  if (d?.id === dropState?.id && d?.nest === dropState?.nest) return
  dropState = d
  dropSubs.forEach((f) => f())
}

function useDrop(id: number): { nest: boolean } | null {
  const [, force] = useState(0)
  useEffect(() => {
    const f = () => force((x) => x + 1)
    dropSubs.add(f)
    return () => {
      dropSubs.delete(f)
    }
  }, [])
  return dropState?.id === id ? dropState : null
}

/** Drop on a row: after it, or (right of its bullet by an indent) as its first child. */
function dropHandlers(row: NodeRow, panel: string) {
  return {
    onDragOver(e: React.DragEvent) {
      if (!dragging.length || dragging.includes(row.id)) return
      e.preventDefault()
      const box = (e.currentTarget as HTMLElement).querySelector('.bullet')?.getBoundingClientRect()
      // Just right of the bullet, by about an indent: nest. Further right (over the text): after.
      const nest = !!box && e.clientX > box.left + 32 && e.clientX < box.left + 32 * 3
      setDrop({ id: row.id, nest })
    },
    onDragLeave() {
      setDrop(null)
    },
    onDrop(e: React.DragEvent) {
      e.preventDefault()
      const d = dropState
      setDrop(null)
      const nodes = dragging
      dragging = []
      if (!d || !nodes.length || nodes.includes(row.id)) return
      const parent = d.nest ? row.id : row.parent
      const index = d.nest ? 0 : (row.index ?? 0) + 1
      if (parent == null) return
      void exec('node.moveMany', { nodes, parent, index }, panel)
    },
  }
}

export function ago(secs: number) {
  const d = Date.now() / 1000 - secs
  if (d < 60) return 'just now'
  if (d < 3600) return `${Math.floor(d / 60)} minutes ago`
  if (d < 86400) return `${Math.floor(d / 3600)} hours ago`
  return `${Math.floor(d / 86400)} days ago`
}

function NodeLine({ row, panel, page, selected, editing }: { row: NodeRow; panel: string; page: Page; selected: boolean; editing: boolean }) {
  const drop = useDrop(row.id)
  const descEdit = useStore((s) => s.descEdit)
  const describing = descEdit?.panel === panel && descEdit.node === row.id
  const blank = !editing && !row.ref && !row.hasKids && !row.name && !row.tags.length && row.kind === 'plain'
  return (
    <div
      className={`row node${selected ? ' selected' : ''}${editing ? ' editing' : ''}${row.done ? ' done' : ''}${row.heading ? ' heading' : ''}`}
      style={{ paddingLeft: `calc(${row.depth} * var(--indent))` }}
      data-id={row.id}
      data-depth={row.depth}
      {...dropHandlers(row, panel)}
    >
      <Guides depth={row.depth} />
      {drop && <span className={`drop-line${drop.nest ? ' nest' : ''}`} style={{ '--d': row.depth } as CSSProperties} />}
      {selected && page.sel[0] === row.id && <SelectionToolbar row={row} panel={panel} />}
      <div className="row-inner">
        {row.hasKids && (
          <span
            className="expander"
            title={row.expanded ? 'Collapse' : 'Expand'}
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => void exec('node.toggleExpand', { node: row.id }, panel)}
          >
            <Icon name={row.expanded ? 'chevronDown' : 'chevronRight'} size={10} />
          </span>
        )}
        {blank ? <span className="bullet blank" /> : <Bullet row={row} panel={panel} onClick={() => void exec('nav.zoomIn', { node: row.id }, panel)} />}
        <span className="line">
          {row.done !== null && (
            <span
              className={`checkbox${row.done ? ' on' : ''}`}
              role="checkbox"
              aria-checked={row.done}
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => void exec('node.toggleDone', { node: row.id }, panel)}
            >
              {row.done && <Icon name="check" size={12} />}
            </span>
          )}
          {row.icon && <span className="node-icon">{row.icon}</span>}
          {editing ? (
            <LineEditor row={row} panel={panel} page={page} />
          ) : (
            <span
              className="text"
              onMouseDown={(e) => {
                if (e.button !== 0) return
                e.preventDefault()
                setActivePanel(panel)
                if (e.shiftKey) {
                  const from = page.sel[0] ?? page.focus ?? row.id
                  void exec('select.set', { from, to: row.id }, panel)
                } else if (row.ref) {
                  void exec('nav.open', { node: row.target }, panel)
                } else {
                  setGhost(panel, row.id, false)
                  focusLine(panel, row.id, caretFromPoint(e, row.name))
                }
              }}
            >
              {row.name ? <RichText text={row.name} refs={page.refs} onRef={(id) => void exec('nav.open', { node: id }, panel)} /> : '\u200b'}
            </span>
          )}
          {row.suffix.map(([t, ph], i) => (
            <span key={i} className={ph ? 'suffix placeholder' : 'suffix'}>
              {t}
            </span>
          ))}
          {row.tags.map((t) => (
            <TagChip key={t.id} chip={t} onOpen={() => void exec('nav.open', { node: t.id }, panel)} />
          ))}
          {row.progress && (
            <span className="progress" title={`${row.progress[0]} of ${row.progress[1]} done`}>
              <span style={{ width: `${row.progress[1] ? (100 * row.progress[0]) / row.progress[1] : 0}%` }} />
            </span>
          )}
        </span>
        {(row.description || describing) && (
          <span className="desc">{describing ? <DescEditor row={row} panel={panel} /> : <span onMouseDown={(e) => (e.preventDefault(), setState({ descEdit: { panel, node: row.id } }))}>{row.description}</span>}</span>
        )}
      </div>
    </div>
  )
}

const FIELD_ICONS: Record<string, IconName> = { date: 'calendar', checkbox: 'check', url: 'link', options: 'list', options_from_tag: 'hash', number: 'hash', user: 'user', email: 'field', plain: 'field' }

function FieldLine({ row, panel }: { row: FieldRow; panel: string }) {
  const fe = useStore((s) => s.fieldEdit)
  const editing = fe && fe.panel === panel && fe.owner === row.owner && fe.field === row.field ? fe.index : null
  const edit = (i: number) => {
    setActivePanel(panel)
    void startFieldEdit(panel, row.owner, row.field, i)
  }
  const values = row.values
  const lines = editing !== null && editing >= values.length ? [...values, null] : values
  return (
    <div className={`row field${row.boxed ? ' boxed' : ''}`} style={{ paddingLeft: `calc(${row.depth} * var(--indent))` }} data-owner={row.owner} data-field={row.field}>
      <Guides depth={row.depth} />
      <div className="row-inner">
        <span className="field-icon-slot">
          <Icon name={FIELD_ICONS[row.type] ?? 'field'} size={13} />
        </span>
        <span className="field-name" onClick={() => void exec('nav.open', { node: row.field }, panel)} title={row.name}>
          {row.name}
        </span>
        <span className="field-values">
          {lines.length === 0 && (
            <span className="field-value field-empty" onMouseDown={(e) => (e.preventDefault(), edit(0))}>
              {FIELD_PLACEHOLDER[row.type] ?? 'Empty'}
            </span>
          )}
          {lines.map((v, i) =>
            editing === i ? (
              <span key={i} className="field-value">
                <span className="bullet">
                  <span className="dot" />
                </span>
                <FieldEditorCell row={row} panel={panel} index={i} initial={v?.raw ?? ''} />
              </span>
            ) : v && v.checked !== null ? (
              <span key={i} className="field-value">
                <span
                  className={`checkbox${v.checked ? ' on' : ''}`}
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={() => void exec('field.set', { node: row.owner, field: row.field, index: i, value: v.checked ? 'no' : 'yes' }, panel)}
                >
                  {v.checked && <Icon name="check" size={12} />}
                </span>
              </span>
            ) : (
              <span key={i} className="field-value" onMouseDown={(e) => (e.preventDefault(), edit(i))}>
                <span className="bullet">
                  <span className="dot" />
                </span>
                <span className="field-text">{v?.text}</span>
              </span>
            ),
          )}
        </span>
        {(row.missing || row.invalid) && (
          <span className="field-warn" title={row.invalid ? 'Not a valid value for this field type' : 'Required'}>
            ?
          </span>
        )}
      </div>
    </div>
  )
}

const FIELD_PLACEHOLDER: Record<string, string> = { date: 'Add date', options: 'Select option', options_from_tag: 'Select option', user: 'Select user', number: 'Add number', url: 'Add link', email: 'Add email' }
const PICKS = new Set(['options', 'options_from_tag', 'user'])

/** A field value being typed: saved as typed (DIFFS N15), one value per line. */
export function FieldEditorCell({ row, panel, index, initial }: { row: FieldRow; panel: string; index: number; initial: string }) {
  const [value, setValue] = useState(initial)
  const ref = useRef<HTMLInputElement>(null)
  const valueRef = useRef(value)
  valueRef.current = value
  const send = (v: string) => {
    void call('run', { command: 'field.set', params: { node: row.owner, field: row.field, index, value: v, keep: true }, panel }).then(() => scheduleQuietRefresh())
    if (PICKS.has(row.type)) {
      const el = ref.current
      const r = el?.getBoundingClientRect()
      const p = getState().picker
      if (p?.mode === 'choice' && p.field === row.field && p.node === row.owner) setPickerQuery(v)
      else openPicker({ mode: 'choice', panel, node: row.owner, field: row.field, index, at: -1, query: v, x: r?.left ?? 0, y: (r?.bottom ?? 0) + 2 })
    }
  }
  useLayoutEffect(() => {
    ref.current?.focus()
    if (PICKS.has(row.type)) send(valueRef.current)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])
  useEffect(() => {
    const h: EditorHandle = {
      kind: 'field',
      panel,
      node: row.owner,
      sel(): [number, number] {
        const el = ref.current
        const v = valueRef.current
        return el ? [toChars(v, el.selectionStart ?? 0), toChars(v, el.selectionEnd ?? 0)] : [charLen(v), charLen(v)]
      },
      value: () => valueRef.current,
      insert(text: string) {
        const el = ref.current
        const v = valueRef.current
        const a = el?.selectionStart ?? v.length
        const b = el?.selectionEnd ?? v.length
        const next = v.slice(0, a) + text + v.slice(b)
        valueRef.current = next
        setValue(next)
        if (el) {
          el.value = next
          el.setSelectionRange(a + text.length, a + text.length)
        }
        send(next)
      },
      setSel() {},
      focus() {
        ref.current?.focus({ preventScroll: true })
      },
      native(k: { key: string; code: string; metaKey: boolean; ctrlKey: boolean }) {
        const el = ref.current
        if (el && (k.metaKey || k.ctrlKey) && k.code === 'KeyA') {
          el.setSelectionRange(0, el.value.length)
          return
        }
        if (k.key === 'Backspace' && el && el.selectionStart !== el.selectionEnd) {
          const v = valueRef.current
          const next = v.slice(0, el.selectionStart ?? 0) + v.slice(el.selectionEnd ?? 0)
          valueRef.current = next
          setValue(next)
          send(next)
          return
        }
        if (k.key === 'Backspace') {
          const v = valueRef.current
          const next = v.slice(0, -1)
          valueRef.current = next
          setValue(next)
          send(next)
        }
      },
    }
    registerEditor(h)
    return () => unregisterEditor(h)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [row.owner, row.field, index])
  return (
    <input
      ref={ref}
      className="editor-input field-input"
      value={value}
      spellCheck={false}
      placeholder={FIELD_PLACEHOLDER[row.type] ?? ''}
      data-field-edit={`${row.owner}:${row.field}:${index}`}
      onChange={(e) => {
        setValue(e.target.value)
        valueRef.current = e.target.value
        send(e.target.value)
      }}
      onBlur={() => {
        // Clicking elsewhere ends typing; the value is already in the engine.
        setTimeout(() => {
          const fe = getState().fieldEdit
          if (fe && fe.owner === row.owner && fe.field === row.field && fe.index === index && !getState().picker && document.activeElement !== ref.current)
            setState({ fieldEdit: null })
        }, 0)
      }}
    />
  )
}

function MoreLine({ row, panel }: { row: MoreRow; panel: string }) {
  return (
    <div className="row more" style={{ paddingLeft: `calc(${row.depth} * var(--indent) + var(--text-from-bullet) - 4.8px)` }}>
      {row.fields.map((f) => (
        <span
          key={f.field}
          className="more-field"
          onClick={() => void exec('field.add', { node: row.owner, field: f.field }, panel)}
        >
          + {f.name}
        </span>
      ))}
    </div>
  )
}

function RowView({ row, panel, page, active }: { row: Row; panel: string; page: Page; active: boolean }) {
  if (row.k === 'node') {
    const s = getState()
    const pf = s.activePanel === panel && !s.fieldEdit
    return <NodeLine row={row} panel={panel} page={page} selected={page.sel.includes(row.id)} editing={active && pf && page.focus === row.id} />
  }
  if (row.k === 'field') return <FieldLine row={row} panel={panel} />
  if (row.k === 'more') return <MoreLine row={row} panel={panel} />
  return <div className="row" />
}

// ---- header ------------------------------------------------------------------------------------

function TitleEditor({ page, panel }: { page: Page; panel: string }) {
  const [value, setValue] = useState(page.name)
  const [editing, setEditing] = useState(false)
  const ref = useRef<HTMLTextAreaElement>(null)
  const pending = useRef(0)
  useEffect(() => {
    if (pending.current === 0 && !editing) setValue(page.name)
  }, [page.name, editing])
  useEffect(() => {
    registerTitle(panel, {
      panel,
      focus(atEnd = true) {
        setEditing(true)
        const el = ref.current
        if (!el) return
        el.focus()
        const n = atEnd ? el.value.length : 0
        el.setSelectionRange(n, n)
      },
      insert(text: string) {
        const el = ref.current
        if (!el) return
        const v = el.value
        const next = v.slice(0, el.selectionStart) + text + v.slice(el.selectionEnd)
        const at = el.selectionStart + text.length
        el.value = next
        el.setSelectionRange(at, at)
        send(next)
      },
    })
    return () => registerTitle(panel, null)
  }, [panel])
  const zoom = page.zoom
  function send(v: string) {
    setValue(v)
    pending.current++
    void call('run', { command: 'node.setName', params: { node: zoom, name: v }, panel }).then(() => {
      pending.current--
      if (pending.current === 0) scheduleQuietRefresh()
    })
  }
  useLayoutEffect(() => {
    const el = ref.current
    if (el) {
      el.style.height = '0px'
      el.style.height = `${el.scrollHeight}px`
    }
  })
  if (!page.titleEditable) {
    return <h1 className="title">{page.title}</h1>
  }
  return (
    <textarea
      ref={ref}
      className="title title-input"
      value={value}
      rows={1}
      spellCheck={false}
      placeholder="Untitled"
      data-title-of={page.zoom}
      data-title-panel={panel}
      onFocus={() => {
        setEditing(true)
        setActivePanel(panel)
      }}
      onBlur={() => setEditing(false)}
      onChange={(e) => send(e.target.value.replace(/\n/g, ' '))}
      onKeyDown={(e) => {
        const k = keyInfo(e)
        if ((k.key === 'Enter' || k.key === 'ArrowDown') && !k.metaKey && !k.ctrlKey) {
          e.preventDefault()
          e.currentTarget.blur()
          const first = page.rows.items.find((r) => r.k === 'node') as NodeRow | undefined
          if (k.key === 'ArrowDown' && first) void exec('focus.set', { node: first.id, caret: 0 }, panel)
          else void exec('node.enter', { node: page.zoom }, panel)
        } else if (k.key === 'Escape') {
          e.preventDefault()
          e.currentTarget.blur()
          const first = page.rows.items.find((r) => r.k === 'node') as NodeRow | undefined
          if (first) void exec('select.set', { from: first.id }, panel)
        }
        e.stopPropagation()
      }}
    />
  )
}

function Crumbs({ page, panel, many }: { page: Page; panel: string; many: boolean }) {
  const c = page.crumbs
  const shown: ({ id: number; name: string } | null)[] = c.length > 4 ? [c[0], null, ...c.slice(-2)] : c
  return (
    <div className="crumbs">
      <button className="icon-btn crumb-back" title="Back" disabled={!page.canBack} onClick={() => void exec('nav.back', {}, panel)}>
        <Icon name="chevronLeft" size={14} />
      </button>
      <button className="ws-avatar" title="Workspace: open the Library" onClick={() => void exec('nav.home', {}, panel)}>
        <Icon name="workspace" size={16} />
      </button>
      {shown.map((x, i) => (
        <span key={i} className="crumb-part">
          <span className="crumb-sep">/</span>
          {x ? (
            <button className="crumb" onClick={() => void exec('nav.open', { node: x.id }, panel)}>
              {x.name}
            </button>
          ) : (
            <span className="crumb more" title={c.slice(1, -2).map((h) => h.name).join(' / ')}>
              …
            </span>
          )}
        </span>
      ))}
      <span className="spacer" />
      {many ? (
        <button className="icon-btn" title="Close panel" onClick={() => void closePanel(panel)}>
          <Icon name="close" size={14} />
        </button>
      ) : (
        <button className="icon-btn" title="Content width">
          <Icon name="width" size={14} />
        </button>
      )}
    </div>
  )
}

function HeadActions() {
  return (
    <div className="head-actions">
      <button className="btn icon" title="AI (not available)">
        <Icon name="sparkle" size={12} />
      </button>
      <button className="btn icon" title="Options">
        <Icon name="more" size={12} />
      </button>
    </div>
  )
}

function DayNav({ panel }: { panel: string }) {
  return (
    <div className="day-nav">
      <span className="btn-group">
        <button className="btn icon" title="Previous day" onClick={() => void exec('nav.previousDay', {}, panel)}>
          <Icon name="chevronLeft" size={12} />
        </button>
        <button className="btn icon" title="Next day" onClick={() => void exec('nav.nextDay', {}, panel)}>
          <Icon name="chevronRight" size={12} />
        </button>
      </span>
      <span className="btn-group">
        <button className="btn" onClick={() => void exec('nav.today', {}, panel)}>
          Today
        </button>
        <button className="btn icon" title="Go to date" onClick={() => void exec('nav.today', {}, panel)}>
          <Icon name="calendar" size={12} />
        </button>
      </span>
    </div>
  )
}

/** The page's footer: where this node is referenced (spec 01 §3; 12: Tagged node page). */
function References({ page, panel }: { page: Page; panel: string }) {
  const [open, setOpen] = useState(true)
  const n = page.references.length
  return (
    <section className="references">
      <button className="references-head" onClick={() => setOpen((o) => !o)}>
        {n} reference{n === 1 ? '' : 's'}
        <Icon name={open ? 'chevronDown' : 'chevronRight'} size={12} />
      </button>
      {open && (
        <>
          <div className="references-group">Referenced in…</div>
          {page.references.map((r) => (
            <div key={r.id} className="reference-row" onClick={() => void exec('nav.open', { node: r.id }, panel)}>
              <span className="bullet closed">
                <span className="dot" />
              </span>
              <span className="reference-name">{r.name || 'Untitled'}</span>
              {r.path && <span className="reference-path">{r.path}</span>}
            </div>
          ))}
        </>
      )}
    </section>
  )
}

function RelatedCard({ page, panel }: { page: Page; panel: string }) {
  return (
    <>
      {page.related.map((r) => (
        <section key={r.id} className="side-card">
          <h3 onClick={() => void exec('nav.open', { node: r.id }, panel)}>{r.name}</h3>
          {r.items.length === 0 && <p className="muted">Nothing here yet</p>}
          {r.items.map((it) => (
            <div key={it.id} className="side-item">
              <Bullet row={{ ...it, k: 'node', depth: 0, key: '' } as NodeRow} onClick={() => void exec('nav.open', { node: it.target }, panel)} />
              {it.done !== null && (
                <span className={`checkbox${it.done ? ' on' : ''}`} onClick={() => void exec('node.toggleDone', { node: it.target }, panel)}>
                  {it.done && <Icon name="check" size={12} />}
                </span>
              )}
              <span className="side-text" onClick={() => void exec('nav.open', { node: it.target }, panel)}>
                <RichText text={it.name} refs={page.refs} />
                {it.tags.map((t) => (
                  <span key={t.id} className="chip mini" style={tagStyle(t.color)}>
                    #
                  </span>
                ))}
              </span>
            </div>
          ))}
        </section>
      ))}
    </>
  )
}

// ---- the panel -------------------------------------------------------------------------------------

export function Panel({ panel, many }: { panel: string; many: boolean }) {
  const page = useStore((s) => s.pages[panel])
  const active = useStore((s) => s.activePanel === panel)
  const scroller = useRef<HTMLDivElement>(null)
  const head = useRef<HTMLDivElement>(null)
  const [headH, setHeadH] = useState(0)
  useLayoutEffect(() => {
    if (!head.current) return
    const ro = new ResizeObserver(() => setHeadH(head.current?.offsetHeight ?? 0))
    ro.observe(head.current)
    return () => ro.disconnect()
  }, [])
  const total = page?.rows.total ?? 0
  const v = useVirtualizer({
    count: total,
    getScrollElement: () => scroller.current,
    estimateSize: () => 28,
    overscan: 12,
    scrollMargin: headH,
    getItemKey: (i) => {
      const it = page && page.rows.items[i - page.rows.start]
      return it ? it.key : `i${i}`
    },
  })
  const items = v.getVirtualItems()
  // Ask for another window of rows when the visible ones are not loaded.
  const first = items[0]?.index ?? 0
  const last = items[items.length - 1]?.index ?? 0
  useEffect(() => {
    if (!page) return
    const { start, items: loaded } = page.rows
    const end = start + loaded.length
    if (total > end - start && (first < start || last >= end)) {
      const s = Math.max(0, first - OVERSCAN_ROWS)
      setState((st) => ({ windows: { ...st.windows, [panel]: { start: s, count: WINDOW } } }))
      void refresh()
    }
  }, [first, last, page, panel, total])
  // The focused line scrolls into view when it is not drawn (DIFFS N32).
  const focusIndex = page?.rows.focusIndex
  const focusScroll = useStore((s) => s.focusScroll)
  const viewSnap = useStore((s) => s.views[panel])
  const special = useStore((s) => s.special[panel])
  useEffect(() => {
    if (focusIndex == null || !page) return
    if (focusIndex < first + 1 || focusIndex > last - 1) v.scrollToIndex(focusIndex, { align: 'center' })
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [focusIndex, page?.focus, focusScroll])

  if (!page) return <div className="panel" />
  if (special) {
    const title = { settings: 'Settings', recents: 'Recents', help: 'Help' }[special]
    return (
      <div className={`panel${active ? ' active' : ''}`} onMouseDown={() => setActivePanel(panel)} data-panel={panel}>
        <div className="crumbs">
          <button className="icon-btn crumb-back" title="Back" onClick={() => setState((s) => ({ special: { ...s.special, [panel]: undefined } }))}>
            <Icon name="chevronLeft" size={14} />
          </button>
        </div>
        <div className="scroller">
          <div className="page">
            <div className="page-main">
              <h1 className="title">{title}</h1>
              <div className="special-body">
                {special === 'settings' && <SettingsPage panel={panel} />}
                {special === 'recents' && <RecentsPage panel={panel} />}
                {special === 'help' && <HelpPage />}
              </div>
            </div>
          </div>
        </div>
      </div>
    )
  }
  const wide = page.related.length > 0
  const vs = viewSnap && viewSnap.node === page.zoom ? viewSnap : null
  const layout = vs && vs.kind !== 'outline' ? vs : null
  const schema = page.kind === 'schema'
  return (
    <div className={`panel${active ? ' active' : ''}`} onMouseDown={() => setActivePanel(panel)} data-panel={panel}>
      <Crumbs page={page} panel={panel} many={many} />
      <div className="scroller" ref={scroller}>
        <div className={`page${wide ? ' with-side' : ''}${layout && layout.kind !== 'list' ? ' full' : ''}`}>
          <div className="page-main">
            <div ref={head} className="page-head">
              <div className="title-row">
                {page.icon && <span className="page-icon">{page.icon}</span>}
                {page.kind === 'tag' && (
                  <span className="tag-square title" style={tagStyle(page.tagColor ?? 'grey')}>
                    #
                  </span>
                )}
                <TitleEditor key={page.zoom} page={page} panel={panel} />
                {page.tags.length === 0 && <HeadActions />}
              </div>
              {page.tags.length > 0 && (
                <div className="head-meta">
                  <div className="tag-row">
                    {page.tags.map((t) => (
                      <TagChip key={t.id} chip={t} onOpen={() => void exec('nav.open', { node: t.id }, panel)} />
                    ))}
                  </div>
                  <HeadActions />
                </div>
              )}
              {page.description && <p className="page-desc">{page.description}</p>}
              {page.kind === 'day' && <DayNav panel={panel} />}
              {page.kind === 'trash' && <p className="banner-note">Deleted nodes wait here. Restore them from their menu, or empty the trash.</p>}
              {page.kind === 'tag' && vs && <TagHeader panel={panel} tag={page.target} color={page.tagColor ?? 'grey'} tabs={vs.tabs} active={page.zoom} />}
              {vs && !layout && <QueryBuilder v={vs} panel={panel} />}
              {vs && !layout && vs.toolbar && <ViewToolbar v={vs} panel={panel} />}
            </div>
            {schema && <SupertagsPage panel={panel} />}
            {layout && <ViewBody v={layout} panel={panel} />}
            <div className="rows" style={{ height: layout || schema ? 0 : v.getTotalSize() - headH, display: layout || schema ? 'none' : undefined }}>
              {items.map((vi) => {
                const row = page.rows.items[vi.index - page.rows.start]
                return (
                  <div
                    key={vi.key}
                    data-index={vi.index}
                    ref={v.measureElement}
                    className="vrow"
                    style={{ transform: `translateY(${vi.start - headH}px)` }}
                  >
                    {row ? <RowView row={row} panel={panel} page={page} active={active} /> : <div className="row placeholder" />}
                  </div>
                )
              })}
            </div>
            {!layout && !schema && <div
              className="row ghost"
              onMouseDown={(e) => {
                e.preventDefault()
                setActivePanel(panel)
                void withBarrier(async () => {
                  const r = await exec('node.add', { parent: page.zoom }, panel)
                  const id = r?.result?.id
                  if (id != null) await exec('focus.set', { node: id, caret: 0 }, panel)
                })
              }}
            >
              <span className="bullet">
                <span className="dot faint" />
              </span>
              <span className="ghost-hint">Click to add a node</span>
            </div>}
            {page.filteredOut > 0 && <p className="muted small">{page.filteredOut} items filtered out</p>}
            {page.references.length > 0 && <References page={page} panel={panel} />}
          </div>
          {wide && (
            <aside className="page-side">
              <RelatedCard page={page} panel={panel} />
            </aside>
          )}
        </div>
      </div>
    </div>
  )
}
