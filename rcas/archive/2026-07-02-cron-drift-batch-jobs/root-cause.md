# Root cause

1. Batch jobs ran 3h late (deferred to the catch-up cycle)
2. Cron window checks rejected the on-time run: wall clock was 47 min
   ahead of real time
3. Chrony had not been syncing since the 06-28 reboot
4. **Root:** chrony's init script does not clean a stale pid/lock file
   after an unclean shutdown, and nothing alerted on NTP sync loss

## Why it wasn't caught

No alert on `chrony_tracking_last_offset` or sync status; drift under
an hour is invisible until a window check fails.
