# Root cause (working hypothesis — unconfirmed)

1. Export job writes no rows after 02:10 UTC
2. Its cursor query sits in `wait_event_type = Lock`
3. An `autovacuum: VACUUM (to prevent wraparound)` on `ledger_entries`
   started 01:58 UTC and holds a conflicting lock
4. Wraparound vacuum is aggressive and won't yield; the table crossed
   `autovacuum_freeze_max_age` yesterday

Still to confirm: why freeze age got this high — suspect the weekly
manual VACUUM cron was disabled during the 2026-06 maintenance window
and never re-enabled.
