# Fix (draft — investigation ongoing)

## Being evaluated

- Let the wraparound vacuum finish (est. 2–3h remaining per
  `pg_stat_progress_vacuum`) and restart the export after
- Re-enable the weekly manual VACUUM cron (confirmed disabled since
  2026-06-14 maintenance)
- Alert on `autovacuum_freeze_max_age` approach, and page (not
  soft-alert) on export freshness
