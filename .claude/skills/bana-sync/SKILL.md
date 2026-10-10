---
name: bana-sync
description: Use when Bana saves, Loro history or sync change.
category: general
---

# Bana saving, history and sync

Never cut a copy's history below what another copy may still need to merge.

- Compact (drop old Loro history) only up to a version that every known copy has acknowledged.
- Until sync exists there are no acknowledgements, so keep the full history. Time-based cuts ("older than a month") are a bug.
- A copy offline longer than any cut can never merge again. Phone sync depends on this.
- Code: `Store::compact(acked)` in `crates/core/src/store.rs` cuts at the common version of the acknowledged versions (none: no cut); `Graph::compact_history(acked)`; `Graph::save` never compacts. When sync lands, keep each paired copy's acknowledged version in the document and pass them on save.
- Tests (`crates/core/tests/merge.rs`): `a_copy_offline_for_sixty_days_still_merges` (the scenario in `references/sync-tests.md`, with daily compaction, save and reload) and `compaction_cuts_only_what_every_paired_copy_acknowledged`. Keep both green for any save, history or sync change.
- Every keystroke must be durable before it counts as saved. Editor rules are in the `bana-ui` skill, "Data safety".
