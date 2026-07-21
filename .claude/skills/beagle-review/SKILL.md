---
name: beagle-review
description: Load a Beagle RCA by id and answer clarifying questions about it. Use when the user gives you a beagle RCA id (a slug like 2026-07-16-users-migration-blocked-by-failed-deploy) and wants to discuss, review, or ask questions about that incident — not write one up (that is /beagle).
---

# Review a Beagle RCA

The user has an RCA open in the Beagle TUI and wants to talk it through —
ask clarifying questions, sanity-check the root cause, poke at the fix.
Your job is to load that incident's full context and reason about it, **not**
to edit the workspace. This is the read-only companion to `/beagle`.

## 1. Get the id

The user pastes a slug (e.g. `2026-07-16-users-migration-blocked-by-failed-deploy`).
In the TUI they copy it by selecting the incident and pressing `y` (yank id).
If they didn't give one, ask for it — or run `beagle list` to show the
choices and let them pick.

## 2. Load the context

Run one command — it prints everything you need to stdout:

```sh
beagle context <id>
```

That bundle is:

- The **full RCA writeup** — the manifest frontmatter (title, severity,
  status, systems, tags) followed by every section: Summary, Timeline,
  Root Cause, Impact, Fix/Remediation, Final Review checklist, Notes, and
  the diagrams.
- The **toolbox** (`toolbox.md`) — the telemetry, CLIs, and dashboards the
  investigating agent had to work with.
- The **`systems/*.md`** docs for the systems this incident touches —
  per-system architecture and quirks.

`beagle` finds the store git-style (walking up for a `.beagle` or `rcas/`),
so run it from the user's checkout. If it errors with no such workspace,
run `beagle list` to confirm the id, or ask the user to pass `--root`.

Read the whole bundle before answering. The toolbox and systems docs are
there so your questions are grounded in how this stack actually works —
use them.

## 3. Answer clarifying questions

Discuss the incident with the context in hand:

- Explain what broke, why, and how the fix addresses the root cause.
- Point out gaps: an unquantified impact, a timeline jump with no
  evidence, a Final Review prediction that isn't actually checkable, a
  root cause that stops one level short.
- Cross-reference the toolbox — if a claim could be confirmed with a query
  or dashboard listed there, say which one.
- Ground answers in the bundle; when it doesn't say, ask rather than
  guess, and be clear about what's inference.

Stay read-only: suggest edits in the conversation, but don't write to the
workspace. If the user decides to change the RCA, that's `/beagle`.
