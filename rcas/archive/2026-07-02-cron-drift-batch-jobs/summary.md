# Summary

Nightly batch jobs on 2026-07-02 started 3 hours late because the
`batch-scheduler` host's clock had drifted 47 minutes after chrony was
wedged by a stale lock file, pushing jobs past their cron windows into
the next catch-up cycle.

**Current state:** finished. Chrony restarted, lock handling fixed,
clock-drift alerting added. Verified over the following week.
