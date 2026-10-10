// The person: onboarding on first launch (skippable), Settings > Profile, and Bana ID
// (docs/bana-id.md): optional, only for sync and recovery; the app works fully without it.
//
// Bana ID signs in with a passkey on a page the relay serves (passkeys belong to the relay's
// host). The engine starts it (`id.begin`) and waits for the page to hand a one-time code back
// to a one-shot listener on 127.0.0.1; this side opens the page and watches `id.status`.

import { useEffect, useRef, useState } from 'react'
import { transport, type Json } from './engine'
import { Icon } from './icons'
import { getState, setState, showToast, useStore } from './store'
import { exec, runUiCommand } from './controller'
import { openSyncSettings, readSync } from './Sync'

export interface Profile {
  name: string
  avatar: string | null
  onboarded: boolean
  me: number | null
}

export interface IdStatus {
  signedIn: boolean
  account: string | null
  relay: string
  defaultRelay: string
  syncOn: boolean
  pending: string | null
  note: string | null
  recoveryCode: string | null
  prf: boolean
  unavailable?: boolean
}

async function rpc(method: string, params: Json = {}): Promise<Json> {
  try {
    return await transport.call(method, params)
  } catch {
    return { error: 'the engine is not reachable' }
  }
}

export async function readProfile(): Promise<Profile | null> {
  const p = await rpc('profile.get')
  if (p && !p.error) setState({ profile: p as Profile })
  return p?.error ? null : (p as Profile)
}

export async function saveProfile(change: Partial<Profile>) {
  const p = await rpc('profile.set', change)
  if (p?.error) showToast(p.error)
  else setState({ profile: p as Profile })
}

/** Which importer reads a file (UC-08.10, UC-08.11): OPML, Roam JSON, Tana Paste, else Markdown. */
export function importCommand(name: string, text: string): string {
  const n = name.toLowerCase()
  if (n.endsWith('.opml')) return 'import.opml'
  if (n.endsWith('.json')) return 'import.roam'
  if (text.trimStart().startsWith('%%tana%%')) return 'import.tanaPaste'
  return 'import.markdown'
}

/** A Notion or Logseq export: a folder of Markdown pages, each a page in the Library. */
async function importFolder(files: FileList) {
  const lib = getState().sidebar?.workspaces[0]?.library
  let n = 0
  for (const f of Array.from(files)) {
    if (!/\.md$/i.test(f.name)) continue
    const page = await exec('node.add', { name: f.name.replace(/\.md$/i, ''), parent: lib })
    if (page?.result?.id == null) continue
    await exec('import.markdown', { text: await f.text(), parent: page.result.id })
    n++
  }
  showToast(n ? `Imported ${n} ${n === 1 ? 'page' : 'pages'}` : 'No Markdown pages in that folder')
}

export function initials(name: string) {
  return (
    name
      .split(/\s+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((w) => w[0]?.toUpperCase())
      .join('') || '?'
  )
}

export function Avatar({ name, avatar, size = 'small' }: { name: string; avatar: string | null; size?: 'small' | 'large' }) {
  return <span className={`avatar ${size}`}>{avatar ? <img src={avatar} alt="" /> : <span aria-hidden="true">{initials(name)}</span>}</span>
}

/** A picked picture made small (128 px square JPEG) so it fits beside the graph. */
function shrink(file: File): Promise<string> {
  return new Promise((done, fail) => {
    const img = new Image()
    img.onload = () => {
      const c = document.createElement('canvas')
      c.width = c.height = 128
      const s = Math.min(img.width, img.height)
      c.getContext('2d')?.drawImage(img, (img.width - s) / 2, (img.height - s) / 2, s, s, 0, 0, 128, 128)
      URL.revokeObjectURL(img.src)
      done(c.toDataURL('image/jpeg', 0.85))
    }
    img.onerror = () => fail(new Error('not a picture'))
    img.src = URL.createObjectURL(file)
  })
}

function AvatarPicker({ name, avatar, onChange }: { name: string; avatar: string | null; onChange: (a: string | null) => void }) {
  const file = useRef<HTMLInputElement>(null)
  return (
    <div className="avatar-picker">
      <Avatar name={name} avatar={avatar} size="large" />
      <input
        ref={file}
        type="file"
        accept="image/png,image/jpeg,image/webp"
        hidden
        aria-label="Picture file"
        onChange={(e) => {
          const f = e.target.files?.[0]
          e.target.value = ''
          if (f) void shrink(f).then(onChange, () => showToast('That file is not a picture'))
        }}
      />
      <button className="btn regular" onClick={() => file.current?.click()}>
        Pick a picture
      </button>
      {avatar && (
        <button className="btn regular" onClick={() => onChange(null)}>
          Use initials
        </button>
      )}
    </div>
  )
}

// ---- Bana ID ------------------------------------------------------------------------------------

const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

async function readId(): Promise<IdStatus | null> {
  const r = await rpc('id.status')
  if (r && !r.error) setState({ bana: r as IdStatus })
  return r?.error ? null : (r as IdStatus)
}

/** Opens the passkey page and waits for it to come back. Answers an error message, or null. */
async function passkeyFlow(mode: 'create' | 'signin' | 'add', params: Json): Promise<string | null> {
  // In a browser the page opens in a new tab (opened now, while the click still counts, so no
  // popup blocker stops it); the desktop app asks the engine to open the system browser.
  const tab = inTauri ? null : window.open('about:blank', '_blank')
  const r = await rpc('id.begin', { mode, ret: 'loopback', open: inTauri, ...params })
  if (r?.error) {
    tab?.close()
    return r.error
  }
  if (tab) tab.location.href = r.url
  else if (!inTauri) window.open(r.url, '_blank')
  for (;;) {
    await new Promise((w) => setTimeout(w, 700))
    const s = await readId()
    if (!s) return 'the engine is not reachable'
    if (!s.pending) {
      await readSync()
      return s.note === 'ok' ? null : (s.note ?? 'the sign-in was cancelled')
    }
  }
}

/** The recovery code, once: shown until the person says it is saved. */
export function RecoveryCodeBox({ code, onSaved }: { code: string; onSaved: () => void }) {
  return (
    <div className="recovery-box" role="group" aria-label="Recovery code">
      <div className="setting-label">Your recovery code</div>
      <p className="setting-help">
        Write it down or print it, and keep it somewhere safe. It is the only way back in if you lose every passkey or the relay's address changes. Bana shows it once.
      </p>
      <code className="recovery-code" data-testid="recovery-code">
        {code}
      </code>
      <div className="sync-row">
        <button className="btn regular" onClick={() => void navigator.clipboard?.writeText(code).then(() => showToast('Recovery code copied'), () => showToast('Select the code and copy it'))}>
          Copy
        </button>
        <button className="btn regular" onClick={() => window.print()}>
          Print
        </button>
        <button className="btn regular confirm" onClick={onSaved}>
          I saved it
        </button>
      </div>
    </div>
  )
}

/** Create a Bana ID, sign in, recover, or pair with a code instead. */
export function BanaIdStart({ person, onDone }: { person: string; onDone?: () => void }) {
  const s = useStore((st) => st.bana)
  const [relay, setRelay] = useState('')
  const [busy, setBusy] = useState<string | null>(null)
  const [error, setError] = useState('')
  const [recover, setRecover] = useState<string | null>(null)
  useEffect(() => void readId(), [])
  if (!s) return null
  if (s.unavailable) return <p className="muted">Bana ID needs a graph file; this engine has none.</p>
  const where = s.syncOn ? s.relay : relay.trim() || s.defaultRelay
  const go = async (mode: 'create' | 'signin') => {
    setBusy(mode)
    setError('')
    const e = await passkeyFlow(mode, { relay: where, person })
    setBusy(null)
    if (e) setError(e)
    else onDone?.()
  }
  return (
    <div className="bana-id-start">
      {busy ? (
        <div className="sync-status" role="status">
          <span className="grow">Finish in the browser window that opened: use your passkey there, then come back.</span>
          <button className="btn regular" onClick={() => void rpc('id.cancel').then(() => readId())}>
            Cancel
          </button>
        </div>
      ) : (
        <>
          <div className="sync-row">
            <button className="btn regular confirm" onClick={() => void go('create')}>
              Create Bana ID
            </button>
            {!s.syncOn && (
              <button className="btn regular" onClick={() => void go('signin')}>
                Sign in with a passkey
              </button>
            )}
            {!s.syncOn && (
              <button className="btn regular" onClick={() => setRecover(recover === null ? '' : null)}>
                Use a recovery code
              </button>
            )}
            <button className="btn regular" onClick={() => (onDone?.(), openSyncSettings())}>
              Pair with a code instead
            </button>
          </div>
          {recover !== null && (
            <div className="sync-row">
              <input className="cfg-input grow" aria-label="Recovery code" placeholder="XXXXX-XXXXX-XXXXX-XXXXX-XXXXX" value={recover} onChange={(e) => setRecover(e.target.value)} />
              <button
                className="btn regular"
                disabled={!recover.trim()}
                onClick={() => {
                  setError('')
                  void rpc('id.recover', { relay: where, code: recover }).then(async (r) => {
                    await readId()
                    await readSync()
                    if (r?.error) setError(r.error)
                    else onDone?.()
                  })
                }}
              >
                Recover
              </button>
            </div>
          )}
          {!s.syncOn && (
            <details className="relay-choice">
              <summary>Relay: {where}</summary>
              <input className="cfg-input" aria-label="Bana ID relay" placeholder={s.defaultRelay} value={relay} onChange={(e) => setRelay(e.target.value)} />
            </details>
          )}
          <p className="setting-help">
            Signing in on a device that has notes replaces them with your synced notes (a copy of the file is kept beside it). Where passkeys cannot unlock notes (no PRF in this browser), pair with a code from another device instead.
          </p>
        </>
      )}
      {error && <p className="sync-error">{error}</p>}
    </div>
  )
}

function day(ms: number) {
  return new Date(ms).toLocaleDateString()
}

/** Settings > Profile. */
export function ProfileSettings() {
  const p = useStore((st) => st.profile)
  const s = useStore((st) => st.bana)
  const [acct, setAcct] = useState<Json>(null)
  const [newCode, setNewCode] = useState<string | null>(null)
  const [renaming, setRenaming] = useState<{ sid: string; name: string } | null>(null)
  const [error, setError] = useState('')
  useEffect(() => {
    void readProfile()
    void readId()
  }, [])
  const loadAccount = () => void rpc('id.account').then((r) => (r?.error ? setError(r.error) : (setAcct(r), setError(''))))
  useEffect(() => {
    if (s?.signedIn) loadAccount()
    else setAcct(null)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [s?.signedIn])
  const act = async (method: string, params: Json = {}) => {
    setError('')
    const r = await rpc(method, params)
    if (r?.error) setError(r.error)
    else if (r?.credentials) setAcct(r)
    await readId()
    await readSync()
    return r
  }
  if (!p) return null
  let host = ''
  try {
    host = new URL(s?.relay ?? '').host
  } catch {
    /* not set */
  }
  return (
    <div className="profile-settings">
      <div className="setting">
        <span className="setting-icon">
          <Icon name="user" size={12} />
        </span>
        <div className="setting-label">Name and picture</div>
        <div className="setting-help">Shown as you in the app and in user fields; your #person node has this name.</div>
        <div className="setting-control profile-me">
          <input className="cfg-input" aria-label="Your name" placeholder="Your name" defaultValue={p.name} key={p.name} onBlur={(e) => e.target.value.trim() && e.target.value.trim() !== p.name && void saveProfile({ name: e.target.value.trim() })} onKeyDown={(e) => (e.stopPropagation(), e.key === 'Enter' && (e.target as HTMLInputElement).blur())} />
          <AvatarPicker name={p.name} avatar={p.avatar} onChange={(avatar) => void saveProfile({ avatar })} />
        </div>
      </div>
      <div className="setting">
        <span className="setting-icon">
          <Icon name="cloud" size={12} />
        </span>
        <div className="setting-label">Bana ID</div>
        {!s?.signedIn ? (
          <>
            <div className="setting-help">Optional. Bana works fully without it. A Bana ID uses passkeys (no passwords) to bring your encrypted notes to a new device and to get them back if you lose one.</div>
            <div className="setting-control">
              <BanaIdStart person={p.name} />
            </div>
          </>
        ) : (
          <>
            <div className="sync-status" data-bana-id="signed-in">
              <span className="grow">Signed in{host ? ` on ${host}` : ''}</span>
            </div>
            <p className="setting-help">
              Passkeys are tied to this address ({host}). Renaming the Cloudflare workers.dev subdomain or the Worker would break them; the recovery code is the way back in.
            </p>
          </>
        )}
      </div>
      {s?.recoveryCode && <RecoveryCodeBox code={s.recoveryCode} onSaved={() => void act('id.recoveryShown')} />}
      {s?.signedIn && acct && (
        <>
          <div className="setting">
            <span className="setting-icon">
              <Icon name="fieldCheck" size={12} />
            </span>
            <div className="setting-label">Passkeys</div>
            <div className="sync-devices">
              {acct.credentials.map((c: Json, i: number) => (
                <div key={c.id} className="shortcut-row passkey-row" data-passkey={i}>
                  <span className="grow">
                    Passkey {i + 1} <span className="muted">added {day(c.created)}, last used {day(c.used)}</span>
                    {!c.unlocks && <span className="muted"> (cannot unlock notes: no PRF)</span>}
                  </span>
                  {acct.credentials.length > 1 && (
                    <button className="ghost-btn" aria-label={`Remove passkey ${i + 1}`} onClick={() => window.confirm('Remove this passkey from your Bana ID?') && void act('id.removePasskey', { id: c.id })}>
                      Remove
                    </button>
                  )}
                </div>
              ))}
            </div>
            <div className="setting-control">
              <button className="btn regular" onClick={() => void passkeyFlow('add', {}).then((e) => (e ? setError(e) : loadAccount()))}>
                Add another passkey
              </button>
            </div>
          </div>
          <div className="setting">
            <span className="setting-icon">
              <Icon name="user" size={12} />
            </span>
            <div className="setting-label">Devices</div>
            <div className="sync-devices">
              {acct.devices.map((d: Json) => (
                <div key={d.sid} className="shortcut-row id-device" data-device={d.name}>
                  {renaming?.sid === d.sid ? (
                    <input
                      className="cfg-input grow"
                      aria-label="Device name"
                      autoFocus
                      value={renaming?.name ?? ''}
                      onChange={(e) => setRenaming({ sid: d.sid, name: e.target.value })}
                      onKeyDown={(e) => {
                        e.stopPropagation()
                        if (e.key === 'Enter') void act('id.renameDevice', { sid: d.sid, name: renaming?.name ?? '' }).then(() => setRenaming(null))
                        if (e.key === 'Escape') setRenaming(null)
                      }}
                    />
                  ) : (
                    <span className="grow">
                      {d.name} <span className="muted">{d.me ? '(this device)' : `seen ${day(d.seen)}`}</span>
                    </span>
                  )}
                  <button className="ghost-btn" aria-label={`Rename ${d.name}`} onClick={() => setRenaming({ sid: d.sid, name: d.name })}>
                    Rename
                  </button>
                  {!d.me && (
                    <button className="ghost-btn" aria-label={`Remove ${d.name}`} onClick={() => window.confirm(`Remove ${d.name}? It is signed out and stops syncing; it keeps the notes it has.`) && void act('id.removeDevice', { sid: d.sid })}>
                      Remove
                    </button>
                  )}
                </div>
              ))}
            </div>
          </div>
          <div className="setting">
            <span className="setting-icon">
              <Icon name="fieldCheck" size={12} />
            </span>
            <div className="setting-label">Recovery code</div>
            <div className="setting-help">{acct.recovery ? 'Set. Make a new one if you lost it: the old one stops working.' : 'None yet.'}</div>
            <div className="setting-control">
              {newCode ? (
                <RecoveryCodeBox code={newCode} onSaved={() => setNewCode(null)} />
              ) : (
                <button className="btn regular" onClick={() => window.confirm('Make a new recovery code? The old one stops working.') && void act('id.newRecoveryCode').then((r) => r?.recoveryCode && (setNewCode(r.recoveryCode), loadAccount()))}>
                  Make a new recovery code
                </button>
              )}
            </div>
          </div>
          <div className="setting">
            <span className="setting-icon">
              <Icon name="cloudOff" size={12} />
            </span>
            <div className="setting-label">Sign out of this device</div>
            <div className="setting-help">Your notes stay here. This device stops syncing until you sign in again.</div>
            <div className="setting-control">
              <button className="btn regular" onClick={() => window.confirm('Sign out of Bana ID on this device? Your notes stay here.') && void act('id.signOut')}>
                Sign out
              </button>
            </div>
          </div>
          <div className="setting">
            <span className="setting-icon">
              <Icon name="trash" size={12} />
            </span>
            <div className="setting-label">Delete Bana ID</div>
            <div className="setting-help">Deletes your Bana ID and its encrypted sync data from the relay. Notes on your devices stay; they stop syncing.</div>
            <div className="setting-control">
              <button
                className="btn regular danger"
                onClick={() =>
                  window.confirm('Delete your Bana ID and its sync data from the relay? This cannot be undone. Notes on your devices stay.') && void act('id.delete', { confirm: true })
                }
              >
                Delete Bana ID
              </button>
            </div>
          </div>
        </>
      )}
      {error && <p className="sync-error">{error}</p>}
    </div>
  )
}

// ---- onboarding ---------------------------------------------------------------------------------

const STEPS = ['welcome', 'you', 'look', 'start', 'sync'] as const

/** First launch only (the profile says `onboarded`); every step can be skipped. */
export function Onboarding() {
  const p = useStore((st) => st.profile)
  const theme = useStore((st) => st.theme)
  const followSystem = useStore((st) => st.followSystem)
  const s = useStore((st) => st.bana)
  const [step, setStep] = useState(0)
  const [name, setName] = useState('')
  const [avatar, setAvatar] = useState<string | null>(null)
  const file = useRef<HTMLInputElement>(null)
  const folder = useRef<HTMLInputElement>(null)
  useEffect(() => void readProfile(), [])
  useEffect(() => {
    folder.current?.setAttribute('webkitdirectory', '')
  })
  if (!p || p.onboarded) return null
  const finish = () => void saveProfile({ onboarded: true, ...(name.trim() ? { name: name.trim() } : {}), avatar })
  const next = () => setStep((i) => Math.min(STEPS.length - 1, i + 1))
  const at = STEPS[step]
  const look = followSystem ? 'system' : theme
  return (
    <div className="scrim dim onboarding-scrim">
      <div className="dialog onboarding" role="dialog" aria-label="Welcome to Bana" data-step={at}>
        <div className="onboarding-dots" aria-hidden="true">
          {STEPS.map((x, i) => (
            <span key={x} className={i === step ? 'on' : ''} />
          ))}
        </div>
        {at === 'welcome' && (
          <>
            <h2 className="onboarding-title">Welcome to Bana</h2>
            <p className="onboarding-text">An outliner for your notes, supertags and daily pages. Everything stays on this device: there is nothing to sign up for. A few quick choices, all of which you can change later in Settings.</p>
          </>
        )}
        {at === 'you' && (
          <>
            <h2 className="onboarding-title">What should Bana call you?</h2>
            <p className="onboarding-text">Your name shows as you on your notes and in user fields. It stays on your devices.</p>
            <input className="cfg-input onboarding-input" aria-label="Your name" placeholder="Your name" autoFocus value={name} onChange={(e) => setName(e.target.value)} onKeyDown={(e) => (e.stopPropagation(), e.key === 'Enter' && next())} />
            <AvatarPicker name={name} avatar={avatar} onChange={setAvatar} />
          </>
        )}
        {at === 'look' && (
          <>
            <h2 className="onboarding-title">Light or dark?</h2>
            <span className="segmented onboarding-look" role="radiogroup" aria-label="Appearance">
              {(['system', 'light', 'dark'] as const).map((v) => (
                <button key={v} role="radio" aria-checked={look === v} className={`seg${look === v ? ' on' : ''}`} onClick={() => setState(v === 'system' ? { followSystem: true } : { followSystem: false, theme: v })}>
                  {v === 'system' ? 'Match the system' : v === 'light' ? 'Light' : 'Dark'}
                </button>
              ))}
            </span>
          </>
        )}
        {at === 'start' && (
          <>
            <h2 className="onboarding-title">How do you want to start?</h2>
            <div className="onboarding-choices">
              <button className="onboarding-choice" onClick={next}>
                <b>Start fresh</b>
                <span>An empty Library and today's page.</span>
              </button>
              <button
                className="onboarding-choice"
                onClick={() =>
                  void rpc('onboarding.sample').then((r) => {
                    if (r?.error) showToast(r.error)
                    else (showToast('The sample is open'), next())
                  })
                }
              >
                <b>Open the sample</b>
                <span>Projects, tasks and people to try things on.</span>
              </button>
              <button className="onboarding-choice" onClick={() => file.current?.click()}>
                <b>Import a file</b>
                <span>Tana Paste, Markdown, OPML or Roam JSON.</span>
              </button>
              <button className="onboarding-choice" onClick={() => folder.current?.click()}>
                <b>Import a folder</b>
                <span>A Notion or Logseq export (Markdown pages).</span>
              </button>
            </div>
            <input
              ref={file}
              type="file"
              accept=".md,.markdown,.txt,.opml,.json"
              hidden
              aria-label="Import file"
              onChange={(e) => {
                const f = e.target.files?.[0]
                e.target.value = ''
                if (f) void f.text().then((text) => exec(importCommand(f.name, text), { text }).then((r) => (r?.error ? showToast(r.error) : (showToast(`Imported ${f.name}`), next()))))
              }}
            />
            <input
              ref={folder}
              type="file"
              multiple
              hidden
              aria-label="Import folder"
              onChange={(e) => {
                const fs = e.target.files
                if (fs?.length) void importFolder(fs).then(next)
                e.target.value = ''
              }}
            />
          </>
        )}
        {at === 'sync' && (
          <>
            <h2 className="onboarding-title">Sync your devices?</h2>
            <p className="onboarding-text">
              Optional. With a Bana ID your notes reach your iPhone and other computers end-to-end encrypted, and you sign in with a passkey (Face ID or Touch ID), never a password. You can also pair devices with a code, or do this later in Settings, Profile.
            </p>
            {s?.signedIn ? (
              <p className="onboarding-text" data-bana-id="signed-in">
                You are signed in. {s.recoveryCode ? 'Save your recovery code below.' : ''}
              </p>
            ) : (
              <BanaIdStart person={name.trim() || p.name} />
            )}
            {s?.recoveryCode && <RecoveryCodeBox code={s.recoveryCode} onSaved={() => void rpc('id.recoveryShown').then(readId)} />}
          </>
        )}
        <div className="onboarding-actions">
          <button className="btn regular" onClick={finish}>
            {at === 'sync' ? 'Not now' : 'Skip setup'}
          </button>
          <span className="grow" />
          {step > 0 && (
            <button className="btn regular" onClick={() => setStep(step - 1)}>
              Back
            </button>
          )}
          {at === 'sync' ? (
            <button className="btn regular confirm" disabled={!!s?.recoveryCode} onClick={finish}>
              Done
            </button>
          ) : (
            <button className="btn regular confirm" onClick={next}>
              {at === 'welcome' ? 'Get started' : 'Continue'}
            </button>
          )}
        </div>
      </div>
    </div>
  )
}

/** Opens Settings at Profile. */
export function openProfile() {
  setState({ settingsTab: 'Profile' })
  runUiCommand('ui.settings')
}
