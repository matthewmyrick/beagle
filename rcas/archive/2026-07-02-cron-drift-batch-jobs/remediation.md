# Fix

- **Done** — remove stale lock, restart chrony, step the clock
- **Done** — init override: clean pid/lock file on start (PR #301)
- **Done** — alert when `|last_offset| > 5s` or sync lost for 15m
  (PR #302)
- **Backlog** — move batch-scheduler to systemd-timesyncd-monitored
  hosts pool
