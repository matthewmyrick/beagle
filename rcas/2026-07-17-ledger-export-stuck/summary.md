# Summary

The nightly ledger export has been stuck since 02:10 UTC — the
`ledger-export` job is running but has written no rows for 9+ hours,
blocking the finance close. Root cause not yet confirmed; leading
hypothesis is a wraparound autovacuum holding a lock the export's
cursor query is waiting on.

**Current state:** investigating. Export job left running while we
inspect locks; finance notified of the delay. Update
