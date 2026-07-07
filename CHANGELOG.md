# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

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

[Unreleased]: https://github.com/matthewmyrick/telemetry/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/matthewmyrick/telemetry/releases/tag/v0.1.0
