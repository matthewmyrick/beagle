# Root cause

Causal chain, symptom → root:

1. Checkout requests time out (ELB 504s, p99 7.4s)
2. `payments-api` handlers block in `session.get()` waiting for a free
   Redis connection — `redis_pool_waiters` at max, wait time unbounded
3. The session pool has 8 connections per pod instead of 64; at normal
   traffic each pod needs ~40 concurrent connections at p99
4. Config PR #4211 set `redis.pool_size: 8` in the **base** kustomize
   overlay instead of the staging overlay — the value was meant to
   starve a staging soak test, not production
5. **Root:** the base file is the default target when editing overlays,
   and nothing validates pool size against expected concurrency

## Why it wasn't caught

- CI validates config *syntax*, not semantics — `pool_size: 8` is valid
- Staging soak ran green because staging *was* meant to have the small
  pool; the regression only exists under production traffic
- No alert on `redis_pool_waiters`; the first signal was user-facing
  latency, 5 minutes after sync

## Contributing factors

- ArgoCD auto-sync applied the change with no canary window
- Overlay-targeting mistakes are invisible in a unified diff unless you
  know the kustomize layout
