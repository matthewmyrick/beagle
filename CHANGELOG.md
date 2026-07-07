# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Pressing `T` with no toolbox scaffolds it on the spot (same as `beagle
  init`) and shows the fresh templates — no round-trip to the CLI.
- Investigation context for agents: `toolbox.md` (available telemetry, CLIs,
  runbooks) and `systems/<name>.md` (per-system dashboards, dependencies,
  known failure modes) at the store root. `beagle init` scaffolds templates;
  `T` in the TUI shows the toolbox plus the systems docs matching the
  selected incident.

- Live investigation experience: a new **Log** tab (`log.md`, tabs are now
  1–8) streams the agent's narration, appended via `beagle log <slug>
  "message"`; `f` toggles follow mode (reloads stick to the bottom, tail -f
  style); tabs whose files changed since last viewed show a `●` marker (and
  the sidebar entry a dot); `investigating` headers tick elapsed time; and
  newly scaffolded workspaces are announced in the status bar.

- Attached PRs: `beagle pr add <slug> <url>` records remediation PRs in a
  new optional `prs` manifest field; the workspace header shows live merge
  status (`fixes: ○ #123 open · ✓ #124 merged`) via a background `gh` poll
  every 30 minutes. Without `gh`, links render plain — no polling, no
  errors. `o` opens a popup of attached PRs plus URLs on the current tab
  and launches the selection in the browser.

### Changed

- **Manifest format:** new optional `prs` field (list of PR URLs). Omitted
  while empty, so existing manifests are untouched — but note that beagle
  binaries older than this release reject manifests that *do* contain
  `prs` (unknown field) and will skip those workspaces with a warning.
  Update via `beagle update` before using `pr add` if you run multiple
  machines.
- The Claude Code skill is now `/beagle` (was `/rca`) and instructs agents
  to read the toolbox/systems context before investigating, keep it updated,
  and narrate every investigation step to `log.md`.

## [0.2.0] - 2026-07-07

The first published release — v0.1.0 was tagged in `Cargo.toml` only and
never had binaries built, so this is the earliest version `beagle update`
can install.

### Added

- Animated spinner for `investigating` workspaces in the sidebar and header,
  driven by the existing 250 ms event-loop wakeup (no extra redraws).
- BEAGLE ASCII banner at the top right of the TUI, beside the workspace
  header so it doesn't push content down (hidden automatically on small
  terminals), and a `beagle banner` command that prints the same art.
- `beagle status <id> <status>` to set a workspace's status from the CLI;
  a running TUI picks the change up live via the filesystem watcher.
- `beagle list --status <status> --severity <sev>` filters.
- `beagle config`: opens `~/.config/beagle/config.toml` in your editor
  (config `editor` → `$VISUAL` → `$EDITOR` → vim) and validates it on close,
  reporting unknown keys and type errors. Config `root` becomes the default
  `--root`, so `beagle` can run from anywhere.
- `beagle update`: self-update to the latest GitHub release —
  sha256-verified download, atomic binary swap. `--version <ver>` installs
  any released version, upgrade or downgrade.
- `beagle version` (version + target triple) and `beagle version list`, an
  interactive release browser where enter installs the selected version
  (plain listing when piped).

## [0.1.0] - 2026-07-07

Initial release.

### Added

- Tabbed RCA workspace TUI: Summary · Timeline · Root Cause · Impact · Fix ·
  Diagrams · Notes, with live filesystem reload while an investigation is
  being written.
- On-disk workspace format (`rcas/<slug>/`): TOML manifest
  (title/severity/status/dates/systems/tags), one markdown file per section,
  ASCII diagrams with ANSI color support (rendered unwrapped, pannable).
- Markdown renderer for the authoring subset: headings, bullets, code fences
  (gutter style, fence markers hidden), blockquotes, rules, bold, inline code.
- Fuzzy incident filter (`/`) over title, slug, systems, and tags.
- Clipboard: `c` copies the current tab, `C` the whole RCA
  (pbcopy/wl-copy/xclip/xsel with OSC 52 fallback).
- Deterministic single-file markdown export with Obsidian-compatible YAML
  frontmatter: `e` in the TUI or `beagle export <slug> [--out <file>]`.
- CLI: `beagle` (TUI), `beagle new`, `beagle list`, `beagle export`.
- Safety: no `unsafe`, no panics on user data, 4 MB read cap, atomic writes,
  corrupt manifests degrade to status-bar warnings, terminal restored on
  every exit path.
- `/rca` Claude Code skill documenting the authoring workflow.

[Unreleased]: https://github.com/matthewmyrick/beagle/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/matthewmyrick/beagle/releases/tag/v0.2.0
[0.1.0]: https://github.com/matthewmyrick/beagle/commit/fceb9d4
