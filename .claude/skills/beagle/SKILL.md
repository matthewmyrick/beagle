---
name: beagle
description: Create and maintain RCA (root-cause analysis) workspaces rendered live by the Beagle TUI. Use when debugging a system or incident, or when asked to write up what broke, why it happened, and how to fix it.
---

# RCA workspaces

The user keeps the `beagle` TUI open in a terminal. It watches `rcas/` and
re-renders **live** as files change — the workspace you write *is* how you
explain the incident to them. Write as you investigate, not at the end.

## 0. Read the toolbox first

Before touching telemetry, read the investigation context at the store root
(next to `rcas/`), if present:

- `toolbox.md` — the tools available to you here: observability stack
  (Grafana dashboards, Loki/Prometheus, Sentry), CLIs you can shell out to,
  runbook locations. Start your investigation with what it lists.
- `systems/<name>.md` — per-system context (dashboards with URLs, log
  labels, dependencies, known failure modes). Read the files matching the
  systems you suspect; their names line up with `systems` in `rca.toml`.

These files are also rendered in the TUI (`T`), so the user sees the same
context you do. **If you learn something durable during the investigation**
— a dashboard moved, a new failure mode, a dependency nobody wrote down —
update the relevant `systems/` file or `toolbox.md`; that knowledge compounds
across incidents. If none of these files exist yet, suggest `beagle init`
to the user (don't run it unprompted mid-incident).

## 1. Scaffold

```sh
beagle new <slug> --title "One-line incident title" --severity <sev> \
  --system <name> --system <name>
# not installed? use: cargo run -q -- new ...
```

- Slug: lowercase `[a-z0-9-]`, ≤64 chars, convention `YYYY-MM-DD-short-description`.
- Severity: `critical` | `high` | `medium` | `low` | `info`.
- One workspace per debugged system/incident; it is the whole debug flow's home.
- Never scaffold by hand-creating files; the CLI validates and writes the
  manifest atomically. Verify with `beagle list`.

## 2. Investigate and write, in this order

All files live in `rcas/<slug>/`. Update them as evidence arrives.

**Narrate as you go:** append one line to `log.md` at every investigation
step — what you're about to check, then what you found:

```sh
beagle log <slug> "p99 dashboard normal; checking Redis pool saturation"
```

This is the user's live view of what you're doing (the Log tab; they often
sit on it in follow mode). A silent minute looks like a stuck agent — log
before long queries. Then the sections:

1. `summary.md` — what broke in ≤3 sentences + current state. **Write first,
   keep it current**; it's the tab responders read.
2. `timeline.md` — `- **HH:MM UTC** — event` bullets from telemetry. Include
   notable *non*-events (what was healthy) — they kill wrong hypotheses.
3. `root-cause.md` — numbered causal chain, symptom → root. Add "why it
   wasn't caught" and contributing factors.
4. `impact.md` — quantify: requests, users, minutes, money, SLO budget burned.
5. `remediation.md` — the Fix tab: immediate mitigation first (with
   timestamps), then durable fixes with owners and status, then how to verify.
6. `notes.md` — raw evidence: metric tables (in code fences so columns align),
   exact queries, log excerpts, links, open questions.
7. `diagrams/NN-name.txt` — see below.

## 3. Keep the manifest honest

Edit `rca.toml` as the investigation progresses:

- `status`: `investigating` → `identified` → `monitoring` → `resolved`.
- `updated`: bump to now (RFC 3339, **quoted string**, e.g. `"2026-07-05T14:32:00Z"`).
- `tags`: **always set these** — 3–6 kebab-case topics (e.g. `webhooks`,
  `config`, `data-loss`, `redis`). They matter downstream: `beagle export`
  emits them as YAML frontmatter `tags`, which Obsidian and similar tools
  index directly. Tag the failure class and the technologies involved, not
  the incident specifics.
- Unknown fields are rejected by the TUI — don't invent new ones.

## Exporting (deterministic, no LLM involved)

The user can press `e` in the TUI, or anyone can run
`beagle export <slug> [--out <file>]`, to produce **one** markdown document:
frontmatter (title/severity/status/dates/systems/tags) + all sections +
diagrams in code fences with ANSI colors stripped. Same files in → same
document out. Default target is `<root>/exports/<slug>.md`. You normally
don't need to do anything here — just keep the manifest and sections good,
and the export takes care of itself.

## 4. Diagrams

- Plain ASCII/Unicode box-drawing `.txt` files in `diagrams/`, rendered
  **unwrapped** (user pans with h/l) — alignment is preserved, so keep lines
  ≤100 columns.
- Number for ordering: `01-request-path.txt`, `02-fix.txt`.
- Mark the failure point (`◀── BUG`) and put real numbers on components
  (pool sizes, latencies, rates).
- A before/after pair is the strongest way to explain a fix.

**Color and bold:** diagrams support ANSI SGR escape codes, which are
zero-width at render so alignment is unaffected. Author them with `printf`
or `perl` (the Write tool can't emit a literal ESC byte):

```sh
perl -pi -e 's/BUG/\e[1;31mBUG\e[0m/' diagrams/01-request-path.txt
```

Supported: `\e[1m` bold, `\e[2m` dim, `\e[31m`…`37m` / `\e[91m`…`97m` colors
(31 red, 32 green, 33 yellow, 34 blue, 36 cyan), `\e[0m` reset. Convention:
**red = broken, green = healthy, yellow = degraded/warning**. Color the
handful of load-bearing tokens, not whole boxes. Style carries across lines
until reset — always close with `\e[0m`.

## Rendering constraints

The TUI renders a markdown subset: `#`/`##`/`###`, `-` bullets (indent to
nest), ``` fences, `>` quotes, `---`, `**bold**`, `` `code` ``. Tables, links,
and images render as plain text — put tabular data in code fences.

A fully worked example (tone, depth, diagram style):
`rcas/2026-06-30-sendgrid-webhook-signature-failures/`.
