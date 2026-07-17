# Notes

```sql
SELECT pid, state, wait_event_type, wait_event, query_start
FROM pg_stat_activity WHERE application_name = 'ledger-export';
-- pid 88231 · active · Lock · relation · 02:03:11 UTC

SELECT * FROM pg_stat_progress_vacuum;
-- ledger_entries · phase: scanning heap · 61% done
```

- Vacuum start correlates with export hang: 01:58 vs 02:10 (first lock
  acquisition attempt on the hot partition)
- Maintenance-window doc (2026-06-14): "disable weekly VACUUM cron
  during migration" — no re-enable step listed

## Open questions

- Can the export cursor use a snapshot that doesn't conflict with
  wraparound vacuum?
