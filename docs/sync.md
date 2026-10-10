# Sync: your Mac and your iPhone

Bana keeps the same notes on your computer and your iPhone. Each device has its own full copy and
works offline. When a device is online it sends its changes to a small relay and picks up the
other devices' changes from it. The relay runs on **your own** Cloudflare account, on the free
plan, and it only ever holds encrypted data.

```
 Mac (Bana)  ──encrypted──▶  relay on your Cloudflare account  ◀──encrypted──  iPhone (Bana)
             ◀─────────────  Worker + one Durable Object per     ─────────────▶
                             synced workspace (a "room")
```

## Setting it up

Nothing is created for you: the Cloudflare account and the deploy are yours. One script walks you
through every step and checks the result:

```sh
bana/cloudflare/sync/wizard.sh
```

It signs Wrangler in, deploys the relay (`bana-sync`, free plan), turns sync on in the running
desktop app (or tells you where to click), shows how to install the iPhone app on your own phone
with Xcode (free Apple ID: 7 days at a time; paid developer account: a year, or TestFlight), and
how to pair the phone.

By hand, the same steps are:

1. `cd cloudflare/sync && npm ci && npx wrangler login && npx wrangler deploy`. The relay answers
   at `https://bana-sync.<your subdomain>.workers.dev`.
2. Desktop: account menu, **Settings, Sync**: paste that address into **Relay address**, then
   **Set up sync**. The top bar shows a small cloud: green dot synced, blue syncing, grey offline,
   red stopped.
3. Desktop: **Add a device** shows a QR code and the same code as text.
4. iPhone: **Manage, Sync, Scan the code** (or paste the code). The phone's notes are replaced by
   the desktop's; its old file stays beside them as `graph.bana.before-sync`.

A second computer joins the same way: **Settings, Sync, Join another device's sync** with a code.

### Or with a Bana ID (no codes)

A Bana ID ([bana-id.md](bana-id.md)) is optional: it signs you in with a passkey instead of a
pairing code. **Profile and Bana ID, Create Bana ID** on the first device sets sync up and keeps
the workspace key on the relay only wrapped (locked by your passkey and by a printable recovery
code). On a new device, **Sign in with a passkey** unlocks the key on that device and joins.
Pairing with a code keeps working, and it is the way to go where a passkey cannot unlock notes
(no PRF in that browser).

## What Cloudflare (or anyone who runs the relay) can and cannot see

It **cannot** see:

- your notes, their titles, supertags, fields or files: every change is encrypted on the device
  that made it (XChaCha20-Poly1305) with the workspace key before it is sent;
- the workspace key: it is made on your computer and only travels inside the pairing code, which
  is shown on your screen and never sent to the relay, put in a web address or written to a log;
- device names: they are encrypted too;
- which notes changed, or anything about their structure;
- with a Bana ID: the workspace key, which it keeps only wrapped by keys it never sees (your
  passkey's PRF secret and your recovery code), see [bana-id.md](bana-id.md).

It **can** see:

- that a room exists (its name is a hash of the key, so it says nothing about the key) and how
  many devices are in it, as public keys;
- when each device connects, from which IP address, and how big each encrypted change is;
- how many changes there are and in what order, and how far each device has read.

It **cannot change** your notes either: an encrypted change that was altered, replayed as
another kind of message, or credited to another device fails to decrypt and is skipped (the
device that sent it sends the content again). A relay can still refuse to pass changes on;
then devices stay apart until it works again, and nothing is lost on either side.

## How it behaves

- **Offline is normal.** Edit on both devices without a network; when they reconnect, both
  sets of edits merge. Typing in the same line on two devices keeps both people's words.
- **Your caret never jumps.** A change from the other device appears in the line you are typing
  in, and your caret stays where it was in your own words.
- **Undo stays yours.** Undo takes back only this device's edits, never the other device's.
- **A day made on both devices** (today's page opened on each while offline) becomes one day.
- **Status**: Synced, Syncing (changes on their way), Offline (no network or relay; it retries
  with growing pauses, up to a minute), Stopped (this device was removed, or a code did not
  match), with the reason in Settings or Manage.
- **On the iPhone** it syncs on launch, when the app comes to the front, while open, and now and
  then in the background (iOS decides when).

## Devices and keys

- **Remove a device** (Settings, Sync, Devices; on the phone swipe it in Manage): the relay closes
  it and lets it back only with a new invite. It keeps the notes it already has, and it still
  knows the key. To shut a lost device out completely, turn sync off on every device and set it
  up again: that makes a new key and a new room.
- **Turn sync off** on a device: it stops syncing and forgets the key; the notes stay.
- **Pairing codes** work once, for 24 hours. A code holds the key: show it only to your devices.
- Where the key lives: on the desktop in `graph.bana.sync` beside the graph (readable by your user
  only), on the iPhone in the app's private Library folder (not in Documents, which the Files app
  and Finder can see).

## History and storage

The notes are a Loro document (`crates/core/src/store.rs`). Each device keeps history only as far
back as every other paired device has confirmed having (they confirm with an encrypted version),
so a phone that was away for months still merges. The relay keeps every change until every device
has read it; then a device replaces the old changes with one encrypted snapshot. A device that
never comes back would keep that from happening: remove it.

## The free plan

The relay is built for the Workers free plan: SQLite-backed Durable Objects (one per room) and the
WebSocket Hibernation API, so an idle connection costs nothing and keep-alive pings are answered
without waking the room. Each push of changes is one stored row; a device pushes at most about once
a second while you type, so a day of steady typing stays well inside the free plan's 100,000 rows
written a day. Limits per room: 16 devices, 32 connections, 64 MB stored, frames up to 1 MB, about
20 messages a second per connection, 10 open invites.

## For developers

| Piece | Where |
|---|---|
| Keys, encryption, pairing codes, the client state machine, the WebSocket service | `crates/sync` (`bana-sync`) |
| The relay: Worker and Durable Object | `cloudflare/sync/src/index.ts`, limits in `src/limits.ts` |
| The engine side: sync on the host's graph, `sync.*` requests | `crates/server/src/sync.rs` |
| Desktop and browser UI | `web/src/Sync.tsx` (Settings, Sync and the top bar button) |
| iPhone | `apps/bana-ios/Bana/Sync.swift` (Manage, Sync, QR scanner), `Engine.startSync`, `Engine.syncNow` |
| Bana ID (passkey sign-in, wrapped keys, recovery) | `crates/sync/src/id.rs`, `account.rs`, `cloudflare/sync/src/id.ts`; see [bana-id.md](bana-id.md) |

Requests (the web UI, the phone and local tools send them like any engine request; they never wait
for the engine lock): `sync.status`, `sync.setup {relay, name}`, `sync.join {code, name}`,
`sync.invite` (answers `{code, qr}`), `sync.devices`, `sync.revoke {device}`, `sync.leave`,
`sync.now {wait}`. Bana ID adds `id.*` (listed in [bana-id.md](bana-id.md)); a device signed in
with it joins the room with an invite the relay makes at sign-in, like a pairing code's.

The protocol (JSON text frames over one WebSocket per device, `GET /v1/rooms/<room>`):
the relay sends a challenge, the device signs it with its Ed25519 key (`hello`, with an invite
secret when joining; the first device creates the room). Then `push` (an encrypted change, in
parts of 256 KB at most), `ack` (read up to here, with the encrypted Loro version), `pull` (the next
page of backlog), `snap` (a snapshot replacing what every device has read), `invite`, `revoke`.
The relay answers `welcome`, `snapshot`, `update`, `synced`, `ok`, `acked`, `devices`, `pruned`,
`error`. `crates/sync/src/proto.rs` has every field.

Tests:

```sh
cargo xtask sync   # all of the below
cargo test -p bana-sync                         # engines through an in-process fake relay
cd cloudflare/sync && npm ci && npm test        # the relay in workerd (vitest)
pnpm --dir web exec playwright test --project=sync   # wrangler dev + two engines, two windows
scripts/ios-test.sh sync                        # the iPhone pairs with a pasted code and syncs
```

The Rust tests run two and three engines with offline edits on every side, typing in one line at
once, moves, deletes against edits, a phone away for 60 simulated days, snapshot pruning and
compaction, a device joining from a compacted snapshot, revoking, a relay that alters data, and a
device with the wrong key.
