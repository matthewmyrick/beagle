# Changelog — Beagle desktop

The desktop app versions and releases independently of the CLI, on
`desktop-v*` tags. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.2.0] - 2026-07-20

### Added

- Diagrams render their ANSI SGR colors as styled HTML — red/green/
  yellow annotations (`◀── BUG`) now show in color instead of being
  stripped, with exact character alignment preserved (#75).
- Rebased on cli 0.18.0: the bundled crate brings the `agent` lifecycle
  status, `skip-final-review` tag handling, `.beagle` project-file
  discovery, and the publish flags along for the ride.

## [0.1.1] - 2026-07-17

### Fixed

- The app is now **Beagle** everywhere — the dock, the menu bar, and
  bundled builds. In `tauri dev` macOS labels the app by the executable
  name, which was the crate's `beagle-desktop`; the binary is now named
  `Beagle`.
- macOS-native **rounded-rectangle app icon** built from the beagle
  logo (an 824px rounded tile on a transparent 1024² canvas, per Apple's
  icon grid), replacing the full-bleed square that rendered flat among
  rounded neighbors.

## [0.1.0] - 2026-07-17

First release of the Beagle desktop app — a Tauri 2 + React viewer over
the same `rcas/` on-disk format the TUI reads, with its Rust backend
built on the `cli/` crate's domain layer.

### Added

- Sidebar of incidents (active + archived, severity/status, fuzzy
  filter) and the full nine-tab incident view: Summary, Timeline, Root
  Cause, Impact, Fix, Final Review, Diagrams, Notes, Log.
- Markdown rendering of the TUI's subset — headings, bullets, ☐/☑
  checklists with progress, code fences, quotes, bold, inline code —
  with hard-wrapped prose re-flowed into paragraphs.
- Diagrams tab with prev/next cycling, rendered unwrapped.
- Light and dark themes (persisted, OS-preference fallback, no
  first-paint flash) and the BEAGLE banner in the brand corner.
- TUI-style keybindings: `?` help, `j`/`k` and tab navigation, `/`·`f`
  fuzzy filter, `a` archived toggle, `t` theme, `\` global finder
  (telescope-style, matched-character highlighting).
- Archive / unarchive from the header; attach PRs with live `gh` merge
  status as clickable chips.
