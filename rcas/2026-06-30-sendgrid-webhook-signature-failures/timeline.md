# Timeline — SendGrid webhook signature failures

All times UTC, reconstructed from Sentry (production Loki logs are unavailable
— log shipping from the production webserver has been down since 2026-06-26,
tracked separately).

- **2026-03-18 13:43** — First `magic_link_bounce` signature-verification
  failure recorded (HADRIUS-BACKEND-CZK). Failures are continuous from this
  moment on: every webhook delivery is rejected with 403.
- **2026-03-18 → present** — SendGrid retries each rejected batch for up to
  24h, then drops the events permanently. Bounced auth emails stop being
  marked on `OAuthMagicLink` rows.
- **~2026-06-30** — The `email_delivery` webhook endpoint ships (per-send
  delivery tracking for `OutboundEmail`). Its very first webhook fails the
  same verification (HADRIUS-BACKEND-ET2/ET3 first seen), and every one
  since. The feature has never successfully processed an event in production.
- **2026-07-06 22:46** — Latest failure observed while writing this RCA;
  both endpoints still rejecting every delivery ("last seen: 0 minutes ago").
- **2026-07-06 22:48** — This investigation opened.

## Notable non-events

- **No missing-header warnings.** The code logs a distinct message when the
  signature/timestamp headers are absent; we only ever see the
  *verification-failed* message. SendGrid **is** signing its requests — the
  requests are real and well-formed; our key just doesn't match.
- **No successful processing logs are known** for either endpoint in
  production (unconfirmable via Loki while log shipping is down, but Sentry
  failure volume ≈ expected total webhook volume, leaving no room for a
  success population).
- **No pages fired** in the 3.5 months of `magic_link_bounce` failures — no
  alert exists on webhook rejection rate.
