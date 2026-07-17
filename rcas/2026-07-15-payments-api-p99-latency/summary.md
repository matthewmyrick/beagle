# Summary

Checkout requests against `payments-api` timed out for 47 minutes on
2026-07-15 (14:32–15:19 UTC) after p99 latency jumped from ~180ms to
7.4s. A config change shrank the Redis session pool from 64 to 8
connections per pod; handlers blocked waiting for a free connection.

**Current state:** mitigated at 15:19 UTC by rolling back the config.
The durable fix (pool-size guardrail + alert on pool saturation) is in
PR review.
