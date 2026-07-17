# Timeline

- **14:20 UTC** — config PR #4211 merges: session-store tuning, intended
  for the *staging* overlay only
- **14:27 UTC** — ArgoCD syncs the change to production `payments-api`
- **14:32 UTC** — p99 latency crosses 1s; first ELB 504s appear
- **14:35 UTC** — PagerDuty fires `payments-api-p99-slo-burn`
- **14:41 UTC** — responder confirms: error rate 11%, p99 7.4s
- **14:48 UTC** — Redis itself healthy: CPU 12%, no slow-log entries
  (notable non-event — ruled out the usual suspect)
- **14:55 UTC** — pod metrics show `redis_pool_waiters` pegged at max;
  pool size found to be 8, was 64 yesterday
- **15:02 UTC** — config diff located: PR #4211 touched the base overlay,
  not the staging one
- **15:11 UTC** — rollback PR merged and synced
- **15:19 UTC** — p99 back under 200ms; error rate 0%
