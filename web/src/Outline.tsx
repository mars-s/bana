// A panel: breadcrumbs, the page header and the outline (virtualized: only rows near the
// viewport are in the DOM; a row never holds unsaved text, every keystroke is already sent).

import { Fragment, useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type MouseEvent as RME } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import type { Chip, FieldRow, MoreRow, NodeRow, Page, Row } from './engine'
import { call } from './engine'
import { Icon, type IconName } from './icons'
import { type Span, charLen, rawSpans, toChars, toUtf16 } from './format'
import { getState, openInNewPanel, refresh, setActivePanel, setState, showToast, closeConfigure, closePanel, useStore } from './store'
import { type EditorHandle, currentEditor, exec, focusLine, keyFromTitle, outlineVisible, runUiCommand, pasteInto, noteSynced, registerEditor, registerTitle, setGhost, unregisterEditor, withBarrier } from './controller'
import { onLineText, openPicker, setPickerQuery, startFieldEdit } from './picker'
import { QueryBuilder, SearchOptions, ViewBody, ViewTitle, ViewToolbar } from './Views'
import orbitBanner from './assets/banner-orbit.webp'
import { HelpPage, RecentsPage, SettingsPage, SupertagsPage, TagHeader } from './Pages'
import { ConfigureTag, FieldPage } from './Schema'
import { type KeyInfo, isTextKey, keyInfo, shortcut } from './keys'

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

/** A tag chip: two joined parts, "#" and the name. On hover, with `onRemove`, the "#" part turns
 * into a remove button and the name part's corners round (motion M12; N53). */
export function TagChip({ chip, onOpen, onRemove }: { chip: Chip; onOpen?: () => void; onRemove?: () => void }) {
  return (
    <span className={`chip${onRemove ? ' removable' : ''}`} style={tagStyle(chip.color)} onMouseDown={(e) => e.preventDefault()} data-tag={chip.name}>
      <span
        className="chip-hash"
        title={onRemove ? `Remove #${chip.name}` : undefined}
        onClick={(e) => {
          e.stopPropagation()
          if (onRemove) onRemove()
          else onOpen?.()
        }}
      >
        <span className="chip-hash-glyph">#</span>
        {onRemove && (
          <span className="chip-x">
            <Icon name="close" size={9} />
          </span>
        )}
      </span>
      <span className="chip-name" onClick={onOpen}>
        {chip.name}
      </span>
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

/** Where offset `i` of `before` sits in `after` when one splice turned one into the other: after
 * the change if it was before `i`, unmoved otherwise (UTF-16 offsets, as the textarea uses). */
function shiftedOffset(before: string, after: string, i: number): number {
  let pre = 0
  while (pre < before.length && pre < after.length && before[pre] === after[pre]) pre++
  let suf = 0
  while (suf < before.length - pre && suf < after.length - pre && before[before.length - 1 - suf] === after[after.length - 1 - suf]) suf++
  if (i <= pre) return i
  if (i >= before.length - suf) return i + after.length - before.length
  return after.length - suf
}

function LineEditor({ row, panel, page }: { row: NodeRow; panel: string; page: Page }) {
  const [value, setValue] = useState(row.name)
  const ta = useRef<HTMLTextAreaElement>(null)
  const pending = useRef(0)
  const valueRef = useRef(value)
  valueRef.current = value
  const caretEpoch = useStore((s) => s.caretEpoch)

  // The engine's text wins once every edit sent from here has arrived. Text another window typed
  // in this line keeps this window's caret where it was in its own words (N60).
  useEffect(() => {
    if (pending.current !== 0 || row.name === valueRef.current) return
    const el = ta.current
    if (el && document.activeElement === el) {
      const move = (i: number) => shiftedOffset(valueRef.current, row.name, i)
      const [a, b] = [move(el.selectionStart), move(el.selectionEnd)]
      el.value = row.name
      el.setSelectionRange(a, b)
    }
    valueRef.current = row.name
    setValue(row.name)
  }, [row.name])

  /** `base`: the text this edit started from, so the engine can merge it with another window's. */
  const sendName = (name: string, caret: number, base: string) => {
    pending.current++
    noteSynced(row.id, caret)
    void call('run', { command: 'node.setName', params: { node: row.id, name, caret, base }, panel }).then((r) => {
      pending.current--
      if (r?.error) setState({ toast: r.error })
      if (pending.current === 0) void scheduleQuietRefresh()
    })
  }

  /** New text in the line (typed, pasted or replayed): to the engine, and menus follow it. */
  const textChanged = (raw: string, caretUtf16: number) => {
    // Keys never insert line breaks (Enter is a command).
    const v = raw.replace(/\n/g, ' ')
    const base = valueRef.current
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
    warnIfReferenced(row, panel)
    sendName(v, caret, base)
  }

  // Put the caret where the engine says: on mount and after commands (not after typing).
  useLayoutEffect(() => {
    const el = ta.current
    if (!el) return
    el.focus({ preventScroll: true })
    // A command changed the text and put the caret in it (format marks, joins): take both.
    if (pending.current === 0 && row.name !== valueRef.current) {
      valueRef.current = row.name
      el.value = row.name
      setValue(row.name)
    }
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
      adopt(text: string, a: number, c: number) {
        const el = ta.current
        if (!el || pending.current > 0) return
        if (text !== valueRef.current) {
          valueRef.current = text
          el.value = text
          setValue(text)
        }
        const ua = toUtf16(text, a)
        const uc = toUtf16(text, c)
        el.setSelectionRange(Math.min(ua, uc), Math.max(ua, uc), ua > uc ? 'backward' : 'forward')
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
          sendName(next, toChars(next, s0), v)
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
        // Several nodes: a small "N nodes" badge follows the pointer.
        if (ids.length > 1) {
          const badge = document.createElement('div')
          badge.className = 'drag-badge'
          badge.textContent = `${ids.length} nodes`
          document.body.appendChild(badge)
          e.dataTransfer.setDragImage(badge, 0, 0)
          setTimeout(() => badge.remove(), 0)
        }
        dragging = ids
        dropSubs.forEach((f) => f())
      }}
      onDragEnd={() => {
        dragging = []
        setDrop(null)
        dropSubs.forEach((f) => f())
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
        // Every selected line gets the tag (UC-02.15).
        const page = getState().pages[panel]
        const rows = (page?.rows.items ?? []).filter((x): x is NodeRow => x.k === 'node' && (page?.sel ?? []).includes(x.id))
        openPicker({ mode: 'tag', panel, node: row.target, nodes: rows.map((x) => x.target), at: -1, x: r.left, y: r.bottom })
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
/** Where a drop lands: above or below a row at its depth, or as the first child of an expanded
 * row (spec 02 §6: the sideways offset never changes the depth; N44). */
type Drop = { id: number; where: 'above' | 'below' | 'child' }
let dropState: Drop | null = null
const dropSubs = new Set<() => void>()

function setDrop(d: Drop | null) {
  if (d?.id === dropState?.id && d?.where === dropState?.where) return
  dropState = d
  dropSubs.forEach((f) => f())
}

/** Re-renders on drag changes; returns this row's drop (if any) and whether it is being dragged. */
function useDrop(id: number): { drop: Drop | null; dragged: boolean } {
  const [, force] = useState(0)
  useEffect(() => {
    const f = () => force((x) => x + 1)
    dropSubs.add(f)
    return () => {
      dropSubs.delete(f)
    }
  }, [])
  return { drop: dropState?.id === id ? dropState : null, dragged: dragging.includes(id) }
}

function dropHandlers(row: NodeRow, panel: string) {
  return {
    onDragOver(e: React.DragEvent) {
      if (!dragging.length || dragging.includes(row.id)) return
      e.preventDefault()
      const r = (e.currentTarget as HTMLElement).getBoundingClientRect()
      const lower = e.clientY > r.top + r.height / 2
      // Below an expanded row with children is its first child; a collapsed row takes nothing in.
      const where = !lower ? 'above' : row.expanded && row.hasKids ? 'child' : 'below'
      setDrop({ id: row.id, where })
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
      dropSubs.forEach((f) => f())
      if (!d || !nodes.length || nodes.includes(row.id)) return
      const parent = d.where === 'child' ? row.id : row.parent
      const index = d.where === 'child' ? 0 : d.where === 'above' ? (row.index ?? 0) : (row.index ?? 0) + 1
      if (parent == null) return
      void withBarrier(async () => {
        await exec('node.moveMany', { nodes, parent, index }, panel)
        // Several nodes moved together stay selected (spec 02 §6).
        if (nodes.length > 1) await exec('select.set', { from: nodes[0], to: nodes[nodes.length - 1] }, panel)
      })
    },
  }
}

/** While dragging: a hint at the top of the panel (spec 02 §6, N45). */
export function DragHint() {
  const { dragged } = useDrop(-1)
  void dragged
  if (!dragging.length) return null
  return <div className="drag-hint">{dragging.length > 1 ? `Moving ${dragging.length} nodes: drop on a line` : 'Drop above or below a line, or under an open one to nest it'}</div>
}

export function ago(secs: number) {
  const d = Date.now() / 1000 - secs
  if (d < 60) return 'just now'
  if (d < 3600) return `${Math.floor(d / 60)} minutes ago`
  if (d < 86400) return `${Math.floor(d / 3600)} hours ago`
  return `${Math.floor(d / 86400)} days ago`
}

function NodeLine({ row, panel, page, selected, editing }: { row: NodeRow; panel: string; page: Page; selected: boolean; editing: boolean }) {
  const { drop, dragged } = useDrop(row.id)
  const descEdit = useStore((s) => s.descEdit)
  const describing = descEdit?.panel === panel && descEdit.node === row.id
  const blank = !editing && !row.ref && !row.hasKids && !row.name && !row.tags.length && row.kind === 'plain'
  return (
    <div
      className={`row node${selected ? ' selected' : ''}${editing ? ' editing' : ''}${row.done ? ' done' : ''}${row.heading ? ' heading' : ''}${dragged ? ' dragged' : ''}`}
      style={{ paddingLeft: `calc(${row.depth} * var(--indent))` }}
      data-id={row.id}
      data-depth={row.depth}
      {...dropHandlers(row, panel)}
    >
      <Guides depth={row.depth} />
      {drop && <span className={`drop-line ${drop.where}`} style={{ '--d': row.depth + (drop.where === 'child' ? 1 : 0) } as CSSProperties} />}
      {selected && page.sel[0] === row.id && <SelectionToolbar row={row} panel={panel} />}
      <div className="row-inner">
        {row.notesHead && <div className="notes-head">{row.notesHead}</div>}
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
        <span
          className="line"
          onMouseDown={(e) => {
            // The empty part of the row right of the text: the caret goes to the end of the line (N72).
            if (e.button !== 0 || e.target !== e.currentTarget || e.shiftKey || row.ref) return
            e.preventDefault()
            const ed = currentEditor()
            if (editing && ed?.kind === 'line' && ed.node === row.id) return ed.setSel(charLen(ed.value()), charLen(ed.value()))
            setActivePanel(panel)
            setGhost(panel, row.id, false)
            focusLine(panel, row.id, charLen(row.name))
          }}
        >
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
          {row.uses != null && (
            <span className="ref-count" title={`Referenced in ${row.uses} ${row.uses === 1 ? 'place' : 'places'}`}>
              {row.uses}
            </span>
          )}
          {row.tags.map((t) => (
            <TagChip key={t.id} chip={t} onOpen={() => void exec('nav.open', { node: t.id }, panel)} onRemove={() => void exec('tag.remove', { node: row.target, tag: t.id }, panel)} />
          ))}
          {row.progress && (
            <span className="progress" title={`${row.progress[0]} of ${row.progress[1]} done`}>
              <span style={{ width: `${row.progress[1] ? (100 * row.progress[0]) / row.progress[1] : 0}%` }} />
            </span>
          )}
        </span>
        {row.link && !editing && <LinkCard text={row.name} />}
        {(row.description || describing) && (
          <span className="desc">{describing ? <DescEditor row={row} panel={panel} /> : <span onMouseDown={(e) => (e.preventDefault(), setState({ descEdit: { panel, node: row.id } }))}>{row.description}</span>}</span>
        )}
      </div>
    </div>
  )
}

/** Link previews (Settings > Link previews, on by default): a line that is just a link shows a
 * card with its site under it (spec 08, URL embeds). */
function LinkCard({ text }: { text: string }) {
  const on = useStore((s) => s.sidebar?.urlEmbeds ?? true)
  const url = /\(([^)\s]+)\)\s*$/.exec(text)?.[1] ?? text.trim()
  if (!on || !/^https?:\/\//.test(url)) return null
  const site = url.replace(/^https?:\/\//, '').split('/')[0].replace(/^www\./, '')
  return (
    <span className="link-wrap">
    <a className="link-card" href={url} target="_blank" rel="noreferrer" onMouseDown={(e) => e.stopPropagation()}>
      <span className="link-site">
        <Icon name="globe" size={12} /> {site}
      </span>
      <span className="link-url">{url}</span>
    </a>
    </span>
  )
}

/** Settings > Warn before editing a referenced node: the first edit of such a node in this window
 * says where else it shows (spec 01 §3). */
const warned = new Set<number>()
function warnIfReferenced(row: NodeRow, panel: string) {
  if (warned.has(row.target)) return
  warned.add(row.target)
  if (!getState().sidebar?.warnReferenced) return
  void call('usedElsewhere', { panel, nodes: [row.target] }).then((r) => {
    const n = Number(r?.count ?? 0)
    if (n > 0) showToast(`This node also appears in ${n} other ${n === 1 ? 'place' : 'places'}; the edit shows everywhere`)
  })
}

const FIELD_ICONS: Record<string, IconName> = { date: 'fieldDate', checkbox: 'fieldCheck', url: 'fieldUrl', options: 'fieldOptions', options_from_tag: 'fieldInstance', number: 'fieldNumber', user: 'fieldUser', email: 'fieldEmail', plain: 'fieldText' }

function FieldLine({ row, panel }: { row: FieldRow; panel: string }) {
  const fe = useStore((s) => s.fieldEdit)
  const fs = useStore((s) => s.fieldSel)
  const selected = !!fs && fs.panel === panel && fs.owner === row.owner && fs.field === row.field
  const editing = fe && fe.panel === panel && fe.owner === row.owner && fe.field === row.field ? fe.index : null
  const edit = (i: number) => {
    setActivePanel(panel)
    void startFieldEdit(panel, row.owner, row.field, i)
  }
  const values = row.values
  const lines = editing !== null && editing >= values.length ? [...values, null] : values
  return (
    <div className={`row field${row.boxed ? ' boxed' : ''}${selected ? ' selected' : ''}`} style={{ paddingLeft: `calc(${row.depth} * var(--indent))` }} data-owner={row.owner} data-field={row.field}>
      <Guides depth={row.depth} />
      <div className="row-inner">
        <span className="field-icon-slot">
          <Icon name={FIELD_ICONS[row.type] ?? 'fieldText'} size={13} />
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
                {/* An option value shows the option marker (I-38, N59.24); others a bullet. */}
                {row.type === 'options' || row.type === 'options_from_tag' ? (
                  <span className="bullet option-mark">
                    <Icon name="option" size={13} />
                  </span>
                ) : (
                  <span className="bullet">
                    <span className="dot" />
                  </span>
                )}
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
export function FieldEditorCell({ row, panel, index, initial, pickDate }: { row: FieldRow; panel: string; index: number; initial: string; pickDate?: boolean }) {
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
  const input = (
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
          if (fe && fe.owner === row.owner && fe.field === row.field && fe.index === index && !getState().picker && document.activeElement !== ref.current && !document.activeElement?.closest('.date-pick-anchor'))
            setState({ fieldEdit: null })
        }, 0)
      }}
    />
  )
  if (row.type !== 'date') return input
  // Dates: typed (2026-10-12, 2026-10-12 09:00-10:00, 2026-10-12/2026-10-14), or picked.
  return (
    <span className="date-edit">
      {input}
      <DatePicker
        value={value}
        openAtStart={pickDate}
        onPick={(v) => {
          setValue(v)
          valueRef.current = v
          send(v)
          ref.current?.focus()
        }}
      />
    </span>
  )
}

const MONTH_SHORT = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec']
const pad2 = (n: number) => String(n).padStart(2, '0')
const isoOf = (y: number, m: number, d: number) => `${y}-${pad2(m)}-${pad2(d)}`
function isoWeekOf(iso: string): number {
  const d = new Date(`${iso}T00:00:00Z`)
  const day = (d.getUTCDay() + 6) % 7
  d.setUTCDate(d.getUTCDate() - day + 3)
  const first = new Date(Date.UTC(d.getUTCFullYear(), 0, 4))
  return 1 + Math.round(((d.getTime() - first.getTime()) / 86400000 - 3 + ((first.getUTCDay() + 6) % 7)) / 7)
}

/** The date picker (spec 03 §8): month grid with ISO weeks, today, the picked day, a start and
 * end time, Add end for a range of days, OK. */
function DatePicker({ value, onPick, title = 'Pick a date', className = 'date-pick', openAtStart }: { value: string; onPick: (v: string) => void; title?: string; className?: string; openAtStart?: boolean }) {
  const today = useStore((s) => s.sidebar?.todayDate) ?? new Date().toISOString().slice(0, 10)
  const [open, setOpen] = useState(false)
  const parsed = /^(\d{4}-\d{2}-\d{2})(?: (\d{2}:\d{2})(?:-(\d{2}:\d{2}))?)?(?:\/(\d{4}-\d{2}-\d{2}))?/.exec(value.trim())
  const [day, setDay] = useState<string | null>(null)
  const [end, setEnd] = useState<string | null>(null)
  const [pickingEnd, setPickingEnd] = useState(false)
  const [t1, setT1] = useState('')
  const [t2, setT2] = useState('')
  const [ym, setYm] = useState<[number, number]>([0, 0])
  // Opened from a table cell it floats over the table (whose rows clip what hangs out of them).
  const anchor = useRef<HTMLSpanElement>(null)
  const [fixed, setFixed] = useState<CSSProperties | null>(null)
  const start = () => {
    const r = anchor.current?.getBoundingClientRect()
    if (openAtStart && r) setFixed({ position: 'fixed', top: r.bottom + 4, right: window.innerWidth - r.right, left: 'auto' })
    const d = parsed?.[1] ?? null
    setDay(d)
    setEnd(parsed?.[4] ?? null)
    setT1(parsed?.[2] ?? '')
    setT2(parsed?.[3] ?? '')
    setPickingEnd(false)
    const base = d ?? today
    setYm([Number(base.slice(0, 4)), Number(base.slice(5, 7))])
    setOpen(true)
  }
  // A date cell in a table opens straight into the picker (N59.21).
  useEffect(() => {
    if (openAtStart) start()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])
  const [y, m] = ym
  const first = isoOf(y, m || 1, 1)
  const lead = (new Date(`${first}T00:00:00Z`).getUTCDay() + 6) % 7
  const inMonth = new Date(Date.UTC(y, m, 0)).getUTCDate()
  const cells = Array.from({ length: Math.ceil((lead + inMonth) / 7) * 7 }, (_, i) => {
    const d = new Date(Date.UTC(y, m - 1, 1 - lead + i))
    return d.toISOString().slice(0, 10)
  })
  const step = (n: number) => setYm(([yy, mm]) => (mm + n < 1 ? [yy - 1, 12] : mm + n > 12 ? [yy + 1, 1] : [yy, mm + n]))
  const ok = () => {
    if (!day) return setOpen(false)
    let v = day
    if (end && end > day) v = `${day}/${end}`
    else if (t1) v = `${day} ${t1}${t2 ? `-${t2}` : ''}`
    setOpen(false)
    onPick(v)
  }
  return (
    <span className="date-pick-anchor" ref={anchor}>
      <button className={className} title={title} tabIndex={-1} onMouseDown={(e) => e.preventDefault()} onClick={() => (open ? setOpen(false) : start())}>
        <Icon name="calendar" size={12} />
      </button>
      {open && (
        <div className="menu pop date-picker" role="dialog" aria-label="Date picker" style={fixed ?? undefined} onMouseDown={(e) => e.preventDefault()}>
          <button className="dp-close" title="Close" onClick={() => setOpen(false)}>
            <Icon name="close" size={9} />
          </button>
          <div className="dp-head">
            <button className="icon-btn" title="Previous month" onClick={() => step(-1)}>
              <Icon name="chevronLeft" size={12} />
            </button>
            <span className="dp-chip">{y}</span>
            <span className="dp-chip">{MONTH_SHORT[m - 1]}</span>
            <button className="icon-btn" title="Next month" onClick={() => step(1)}>
              <Icon name="chevronRight" size={12} />
            </button>
          </div>
          <div className="dp-grid">
            <span />
            {['M', 'T', 'W', 'T', 'F', 'S', 'S'].map((d, i) => (
              <span key={i} className="dp-dow">
                {d}
              </span>
            ))}
            {cells.map((d, i) => (
              <Fragment key={d}>
                {i % 7 === 0 && <span className="dp-week">W{isoWeekOf(d)}</span>}
                <button
                  className={`dp-day${d.slice(5, 7) !== pad2(m) ? ' out' : ''}${d === today ? ' today' : ''}${d === day || d === end ? ' sel' : ''}${day && end && d > day && d < end ? ' range' : ''}`}
                  onClick={() => {
                    if (pickingEnd && day && d >= day) (setEnd(d), setPickingEnd(false))
                    else (setDay(d), setEnd(null))
                  }}
                >
                  {Number(d.slice(8))}
                </button>
              </Fragment>
            ))}
          </div>
          <div className="dp-foot">
            <input className="dp-time" type="time" aria-label="Start time" value={t1} onChange={(e) => setT1(e.target.value)} />
            {t1 && <input className="dp-time" type="time" aria-label="End time" value={t2} onChange={(e) => setT2(e.target.value)} />}
            <button className="btn" onClick={() => setPickingEnd(true)} disabled={!day}>
              {end ? `To ${Number(end.slice(8))} ${MONTH_SHORT[Number(end.slice(5, 7)) - 1]}` : 'Add end'}
            </button>
            <span className="spacer" />
            <button className="btn regular" onClick={ok}>
              OK
            </button>
          </div>
        </div>
      )}
    </span>
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

/** The empty child line of an open node with no children: click to start one, or drop a node on
 * it to put it inside (spec 02 §6, §7; N45). */
function EmptyLine({ parent, depth, panel }: { parent: number; depth: number; panel: string }) {
  const [over, setOver] = useState(false)
  return (
    <div
      className="row empty-child"
      style={{ paddingLeft: `calc(${depth} * var(--indent))` }}
      onMouseDown={(e) => {
        e.preventDefault()
        setActivePanel(panel)
        void withBarrier(async () => {
          const r = await exec('node.add', { parent, index: 0, name: '' }, panel)
          if (r?.result?.id != null) await exec('focus.set', { node: r.result.id, caret: 0 }, panel)
        })
      }}
      onDragOver={(e) => {
        if (!dragging.length || dragging.includes(parent)) return
        e.preventDefault()
        setOver(true)
      }}
      onDragLeave={() => setOver(false)}
      onDrop={(e) => {
        e.preventDefault()
        setOver(false)
        const nodes = dragging
        dragging = []
        dropSubs.forEach((f) => f())
        if (nodes.length && !nodes.includes(parent)) void exec('node.moveMany', { nodes, parent, index: 0 }, panel)
      }}
    >
      <Guides depth={depth} />
      {over && <span className="drop-line above" style={{ '--d': depth } as CSSProperties} />}
      <span className="bullet">
        <span className="dot faint" />
      </span>
    </div>
  )
}

function RowView({ row, panel, page, active }: { row: Row; panel: string; page: Page; active: boolean }) {
  if (row.k === 'empty') return <EmptyLine parent={row.parent} depth={row.depth} panel={panel} />
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
  const pageRef = useRef(page)
  pageRef.current = page
  useEffect(() => {
    if (pending.current === 0 && !editing) setValue(page.name)
  }, [page.name, editing])
  function send(v: string) {
    setValue(v)
    pending.current++
    void call('run', { command: 'node.setName', params: { node: pageRef.current.zoom, name: v }, panel }).then(() => {
      pending.current--
      if (pending.current === 0) scheduleQuietRefresh()
    })
  }
  /** The title's own keys (true: used). Live key presses and keys replayed from the buffer both
   * come here, so a replayed Down leaves the title just as a typed one does. */
  function titleKey(k: KeyInfo, el: HTMLTextAreaElement): boolean {
    const page = pageRef.current
    const sc = shortcut(k)
    const first = () => page.rows.items.find((r) => r.k === 'node') as NodeRow | undefined
    // spec-gap: the title is a node editor (02 §1): Cmd+Shift+Up/Down move the page node, and
    // Tab does nothing (it would otherwise move the keyboard to a button and drop what is typed).
    if (sc === 'Mod+Shift+Up' || sc === 'Mod+Shift+Down') {
      void exec(sc.endsWith('Up') ? 'node.moveUp' : 'node.moveDown', { node: page.zoom }, panel)
      return true
    }
    if (sc === 'Tab' || sc === 'Shift+Tab') return true
    // Commands work from the title too (Cmd+K, zoom out, back; N40): the title lets go first.
    if ((k.metaKey || k.ctrlKey) && !['Meta', 'Control', 'Shift', 'Alt'].includes(k.key) && !TITLE_TEXT_KEYS.test(sc)) {
      el.blur()
      keyFromTitle(panel, k)
      return true
    }
    if ((k.key === 'Enter' || k.key === 'ArrowDown') && !k.metaKey && !k.ctrlKey) {
      el.blur()
      if (!outlineVisible(panel)) return true
      const f = first()
      if (k.key === 'ArrowDown' && f) void exec('focus.set', { node: f.id, caret: 0 }, panel)
      else void exec('node.enter', { node: page.zoom }, panel)
      return true
    }
    if (k.key === 'Escape') {
      el.blur()
      const f = first()
      if (f && outlineVisible(panel)) void exec('select.set', { from: f.id }, panel)
      return true
    }
    return false
  }
  const titleKeyRef = useRef(titleKey)
  titleKeyRef.current = titleKey
  useEffect(() => {
    const insert = (text: string) => {
      const el = ref.current
      if (!el) return
      const v = el.value
      const next = v.slice(0, el.selectionStart) + text + v.slice(el.selectionEnd)
      const at = el.selectionStart + text.length
      el.value = next
      el.setSelectionRange(at, at)
      send(next)
    }
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
      insert,
      key(k: KeyInfo) {
        const el = ref.current
        if (!el || titleKeyRef.current(k, el)) return
        if (isTextKey(k)) return insert(k.key)
        // Plain editing keys that waited in the buffer.
        const v = el.value
        let s0 = el.selectionStart
        let s1 = el.selectionEnd
        if (k.key === 'Backspace' || k.key === 'Delete') {
          if (s0 === s1) {
            if (k.key === 'Backspace') s0 = Math.max(0, s0 - 1)
            else s1 = Math.min(v.length, s1 + 1)
          }
          el.value = v.slice(0, s0) + v.slice(s1)
          el.setSelectionRange(s0, s0)
          send(el.value)
        } else if (k.key === 'ArrowLeft') el.setSelectionRange(Math.max(0, s0 - 1), Math.max(0, s0 - 1))
        else if (k.key === 'ArrowRight') el.setSelectionRange(Math.min(v.length, s1 + 1), Math.min(v.length, s1 + 1))
        else if (k.key === 'Home' || k.key === 'ArrowUp') el.setSelectionRange(0, 0)
        else if (k.key === 'End') el.setSelectionRange(v.length, v.length)
      },
    })
    return () => registerTitle(panel, null)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [panel])
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
        if (titleKey(keyInfo(e), e.currentTarget)) e.preventDefault()
        e.stopPropagation()
      }}
    />
  )
}

/** Chords that edit the title's text and stay with it. */
const TITLE_TEXT_KEYS = /^Mod\+(Shift\+)?(A|C|V|X|Z|Y|Left|Right|Up|Down|Backspace|Delete)$/

function Crumbs({ page, panel, many }: { page: Page; panel: string; many: boolean }) {
  // A Configure supertag panel closes here (spec 12 has no Done button).
  const configuring = useStore((s) => !!s.configure[panel]) && page.kind === 'tag'
  const wsName = useStore((s) => s.sidebar?.workspaces[0]?.name ?? '')
  const c = page.crumbs
  const shown: ({ id: number; name: string } | null)[] = c.length > 4 ? [c[0], null, ...c.slice(-2)] : c
  return (
    <div className="crumbs">
      {/* The Supertags overview has an empty breadcrumb row (N69). */}
      {page.kind !== 'schema' && (
        <>
          <button className="icon-btn crumb-back" title="Back" disabled={!page.canBack} onClick={() => void exec('nav.back', {}, panel)}>
            <Icon name="chevronLeft" size={14} />
          </button>
          {/* The workspace's 15 px initials avatar (P04; N59.5). */}
          <button className="ws-avatar" title="Workspace: open the Library" onClick={() => void exec('nav.home', {}, panel)}>
            <span className="ws-dot">{(wsName || 'W').slice(0, 1).toUpperCase()}</span>
          </button>
        </>
      )}
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
      {many || configuring ? (
        <button className="icon-btn" title="Close panel" onClick={() => void (configuring ? closeConfigure(panel) : closePanel(panel))}>
          <Icon name="close" size={14} />
        </button>
      ) : (
        <button className="crumb-tool" title="Content width">
          <Icon name="width" size={12} />
        </button>
      )}
    </div>
  )
}

/** A page banner: a shipped picture, or a band in a tag colour. */
function Banner({ banner }: { banner: string }) {
  if (banner === 'picture:orbit') return <img className="page-banner" src={orbitBanner} alt="" />
  return <div className="page-banner colour" style={tagStyle(banner)} />
}

/** "Icon" and "Banner" buttons that show over the title on hover (spec 00 §4; 14 M15). */
function Decorate({ page, panel }: { page: Page; panel: string }) {
  const [menu, setMenu] = useState<'icon' | 'banner' | null>(null)
  const [emoji, setEmoji] = useState('')
  return (
    <div className={`decorate${menu ? ' open' : ''}`}>
      <span className="menu-anchor">
        <button className="ghost-btn" onClick={() => setMenu(menu === 'icon' ? null : 'icon')}>
          <Icon name="face" size={12} /> Icon
        </button>
        {menu === 'icon' && (
          <div className="menu pop">
            <input
              className="filter-input small"
              autoFocus
              placeholder="Type or paste an emoji"
              value={emoji}
              onChange={(e) => setEmoji(e.target.value)}
              onKeyDown={(e) => {
                e.stopPropagation()
                if (e.key === 'Enter') {
                  void exec('node.setIcon', { node: page.zoom, icon: emoji.trim() }, panel)
                  setEmoji('')
                  setMenu(null)
                } else if (e.key === 'Escape') setMenu(null)
              }}
            />
            {page.icon && (
              <div className="menu-item" onMouseDown={(e) => (e.preventDefault(), void exec('node.setIcon', { node: page.zoom, icon: '' }, panel), setMenu(null))}>
                Remove icon
              </div>
            )}
          </div>
        )}
      </span>
      <span className="menu-anchor">
        <button className="ghost-btn" onClick={() => setMenu(menu === 'banner' ? null : 'banner')}>
          <Icon name="image" size={12} /> Banner
        </button>
        {menu === 'banner' && (
          <div className="menu pop">
            {[['picture:orbit', 'Orbit (picture)'], ['blue', 'Blue'], ['green', 'Green'], ['amber', 'Amber'], ['pink', 'Pink'], ['grey', 'Grey'], ['', 'No banner']].map(([v, l]) => (
              <div key={l} className="menu-item" onMouseDown={(e) => (e.preventDefault(), void exec('node.setBanner', { node: page.zoom, color: v }, panel), setMenu(null))}>
                {l}
              </div>
            ))}
          </div>
        )}
      </span>
    </div>
  )
}

/** The page's description typed in place under the title (a field page shows the line even when
 * empty, spec 12). Saved as typed. */
function PageDescription({ page, panel }: { page: Page; panel: string }) {
  const [value, setValue] = useState(page.description)
  const [editing, setEditing] = useState(false)
  useEffect(() => {
    if (!editing) setValue(page.description)
  }, [page.description, editing])
  return (
    <textarea
      className="page-desc desc-input"
      rows={1}
      value={value}
      placeholder="Add a description"
      onFocus={() => setEditing(true)}
      onBlur={() => setEditing(false)}
      onChange={(e) => {
        const v = e.target.value.replace(/\n/g, ' ')
        setValue(v)
        void call('run', { command: 'node.setDescription', params: { node: page.target, text: v }, panel })
      }}
      onKeyDown={(e) => e.stopPropagation()}
    />
  )
}

function HeadActions({ page, panel, helpOnly }: { page: Page; panel: string; helpOnly?: boolean }) {
  const search = page.kind === 'search'
  // The Supertags page has Browse templates at the right (spec 12; no template gallery yet).
  if (page.kind === 'schema')
    return (
      <div className="head-actions">
        <button className="btn outline" onClick={() => showToast('There is no template gallery in this local app yet')}>
          Browse templates
        </button>
      </div>
    )
  // A field page and a Configure supertag panel have only help at the right (spec 12).
  if (page.kind === 'field' || helpOnly)
    return (
      <div className="head-actions">
        <button className="btn icon" title="Help" onClick={() => runUiCommand('ui.help')}>
          <Icon name="help" size={12} />
        </button>
      </div>
    )
  return (
    <div className="head-actions">
      <button className="btn icon" title="AI (not available)">
        <Icon name="sparkle" size={12} />
      </button>
      {/* A supertag's page: + Create new and help (spec 03 §1). */}
      {page.kind === 'tag' && (
        <button className="btn regular" onClick={() => void exec('view.addItem', { node: page.target, name: '' }, panel)}>
          <Icon name="plus" size={12} /> Create new
        </button>
      )}
      {page.kind === 'tag' && (
        <button className="btn icon" title="Help" onClick={() => runUiCommand('ui.help')}>
          <Icon name="help" size={12} />
        </button>
      )}
      {/* A search node's page: add an item that matches, and the search options (N54). */}
      {search && (
        <button className="btn icon" title="Add an item that matches" onClick={() => void exec('view.addItem', { node: page.zoom, name: '' }, panel)}>
          <Icon name="plus" size={12} />
        </button>
      )}
      {search && <SearchOptions node={page.zoom} panel={panel} />}
      <button
        className="btn icon"
        title="Options"
        onMouseDown={(e) => e.stopPropagation()}
        onClick={(e) => {
          // The page's menu right under the button, its left edge at the button (N37, N65 d79).
          const r = e.currentTarget.getBoundingClientRect()
          setState({ nodeMenu: { panel, node: page.zoom, x: r.left, y: r.bottom } })
        }}
      >
        <Icon name="more" size={12} />
      </button>
    </div>
  )
}

function DayNav({ panel }: { panel: string }) {
  const today = useStore((s) => s.sidebar?.todayDate) ?? ''
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
        <DatePicker value={today} title="Go to date" className="btn icon" onPick={(v) => void exec('nav.day', { date: v.slice(0, 10) }, panel)} />
      </span>
      {/* spec-gap: syncing an online calendar needs an account this local app does not have. */}
      <button className="btn icon" title="Set up calendar sync" onClick={() => showToast('Calendar sync needs an online calendar account, which this local app does not have yet')}>
        <Icon name="calendarSync" size={12} />
      </button>
      <WorkspacesButton panel={panel} />
    </div>
  )
}

/** "Workspaces ▾" at the right of the day toolbar (spec 12, N59.9): each workspace's Library, and
 * Manage workspaces. spec-gap: what Tana's menu offers is not written down; this is ours. */
function WorkspacesButton({ panel }: { panel: string }) {
  const [open, setOpen] = useState(false)
  const ws = useStore((s) => s.sidebar?.workspaces) ?? []
  const ref = useRef<HTMLSpanElement>(null)
  useEffect(() => {
    if (!open) return
    const off = (e: MouseEvent) => !ref.current?.contains(e.target as Node) && setOpen(false)
    window.addEventListener('mousedown', off)
    return () => window.removeEventListener('mousedown', off)
  }, [open])
  const pick = (f: () => void) => (e: RME) => (e.preventDefault(), setOpen(false), f())
  return (
    <span className="menu-anchor ws-menu" ref={ref}>
      <button className="btn ws-btn" onClick={() => setOpen((o) => !o)}>
        Workspaces
        <Icon name="chevronDown" size={12} />
      </button>
      {open && (
        <div className="menu pop" role="menu">
          {ws.map((w) => (
            <div key={w.id} className="menu-item" onMouseDown={pick(() => void exec('nav.open', { node: w.library }, panel))}>
              <span className="menu-icon">
                <span className="ws-dot">{w.name.slice(0, 1).toUpperCase()}</span>
              </span>
              <span className="grow">{w.name}</span>
            </div>
          ))}
          <div className="menu-sep" />
          <div className="menu-item" onMouseDown={pick(() => setState({ overlay: { kind: 'workspaces' } }))}>
            <span className="menu-icon" />
            <span className="grow">Manage workspaces</span>
          </div>
        </div>
      )}
    </span>
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
  const tagView = useStore((s) => s.tagView[panel])
  const subSnap = useStore((s) => s.subViews[panel])
  const special = useStore((s) => s.special[panel])
  const grow = useStore((s) => s.panelGrow[panel] ?? 1)
  const configureOn = useStore((s) => !!s.configure[panel])
  useEffect(() => {
    if (focusIndex == null || !page) return
    if (focusIndex < first + 1 || focusIndex > last - 1) v.scrollToIndex(focusIndex, { align: 'center' })
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [focusIndex, page?.focus, focusScroll])

  if (!page) return <div className="panel" />
  if (special) {
    const title = { settings: 'Settings', recents: 'Recents', help: 'Help' }[special]
    return (
      <div className={`panel${active ? ' active' : ''}${special === 'settings' ? ' settings-panel' : ''}`} style={{ flexGrow: grow }} onMouseDown={() => setActivePanel(panel)} data-panel={panel}>
        <div className="crumbs">
          <button className="icon-btn crumb-back" title="Back" onClick={() => setState((s) => ({ special: { ...s.special, [panel]: undefined } }))}>
            <Icon name="chevronLeft" size={14} />
          </button>
          {special === 'settings' && <span className="crumb">User settings</span>}
          <span className="spacer" />
          {many && (
            <button className="icon-btn" title="Close panel" onClick={() => void closePanel(panel)}>
              <Icon name="close" size={14} />
            </button>
          )}
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
  const tagVs = viewSnap && viewSnap.node === page.zoom ? viewSnap : null
  // A tag page's view tab shows in place under the tag header (N36).
  const tabId = page.kind === 'tag' && tagVs && tagView != null && tagVs.tabs.some((t) => t.id === tagView) ? tagView : null
  const vs = tabId != null && subSnap?.node === tabId ? subSnap : tagVs
  const configuring = page.kind === 'tag' && configureOn
  const fieldPage = page.kind === 'field'
  const layout = vs && vs.kind !== 'outline' && !configuring ? vs : null
  const schema = page.kind === 'schema' || configuring || fieldPage
  return (
    <div className={`panel${active ? ' active' : ''}`} style={{ flexGrow: grow }} onMouseDown={() => setActivePanel(panel)} data-panel={panel}>
      <Crumbs page={page} panel={panel} many={many} />
      <div className="scroller" ref={scroller}>
        {page.banner && <Banner banner={page.banner} />}
        {/* A tagged page gets its first tag's colour washed behind the header (N59.23). */}
        {page.kind !== 'tag' && page.tags.length > 0 && <div className="tag-glow" style={tagStyle(page.tags[0].color)} />}
        <div className={`page${wide ? ' with-side' : ''}${layout && layout.kind !== 'list' ? ' full' : ''}${configuring || fieldPage ? ' schema-col' : ''}`}>
          <div className="page-main">
            <div ref={head} className={`page-head${page.kind === 'tag' ? ' tag-head' : ''}${fieldPage ? ' field-head' : ''}${page.kind === 'schema' ? ' schema-head' : ''}`}>
              {/* Not in a Configure supertag panel: its header starts with the title (N59.32). */}
              {(page.titleEditable || page.kind === 'day') && !configuring && <Decorate page={page} panel={panel} />}
              <div className={`title-row${page.kind === 'day' ? ' day' : ''}`}>
                {page.icon && <span className="page-icon">{page.icon}</span>}
                {/* A field definition: its type's icon, a small title (spec 12). */}
                {fieldPage && <Icon name={FIELD_ICONS[page.fieldType ?? ''] ?? 'fieldText'} size={16} />}
                {page.kind === 'tag' && (
                  <span className="tag-square title" style={tagStyle(page.tagColor ?? 'grey')}>
                    #
                  </span>
                )}
                {page.kind === 'tag' && <span className="title-slot" aria-hidden />}
                {/* A page with a checkbox shows it before the title (N59.22). */}
                {page.done != null && page.kind !== 'tag' && (
                  <span
                    className={`checkbox title-check${page.done ? ' on' : ''}`}
                    role="checkbox"
                    aria-checked={page.done}
                    onClick={() => void exec('node.toggleDone', { node: page.target }, panel)}
                  >
                    {page.done && <Icon name="check" size={16} />}
                  </span>
                )}
                <TitleEditor key={page.zoom} page={page} panel={panel} />
                {page.tags.length === 0 && <HeadActions page={page} panel={panel} helpOnly={configuring} />}
              </div>
              {page.tags.length > 0 && (
                <div className="head-meta">
                  <div className="tag-row">
                    {page.tags.map((t) => (
                      <TagChip key={t.id} chip={t} onOpen={() => void exec('nav.open', { node: t.id }, panel)} onRemove={() => void exec('tag.remove', { node: page.target, tag: t.id }, panel)} />
                    ))}
                  </div>
                  <HeadActions page={page} panel={panel} />
                </div>
              )}
              {fieldPage || configuring ? <PageDescription page={page} panel={panel} /> : page.description && <p className="page-desc">{page.description}</p>}
              {page.kind === 'day' && <DayNav panel={panel} />}
              {page.kind === 'trash' && <p className="banner-note">Deleted nodes wait here. Restore them from their menu, or empty the trash.</p>}
              {page.kind === 'tag' && tagVs && <TagHeader panel={panel} tag={page.target} color={page.tagColor ?? 'grey'} tabs={tagVs.tabs} active={tabId ?? page.zoom} />}
              {page.kind === 'tag' && tagVs && vs && !configuring && (
                <ViewTitle v={vs} panel={panel} name={tabId == null ? 'Everything tagged' : (tagVs.tabs.find((t) => t.id === tabId)?.name ?? vs.name)} description={tabId != null ? vs.description : ''} />
              )}
              {vs && !layout && !schema && <QueryBuilder v={vs} panel={panel} />}
              {vs && !layout && !schema && vs.toolbar && <ViewToolbar v={vs} panel={panel} />}
            </div>
            {page.kind === 'schema' && <SupertagsPage panel={panel} />}
            {configuring && <ConfigureTag panel={panel} tag={page.target} />}
            {fieldPage && <FieldPage panel={panel} field={page.target} />}
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
                    {row ? <RowView row={row} panel={panel} page={page} active={active && !layout && !schema} /> : <div className="row placeholder" />}
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
