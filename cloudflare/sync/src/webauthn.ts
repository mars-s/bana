// WebAuthn checks for Bana ID (docs/bana-id.md): registration ("none" attestation; the key is
// trusted on first use, as passkeys synced through iCloud Keychain always are) and assertions.
// ES256 (P-256) keys only: what iCloud Keychain, Chrome and the virtual authenticator make.
// ponytail: ES256 only; add RS256/EdDSA when someone's authenticator offers nothing else.

export const b64url = (b: ArrayBuffer | Uint8Array): string => {
  const u = b instanceof Uint8Array ? b : new Uint8Array(b)
  let s = ''
  for (const x of u) s += String.fromCharCode(x)
  return btoa(s).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '')
}

export function fromB64url(s: unknown): Uint8Array | null {
  if (typeof s !== 'string' || s.length > 200_000 || !/^[A-Za-z0-9_-]*={0,2}$/.test(s)) return null
  try {
    const t = s.replace(/=+$/, '').replace(/-/g, '+').replace(/_/g, '/')
    return Uint8Array.from(atob(t + '='.repeat((4 - (t.length % 4)) % 4)), (c) => c.charCodeAt(0))
  } catch {
    return null
  }
}

export const sha256 = async (b: Uint8Array | string): Promise<Uint8Array> =>
  new Uint8Array(await crypto.subtle.digest('SHA-256', typeof b === 'string' ? new TextEncoder().encode(b) : b))

const eq = (a: Uint8Array, b: Uint8Array) => a.length === b.length && a.every((x, i) => x === b[i])

// ---- CBOR (just what attestation objects and COSE keys use) ------------------------------------

type Cbor = number | Uint8Array | string | Cbor[] | Map<Cbor, Cbor> | boolean | null

export function cborDecode(buf: Uint8Array): { value: Cbor; end: number } {
  let p = 0
  const need = (n: number) => {
    if (p + n > buf.length) throw new Error('cbor: truncated')
  }
  const len = (info: number): number => {
    if (info < 24) return info
    const n = info === 24 ? 1 : info === 25 ? 2 : info === 26 ? 4 : 0
    if (!n) throw new Error('cbor: unsupported length')
    need(n)
    let v = 0
    for (let i = 0; i < n; i++) v = v * 256 + buf[p++]
    return v
  }
  const item = (depth: number): Cbor => {
    if (depth > 8) throw new Error('cbor: too deep')
    need(1)
    const b = buf[p++]
    const major = b >> 5
    const info = b & 31
    switch (major) {
      case 0:
        return len(info)
      case 1:
        return -1 - len(info)
      case 2: {
        const n = len(info)
        need(n)
        const v = buf.slice(p, p + n)
        p += n
        return v
      }
      case 3: {
        const n = len(info)
        need(n)
        const v = new TextDecoder().decode(buf.slice(p, p + n))
        p += n
        return v
      }
      case 4: {
        const n = len(info)
        if (n > 64) throw new Error('cbor: array too long')
        return Array.from({ length: n }, () => item(depth + 1))
      }
      case 5: {
        const n = len(info)
        if (n > 64) throw new Error('cbor: map too long')
        const m = new Map<Cbor, Cbor>()
        for (let i = 0; i < n; i++) m.set(item(depth + 1), item(depth + 1))
        return m
      }
      case 7:
        if (info === 20) return false
        if (info === 21) return true
        if (info === 22) return null
        throw new Error('cbor: unsupported simple value')
      default:
        throw new Error('cbor: unsupported type')
    }
  }
  const value = item(0)
  return { value, end: p }
}

// ---- authenticator data -------------------------------------------------------------------------

export interface AuthData {
  rpIdHash: Uint8Array
  flags: number
  counter: number
  credId?: Uint8Array
  /** The credential's public key as a JWK (registration only). */
  jwk?: JsonWebKey
}

const UP = 0x01
const UV = 0x04
const AT = 0x40

function coseToJwk(cose: Cbor): JsonWebKey {
  if (!(cose instanceof Map)) throw new Error('the credential key is not a COSE key')
  const kty = cose.get(1)
  const alg = cose.get(3)
  const crv = cose.get(-1)
  const x = cose.get(-2)
  const y = cose.get(-3)
  if (kty !== 2 || alg !== -7 || crv !== 1 || !(x instanceof Uint8Array) || !(y instanceof Uint8Array) || x.length !== 32 || y.length !== 32) {
    throw new Error('only P-256 (ES256) passkeys are supported')
  }
  return { kty: 'EC', crv: 'P-256', x: b64url(x), y: b64url(y) }
}

export function parseAuthData(d: Uint8Array): AuthData {
  if (d.length < 37) throw new Error('authenticator data too short')
  const out: AuthData = { rpIdHash: d.slice(0, 32), flags: d[32], counter: new DataView(d.buffer, d.byteOffset + 33, 4).getUint32(0) }
  if (out.flags & AT) {
    if (d.length < 55) throw new Error('attested credential data too short')
    const n = (d[53] << 8) | d[54]
    if (n > 1023 || d.length < 55 + n) throw new Error('bad credential id')
    out.credId = d.slice(55, 55 + n)
    out.jwk = coseToJwk(cborDecode(d.slice(55 + n)).value)
  }
  return out
}

// ---- client data ----------------------------------------------------------------------------------

export interface Expect {
  type: 'webauthn.create' | 'webauthn.get'
  challenge: string
  rpId: string
  origins: (origin: string) => boolean
}

async function checkCommon(clientDataJSON: Uint8Array, authData: AuthData, e: Expect) {
  let c: { type?: string; challenge?: string; origin?: string; crossOrigin?: boolean }
  try {
    c = JSON.parse(new TextDecoder().decode(clientDataJSON))
  } catch {
    throw new Error('bad client data')
  }
  if (c.type !== e.type) throw new Error('wrong ceremony')
  if (c.challenge !== e.challenge) throw new Error('wrong challenge')
  if (typeof c.origin !== 'string' || !e.origins(c.origin)) throw new Error('wrong origin')
  if (c.crossOrigin) throw new Error('cross-origin ceremonies are refused')
  if (!eq(authData.rpIdHash, await sha256(e.rpId))) throw new Error('wrong relying party')
  if (!(authData.flags & UP) || !(authData.flags & UV)) throw new Error('the passkey must check it is you (user verification)')
}

/** A new credential: its id and public key, after checking the ceremony. */
export async function verifyRegistration(clientDataJSON: Uint8Array, attestationObject: Uint8Array, e: Expect): Promise<{ credId: string; jwk: JsonWebKey; counter: number }> {
  const att = cborDecode(attestationObject).value
  if (!(att instanceof Map)) throw new Error('bad attestation object')
  const raw = att.get('authData')
  if (!(raw instanceof Uint8Array)) throw new Error('no authenticator data')
  const ad = parseAuthData(raw)
  await checkCommon(clientDataJSON, ad, e)
  if (!ad.credId || !ad.jwk) throw new Error('no credential in the registration')
  return { credId: b64url(ad.credId), jwk: ad.jwk, counter: ad.counter }
}

/** WebAuthn signs ES256 as DER; WebCrypto takes r and s as 32 bytes each. */
export function derToRaw(der: Uint8Array): Uint8Array {
  const bad = () => new Error('bad signature')
  if (der[0] !== 0x30) throw bad()
  let p = 2
  const out = new Uint8Array(64)
  for (let k = 0; k < 2; k++) {
    if (der[p] !== 0x02) throw bad()
    let n = der[p + 1]
    p += 2
    let v = der.slice(p, p + n)
    p += n
    while (v.length > 32 && v[0] === 0) v = v.slice(1)
    n = v.length
    if (n > 32) throw bad()
    out.set(v, k * 32 + 32 - n)
  }
  return out
}

/** Checks an assertion with the stored key; answers the authenticator's new counter. */
export async function verifyAssertion(clientDataJSON: Uint8Array, authenticatorData: Uint8Array, signature: Uint8Array, jwk: JsonWebKey, e: Expect): Promise<number> {
  const ad = parseAuthData(authenticatorData)
  await checkCommon(clientDataJSON, ad, e)
  const key = await crypto.subtle.importKey('jwk', jwk, { name: 'ECDSA', namedCurve: 'P-256' }, false, ['verify'])
  const signed = new Uint8Array(authenticatorData.length + 32)
  signed.set(authenticatorData)
  signed.set(await sha256(clientDataJSON), authenticatorData.length)
  if (!(await crypto.subtle.verify({ name: 'ECDSA', hash: 'SHA-256' }, key, derToRaw(signature), signed))) throw new Error('the signature does not match')
  return ad.counter
}
