# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.8.0] - 2026-07-17

### Added

- `beagle unarchive <slug>` moves an archived RCA back to the active
  list — the previously missing inverse of `archive` (also available in
  the desktop app's header).
- Git-style root discovery: without `--root` or a config `root`, beagle
  now walks up from the working directory to the nearest ancestor
  containing `rcas/` — run it from any subdirectory of an oncall
  checkout and it finds the right workspaces instead of scaffolding an
  empty `rcas/` where you stand.

### Changed

- The `\` finder opens quiet: an empty query matches nothing, and the
  popup is a slim bar that grows a row per match (capped) as you type,
  shrinking as the query narrows — the incident stays visible behind it.
- The CLI self-updater explicitly skips the repo's new `desktop-v*`
  release tags (the desktop app releases independently); `beagle
  update` continues to track `v*` only.

## [0.7.1] - 2026-07-17

### Changed

- Repository layout: the Rust crate moved wholesale into `cli/` (history
  preserved), the first step toward the cli/web/desktop monorepo (#41).
  CI and the release workflow follow it. **No behavior changes**, and
  release assets keep the exact same names and layout, so `beagle
  update` keeps working across the move. From a checkout:
  `cargo install --path cli`, or
  `cargo run --manifest-path cli/Cargo.toml`.

## [0.7.0] - 2026-07-17

### Added

- Archive flow: `beagle archive <slug>` moves a finished RCA to
  `rcas/archive/<slug>` — out of the sidebar, never out of the knowledge
  base (refuses unless status is `finished`; `--force` overrides). The
  TUI hides archived incidents by default; `a` toggles them in, rendered
  dimmed and sorted below everything active. `beagle list --archived`
  includes them, and reads, exports, `beagle log`, and similar-ranking
  (`R` / `beagle similar`) work on archived workspaces transparently.
  The `archive` slug is reserved at scaffold time (#19).
- Mouse support: the wheel scrolls whatever is under the cursor (content
  pane, sidebar selection, or an open overlay); left-click selects a
  sidebar row, switches to a clicked tab label, focuses the content
  pane, or closes the help sheet. Capture is released on every exit
  path, panics included; keys remain the primary interface (#20).
- `E` opens the current tab's backing file in your editor (config
  `editor` → `$VISUAL` → `$EDITOR` → vim — the same resolution as
  `beagle config`), suspending and restoring the TUI around it. On the
  Diagrams tab it opens the diagram on screen. Your own edit is not
  flagged unread for you; editor failures land in the status bar (#21).
- Global fuzzy finder: `\` opens a telescope-style popup over every
  line of every incident (archived included) plus incident titles. A
  few fuzzy characters re-rank results live with matched characters
  highlighted; enter jumps straight to the incident, tab, and line —
  revealing filtered or archived targets as needed. Distinct from `/`,
  which stays the precise in-incident search (#25).

## [0.6.1] - 2026-07-17

### Fixed

- Broken-workspace rows now show a compact, path-free reason. A missing
  manifest reads `no rca.toml — not a beagle workspace` and a corrupt one
  leads with the TOML error itself — previously both led with the
  manifest's absolute path, which is exactly what a narrow sidebar
  truncates away (`i/o error at /Users/matthew…`).

## [0.6.0] - 2026-07-17

### Fixed

- Workspaces that fail to load (invalid status name, corrupt manifest, or
  a missing `rca.toml`) are no longer silently invisible: the sidebar now
  shows a red `⚠ broken` row with the directory name and the reason, and
  `beagle list` prints matching `⚠ broken` lines. The most common cause
  is version skew — an agent writing status names (`final-review`,
  `finished`) that an older installed binary does not know (#46).
- `beagle status <slug> <status>` can now repair a manifest whose *only*
  problem is an invalid status value — the bad value is exactly what the
  command overwrites anyway. Other corruption still errors as before.

### Added

- Checklist rendering: `- [ ]` / `- [x]` bullets render as ☐/☑ glyphs —
  checked items green, unchecked with a yellow box — and aggregate
  progress surfaces as `☑ 3/7` in the sidebar detail line and the
  workspace header, turning green when complete. Counts re-scan only
  when a file's mtime changes; fenced code is ignored.
- Collapsible sidebar: `s` collapses the incident list so the content
  pane takes the full terminal width (wide diagrams get every column);
  `s` again — or any back-to-list key (`b`, `esc`, `f`) — brings it back.
  The sidebar is never collapsed while the list has focus.
- Agent liveness: `investigating` headers now show how fresh the workspace
  is — `active 2m ago` from the newest section-file write (the mtime
  snapshot already kept for unread markers, zero extra I/O), turning
  yellow (`quiet 12m`) once the agent has been silent past 10 minutes.
- Settings overlay: `S` opens a floating pane showing every config field
  (root, editor, notify) with its current value — space toggles booleans,
  enter inline-edits strings, and every change writes the config file
  through a comment-preserving, validated, atomic line edit. `notify`
  applies to the running TUI immediately.
- Desktop notifications (opt-in, config `notify = true`): new incidents
  and status transitions fire a native notification — `osascript` on
  macOS, `notify-send` on Linux, same shell-out philosophy as the
  clipboard; a missing tool is a silent no-op. Every reload path notifies
  consistently (watcher, manual reload, auto-advance, sign-off).

- Filter facets: inside `f` filter mode, single keys toggle facets
  instantly — `i`/`r`/`v`/`f` by status, `c`/`h`/`m`/`l` by severity —
  stacking across dimensions (high AND investigating) and toggling off on
  a second press. `/` switches to free-text typing, which ranks within the
  facet set. Active facets show in the sidebar title
  (`Incidents (2/7) [high · investigating]`) and the filter prompt; esc
  peels typing → facets → clear; enter keeps the filter; opening an
  incident still consumes everything.

### Changed

- **Keybindings** (muscle-memory alert): `/` now *always* searches the
  selected incident's content, from either pane — committing a search (or
  pressing `n`/`N`) focuses the content on the hit. `f` now opens the
  incident-list filter (previously on `/` when the list was focused), and
  follow mode moves to `F`.

### Added

- Final-review lifecycle: a new **Final Review** tab (`final-review.md`,
  tabs are now 1–9) holds the verification checklist the agent writes
  *during* the investigation — concrete, checkable predictions of what
  "fixed" looks like. When **every attached fix PR has merged**, beagle
  automatically moves the workspace from `review` to `final-review` (fed by
  the existing `gh` poll); after working the checklist, `V` signs it off as
  verified → `finished`. Viewing never mutates state.

### Changed

- **Status vocabulary (manifest format):** `identified` → `review`,
  `monitoring` → `final-review`, `resolved` → `finished`. Old names still
  parse (manifests and `beagle status` both accept them) but beagle now
  writes the new names — **binaries older than this release reject the new
  names** and skip those workspaces with a warning; `beagle update` first.
  Sidebar order follows the lifecycle: `investigating` on top, then
  `review`, `final-review`, and `finished` at the very bottom, with
  severity ordering within each.

## [0.5.0] - 2026-07-13

### Added

- Related incidents: `R` opens a popup of past RCAs ranked by shared
  `systems` (weighted 3×) and `tags` (1×), newest first on ties — enter
  jumps to the workspace, clearing any filter that would hide it. `beagle
  similar <slug>` prints the same ranking for scripts, and the `/beagle`
  skill now checks history right after scaffolding and cites prior
  incidents in root-cause writeups. Ranking runs entirely over the
  manifests already in memory.

## [0.4.0] - 2026-07-13

### Added

- In-content search: `/` with the content pane focused searches **every
  section tab of the selected incident** (the list filter keeps `/` when
  the list is focused). Case-insensitive substring match; the matched text
  itself highlights live as you type (current hit amber, others tinted —
  the occurrence, not the whole line), enter commits, and `n`/`N` walk the
  hits in tab order —
  hopping tabs automatically when the next hit lives on another one —
  wrapping at the ends. Esc clears. The status bar shows
  `match 3/17 for "429" · Notes`.

### Fixed

- Selecting a sidebar row no longer wipes out the severity badge's colored
  background: selection styling is applied per-span (the badge keeps its
  own colors, the rest of the row gets the tint, padded to full width).

## [0.3.0] - 2026-07-10

The agent-investigation release: context files agents read before digging
in, a live Log tab to watch them think, and PR tracking so an RCA follows
its fix to the merge. Also includes the module-split refactor (#12) — no
behavior change, but every file now respects the 400-line cap.

### Fixed

- Tab-switching keys (tab, arrows, 1-8) pressed with no incidents on
  screen now explain themselves in the status bar instead of silently
  doing nothing (the welcome screen has no tab bar, so the silence read
  as a broken keybinding).

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

[Unreleased]: https://github.com/matthewmyrick/beagle/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/matthewmyrick/beagle/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/matthewmyrick/beagle/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/matthewmyrick/beagle/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/matthewmyrick/beagle/releases/tag/v0.2.0
[0.1.0]: https://github.com/matthewmyrick/beagle/commit/fceb9d4
