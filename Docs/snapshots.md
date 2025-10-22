# Snapshot Plan & Replay Strategy

- WAL-first: all events appended before effects.
- Replay-on-start: scan WAL at startup to rebuild minimal in-memory indexes (last_event_id_by_run, seen_ids). Missing derived state is rebuilt lazily.
- Snapshots: periodic checkpoint of run state (every N events or M minutes) to reduce replay latency. Snapshots are written atomically (temp + rename) and validated on load.
- Recovery order: load latest snapshot (if any) → replay WAL entries after snapshot.
- Crash tests: simulate abrupt stop during write; ensure previous data intact (append only); on restart verify replay reconstructs the same indexes.
