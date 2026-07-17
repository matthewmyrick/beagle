# Contributing to beagle

`beagle` is a terminal UI for explaining broken systems. Claude (or a human)
debugs a system, writes its findings into an **RCA workspace** on disk, and this
TUI renders those workspaces — notes, timelines, and diagrams — so a person can
understand *what broke, why it broke, and how to fix it*.

Because this tool is used **during incidents**, the bar for reliability is
higher than for a typical side project. A crash or a hung redraw while someone
is debugging production is unacceptable. Read this document and
[`docs/CODING_STANDARDS.md`](docs/CODING_STANDARDS.md) before opening a PR.

## Ground rules

1. **The TUI must never panic on user data.** RCA files are written by external
   tools (including LLMs). Malformed TOML, missing sections, invalid UTF-8,
   10 MB markdown files — all of it must degrade gracefully, never crash.
2. **The data format is the API.** The on-disk layout under `rcas/` (see
   [`README.md`](README.md)) is consumed by Claude and by humans with a text
   editor. Changes to it are breaking changes: they require a documented
   migration note in the PR description and an update to `CLAUDE.md`.
3. **No `unsafe`.** The crate forbids it (`#![forbid(unsafe_code)]` via
   `[lints]`). If you believe you need it, open an issue first with benchmarks.
4. **Warnings are errors in CI.** `cargo clippy --all-targets -- -D warnings`
   must pass, including the pedantic set enabled in `Cargo.toml`.

## Which component?

The repo is a monorepo. Each component has its own standards and checks:

| Directory | What | Standards | Checks |
|---|---|---|---|
| `cli/` | The Rust TUI + CLI | `docs/CODING_STANDARDS.md` | `cargo fmt` / pedantic `clippy -D warnings` / `cargo test` |
| `desktop/` | Tauri 2 + React desktop app | `docs/CODING_STANDARDS_TS.md` (frontend) + the Rust rules (`src-tauri/`) | `npm run check` + cargo fmt/clippy in `src-tauri/` |
| `web/` | Astro static site — public postmortems | `docs/CODING_STANDARDS_TS.md` | `npm run check` (prettier / astro check / vitest) |

The on-disk RCA format under `rcas/` is the **shared public API** — every
component reads the same files. Changing it needs a migration note and an
update to `CLAUDE.md`, regardless of which component you touch.

## Development workflow

```sh
# one-time setup
rustup default stable
rustup component add rustfmt clippy

# the loop — the Rust crate lives under cli/
cd cli
cargo fmt                                  # format
cargo clippy --all-targets -- -D warnings  # lint (pedantic, zero warnings)
cargo test                                 # unit + integration tests
cargo run                                  # open the TUI against ./rcas
```

Before pushing, run all three of `fmt`, `clippy`, `test`. CI runs exactly these
(see `.github/workflows/ci.yml`) and will reject anything that fails locally.

## Branches, commits, PRs

- Branch from `main`: `feat/<slug>`, `fix/<slug>`, `docs/<slug>`, `perf/<slug>`.
- Commits follow [Conventional Commits](https://www.conventionalcommits.org):
  `feat(ui): add horizontal scroll to diagrams tab`,
  `fix(store): skip RCA dirs with unreadable manifests instead of failing list`.
- Keep PRs small and single-purpose. A PR that touches the data format, the
  store, *and* the renderer is three PRs.
- Every PR description answers: **what** changed, **why**, and **how it was
  tested** (paste the test names or a terminal recording for UI changes).

## What needs tests

| Change | Required tests |
|---|---|
| Data model (`model.rs`) | Serde round-trips, invalid-input rejection |
| Store (`store.rs`) | Filesystem round-trips against a temp dir, missing/corrupt file handling |
| Markdown/diagram rendering | Snapshot-style assertions on rendered `Text` |
| Key handling / app state | State-transition unit tests (no terminal required) |
| Pure UI layout | Manual verification is acceptable; describe it in the PR |

Tests must not touch the real filesystem outside `tempfile` directories, must
not require a TTY, and must not sleep.

## Performance expectations

This tool runs *next to* whatever is being debugged, possibly on a starved box.
Budgets (see the standards doc for how to measure):

- Cold start to first frame: **< 50 ms** with 100 RCAs on disk.
- Redraw on keypress: **< 5 ms** typical.
- Idle CPU: **~0%** — we block on input with a timeout; no busy loops.
- Memory: proportional to the *open* RCA, not the whole corpus. Section content
  is loaded lazily and evicted on workspace switch.

If your change adds a dependency, justify it in the PR: what it does, why we
can't reasonably do it ourselves, its transitive dependency count, and its
effect on compile time and binary size. The dependency budget is deliberately
tight — this is a tool people install with `cargo install`.

## Releasing (maintainers)

Releases are cut from `main` by pushing a version tag; CI does the rest
(`.github/workflows/release.yml` re-runs the full gate, cross-builds macOS
arm64/x86_64 and Linux x86_64-musl binaries, and publishes them with sha256
checksums to a GitHub Release).

1. Bump `version` in `Cargo.toml` and move the `[Unreleased]` notes in
   `CHANGELOG.md` under the new version heading.
2. Commit: `chore(release): vX.Y.Z`.
3. Tag and push: `git tag vX.Y.Z && git push origin main vX.Y.Z`.

The workflow refuses to publish if the tag doesn't match `Cargo.toml`'s
version, so a mismatched bump fails loudly instead of shipping a lie.

## Reporting bugs

Include: OS + terminal emulator, `beagle --version`, the RCA directory that
triggers the bug (or a minimized copy), and what you expected vs. saw. If the
TUI panicked, the panic message and backtrace (`RUST_BACKTRACE=1`).
