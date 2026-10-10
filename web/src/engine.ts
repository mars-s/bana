// The bridge to the Rust engine. The UI never owns data: every edit is a command sent here, in
// order, and what is drawn is a snapshot read back from the engine.
//
// Two transports, one surface: Tauri `invoke` inside the desktop app, HTTP (`POST /rpc`,
// `GET /events`) to `tanacraft-cli serve` in a normal browser.

export type Json = any // eslint-disable-line @typescript-eslint/no-explicit-any

export interface Transport {
  call(method: string, params: Json): Promise<Json>
  /** Calls `cb` with the engine's version whenever something changed. */
  watch(cb: (version: number) => void): void
  /** Sends requests while the page unloads (they run in order, then the engine saves). */
  flushBeacon(batch: Json[]): void
}

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown
  }
}

class HttpTransport implements Transport {
  constructor(private base: string) {}
  async call(method: string, params: Json) {
    const r = await fetch(`${this.base}/rpc`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ method, params }),
    })
    return r.json()
  }
  watch(cb: (v: number) => void) {
    let since = 0
    const loop = async () => {
      for (;;) {
        try {
          const r = await fetch(`${this.base}/events?since=${since}`)
          const { version } = await r.json()
          if (version > since) {
            since = version
            cb(version)
          }
        } catch {
          await new Promise((res) => setTimeout(res, 1000))
        }
      }
    }
    void loop()
  }
  flushBeacon(batch: Json[]) {
    // text/plain keeps the beacon a simple request (no preflight); the server reads it as JSON.
    navigator.sendBeacon?.(`${this.base}/rpc`, new Blob([JSON.stringify(batch)], { type: 'text/plain' }))
  }
}

class TauriTransport implements Transport {
  async call(method: string, params: Json) {
    const { invoke } = await import('@tauri-apps/api/core')
    return invoke('rpc', { method, params })
  }
  watch(cb: (v: number) => void) {
    void import('@tauri-apps/api/event').then(({ listen }) => listen<number>('changed', (e) => cb(e.payload)))
  }
  flushBeacon(batch: Json[]) {
    for (const b of batch) void this.call(b.method, b.params)
  }
}

function makeTransport(): Transport {
  if (window.__TAURI_INTERNALS__) return new TauriTransport()
  const server = new URLSearchParams(location.search).get('server')
  return new HttpTransport(server ?? '')
}

export const transport = makeTransport()

// ---- the ordered queue and its write-ahead log ------------------------------------------------------

interface Item {
  method: string
  params: Json
  /** Write-ahead-log number of an edit: [this page's client id, sequence]. */
  wal?: [string, number]
  done: ((r: Json) => void)[]
  fail: ((e: unknown) => void)[]
}

/** Requests not sent yet, in order. Every call to the engine goes through here, so a keystroke
 * can never overtake an earlier one. */
const pending: Item[] = []
let sending = false
const idleWaiters: (() => void)[] = []

const isRename = (i: { method: string; params: Json }) => i.method === 'run' && i.params?.command === 'node.setName'

/** Increases across reloads: the engine drops text older than what it already has. */
let lastRev = 0
function rev() {
  lastRev = Math.max(lastRev + 1, Date.now() * 1000)
  return lastRev
}

// Every edit is written to sessionStorage the moment it is made (in the same task as the key
// press), numbered per page load, and leaves the log only once the engine has answered. A reload
// (or a crash of the page) replays what is left; the engine runs each (client, seq) once, so an
// edit that did arrive is skipped. Reads are not logged.
const WAL = 'tanacraft.wal'
const EDITS = new Set(['run', 'fieldDraft', 'panel.open', 'panel.close'])
const client = `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`
let walSeq = 0
type WalEntry = { method: string; params: Json; wal: [string, number] }
let wal: WalEntry[] = []

function writeWal() {
  try {
    if (wal.length) sessionStorage.setItem(WAL, JSON.stringify(wal))
    else sessionStorage.removeItem(WAL)
  } catch {
    /* storage off or full: the queue and the unload beacon still carry the edits */
  }
}

/** The engine answered `it`: it and everything logged before it by the same page are in. */
function ack(it: Item) {
  if (!it.wal) return
  const [c, n] = it.wal
  const before = wal.length
  wal = wal.filter((e) => e.wal[0] !== c || e.wal[1] > n)
  if (wal.length !== before) writeWal()
}

export function call(method: string, params: Json = {}): Promise<Json> {
  if (method === 'run' && (params?.command === 'node.setName' || params?.command === 'field.set') && params.rev == null) params = { ...params, rev: rev() }
  return new Promise((resolve, reject) => {
    const last = pending[pending.length - 1]
    // Typing fast: a rename waiting behind another rename of the same line replaces it (only the
    // latest text matters, and both callers get the answer).
    // (Never the request already on its way: it is pending[0] while sending.)
    const waiting = pending.length > (sending ? 1 : 0)
    if (waiting && last && isRename(last) && isRename({ method, params }) && last.params.panel === params.panel && last.params.params?.node === params.params?.node) {
      last.params = params
      const e = wal.find((x) => last.wal && x.wal[0] === last.wal[0] && x.wal[1] === last.wal[1])
      if (e) {
        e.params = params
        writeWal()
      }
      last.done.push(resolve)
      last.fail.push(reject)
      return
    }
    const it: Item = { method, params, done: [resolve], fail: [reject] }
    if (EDITS.has(method)) {
      it.wal = [client, ++walSeq]
      wal.push({ method, params, wal: it.wal })
      writeWal()
    }
    pending.push(it)
    void pump()
  })
}

const send = (it: { method: string; params: Json; wal?: [string, number] }) => (it.wal ? { ...it.params, wal: it.wal } : it.params)

async function pump() {
  if (sending) return
  sending = true
  while (pending.length) {
    const it = pending[0]
    try {
      const r = await transport.call(it.method, send(it))
      pending.shift()
      ack(it)
      it.done.forEach((f) => f(r))
    } catch (e) {
      // Not answered: the edit stays logged (a later answer, or the next load, settles it).
      pending.shift()
      it.fail.forEach((f) => f(e))
    }
  }
  sending = false
  idleWaiters.splice(0).forEach((w) => w())
}

export function idle(): Promise<void> {
  return !sending && pending.length === 0 ? Promise.resolve() : new Promise((r) => idleWaiters.push(r))
}

export function busy() {
  return sending || pending.length > 0
}

/** On load: edits the last page logged but the engine never answered go in first, in order. */
export async function replayWal() {
  let old: WalEntry[] = []
  try {
    old = JSON.parse(sessionStorage.getItem(WAL) ?? '[]')
  } catch {
    /* nothing logged */
  }
  if (!Array.isArray(old) || !old.length) return
  // They stay logged until answered, so a reload during the replay loses nothing either.
  wal = old.filter((e) => e && typeof e.method === 'string' && Array.isArray(e.wal))
  writeWal()
  for (const e of [...wal]) pending.push({ method: e.method, params: e.params, wal: e.wal, done: [], fail: [] })
  void pump()
  await idle()
}

/** The window is going away (also when the tab closes, which drops sessionStorage): whatever is
 * still waiting, the request in flight included, goes out now, then the engine saves. The engine
 * runs a logged edit once, so the next page replaying the log does no harm. */
export function flushOnUnload() {
  const batch = pending.map((i) => ({ method: i.method, params: send(i) }))
  transport.flushBeacon([...batch, { method: 'save', params: {} }])
}

// ---- snapshot types ------------------------------------------------------------------------------

export interface Chip {
  id: number
  name: string
  color: string
}

export interface NodeRow {
  k: 'node'
  key: string
  id: number
  target: number
  parent: number | null
  index: number | null
  pinned: boolean
  inTrash: boolean
  depth: number
  name: string
  ref: boolean
  kind: string
  tags: Chip[]
  done: boolean | null
  hasKids: boolean
  expanded: boolean
  heading: boolean
  description: string
  icon: string | null
  link: boolean
  suffix: [string, boolean][]
  notes: boolean
  progress: [number, number] | null
  /** On the first line of a reference's notes: their heading. */
  notesHead?: string | null
  /** Places that point at this node (only with the reference counter on). */
  uses?: number
  created: number
  edited: number
}

export interface FieldValueShown {
  text: string
  raw: string
  node: number | null
  checked: boolean | null
}

export interface FieldRow {
  k: 'field'
  key: string
  owner: number
  field: number
  depth: number
  name: string
  type: string
  values: FieldValueShown[]
  missing: boolean
  invalid: boolean
  boxed: boolean
}

export interface MoreRow {
  k: 'more'
  key: string
  owner: number
  depth: number
  fields: { field: number; name: string; optional: boolean }[]
}

export type Row = NodeRow | FieldRow | MoreRow | { k: 'gone'; key: string; depth: number } | { k: 'empty'; key: string; parent: number; depth: number }

export interface Page {
  version: number
  zoom: number
  target: number
  title: string
  name: string
  titleEditable: boolean
  /** The page node's checkbox (null: none). */
  done: boolean | null
  kind: string
  tagColor: string | null
  icon: string | null
  banner: string | null
  description: string
  tags: Chip[]
  crumbs: { id: number; name: string }[]
  canBack: boolean
  canForward: boolean
  focus: number | null
  caret: number
  anchor: number
  sel: number[]
  view: { kind: string; isView: boolean; toolbar: boolean }
  related: { id: number; name: string; items: NodeRow[] }[]
  references: { id: number; name: string; path: string }[]
  filteredOut: number
  rows: { total: number; start: number; items: Row[]; focusIndex: number | null }
  refs: Record<string, string>
  status: string
  canUndo: boolean
  canRedo: boolean
}

export interface SidebarItem {
  id: number
  name: string
  kind: string
  color: string | null
  hasKids: boolean
  icon: string | null
}

export interface Sidebar {
  today: number
  /** Today as YYYY-MM-DD (the engine's today). */
  todayDate: string
  zoom: number
  pinned: SidebarItem[]
  workspaces: { id: number; name: string; library: number; trash: number; main: boolean; entries: SidebarItem[] }[]
  journal: number
  schema: number
  hidden: string[]
  /** Custom keybindings: shortcut to command id (spec 09). */
  keybindings: Record<string, string>
  warnReferenced: boolean
  urlEmbeds: boolean
  recents: { id: number; name: string; kind: string }[]
}

export interface CommandInfo {
  id: string
  label: string
  location: string
  shortcut: string
  params: string
  mutates: boolean
}
