// A software passkey authenticator with the PRF extension, and the Bana ID page's steps on top
// of it, for tests (the relay's vitest, and scripts/id-helper.mjs for the iPhone UI test). It
// makes P-256 credentials, "none" attestation, discoverable (user handle = the account), PRF as
// HMAC-SHA256 of a per-credential secret (as CTAP2 hmac-secret does). Plain WebCrypto only, so it
// runs in workerd and in Node.

const te = new TextEncoder()

export const enc = (b: ArrayBuffer | Uint8Array): string => {
  const u = b instanceof Uint8Array ? b : new Uint8Array(b)
  let s = ''
  for (const x of u) s += String.fromCharCode(x)
  return btoa(s).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '')
}
export const dec = (s: string): Uint8Array => Uint8Array.from(atob(s.replace(/-/g, '+').replace(/_/g, '/') + '==='.slice((s.length + 3) % 4)), (c) => c.charCodeAt(0))
const cat = (...p: Uint8Array[]) => {
  const out = new Uint8Array(p.reduce((n, x) => n + x.length, 0))
  let o = 0
  for (const x of p) (out.set(x, o), (o += x.length))
  return out
}
const sha = async (b: Uint8Array | string) => new Uint8Array(await crypto.subtle.digest('SHA-256', typeof b === 'string' ? te.encode(b) : b))

// ---- CBOR encoding (what an attestation object needs) ------------------------------------------

type C = number | string | Uint8Array | Map<C, C>
function head(major: number, n: number): Uint8Array {
  if (n < 24) return new Uint8Array([(major << 5) | n])
  if (n < 256) return new Uint8Array([(major << 5) | 24, n])
  return new Uint8Array([(major << 5) | 25, n >> 8, n & 255])
}
export function cbor(v: C): Uint8Array {
  if (typeof v === 'number') return v >= 0 ? head(0, v) : head(1, -1 - v)
  if (typeof v === 'string') return cat(head(3, te.encode(v).length), te.encode(v))
  if (v instanceof Uint8Array) return cat(head(2, v.length), v)
  return cat(head(5, v.size), ...[...v].flatMap(([k, x]) => [cbor(k), cbor(x)]))
}

function rawToDer(raw: Uint8Array): Uint8Array {
  const int = (b: Uint8Array) => {
    let i = 0
    while (i < b.length - 1 && b[i] === 0) i++
    let v = b.slice(i)
    if (v[0] & 0x80) v = cat(new Uint8Array([0]), v)
    return cat(new Uint8Array([2, v.length]), v)
  }
  const body = cat(int(raw.slice(0, 32)), int(raw.slice(32)))
  return cat(new Uint8Array([0x30, body.length]), body)
}

interface Cred {
  id: Uint8Array
  keys: CryptoKeyPair
  rpId: string
  user: Uint8Array
  prfSecret: Uint8Array
  counter: number
}

export class SoftAuthenticator {
  creds: Cred[] = []
  /** Turn PRF off to act as a platform without it. */
  prf = true
  /** Count signatures (some platforms always send 0). */
  counting = true

  async prfOf(c: Cred, salt: Uint8Array): Promise<Uint8Array> {
    const k = await crypto.subtle.importKey('raw', c.prfSecret, { name: 'HMAC', hash: 'SHA-256' }, false, ['sign'])
    return new Uint8Array(await crypto.subtle.sign('HMAC', k, cat(te.encode('WebAuthn PRF\0'), salt)))
  }

  async create(pk: any, origin: string, salt: Uint8Array) {
    const keys = (await crypto.subtle.generateKey({ name: 'ECDSA', namedCurve: 'P-256' }, true, ['sign', 'verify'])) as CryptoKeyPair
    const jwk = (await crypto.subtle.exportKey('jwk', keys.publicKey)) as JsonWebKey
    const c: Cred = { id: crypto.getRandomValues(new Uint8Array(16)), keys, rpId: pk.rp.id, user: dec(pk.user.id), prfSecret: crypto.getRandomValues(new Uint8Array(32)), counter: 0 }
    this.creds.push(c)
    const cose = cbor(new Map<C, C>([[1, 2], [3, -7], [-1, 1], [-2, dec(jwk.x!)], [-3, dec(jwk.y!)]]))
    const authData = cat(await sha(c.rpId), new Uint8Array([0x45, 0, 0, 0, 0]), new Uint8Array(16), new Uint8Array([0, c.id.length]), c.id, cose)
    const clientDataJSON = te.encode(JSON.stringify({ type: 'webauthn.create', challenge: pk.challenge, origin, crossOrigin: false }))
    return {
      id: enc(c.id),
      response: { clientDataJSON: enc(clientDataJSON), attestationObject: enc(cbor(new Map<C, C>([['fmt', 'none'], ['attStmt', new Map()], ['authData', authData]]))) },
      prf: this.prf ? await this.prfOf(c, salt) : null,
    }
  }

  async get(pk: any, origin: string, salt: Uint8Array, pick?: (c: Cred) => boolean) {
    const c = this.creds.find((x) => x.rpId === pk.rpId && (!pick || pick(x)))
    if (!c) throw new Error('NotAllowedError: no passkey for ' + pk.rpId)
    if (this.counting) c.counter++
    const counter = new Uint8Array(4)
    new DataView(counter.buffer).setUint32(0, c.counter)
    const authData = cat(await sha(c.rpId), new Uint8Array([0x05]), counter)
    const clientDataJSON = te.encode(JSON.stringify({ type: 'webauthn.get', challenge: pk.challenge, origin, crossOrigin: false }))
    const raw = new Uint8Array(await crypto.subtle.sign({ name: 'ECDSA', hash: 'SHA-256' }, c.keys.privateKey, cat(authData, await sha(clientDataJSON))))
    return {
      id: enc(c.id),
      response: { clientDataJSON: enc(clientDataJSON), authenticatorData: enc(authData), signature: enc(rawToDer(raw)), userHandle: enc(c.user) },
      prf: this.prf ? await this.prfOf(c, salt) : null,
    }
  }
}

/** What the page does with the PRF output: masks it for the app's X25519 key. */
export async function mask(prf: Uint8Array, appPub: Uint8Array) {
  const kp = (await crypto.subtle.generateKey({ name: 'X25519' }, true, ['deriveBits'])) as CryptoKeyPair
  const pagePub = new Uint8Array((await crypto.subtle.exportKey('raw', kp.publicKey)) as ArrayBuffer)
  const appKey = await crypto.subtle.importKey('raw', appPub, { name: 'X25519' }, false, [])
  const shared = new Uint8Array(await crypto.subtle.deriveBits({ name: 'X25519', public: appKey } as any, kp.privateKey, 256))
  const pad = await sha(cat(te.encode('bana-id/transport/v1\0'), shared, appPub, pagePub))
  return { page: enc(pagePub), masked: enc(prf.map((x, i) => x ^ pad[i])) }
}

/** The app's side of the mask: an X25519 key pair, and unmasking. */
export async function appKeys() {
  const kp = (await crypto.subtle.generateKey({ name: 'X25519' }, true, ['deriveBits'])) as CryptoKeyPair
  const pub = new Uint8Array((await crypto.subtle.exportKey('raw', kp.publicKey)) as ArrayBuffer)
  return {
    pub: enc(pub),
    async unmask(page: string, masked: string) {
      const pk = await crypto.subtle.importKey('raw', dec(page), { name: 'X25519' }, false, [])
      const shared = new Uint8Array(await crypto.subtle.deriveBits({ name: 'X25519', public: pk } as any, kp.privateKey, 256))
      const pad = await sha(cat(te.encode('bana-id/transport/v1\0'), shared, pub, dec(page)))
      return dec(masked).map((x, i) => x ^ pad[i])
    },
  }
}

export type Post = (path: string, body: unknown) => Promise<{ status: number; body: any }>

/** The page's whole ceremony: options, the passkey, finish. Answers finish's reply (a code). */
export async function ceremony(post: Post, auth: SoftAuthenticator, origin: string, q: { mode: string; ret: string; app: string; ticket?: string; name?: string }) {
  const o = await post('options', q)
  if (o.status !== 200) return o
  const salt = dec(o.body.prfSalt)
  const r = q.mode === 'signin' ? await auth.get(o.body.publicKey, origin, salt) : await auth.create(o.body.publicKey, origin, salt)
  const m = r.prf ? await mask(r.prf, dec(q.app)) : { page: null, masked: null }
  return post('finish', { challengeId: o.body.challengeId, id: r.id, response: r.response, ...m })
}
