# Log

- **14:41 UTC** — paged; confirming symptom on the golden dashboard
- **14:44 UTC** — p99 7.4s, error rate 11%; checking Redis health first
- **14:48 UTC** — Redis healthy (CPU 12%, no slow log). Not the store
  itself; checking client-side pool metrics
- **14:55 UTC** — `redis_pool_waiters` pegged at 8/8 on every pod. Pool
  was 64 yesterday — hunting the config change
- **15:02 UTC** — found it: PR #4211 hit the base overlay, not staging.
  Preparing rollback
- **15:11 UTC** — rollback merged, ArgoCD syncing
- **15:19 UTC** — recovery confirmed; p99 176ms. Writing up
