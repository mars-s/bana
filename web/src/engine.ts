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

// ---- the ordered queue ---------------------------------------------------------------------------

interface Item {
  method: string
  params: Json
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
let revSeq = 0
function rev() {
  return Date.now() * 1000 + (revSeq++ % 1000)
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
      last.done.push(resolve)
      last.fail.push(reject)
      return
    }
    pending.push({ method, params, done: [resolve], fail: [reject] })
    void pump()
  })
}

async function pump() {
  if (sending) return
  sending = true
  while (pending.length) {
    const it = pending[0]
    try {
      const r = await transport.call(it.method, it.params)
      pending.shift()
      it.done.forEach((f) => f(r))
    } catch (e) {
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

/** The window is going away: whatever is still waiting goes out now, then the engine saves. */
export function flushOnUnload() {
  // The request in flight may be cut off by the unload. A rename or a field value carries the
  // whole text, so sending it again is safe; other commands are not repeated.
  const inFlight = sending ? pending[0] : undefined
  const again = inFlight && (isRename(inFlight) || (inFlight.method === 'run' && inFlight.params?.command === 'field.set'))
  const batch = pending.slice(sending && !again ? 1 : 0).map((i) => ({ method: i.method, params: i.params }))
  // The next page waits until this batch is in (see waitForUnloadBatch).
  const token = `${Date.now().toString(36)}${Math.random().toString(36).slice(2)}`
  try {
    sessionStorage.setItem(MARK, token)
  } catch {
    /* storage off */
  }
  transport.flushBeacon([...batch, { method: 'save', params: {} }, { method: 'flushMark', params: { token } }])
}

const MARK = 'tanacraft.flushMark'

/** After a reload: waits (up to 3 s) until the batch the last page sent while unloading is in. */
export async function waitForUnloadBatch() {
  let token: string | null = null
  try {
    token = sessionStorage.getItem(MARK)
    sessionStorage.removeItem(MARK)
  } catch {
    return
  }
  if (!token) return
  for (let i = 0; i < 60; i++) {
    const r = await call('marked', { token }).catch(() => null)
    if (r?.marked) return
    await new Promise((res) => setTimeout(res, 50))
  }
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
