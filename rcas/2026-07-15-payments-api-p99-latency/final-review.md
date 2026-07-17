# Final review

Checkable predictions of what "fixed" looks like. Work through after the
fix PRs merge; sign off with V when all hold.

- [ ] p99 latency stays under 200ms for 24h after guardrail PR deploys
- [ ] `redis_pool_waiters` alert fires in staging drill (forced pool of 2)
- [ ] Config schema check rejects a test PR setting `pool_size: 4` on base
- [ ] No 504s attributable to pool exhaustion in the 7 days post-merge
