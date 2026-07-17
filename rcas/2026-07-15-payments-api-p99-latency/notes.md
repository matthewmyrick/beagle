# Notes

## Queries

- Loki: `{app="payments-api"} |= "pool timeout"` — first hit 14:31:52 UTC
- PromQL: `max by (pod) (redis_pool_waiters{app="payments-api"})`

## Evidence

```text
pod                        pool_size   waiters(peak)   p99
payments-api-7d9f-x2lq     8           8               7.39s
payments-api-7d9f-m8na     8           8               7.41s
payments-api-7d9f-c4rr     8           8               7.38s
```

## Links

- Grafana: https://grafana.internal/d/payments-api-golden
- Config PR that caused it: https://github.com/acme/infra/pull/4211

## Open questions

- Should ArgoCD auto-sync be gated on a canary for `payments-api` at all?
