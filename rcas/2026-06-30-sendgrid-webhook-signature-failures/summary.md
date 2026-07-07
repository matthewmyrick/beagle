# Summary — SendGrid webhook signature failures

Every SendGrid event webhook that reaches `hadrius-backend` production fails
ECDSA signature verification and is rejected with a 403 — on **both**
endpoints: `magic_link_bounce` (failing since **2026-03-18**, ~130k rejected
requests) and `email_delivery` (failing since it shipped ~**2026-06-30**,
~139k rejected requests).

The consequence is silent data loss, not an outage: outbound-email delivery
statuses never leave `SENT`, bounce reasons are never captured, and magic
links whose auth emails bounce are never marked `BOUNCED`.

**Current state:** root cause identified — the single
`SENDGRID_WEBHOOK_VERIFICATION_KEY` env var does not match the public key of
the SendGrid webhook configuration(s) posting to us (SendGrid issues a
distinct signing keypair per webhook config). Fix is a config change; see the
Fix tab. Verification cannot be confirmed from logs right now because
production log shipping is itself down (separate incident).
