---
name: bana-sync
description: Use when Bana saves, Loro history or sync change.
category: general
---

# Bana saving, history and sync

Never cut a copy's history below what another copy may still need to merge.

- Compact (drop old Loro history) only up to a version that every known copy has acknowledged.
- Acknowledgements come from sync: each device sends its Loro version, sealed, with its relay `ack`. A copy that is not paired keeps the full history. Time-based cuts ("older than a month") are a bug.
- A copy offline longer than any cut can never merge again. Phone sync depends on this.
- Code: `Store::compact(acked)` in `crates/core/src/store.rs` cuts at the common version of the acknowledged versions (none: no cut); `Graph::compact_history(acked)`; `Graph::save` never compacts. The sync client (`crates/sync/src/client.rs`, `Client::compact`) compacts at most hourly, up to the other devices' acknowledged versions and its own `sent` version, and only when this copy has everything they acknowledged (otherwise an op made before an ack could arrive concurrent to the cut and fail to import).
- The relay drops updates only for a snapshot up to what every non-revoked device acknowledged. A lost device blocks that: revoke it.
- Sync rules: merges go through `Graph::merge_all` (one rebuild per batch) and are journaled before the cursor is saved and acknowledged; edits that came from the relay are never pushed back (`Client::import` advances `sent`); a joining copy replaces its graph (`Replica::replace`, old file kept as `.before-sync`). The key lives in `graph.bana.sync` (0600) or the phone's Library, never in a URL, a log or Documents.
- Sync tests: `cargo xtask sync` (fake-relay engines in `crates/sync/tests/sync.rs`, relay vitest in `cloudflare/sync/test`, Playwright project `sync` with `wrangler dev`) and `scripts/ios-test.sh sync`. The fake relay (`crates/sync/tests/relay/mod.rs`) must follow the Durable Object's rules: change both together.
- Tests (`crates/core/tests/merge.rs`): `a_copy_offline_for_sixty_days_still_merges` (the scenario in `references/sync-tests.md`, with daily compaction, save and reload) and `compaction_cuts_only_what_every_paired_copy_acknowledged`. Keep both green for any save, history or sync change.
- Every keystroke must be durable before it counts as saved. Editor rules are in the `bana-ui` skill, "Data safety".
