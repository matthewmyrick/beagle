# Notes — SendGrid webhook signature failures

## Sentry evidence (org `quantbase`, project `hadrius-backend`)

```text
issue                 endpoint            first seen        events    note
HADRIUS-BACKEND-CZK   magic_link_bounce   2026-03-18 13:43  129,558   log.error event
HADRIUS-BACKEND-CZJ   magic_link_bounce   2026-03-18        ~129k     capture_exception twin
HADRIUS-BACKEND-ET2   email_delivery      ~2026-06-30       138,985   log.error event
HADRIUS-BACKEND-ET3   email_delivery      ~2026-06-30       138,990   capture_exception twin
```

- Each rejected request emits BOTH a logger event and a captured exception —
  the four issues are two incidents' worth of pairs, not four causes.
- Latest event inspected: `b9e36427701c49acba560bcd497615a1`
  (2026-07-06 22:46:57, POST from 159.26.150.9 — a SendGrid IP).
- Search used: `is:unresolved level:error` sorted by freq, then
  `get_sentry_resource` on HADRIUS-BACKEND-CZK.

## Code references (repo `hadrius_backend`)

- `hadrius/hadrius/views_webhooks.py:86-89` — bounce endpoint 403s on failed
  verification.
- `hadrius/hadrius/views_webhooks.py:172-175` — delivery endpoint, same gate.
- `common/third_party/sendgrid/sendgrid_client_impl.py:91-107` — the check:
  single `settings.SENDGRID_WEBHOOK_VERIFICATION_KEY`, ECDSA verify via
  SendGrid's `EventWebhook` helper. **Empty key ⇒ verification skipped**
  (returns True) — why staging never reproduces this.
- `hadrius/hadrius/settings.py:64` — key comes from the environment with
  `default=""`.

## Open questions

- How many Event Webhook configs exist in the SendGrid dashboard, and which
  key does the env var actually hold? (Needs dashboard access — this
  determines whether the fix is "paste the right key" or "split the vars".)
- Did something happen around 2026-03-18 — webhook recreated, signing
  re-enabled, key rotated? Check SendGrid audit trail / infra change history.
- Was `email_delivery` load-tested against a signed webhook anywhere before
  ship? (Staging skips verification, so probably not.)

## Related

- Production Loki log shipping down since 2026-06-26 (separate incident) —
  blocked log-based confirmation here; RCA it next.
