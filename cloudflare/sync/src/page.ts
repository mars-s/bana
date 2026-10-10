// The Bana ID page (GET /id): runs the passkey ceremony on the relay's own origin, the relying
// party, then hands a one-time code back to the app. The app's address comes in the query
// (mode, app = its X25519 public key, state, ret = where to send the code, name, ticket); nothing
// from the query is put into the HTML. Colours mirror DESIGN.md (dark and light).

export function page(nonce: string): string {
  return `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Bana ID</title>
<style nonce="${nonce}">
:root{--bg:#1a1c1e;--card:#232526;--text:#cdcdcd;--strong:#eaeaea;--muted:#828282;--outline:#3b3c3d;--btn:#297dd9;--btn-text:#fff;--err:#e5735c;--ok:#94cba0;color-scheme:dark}
@media (prefers-color-scheme:light){:root{--bg:#f4f4f4;--card:#fff;--text:#282a2b;--strong:#141719;--muted:#8c8c8c;--outline:#dcdddd;--btn:#297dd9;--err:#c4442b;--ok:#437c5d;color-scheme:light}}
*{box-sizing:border-box}
body{margin:0;min-height:100vh;display:grid;place-items:center;background:var(--bg);color:var(--text);font:15px/1.45 -apple-system,system-ui,'Inter',sans-serif;padding:16px}
main{width:100%;max-width:380px;background:var(--card);border:1px solid var(--outline);border-radius:11.25px;padding:28px 24px}
.mark{width:40px;height:40px;border-radius:7.5px;background:var(--btn);color:#fff;display:grid;place-items:center;font-weight:600;font-size:19px;margin-bottom:16px}
h1{margin:0 0 8px;font-size:19px;color:var(--strong);font-weight:600}
p{margin:0 0 16px}
.muted{color:var(--muted);font-size:12px}
button{width:100%;border:0;border-radius:7.5px;padding:11px 14px;font:inherit;font-weight:600;background:var(--btn);color:var(--btn-text);cursor:pointer}
button:disabled{opacity:.6;cursor:default}
button:focus-visible{outline:2px solid var(--strong);outline-offset:2px}
#msg{min-height:20px;margin-top:14px;font-size:14px}
#msg.err{color:var(--err)}#msg.ok{color:var(--ok)}
</style></head>
<body><main>
<div class="mark" aria-hidden="true">B</div>
<h1 id="title">Bana ID</h1>
<p id="lead"></p>
<button id="go" type="button">Continue</button>
<div id="msg" role="status"></div>
<p class="muted" id="host"></p>
</main>
<script nonce="${nonce}">
(() => {
  const q = new URLSearchParams(location.search)
  const mode = q.get('mode')
  const $ = (id) => document.getElementById(id)
  const copy = {
    create: ['Create your Bana ID', 'Bana signs you in with a passkey, never a password. Your device saves it (in iCloud Keychain on Apple devices) and checks it is you with Face ID, Touch ID or your device code.', 'Create passkey'],
    signin: ['Sign in to Bana', 'Use the passkey you made for your Bana ID. Your notes come to this device from your other devices, still encrypted.', 'Sign in with passkey'],
    add: ['Add a passkey', 'Another passkey for the same Bana ID, for example on a device that does not share your iCloud Keychain.', 'Add passkey'],
  }[mode]
  $('host').textContent = 'Passkeys for Bana ID belong to ' + location.host + '.'
  const say = (t, cls) => { $('msg').textContent = t; $('msg').className = cls || '' }
  if (!copy) { $('go').hidden = true; say('This link is incomplete. Start again from the Bana app.', 'err'); return }
  $('title').textContent = copy[0]; $('lead').textContent = copy[1]; $('go').textContent = copy[2]
  if (!window.PublicKeyCredential) { $('go').disabled = true; say('This browser cannot use passkeys.', 'err'); return }
  const enc = (b) => btoa(String.fromCharCode(...new Uint8Array(b))).replace(/\\+/g, '-').replace(/\\//g, '_').replace(/=+$/, '')
  const dec = (s) => Uint8Array.from(atob(s.replace(/-/g, '+').replace(/_/g, '/') + '==='.slice((s.length + 3) % 4)), (c) => c.charCodeAt(0))
  const post = async (path, body) => {
    const r = await fetch('/id/api/' + path, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) })
    const j = await r.json().catch(() => ({ error: 'the relay did not answer' }))
    if (!r.ok || j.error) throw new Error(j.error || 'the relay said no')
    return j
  }
  const cat = (...parts) => { const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0)); let o = 0; for (const p of parts) { out.set(p, o); o += p.length } return out }
  // The PRF output goes back to the app masked with an X25519 secret only the app can rebuild.
  async function mask(prf) {
    const appPub = dec(q.get('app') || '')
    const kp = await crypto.subtle.generateKey({ name: 'X25519' }, true, ['deriveBits'])
    const pagePub = new Uint8Array(await crypto.subtle.exportKey('raw', kp.publicKey))
    const appKey = await crypto.subtle.importKey('raw', appPub, { name: 'X25519' }, false, [])
    const shared = new Uint8Array(await crypto.subtle.deriveBits({ name: 'X25519', public: appKey }, kp.privateKey, 256))
    const pad = new Uint8Array(await crypto.subtle.digest('SHA-256', cat(new TextEncoder().encode('bana-id/transport/v1\\0'), shared, appPub, pagePub)))
    const p = new Uint8Array(prf)
    return { page: enc(pagePub), masked: enc(p.map((x, i) => x ^ pad[i])) }
  }
  async function go() {
    $('go').disabled = true
    say('Waiting for your passkey…')
    try {
      const o = await post('options', { mode, ret: q.get('ret'), app: q.get('app'), ticket: q.get('ticket') || undefined, name: q.get('name') || undefined })
      const pk = o.publicKey
      const salt = dec(o.prfSalt)
      const ext = { prf: { eval: { first: salt } } }
      let cred, prf
      if (mode === 'signin') {
        cred = await navigator.credentials.get({ publicKey: { ...pk, challenge: dec(pk.challenge), allowCredentials: [], extensions: ext } })
        prf = cred.getClientExtensionResults().prf?.results?.first
      } else {
        cred = await navigator.credentials.create({ publicKey: { ...pk, challenge: dec(pk.challenge), user: { ...pk.user, id: dec(pk.user.id) }, excludeCredentials: pk.excludeCredentials.map((c) => ({ ...c, id: dec(c.id) })), extensions: ext } })
        const r = cred.getClientExtensionResults().prf
        prf = r?.results?.first
        // Some platforms evaluate PRF only when signing in: ask once more, for this passkey.
        if (!prf && r?.enabled) {
          const again = await navigator.credentials.get({ publicKey: { challenge: crypto.getRandomValues(new Uint8Array(32)), rpId: pk.rp.id, userVerification: 'required', allowCredentials: [{ type: 'public-key', id: cred.rawId }], extensions: ext } })
          prf = again.getClientExtensionResults().prf?.results?.first
        }
      }
      const res = cred.response
      const response = { clientDataJSON: enc(res.clientDataJSON) }
      if (mode === 'signin') {
        response.authenticatorData = enc(res.authenticatorData)
        response.signature = enc(res.signature)
        if (res.userHandle) response.userHandle = enc(res.userHandle)
      } else response.attestationObject = enc(res.attestationObject)
      const m = prf ? await mask(prf) : { page: null, masked: null }
      const f = await post('finish', { challengeId: o.challengeId, id: cred.id, response, ...m })
      say(prf ? 'Done. Going back to Bana…' : 'Done. This passkey cannot unlock your notes on a new device (no PRF here): use your recovery code or pair with a code.', prf ? 'ok' : 'err')
      const back = f.ret + '?code=' + encodeURIComponent(f.code) + '&state=' + encodeURIComponent(q.get('state') || '') + (prf ? '' : '&prf=none')
      setTimeout(() => { location.href = back }, prf ? 0 : 1500)
    } catch (e) {
      $('go').disabled = false
      say(e && e.name === 'NotAllowedError' ? 'Cancelled, or the passkey was not found. Try again.' : 'Something went wrong: ' + (e && e.message), 'err')
    }
  }
  $('go').addEventListener('click', go)
})()
</script></body></html>`
}
