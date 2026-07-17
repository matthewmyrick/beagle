---
name: beagle
description: Create and maintain RCA (root-cause analysis) workspaces rendered live by the Beagle apps (terminal TUI and desktop). Use when debugging a system or incident, or when asked to write up what broke, why it happened, and how to fix it.
---

# RCA workspaces

The user keeps a Beagle app open — the terminal TUI (`beagle`) or the
desktop app — while you work. Both watch `rcas/` and re-render **live**
as files change: the workspace you write *is* how you explain the
incident to them. Write as you investigate, not at the end.

Everything you and the apps do goes through the same on-disk format
(`rcas/<slug>/` + `rca.toml` + markdown + diagrams). Beagle finds the
store git-style: run from any subdirectory of a checkout and it walks up
to the nearest `rcas/`; an explicit `--root` or the config file's `root`
wins over that.

## 0. Read the toolbox first

Before touching telemetry, read the investigation context at the store
root (next to `rcas/`), if present:

- `toolbox.md` — the tools available to you here: observability stack
  (Grafana dashboards, Loki/Prometheus, Sentry), CLIs you can shell out to,
  runbook locations. Start your investigation with what it lists.
- `systems/<name>.md` — per-system context (dashboards with URLs, log
  labels, dependencies, known failure modes). Read the files matching the
  systems you suspect; their names line up with `systems` in `rca.toml`.

The user sees the same context (`T` in the TUI). **If you learn something
durable during the investigation** — a dashboard moved, a new failure
mode, a dependency nobody wrote down — update the relevant `systems/`
file or `toolbox.md`; that knowledge compounds across incidents. If none
of these files exist yet, suggest `beagle init` to the user (don't run it
unprompted mid-incident).

## 0.5 Check history — the archive is the memory

Right after scaffolding, run:

```sh
beagle similar <slug>
```

It ranks past RCAs sharing this incident's systems and tags — **including
archived ones** (finished incidents moved to `rcas/archive/`; they leave
the sidebar, never the knowledge base). If something ranks, read its
root-cause and remediation before investigating — the same system usually
breaks the same way twice — and cite it in `root-cause.md` ("similar to
<slug>, where …"). The user sees the same ranking with `R`, and can
fuzzy-search every line of every incident (archived included) with `\`.

## 1. Scaffold

```sh
beagle new <slug> --title "One-line incident title" --severity <sev> \
  --system <name> --system <name>
# not installed? from a beagle checkout:
#   cargo run -q --manifest-path cli/Cargo.toml -- new ...
```

- Slug: lowercase `[a-z0-9-]`, ≤64 chars, convention `YYYY-MM-DD-short-description`.
  (`archive` is reserved.)
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
before long queries.

**Keep a plan as a checklist:** put your investigation plan in `notes.md`
(or `summary.md`) as `- [ ]` items and tick them (`- [x]`) as evidence
lands. Both apps render them as ☐/☑ (done items green) and show aggregate
progress (`☑ 3/7`) beside the incident — the user watches boxes tick
instead of wondering where you are.

**Write finder-friendly lines:** the `\` finder fuzzy-searches individual
lines across every incident. One self-contained fact per line ("redis
pool exhausted at 14:55, waiters pegged at 8/8") jumps well; facts spread
across vague lines don't.

Then the sections:

1. `summary.md` — what broke in ≤3 sentences + current state. **Write first,
   keep it current**; it's the tab responders read.
2. `timeline.md` — `- **HH:MM UTC** — event` bullets from telemetry. Include
   notable *non*-events (what was healthy) — they kill wrong hypotheses.
3. `root-cause.md` — numbered causal chain, symptom → root. Add "why it
   wasn't caught" and contributing factors.
4. `impact.md` — quantify: requests, users, minutes, money, SLO budget burned.
5. `remediation.md` — the Fix tab: immediate mitigation first (with
   timestamps), then durable fixes with owners and status, then how to verify.
6. `final-review.md` — the verification checklist, written **while the fix
   is still forming, not after**. One `- [ ]` checkbox per concrete,
   checkable prediction of what "fixed" looks like — metric thresholds,
   error counts, alert silence, each with a how-to-check (query, dashboard):
   `- [ ] p99 < 200ms for 24h (grafana: payments-overview)`. When every
   attached PR merges, beagle moves the RCA to `final-review` and the user
   works this list before signing off with `V`. Vague items ("looks
   healthy") are useless here — write predictions someone else could verify.
7. `notes.md` — raw evidence: metric tables (in code fences so columns align),
   exact queries, log excerpts, links, open questions.
8. `diagrams/NN-name.txt` — see below.

## 3. Keep the manifest honest

Edit `rca.toml` as the investigation progresses:

- `status`: `investigating` → `review` (fix PR open) → `final-review`
  (beagle flips this automatically when every attached PR merges) →
  `finished` (the user signs off with `V` after working your Final Review
  checklist). Old names (identified/monitoring/resolved) still parse.
  The easy way: `beagle status <slug> <status>` stamps `updated` for you.
- `updated`: bump to now (RFC 3339, **quoted string**, e.g. `"2026-07-05T14:32:00Z"`).
- `tags`: **always set these** — 3–6 kebab-case topics (e.g. `webhooks`,
  `config`, `data-loss`, `redis`). They matter downstream: `beagle export`
  emits them as YAML frontmatter `tags`, which Obsidian and similar tools
  index directly, and `similar` ranks on them. Tag the failure class and
  the technologies involved, not the incident specifics.
- Unknown fields are rejected — don't invent new ones.
- **When remediation lands as a PR**, attach it:
  `beagle pr add <slug> <url>`. Both apps then track its merge status live
  (via `gh`) — the TUI in the header, the desktop app as clickable chips —
  and the merge of every attached PR is what advances the lifecycle.
  Attach every fix PR you open or find.

## After sign-off: archiving

Finished RCAs can move out of the sidebar with `beagle archive <slug>`
(to `rcas/archive/`; `beagle unarchive <slug>` reverses it). Archived
incidents stay fully readable, exportable, searchable (`\`), and ranked
by `similar`. **Don't archive unprompted** — whether an incident is
"done being looked at" is the user's call; suggest it when a finished
write-up has clearly gone quiet.

## Exporting (deterministic, no LLM involved)

The user can press `e`, or anyone can run
`beagle export <slug> [--out <file>]`, to produce **one** markdown
document: frontmatter (title/severity/status/dates/systems/tags) + all
sections + diagrams in code fences with ANSI colors stripped. Same files
in → same document out. Default target is `<root>/exports/<slug>.md`. You
normally don't need to do anything here — keep the manifest and sections
good, and the export takes care of itself.

## 4. Diagrams

- Plain ASCII/Unicode box-drawing `.txt` files in `diagrams/`, rendered
  **unwrapped** in both apps (the TUI pans with h/l) — alignment is
  preserved, so keep lines ≤100 columns.
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
until reset — always close with `\e[0m`. (The desktop app currently strips
ANSI in diagrams; color is a TUI nicety, alignment is the contract.)

## Rendering constraints

Both apps render the same markdown subset: `#`/`##`/`###`, `-` bullets
(indent to nest), `- [ ]`/`- [x]` checklists (☐/☑ with aggregate
progress), ``` fences, `>` quotes, `---`, `**bold**`, `` `code` ``.
Tables, links, and images render as plain text — put tabular data in code
fences. The desktop app re-flows hard-wrapped prose into paragraphs, so
wrap freely; keep *facts* on their own lines (finder + diffs), not split
mid-thought across many.
