# Resolution

**Immediate:** we rolled back the configuration change; checkout
recovered within minutes.

**Preventing recurrence:**

- Configuration checks now compare pool sizes against expected peak
  traffic and block changes that fall short.
- Connection-pool saturation now alerts before it becomes customer-facing.
- Production configuration rollouts are gated behind a brief canary.
