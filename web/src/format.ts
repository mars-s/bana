// Inline marks in a node's text, read the same way as `tanacraft_core::format::raw_spans`:
// `**bold**`, `*italic*`, `__underline__`, `==highlight==`, `~~strike~~`, `` `code` ``, `[[id:42]]` references
// and `http(s)://` links. Only for drawing; the engine owns the text.

export interface Span {
  text: string
  bold?: boolean
  italic?: boolean
  underline?: boolean
  highlight?: boolean
  strike?: boolean
  code?: boolean
  node?: number
  /** An inline reference's own text (UC-06.37). */
  alias?: string
  url?: string
  /** A mark or reference token itself (shown faint while editing, hidden otherwise). */
  marker?: boolean
  /** Index (UTF-16) of this piece in the raw text. */
  at: number
}

type Mark = 'bold' | 'underline' | 'highlight' | 'strike' | 'code' | 'italic'
const TOKENS: [Mark, string][] = [
  ['bold', '**'],
  ['underline', '__'],
  ['highlight', '=='],
  ['strike', '~~'],
  ['code', '`'],
  ['italic', '*'],
]

const CODE: [Mark, string] = ['code', '`']

export function rawSpans(s: string): Span[] {
  const out: Span[] = []
  const cur: Omit<Span, 'at'> & { at: number } = { text: '', at: 0 }
  const flush = () => {
    if (cur.text) out.push({ ...cur })
    cur.text = ''
  }
  let i = 0
  while (i < s.length) {
    if (!cur.code && s.startsWith('[[id:', i)) {
      const end = s.indexOf(']]', i + 5)
      // `[[id:42]]` or `[[id:42|alias]]` (UC-06.37).
      const inner = end > 0 ? s.slice(i + 5, end) : ''
      const bar = inner.indexOf('|')
      const idText = bar >= 0 ? inner.slice(0, bar) : inner
      const id = Number(idText)
      if (end > 0 && /^\d+$/.test(idText) && Number.isFinite(id)) {
        flush()
        out.push({ text: '', node: id, at: i, ...(bar >= 0 ? { alias: inner.slice(bar + 1) } : {}) })
        out.push({ text: s.slice(i, end + 2), node: id, marker: true, at: i })
        i = end + 2
        cur.at = i
        continue
      }
    }
    const atWord = i === 0 || /\s/.test(s[i - 1])
    if (!cur.code && atWord && (s.startsWith('http://', i) || s.startsWith('https://', i))) {
      const m = /\s/.exec(s.slice(i))
      const len = m ? m.index : s.length - i
      flush()
      const url = s.slice(i, i + len)
      out.push({ ...cur, text: url, url, at: i })
      i += len
      cur.at = i
      continue
    }
    const found = cur.code ? (s[i] === '`' ? CODE : undefined) : TOKENS.find(([, t]) => s.startsWith(t, i))
    if (found) {
      const [m, tok] = found
      const open = !!cur[m]
      if (open || s.indexOf(tok, i + tok.length) >= 0) {
        flush()
        out.push({ text: tok, marker: true, at: i })
        cur[m] = !open
        i += tok.length
        cur.at = i
        continue
      }
    }
    if (!cur.text) cur.at = i
    const cp = s.codePointAt(i) ?? 0
    const ch = String.fromCodePoint(cp)
    cur.text += ch
    i += ch.length
  }
  flush()
  return out
}

/** UTF-16 index to char (code point) index, as the engine counts carets. */
export function toChars(s: string, utf16: number): number {
  return Array.from(s.slice(0, utf16)).length
}

/** Char (code point) index to UTF-16 index. */
export function toUtf16(s: string, chars: number): number {
  let n = 0
  let i = 0
  for (const ch of s) {
    if (n >= chars) break
    i += ch.length
    n++
  }
  return i
}

export function charLen(s: string): number {
  return Array.from(s).length
}

export function isLinkOnly(s: string): boolean {
  const sp = rawSpans(s.trim()).filter((x) => !x.marker)
  return sp.length === 1 && !!sp[0].url
}
