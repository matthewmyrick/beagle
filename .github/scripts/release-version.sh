#!/usr/bin/env bash
# Release-version helper for auto-release.yml.
#
#   release-version.sh current   print cli/Cargo.toml's version
#   release-version.sh current   print cli/Cargo.toml's version
#   release-version.sh changes   list the PR's files that change the shipped
#                                binary (diff $BASE_SHA...$HEAD_SHA)
#   release-version.sh check     validate a PR's release label against its
#                                version bump and its changes (labels as a
#                                JSON array in $LABELS)
#
# Exactly one of `patch` / `minor` / `major` (or none) may be set. With a
# label, the PR must change the binary (see `changes`), cli/Cargo.toml must
# equal the latest v* tag bumped by the label, and CHANGELOG.md must carry
# that version's heading. Without one, the version must not move past the
# latest tag.
set -euo pipefail

current() {
  sed -n 's/^version = "\(.*\)"$/\1/p' cli/Cargo.toml | head -n 1
}

# Files whose change alters the released `beagle` binary: its source and
# assets (not tests), the two skills it embeds via include_str!, and the
# Cargo manifests/lockfile — those only when something beyond a bare
# `version = "..."` line moved, so the release bump itself doesn't count.
# Docs, CHANGELOG, CI, desktop/, web/, and crates/ never release the CLI.
changes() {
  local file
  git diff --name-only "$BASE_SHA...$HEAD_SHA" | while read -r file; do
    case $file in
      cli/src/*tests/*) ;; # test modules, at any depth
      cli/src/* | cli/assets/* | cli/build.rs) echo "$file" ;;
      .claude/skills/beagle/SKILL.md | .claude/skills/beagle-review/SKILL.md) echo "$file" ;;
      Cargo.toml | Cargo.lock | cli/Cargo.toml)
        if git diff "$BASE_SHA...$HEAD_SHA" -- "$file" |
          grep -E '^[+-]' | grep -vE '^(\+\+\+|---) ' | grep -qvE '^[+-]version = "'; then
          echo "$file"
        fi
        ;;
    esac
  done
}

bump() {
  local version=$1 part=$2 major minor patch
  IFS=. read -r major minor patch <<<"$version"
  case $part in
    major) echo "$((major + 1)).0.0" ;;
    minor) echo "$major.$((minor + 1)).0" ;;
    patch) echo "$major.$minor.$((patch + 1))" ;;
  esac
}

check() {
  local labels latest version expected
  labels=$(jq -r '.[] | select(. == "patch" or . == "minor" or . == "major")' <<<"${LABELS:-[]}")
  latest=$(git tag -l 'v[0-9]*.[0-9]*.[0-9]*' --sort=-v:refname | head -n 1)
  latest=${latest#v}
  version=$(current)

  case $(grep -c . <<<"$labels" || true) in
    0)
      if [ "$version" != "$latest" ]; then
        echo "::error::cli/Cargo.toml is $version but the latest release is $latest — add a patch, minor, or major label to release it"
        exit 1
      fi
      echo "no release label — merging will not release"
      ;;
    1)
      if [ -z "$(changes)" ]; then
        echo "::error::'$labels' label, but this PR changes no application code (docs, CI, tests, and version-only bumps don't release) — remove the label and the version bump"
        exit 1
      fi
      expected=$(bump "$latest" "$labels")
      if [ "$version" != "$expected" ]; then
        echo "::error::'$labels' label: cli/Cargo.toml must be $expected (latest release $latest), found $version"
        exit 1
      fi
      if ! grep -q "^## \[$expected\]" CHANGELOG.md; then
        echo "::error::CHANGELOG.md has no '## [$expected]' heading — move the [Unreleased] notes under it"
        exit 1
      fi
      echo "'$labels' label: merging releases v$expected"
      ;;
    *)
      echo "::error::use exactly one release label, found: $(tr '\n' ' ' <<<"$labels")"
      exit 1
      ;;
  esac
}

case ${1:-} in
  current) current ;;
  changes) changes ;;
  check) check ;;
  *)
    echo "usage: $0 current|changes|check" >&2
    exit 2
    ;;
esac
