# beagle

An **AI-first debugging TUI**. Claude debugs a system from its telemetry, then
writes what it found into an **RCA workspace** — plain files on disk. This TUI
renders those workspaces as a tabbed incident view so a human can immediately
see *what broke, why it broke, and how to fix it*.

Think `hunk` for git diffs, but for incidents: the AI does the investigation,
the TUI is how it explains itself.

```text
┌ Incidents (3) ──────────┐┌ Payments API p99 latency 40x regression ─────────┐
│ HIGH  Payments API p99… ││ ● review · HIGH · payments-api, redis-sess…      │
│  ◐ review               ││                                                  │
│ CRIT  Ledger export st… ││ 1 Summary·2 Timeline·3 Root Cause·4 Impact·5 Fix │
│  ● investigating        ││ ·6 Diagrams·7 Notes                              │
│ LOW   Cron drift on ba… ││ ┌ Root Cause ───────────────────────────────────┐│
│  ✔ finished             ││ │ Causal chain, symptom → root                  ││
│                         ││ │  1. checkout requests time out (ELB 504)      ││
│                         ││ │  2. handlers block waiting for a free Redis…  ││
└─────────────────────────┘└──────────────────────────────────────────────────┘
  j/k select · enter open · tab/1-7 tabs · r reload · ? help · q quit
```

## How it works

1. Claude (or you) investigates an incident and scaffolds a workspace:
   `beagle new 2026-06-30-sendgrid-webhooks --title "..." --severity high`
2. The investigation is written into that workspace as markdown sections and
   ASCII diagrams — see the data format below and [`CLAUDE.md`](CLAUDE.md) for
   the authoring guide Claude follows.
3. You keep `beagle` open in a terminal. It **watches the filesystem** and
   re-renders live as the investigation is written — no refresh needed.

Every RCA gets nine tabs: **Summary · Timeline · Root Cause · Impact · Fix ·
Final Review · Diagrams · Notes · Log**. The Log tab is the live investigation stream — the
agent appends a timestamped line at every step (`beagle log <slug> "..."`),
and `f` (follow mode) keeps the tab pinned to the newest line, tail-f style.
Tabs whose files changed since you last looked get a `●` marker, workspaces
in `investigating` status show a live spinner, ticking elapsed time, and a
liveness read — `active 2m ago`, turning yellow (`quiet 12m`) when the agent
has gone silent — and
new workspaces announce themselves in the status bar as agents scaffold them.

## Install & run

From a release binary (macOS arm64/x86_64, Linux x86_64 — static musl):

```sh
curl -fsSL https://github.com/matthewmyrick/beagle/releases/latest/download/beagle-aarch64-apple-darwin.tar.gz \
  | tar xz && mv beagle-*/beagle /usr/local/bin/
```

Or with cargo:

```sh
cargo install --git https://github.com/matthewmyrick/beagle   # from GitHub
cargo install --path .                                           # from a checkout
```

Then:

```sh
beagle                    # open the TUI against ./rcas
beagle --root ~/oncall    # or point it anywhere
beagle list               # print workspaces to stdout
beagle list --status investigating --severity high       # filtered
beagle new <slug> --title "..." [--severity high] [--system payments-api]...
beagle status <slug> investigating   # flip status; a running TUI updates live
beagle log <slug> "checking redis pool"  # append to the live Log tab
beagle similar <slug>     # past RCAs sharing systems/tags, ranked (R in the TUI)
beagle pr add <slug> https://github.com/org/repo/pull/123  # attach a fix PR
beagle pr list <slug>     # attached PRs, with live state when gh is available
beagle export <slug>      # one markdown file → exports/<slug>.md
beagle export <slug> --out ~/vault/incidents/<slug>.md   # e.g. an Obsidian vault
beagle banner             # print the BEAGLE banner
beagle init               # scaffold toolbox.md + systems/ agent context
beagle config             # edit + validate ~/.config/beagle/config.toml
beagle version            # print the installed version
beagle version list       # browse releases; enter installs the selection
beagle update             # self-update to the latest release
beagle update --version 0.1.0    # or move to any release, up or down
```

## Give agents context: the toolbox

An investigating agent works much faster when it knows what telemetry exists
before it starts. `beagle init` scaffolds two things at the store root:

- `toolbox.md` — the tools available here: Grafana dashboards, Loki/Sentry,
  CLIs the agent may shell out to, runbooks, escalation paths.
- `systems/<name>.md` — one file per service (names match `systems` in
  `rca.toml`): its dashboards, log labels, dependencies, known failure modes.

The `/beagle` Claude Code skill reads these before every investigation and
updates them when it learns something durable. Press `T` in the TUI to see
the toolbox plus the systems docs for the selected incident — you and the
agent share the same source of truth.

## Config & updates

`beagle config` opens `~/.config/beagle/config.toml` in your editor (config
`editor`, then `$VISUAL`/`$EDITOR`, then vim) and **validates it when the
editor closes** — typos and unknown keys are reported immediately. Every
setting is optional and overridden by flags:

```toml
root = "/path/to/oncall"   # default --root, so `beagle` works from anywhere
notify = true              # desktop pings: new incidents, status changes
editor = "code -w"         # editor for `beagle config`
```

`beagle update` downloads the release binary for your platform, **verifies
its sha256** against the published checksum, and atomically swaps the
installed binary — never a half-written executable. `--version <ver>` moves
to any released version, older or newer, so a bad release is one command to
back out of. `beagle version list` shows every release (latest and current
marked); pick one with `j`/`k` + enter to install it. On platforms without
prebuilt binaries, update via `cargo install` instead.

Keys: `j/k` navigate · `enter` open · `b` back to the list · `←/→` / `tab` /
`1`–`9` switch tabs · `/` search the incident (all tabs, `n`/`N` between
hits) · `f` filter the list (i/r/v/f status · c/h/m/l severity · `/` free text,
stacking + toggling) · `F` follow (tail -f) · `T` toolbox ·
`o` open links/PRs · `R` related incidents · `V` sign off final-review · `S` settings ·
`c` copy tab / `C` copy whole RCA (pbcopy or OSC 52) · `e` export to
`exports/<slug>.md` · `n/p` cycle diagrams · `h/l` pan diagrams · `r`
reload · `?` help · `Q` / `ctrl-c` quit.

## Track the fix: attached PRs

Remediation lands as pull requests, and a merged PR isn't a verified fix.
The lifecycle follows the fix all the way:

```text
investigating ──▶ review ──▶ final-review ──▶ finished
   (digging)   (fix PR open)  (PR merged —      (verified,
                               verify it!)       signed off)
```

`beagle pr add <slug> <url>` attaches a PR to the manifest; the workspace
header shows `fixes: ○ #123 open · ✓ #124 merged`, refreshed by a background
`gh` poll every 30 minutes (plus whenever the set changes). **When every
attached PR has merged, beagle automatically moves the RCA from `review` to
`final-review`** — time to work the Final Review tab, the checklist of
checkable predictions the agent wrote *during* the investigation ("p99 back
under 200ms for 24h"). Confirmed it held? Press **`V`** to sign off →
`finished`. Viewing never changes state; only `V` (or `beagle status <slug>
finished`) does.

No `gh` installed? PR links still show — just without live state or the
auto-transition. Press `o` to open any attached PR or URL in your browser.

## Export

`e` (or `beagle export`) renders a workspace to a **single markdown file**:
YAML frontmatter (title, severity, status, dates, systems, tags) followed by
every section and the diagrams in code fences (ANSI colors stripped). It is
**deterministic** — same files in, same document out, no LLM involved — so
you can diff it, script it, and sync it anywhere. The frontmatter `tags` come
straight from `rca.toml`, so tools like Obsidian index the export natively.

## Data format (the API)

A workspace is a directory — no database, no lock-in, `git`-friendly:

```text
rcas/
  2026-06-30-sendgrid-webhook-signature-failures/   # id: lowercase slug [a-z0-9-]
    rca.toml          # title, severity, status, created, systems, tags
    summary.md        # what broke, in three sentences
    timeline.md       # what happened when
    root-cause.md     # why it broke, symptom → root
    impact.md         # who/what was affected, quantified
    remediation.md    # the Fix tab: mitigation + durable fixes
    final-review.md   # verification checklist, worked after the fix merges
    notes.md          # raw evidence, queries, links
    diagrams/
      01-topology.txt # ASCII diagrams, rendered unwrapped; ANSI colors supported
```

`rca.toml`:

```toml
title = "Payments API p99 latency 40x regression"
severity = "high"          # critical | high | medium | low | info
status = "review"          # investigating | review | final-review | finished
created = "2026-07-05T14:32:00Z"   # RFC 3339, quoted
systems = ["payments-api", "redis-sessions"]
tags = ["latency"]
prs = ["https://github.com/org/repo/pull/123"]   # optional; `beagle pr add`
```

Any section may be absent (the tab shows a hint instead); a corrupt manifest
skips that workspace with a status-bar warning and never crashes the TUI. The
repo ships one fully worked example under `rcas/` — run `cargo run` here to
explore it.

## Design goals

- **Airtight:** no `unsafe`, no panics on user data, pedantic clippy at
  `-D warnings`, terminal always restored — even on panic.
- **Memory-efficient:** manifests only at startup; section content loads
  lazily per tab and is evicted on switch; markdown renders once per change,
  not per frame.
- **I/O-efficient:** event-driven redraws via filesystem notifications
  (coalesced), blocking input loop (~0% idle CPU), bounded reads (4 MB cap),
  atomic scaffold writes.
- **Type-safe:** validated newtypes (`RcaId`), closed enums for
  severity/status/tabs, `deny_unknown_fields` manifests, one `thiserror`
  error type with path context.

See [`CONTRIBUTING.md`](CONTRIBUTING.md) and
[`docs/CODING_STANDARDS.md`](docs/CODING_STANDARDS.md).

## License & maintainer

MIT — see [`LICENSE`](LICENSE). Maintained by
[Matthew Myrick](https://github.com/matthewmyrick); issues and PRs welcome
(read the contributing guide first).
