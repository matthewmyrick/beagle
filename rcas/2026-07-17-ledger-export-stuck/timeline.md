# Timeline

- **02:00 UTC** — nightly `ledger-export` job starts (normal)
- **02:10 UTC** — last row written to the export bucket
- **06:30 UTC** — downstream freshness check flags the export as stale
  (soft alert, no page)
- **09:47 UTC** — finance reports the close is blocked; incident opened
- **10:02 UTC** — job pod alive, low CPU, no errors in logs (notable:
  it is *waiting*, not crashing)
- **10:31 UTC** — `pg_stat_activity` shows the export's cursor query
  in `Lock` wait state for 8h
