// The relay's limits (a module of its own: a Worker's main module may export only handlers).

/** Limits that keep a room inside the free plan and stop a misbehaving client. */
export const LIMITS = {
  /** Biggest frame a device may send (characters). */
  frame: 1024 * 1024,
  /** Biggest base64 payload in one frame (devices send 256 KiB parts). */
  data: 400_000,
  /** Parts per message (a snapshot of up to about 32 MB). */
  parts: 128,
  /** Bytes of updates and snapshot a room may hold. */
  roomBytes: 64 * 1024 * 1024,
  devices: 16,
  sockets: 32,
  invites: 10,
  inviteMs: 24 * 3600 * 1000,
  /** Messages per socket: a burst, then this many per second. */
  burst: 200,
  perSecond: 20,
  /** A backlog page: at most this many rows or bytes. */
  pageRows: 200,
  pageBytes: 4 * 1024 * 1024,
}

/** Bana ID (src/id.ts): token buckets per IP address, and what one relay holds. */
export const ID_LIMITS = {
  /** Sign-in steps (options, finish, exchange): a burst, then this many a minute. */
  auth: { burst: 30, perMinute: 20 },
  /** New Bana IDs from one address. */
  create: { burst: 5, perMinute: 0.2 },
  /** Recovery code tries from one address. */
  recover: { burst: 5, perMinute: 0.1 },
  /** Calls with a session (the profile page): per account. */
  session: { burst: 60, perMinute: 60 },
  /** Bana IDs on one relay (it is yours: a few people, not a public service). */
  accounts: 100,
  credentials: 10,
  sessions: 20,
  challengeMs: 5 * 60 * 1000,
  codeMs: 2 * 60 * 1000,
  ticketMs: 10 * 60 * 1000,
  body: 64 * 1024,
}

export const CLOSE_AUTH = 4001
export const CLOSE_REVOKED = 4003
export const CLOSE_LIMIT = 4008
