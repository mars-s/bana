// Sync with the phone (docs/sync.md): the status button in the top bar and Settings > Sync.
//
// Sync requests go straight to the engine (`transport.call`), not through the ordered edit queue:
// asking the relay for an invite can take a few seconds and must never hold typing up. The
// pairing code holds the workspace key: it is shown on screen and copied on request, never logged.

import { useEffect, useState } from 'react'
import { transport, type Json } from './engine'
import { Icon } from './icons'
import { getState, setState, showToast, useStore } from './store'
import { runUiCommand } from './controller'

export interface SyncStatus {
  state: 'off' | 'offline' | 'syncing' | 'synced' | 'error'
  message?: string | null
  relay?: string
  name?: string
  device?: string
  devices?: number
  lastSynced?: number | null
  pending?: boolean
  joining?: boolean
  unavailable?: boolean
}

interface DeviceRow {
  device: string
  name: string
  cursor: number
  seen: number
  revoked: boolean
  me: boolean
}

async function sync(method: string, params: Json = {}): Promise<Json> {
  try {
    return await transport.call(`sync.${method}`, params)
  } catch {
    return { error: 'the engine is not reachable' }
  }
}

/** Reads the status now (and every few seconds from `watchSync`). */
export async function readSync() {
  const r = await sync('status')
  if (r && !r.error) setState({ sync: r as SyncStatus })
}

let watching = false
/** Keeps the status fresh while the app is open. */
export function watchSync() {
  if (watching) return
  watching = true
  void readSync()
  window.setInterval(() => void readSync(), 3000)
  window.addEventListener('focus', () => void readSync())
}

const LABEL: Record<SyncStatus['state'], string> = {
  off: 'Sync is off',
  offline: 'Offline',
  syncing: 'Syncing',
  synced: 'Synced',
  error: 'Sync stopped',
}

function ago(ms?: number | null): string {
  if (!ms) return 'never'
  const s = Math.max(0, Math.round((Date.now() - ms) / 1000))
  if (s < 10) return 'just now'
  if (s < 60) return `${s} seconds ago`
  if (s < 3600) return `${Math.round(s / 60)} minutes ago`
  if (s < 86400) return `${Math.round(s / 3600)} hours ago`
  return new Date(ms).toLocaleDateString()
}

function statusLine(s: SyncStatus): string {
  const base = LABEL[s.state]
  if (s.state === 'synced') return `${base} · ${s.devices ?? 1} ${s.devices === 1 ? 'device' : 'devices'}`
  if (s.message) return `${base}: ${s.message}`
  return base
}

/** Opens Settings at the Sync section. */
export function openSyncSettings() {
  setState({ settingsTab: 'Sync' })
  runUiCommand('ui.settings')
}

/** The top bar's sync status (nothing while sync is off). */
export function SyncButton() {
  const s = useStore((st) => st.sync)
  useEffect(watchSync, [])
  if (!s || s.state === 'off') return null
  return (
    <button className={`icon-btn sync-btn sync-${s.state}`} title={statusLine(s)} aria-label={`Sync: ${LABEL[s.state]}`} data-sync={s.state} onClick={openSyncSettings}>
      <Icon name={s.state === 'offline' || s.state === 'error' ? 'cloudOff' : 'cloud'} size={14} />
      <span className="sync-dot" />
    </button>
  )
}

/** Settings > Sync. */
export function SyncSettings() {
  const s = useStore((st) => st.sync)
  const [relay, setRelay] = useState('')
  const [name, setName] = useState('')
  const [code, setCode] = useState('')
  const [invite, setInvite] = useState<{ code: string; qr: string } | null>(null)
  const [devices, setDevices] = useState<DeviceRow[]>([])
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  useEffect(watchSync, [])
  const on = !!s && s.state !== 'off'
  const loadDevices = () => void sync('devices').then((r) => setDevices(r?.devices ?? []))
  useEffect(() => {
    if (on) loadDevices()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [on, s?.devices, s?.state])

  const act = async (method: string, params: Json = {}): Promise<Json> => {
    setBusy(true)
    setError('')
    const r = await sync(method, params)
    setBusy(false)
    if (r?.error) setError(r.error)
    await readSync()
    return r
  }

  if (s?.unavailable) return <p className="muted">Sync needs a graph file; this engine has none.</p>

  if (!on) {
    return (
      <div className="sync-settings">
        <p className="setting-help">
          Keep this computer and your iPhone in step through a relay you run on your own Cloudflare account (free plan). Everything is encrypted on your devices first; the relay only stores and forwards ciphertext. See docs/sync.md and run cloudflare/sync/wizard.sh to deploy the relay.
        </p>
        <div className="setting">
          <span className="setting-icon">
            <Icon name="cloud" size={12} />
          </span>
          <div className="setting-label">Set up sync</div>
          <div className="setting-help">The relay's address, as the wizard printed it.</div>
          <div className="setting-control sync-row">
            <input className="cfg-input grow" aria-label="Relay address" placeholder="https://bana-sync.you.workers.dev" value={relay} onChange={(e) => setRelay(e.target.value)} />
            <input className="cfg-input" aria-label="Device name" placeholder="This computer's name" value={name} onChange={(e) => setName(e.target.value)} />
            <button className="btn regular" disabled={busy || !relay.trim()} onClick={() => void act('setup', { relay: relay.trim(), name: name.trim() })}>
              Set up sync
            </button>
          </div>
        </div>
        <div className="setting">
          <span className="setting-icon">
            <Icon name="cloud" size={12} />
          </span>
          <div className="setting-label">Join another device's sync</div>
          <div className="setting-help">Paste a code from Add a device on the other device. This copy's notes are replaced by the synced ones (a copy of the file is kept beside it).</div>
          <div className="setting-control sync-row">
            <input className="cfg-input grow" aria-label="Sync code" placeholder="bana-sync:..." value={code} onChange={(e) => setCode(e.target.value)} />
            <button className="btn regular" disabled={busy || !code.trim()} onClick={() => void act('join', { code: code.trim(), name: name.trim() })}>
              Join
            </button>
          </div>
        </div>
        {error && <p className="sync-error">{error}</p>}
      </div>
    )
  }

  return (
    <div className="sync-settings">
      <div className={`sync-status sync-${s.state}`} data-sync={s.state}>
        <span className="sync-dot" />
        <span className="grow">{statusLine(s)}</span>
        <span className="muted">Last synced {ago(s.lastSynced)}</span>
      </div>
      <p className="setting-help">
        Relay {s.relay} · this device is “{s.name}”. The relay sees encrypted data and which device sent it, never your notes, names or keys.
      </p>
      <div className="setting">
        <span className="setting-icon">
          <Icon name="plus" size={12} />
        </span>
        <div className="setting-label">Add a device</div>
        <div className="setting-help">Shows a code for your iPhone (scan it in Manage, Sync) or another computer. It works once, for 24 hours. It holds the key to your notes: show it only to your own devices.</div>
        <div className="setting-control">
          {!invite ? (
            <button className="btn regular" disabled={busy || s.state === 'offline' || s.state === 'error'} onClick={() => void act('invite').then((r) => r?.code && setInvite({ code: r.code, qr: r.qr }))}>
              Add a device
            </button>
          ) : (
            <div className="sync-invite">
              {/* The SVG comes from the engine's QR encoder (no outside content). */}
              <div className="sync-qr" aria-label="Pairing QR code" dangerouslySetInnerHTML={{ __html: invite.qr }} />
              <div className="sync-code-box">
                <textarea className="cfg-input sync-code" aria-label="Pairing code" readOnly value={invite.code} onFocus={(e) => e.currentTarget.select()} />
                <div className="sync-row">
                  <button
                    className="btn regular"
                    onClick={() =>
                      void navigator.clipboard?.writeText(invite.code).then(
                        () => showToast('Code copied'),
                        () => showToast('Select the code and copy it'),
                      )
                    }
                  >
                    Copy code
                  </button>
                  <button className="btn regular" onClick={() => setInvite(null)}>
                    Done
                  </button>
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
      <div className="setting">
        <span className="setting-icon">
          <Icon name="user" size={12} />
        </span>
        <div className="setting-label">Devices</div>
        <div className="sync-devices">
          {devices.map((d) => (
            <div key={d.device} className={`shortcut-row sync-device${d.revoked ? ' revoked' : ''}`} data-device={d.name}>
              <span className="grow">
                {d.name}
                {d.me && <span className="muted"> (this device)</span>}
                {d.revoked && <span className="muted"> (removed)</span>}
              </span>
              <span className="muted">{d.me ? '' : `seen ${ago(d.seen)}`}</span>
              {!d.me && !d.revoked && (
                <button
                  className="ghost-btn"
                  aria-label={`Remove ${d.name}`}
                  onClick={() => {
                    if (window.confirm(`Remove ${d.name} from sync? It keeps the notes it has but gets no more changes. To shut out a lost device completely, also turn sync off here and set it up again.`))
                      void act('revoke', { device: d.device }).then(loadDevices)
                  }}
                >
                  Remove
                </button>
              )}
            </div>
          ))}
        </div>
      </div>
      <div className="setting">
        <span className="setting-icon">
          <Icon name="cloudOff" size={12} />
        </span>
        <div className="setting-label">Turn sync off on this device</div>
        <div className="setting-help">The notes stay here; this device stops syncing and forgets the key.</div>
        <div className="setting-control">
          <button
            className="btn regular"
            disabled={busy}
            onClick={() => {
              if (window.confirm('Turn sync off on this device?')) void act('leave').then(() => setInvite(null))
            }}
          >
            Turn off
          </button>
        </div>
      </div>
      {error && <p className="sync-error">{error}</p>}
    </div>
  )
}

/** For tests and the console: the status as last read. */
export const syncStatus = () => getState().sync
