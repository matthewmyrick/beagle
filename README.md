# beagle

An **AI-first debugging TUI**. Claude debugs a system from its telemetry, then
writes what it found into an **RCA workspace** — plain files on disk. This TUI
renders those workspaces as a tabbed incident view so a human can immediately
see *what broke, why it broke, and how to fix it*.

Think `hunk` for git diffs, but for incidents: the AI does the investigation,
the TUI is how it explains itself.

```text
┌ Incidents (3) ──────────┐┌ Payments API p99 latency 40x regression ─────────┐
│ HIGH  Payments API p99… ││ ● identified · HIGH · payments-api, redis-sess…  │
│  ◐ identified           ││                                                  │
│ CRIT  Ledger export st… ││ 1 Summary·2 Timeline·3 Root Cause·4 Impact·5 Fix │
│  ● investigating        ││ ·6 Diagrams·7 Notes                              │
│ LOW   Cron drift on ba… ││ ┌ Root Cause ───────────────────────────────────┐│
│  ✔ resolved             ││ │ Causal chain, symptom → root                  ││
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

Every RCA gets seven tabs: **Summary · Timeline · Root Cause · Impact · Fix ·
Diagrams · Notes**.

## Install & run

From a release binary (macOS arm64/x86_64, Linux x86_64 — static musl):

```sh
curl -fsSL https://github.com/matthewmyrick/telemetry/releases/latest/download/beagle-aarch64-apple-darwin.tar.gz \
  | tar xz && mv beagle-*/beagle /usr/local/bin/
```

Or with cargo:

```sh
cargo install --git https://github.com/matthewmyrick/telemetry   # from GitHub
cargo install --path .                                           # from a checkout
```

Then:

```sh
beagle                    # open the TUI against ./rcas
beagle --root ~/oncall    # or point it anywhere
beagle list               # print workspaces to stdout
beagle new <slug> --title "..." [--severity high] [--system payments-api]...
beagle export <slug>      # one markdown file → exports/<slug>.md
beagle export <slug> --out ~/vault/incidents/<slug>.md   # e.g. an Obsidian vault
```

Keys: `j/k` navigate · `enter` open · `b` back to the list · `←/→` / `tab` /
`1`–`7` switch tabs · `/` fuzzy-filter incidents · `c` copy tab / `C` copy
whole RCA (pbcopy or OSC 52) · `e` export to `exports/<slug>.md` · `n/p` cycle
diagrams · `h/l` pan diagrams · `r` reload · `?` help · `Q` / `ctrl-c` quit.

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
    notes.md          # raw evidence, queries, links
    diagrams/
      01-topology.txt # ASCII diagrams, rendered unwrapped; ANSI colors supported
```

`rca.toml`:

```toml
title = "Payments API p99 latency 40x regression"
severity = "high"          # critical | high | medium | low | info
status = "identified"      # investigating | identified | monitoring | resolved
created = "2026-07-05T14:32:00Z"   # RFC 3339, quoted
systems = ["payments-api", "redis-sessions"]
tags = ["latency"]
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
