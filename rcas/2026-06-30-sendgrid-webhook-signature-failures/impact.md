# Impact — SendGrid webhook signature failures

## Data loss (the real impact)

- **~268,000 webhook requests rejected** and their events permanently lost
  after SendGrid's ~24h retry window:
  - `magic_link_bounce`: ~129.6k requests since **2026-03-18** (3.5 months).
  - `email_delivery`: ~139k requests since **~2026-06-30** (feature has never
    worked in production).
- **All `OutboundEmail` delivery tracking is fiction:** every row stays
  `SENT`; delivered/opened/bounced/dropped/deferred/spam transitions and
  bounce reasons were never recorded. Any dashboard, review workflow, or
  compliance evidence built on these statuses is silently empty or wrong.
- **Bounced auth emails are invisible:** `OAuthMagicLink` rows keep
  `email_dispatch_status=SENT` when the email actually bounced, so employees
  who never received their magic link are not flagged for follow-up —
  onboarding/auth flows quietly stall for anyone with a bad address.

## What was NOT impacted

- Email **sending** — unaffected throughout; this is feedback-loop-only.
- Security posture — verification failed *closed* (403), so no spoofed
  webhook was ever accepted. The bug rejects real traffic; it does not admit
  fake traffic.

## Cost and noise

- ~536k Sentry error events from this one cause (each rejected request emits
  a log-error event *and* a captured exception — double-counted), consuming
  Sentry quota and burying real issues for 3.5 months.
- "Users impacted: 36–60" in Sentry is misleading — those are SendGrid's
  webhook source IPs, not people. The human impact is the unflagged
  bounced-email population, which needs the backfill in the Fix tab to size.
