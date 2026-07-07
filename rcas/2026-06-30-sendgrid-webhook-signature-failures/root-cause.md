# Root Cause — SendGrid webhook signature failures

## Causal chain, symptom → root

1. **Symptom:** Sentry fills with `SendGrid webhook signature verification
   failed` from both `/api/third_party/sendgrid/webhook/magic_link_bounce/`
   and `/webhook/email_delivery/`; delivery statuses and bounce flags never
   update.
2. Both Django views gate on `sendgrid_client().verify_webhook_signature()`
   and return **403** when it fails
   (`hadrius/hadrius/views_webhooks.py:86` and `:172`).
3. `verify_webhook_signature`
   (`common/third_party/sendgrid/sendgrid_client_impl.py:91`) verifies the
   request's ECDSA signature against **one** key:
   `settings.SENDGRID_WEBHOOK_VERIFICATION_KEY`. The signature and timestamp
   headers are present (the missing-header branch never logs), so the
   cryptographic check itself is what fails — for **every** request.
4. A signature that fails 100% of the time for months is not intermittent
   corruption; the verification public key does not correspond to the
   private key SendGrid is signing with.
5. **Root cause:** key mismatch between the SendGrid Event Webhook
   configuration and the deployed env var. SendGrid issues a **separate
   signing keypair per webhook configuration** — Hadrius has (at least) two
   configurations posting to two endpoints, but the backend holds a single
   `SENDGRID_WEBHOOK_VERIFICATION_KEY` for both. The stored key matches
   neither active webhook config: most likely it belongs to a deleted/old
   webhook config, or was never updated after the webhook was recreated or
   its signing was re-enabled (which rotates the keypair).

## Why it wasn't caught

- **The endpoint returns 403 and nothing breaks visibly.** Email *sending*
  works fine; only the feedback loop dies. No user-facing error ever occurs.
- **No alert on webhook rejection rate**, and the Sentry issues drowned in a
  noisy unresolved queue (this org has 15+ six-figure-event issues).
- **The failure mode is fail-open in reverse:** with *no* key configured the
  code skips verification entirely and processes events
  (`sendgrid_client_impl.py:93-95`) — so staging (key unset) processed
  webhooks happily while production (wrong key) silently rejected them.
  The environments behaved differently in exactly the way tests don't catch.
- **`email_delivery` shipped on top of the already-broken key** (~2026-06-30)
  and was presumably validated in staging, where verification is skipped.

## Contributing factors

- One env var shared by two webhook configs that can never both match if the
  configs have distinct keypairs.
- The empty-key escape hatch means "verification works" is never actually
  exercised outside production.
- Production Loki logging has been down since 2026-06-26, removing the log
  trail that would have made this obvious sooner.
