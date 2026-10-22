# Releasing beagle

How code gets from a branch to `beagle update` on someone's machine.

```text
 branch ──▶ PR ──▶ label patch|minor|major ──▶ CI green ──▶ merge ──▶ tag vX.Y.Z ──▶ GitHub Release
                   + bump cli/Cargo.toml       (check,                 (auto-release.yml)  (release.yml)
                   + CHANGELOG heading          validate)
```

Nobody pushes tags or cuts release PRs by hand. Merging a correctly labeled
PR is the release.

## What ships

| Artifact | Built from | Trigger | Workflow |
|---|---|---|---|
| `beagle` CLI/TUI binaries (macOS arm64/x86_64, Linux x86_64-musl) | `cli/` | labeled PR merged, or a `vX.Y.Z` tag | `auto-release.yml` → `release.yml` |
| Desktop app bundles | `desktop/` | `desktop-vX.Y.Z` tag (manual) | `release-desktop.yml` |
| Web image (GHCR) | `web/` | `web-vX.Y.Z` tag (manual) | `web-image.yml` |

This document covers the CLI, which is what `beagle update` installs. The
desktop and web artifacts still release by pushing their own tags.

## Releasing a change, step by step

1. **Branch and change code.** Normal PR workflow; CI (`ci.yml`, plus
   `desktop.yml` / `web.yml` when those paths change) runs on every push.
2. **Bump the version in the same PR.**
   - `cli/Cargo.toml` → `version = "X.Y.Z"`, then `cargo check` so
     `Cargo.lock` follows.
   - `CHANGELOG.md` → move the `[Unreleased]` notes under
     `## [X.Y.Z] - YYYY-MM-DD`.
3. **Add exactly one label:**
   - `patch`: bug fixes, no new behavior (`0.31.0 → 0.31.1`)
   - `minor`: new features, backwards compatible (`0.31.0 → 0.32.0`)
   - `major`: breaking changes to the CLI or the on-disk format (`0.31.0 → 1.0.0`)
4. **Wait for green**, including the `validate` check (below).
5. **Squash-merge.** `auto-release.yml` tags the merge commit `vX.Y.Z` and
   runs `release.yml`. That re-runs the gate (fmt, pedantic clippy, tests),
   checks the tag matches `cli/Cargo.toml`, cross-builds, and publishes the
   tarballs and `.sha256` files to a GitHub Release, about 2.5 minutes after
   the merge.
6. **Install:** `beagle update` (it verifies the checksum, then swaps the
   binary atomically). `beagle --version` confirms it.

No label means no release: the PR merges and nothing is published. Its
changes go out with the next labeled PR.

## What counts as a release (and what never does)

A release label is only valid on a PR that changes the **shipped binary**:

- **Releases:**
  - `cli/src/**` (excluding test modules)
  - `cli/assets/**`
  - the two skills compiled into the binary (`.claude/skills/beagle/SKILL.md`,
    `.claude/skills/beagle-review/SKILL.md`)
  - dependency changes in `Cargo.toml`, `cli/Cargo.toml`, or `Cargo.lock`
- **Never releases:**
  - docs (`README.md`, `docs/`, `CHANGELOG.md`, other Markdown)
  - CI (`.github/`)
  - tests
  - `desktop/`, `web/`, `crates/` (the agent runtime isn't in the CLI binary)
  - a version-only bump with nothing else

`.github/scripts/release-version.sh changes` holds this list. Edit it there
if the binary's inputs change, for example a new `include_str!`.

## The `validate` check

Runs on every PR (`auto-release.yml`) and fails when:

- more than one of `patch` / `minor` / `major` is set;
- a release label is set but no application code changed;
- the label doesn't match the bump (e.g. `minor` on `0.31.0` requires
  `0.32.0`, computed from the latest `v*` tag, not from main);
- the `## [X.Y.Z]` CHANGELOG heading is missing;
- there's no label but `cli/Cargo.toml`'s version changed.

`validate` isn't a required check in the `main-2` ruleset, so a red
`validate` doesn't block the merge. The tag job re-checks for application
changes and refuses to release without them, but it doesn't re-check the
version. Add `validate` to the ruleset's required checks to make it
blocking.

## Why it works this way

- **The bump goes in the PR.** main only accepts changes through PRs
  (ruleset `main-2`), so a workflow can't commit a version bump after the
  merge. The PR carries it, and the workflow only creates the tag.
- **`release.yml` is called, not triggered.** A tag pushed with the default
  `GITHUB_TOKEN` doesn't start other workflows, so `auto-release.yml` calls
  `release.yml` directly as a reusable workflow, passing the tag.
- **The tag must match `cli/Cargo.toml`.** `release.yml` refuses to publish
  otherwise, so the binary's `--version` always equals the release it came
  from.

## Fallback: manual release

If the automation is down, a maintainer can still release by hand. On main
with the bumped version merged:

```sh
git tag vX.Y.Z && git push origin vX.Y.Z
```

That runs `release.yml` exactly as before.

## Troubleshooting

- **`validate` says "changes no application code":** remove the label and
  the version bump. The PR is docs/CI/tests only and doesn't need a release.
- **Tag job fails with "already exists":** the PR didn't bump
  `cli/Cargo.toml`. Open a follow-up PR with the bump and the label.
- **Tag job fails pushing the tag (403):** allow workflows read and write
  access under Settings → Actions → General → Workflow permissions.
- **`release.yml` gate fails:** fix it on main, then either merge a new
  labeled PR or push the tag by hand.
- **`beagle update` says "already on X":** the release hasn't published
  yet. Check `gh run list --workflow release.yml`.
