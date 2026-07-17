# Timeline

- **2026-06-28** — host rebooted for kernel patching; chrony left a
  stale lock file behind
- **06-28 → 07-02** — clock drifts ~11 min/day, unnoticed
- **07-02 01:00 UTC (wall)** — cron fires "01:00" jobs at 01:47 real
  time; window checks reject them; jobs deferred to catch-up at 04:00
- **07-02 07:15 UTC** — data-freshness report flags the delay
- **07-02 08:05 UTC** — drift confirmed with `chronyc tracking`
- **07-02 08:20 UTC** — stale lock removed, chrony restarted, clock
  stepped back; catch-up run completed 09:40 UTC
