# Fix — SendGrid webhook signature failures

## Immediate fix (config only, no deploy)

1. In the SendGrid dashboard (**Settings → Mail Settings → Event Webhooks**),
   open each webhook configuration and copy its **Verification Key** (the
   ECDSA public key shown when Signed Event Webhook is enabled).
2. Confirm how many webhook configs post to hadrius-backend. Two endpoints
   are hit (`magic_link_bounce`, `email_delivery`), which usually means two
   configs — **each has its own key**.
3. Set the production env var(s) and restart:
   - If one webhook config serves both URLs: update
     `SENDGRID_WEBHOOK_VERIFICATION_KEY` to its current key.
   - If two configs: the single shared env var **cannot** be correct for
     both — split it (durable fix #1) or consolidate to one SendGrid config.
4. Verify: POST volume on both endpoints should flip from 403 to 200 within
   minutes (watch the Sentry issues stop; Loki is unavailable until log
   shipping is fixed).

## Durable fixes

1. **Per-endpoint verification keys** — replace the single setting with
   `SENDGRID_DELIVERY_WEBHOOK_KEY` / `SENDGRID_BOUNCE_WEBHOOK_KEY` (falling
   back to the shared var) so two SendGrid configs can both verify.
   Owner: backend. *(todo)*
2. **Fail closed in production** — the empty-key branch currently *skips*
   verification (`sendgrid_client_impl.py:93-95`). Refuse to boot in
   production with an unset key, and make staging set a real key so
   verification is exercised before prod. Owner: backend. *(todo)*
3. **Alert on webhook rejection** — page when signature failures on these
   endpoints exceed ~10/min for 15 min. This incident would have paged on
   2026-03-18 instead of surfacing 3.5 months later. Owner: infra. *(todo)*
4. **Backfill the lost state** — SendGrid's Email Activity API retains ~30
   days: reconcile `OutboundEmail` statuses for that window and sweep
   `OAuthMagicLink` rows stuck `SENT`+`PENDING` (bounce state older than the
   retention window is unrecoverable). Owner: backend. *(todo)*
5. **Resolve the four Sentry issues** (ET2, ET3, CZJ, CZK) once 200s flow,
   so a regression reopens them cleanly as *new* signal. Owner: backend.
   *(todo)*

## Verification

- Send a test event from SendGrid's webhook UI ("Test Your Integration") →
  expect 200 and a `Processed SendGrid bounce webhook` log line.
- Confirm an `OutboundEmail` row transitions `SENT → DELIVERED` end-to-end.
- Negative check: tamper one byte of a replayed payload → expect 403 (proves
  verification is still on, not skipped).
