# Fix

## Immediate mitigation (done)

- 15:11 UTC — revert PR restoring `redis.pool_size: 64`, auto-synced;
  recovery confirmed 15:19 UTC

## Durable fixes

- **Pool-size guardrail** — config schema check: `pool_size` in any
  production overlay must be ≥ expected p99 concurrency (owner: platform,
  status: PR open)
- **Alert on saturation** — page when `redis_pool_waiters > 0` for 2m;
  this fires ~4 minutes before user-facing latency did (owner: payments,
  status: PR open)
- **Overlay lint** — CI fails when a PR titled/labelled staging touches
  the base overlay (owner: platform, status: backlog)
