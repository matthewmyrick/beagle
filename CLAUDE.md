# CLAUDE.md — how to author RCA workspaces

This repo is the `beagle` TUI: it renders RCA (root-cause analysis)
workspaces that **you** write while debugging a system. There is a dedicated
skill for this workflow — `/rca` (`.claude/skills/rca/SKILL.md`); invoke it
whenever you are debugging an incident or writing one up. The user keeps the TUI
open; it live-reloads as you write files. Your job when debugging: investigate,
then explain **what broke, why it happened, and how to fix it** through these
files.

## Creating a workspace

Prefer the CLI (it validates and scaffolds everything):

```sh
beagle new <slug> --title "One-line incident title" --severity high \
  --system payments-api --system redis-sessions
# or: cargo run -- new ...
```

Slug rules: lowercase `[a-z0-9-]`, max 64 chars. Convention:
`YYYY-MM-DD-short-description` (e.g. `2026-06-30-sendgrid-webhook-signature-failures`).
One workspace per debugged system/incident — it is the whole debug flow's home.

## Filling it in

Write these files under `rcas/<slug>/` as the investigation progresses — don't
wait until the end; the user watches live. Update `status` in `rca.toml` as you
go: `investigating → identified → monitoring → resolved` — the easy way is
`beagle status <slug> <status>`, which stamps `updated` for you.

| File | Tab | What belongs there |
|---|---|---|
| `summary.md` | Summary | What broke, in ≤3 sentences, then current state. Write first, keep updated. |
| `timeline.md` | Timeline | `- **HH:MM UTC** — event` bullets from telemetry. Include notable *non*-events. |
| `root-cause.md` | Root Cause | Numbered causal chain from symptom down to root, plus "why it wasn't caught". |
| `impact.md` | Impact | Quantified: requests, users, duration, money, SLO budget burned. |
| `remediation.md` | Fix | Immediate mitigation first, then durable fixes with owners and status. |
| `notes.md` | Notes | Raw evidence: metrics tables, queries, log excerpts, links, open questions. |
| `diagrams/NN-name.txt` | Diagrams | ASCII diagrams (see below). |

Manifest (`rca.toml`) fields: `title`, `severity`
(`critical|high|medium|low|info`), `status`, `created` (RFC 3339, **quoted**
string), optional `updated`, `systems`, `tags`. Unknown fields are rejected —
don't invent new ones.

## Markdown that renders well

The TUI renders a deliberate subset: `#`/`##`/`###` headings, `-` bullets
(nesting via indentation), ```` ``` ```` code fences, `>` blockquotes, `---`
rules, `**bold**`, and `` `inline code` ``. Tables, images, and links render as
plain text — put tabular data in a code fence so columns align.

## Diagrams

- Plain ASCII/Unicode box drawing in `.txt` files under `diagrams/`; rendered
  **unwrapped** with horizontal panning, so alignment is preserved.
- Prefix with a number for ordering: `01-request-path.txt`, `02-fix.txt`.
- Keep lines ≤ 100 columns. Annotate the failure point (e.g. `◀── BUG`) and
  show numbers (pool sizes, latencies, rates) on the components.
- A before/after pair is the strongest way to explain a fix.
- **Color/bold via ANSI SGR codes** (zero-width, alignment-safe): inject with
  `perl -pi -e 's/BUG/\e[1;31mBUG\e[0m/' <file>`. Red = broken, green =
  healthy, yellow = degraded. Always close with `\e[0m`.

See `rcas/2026-06-30-sendgrid-webhook-signature-failures/` for a fully worked
example of tone, depth, and diagram style.

## Working on the TUI code itself

Read `CONTRIBUTING.md` and `docs/CODING_STANDARDS.md` first. Non-negotiables:
no `unsafe`, no panics reachable from user data, `cargo fmt` + pedantic
`cargo clippy --all-targets -- -D warnings` + `cargo test` must pass. The
on-disk format above is the public API — changing it needs a migration note
and an update to this file.
