# Log

- **09:52 UTC** — incident opened from finance report; checking job pod
- **10:02 UTC** — pod alive, 0.02 CPU, no errors. It's waiting on
  something — checking the database side
- **10:18 UTC** — reading pg_stat_activity for the export's connection
- **10:31 UTC** — cursor query in Lock wait since 02:03. Looking for the
  lock holder
- **10:44 UTC** — holder is a wraparound autovacuum on ledger_entries,
  running since 01:58. Checking why freeze age got this high
- **10:58 UTC** — found the disabled weekly VACUUM cron from the June
  maintenance window. Drafting remediation while vacuum finishes (61%)
