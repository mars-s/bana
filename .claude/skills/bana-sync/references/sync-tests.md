# Sync test: long offline copy

Setup: two copies of one outline, both start from the same snapshot.

1. Copy A edits every day for 10 days.
2. Copy B stays offline. Simulate 60 days with no edits, then 20 edits.
3. Exchange updates both ways.
4. Pass: both copies show the same outline, and no edit from either side is lost.

Fail the test if compaction has removed history that copy B still needs.
