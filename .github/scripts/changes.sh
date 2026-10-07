#!/usr/bin/env bash
# Tells a CI job what this run needs to check, as two step outputs:
#
#   rust=true       anything other than Markdown changed, or anything under
#                   packs/, so the Rust checks run. A PR that only touches
#                   docs skips them, and the job still reports as passed, so
#                   required checks never hang (#15 §4). Warning: this skip
#                   assumes no Rust check reads a `.md` file outside packs/;
#                   the Pack checker reads each Quad's feel-tests.md, which is
#                   why any change under packs/ runs the checks.
#   libraries=true  Cargo.lock or deny.toml changed: the PR adds or upgrades a
#                   library, or changes the policy, so known security
#                   advisories are checked (ADR-0014).
#
# Pushes to main always run the Rust checks. A pull request is checked out as
# GitHub's merge commit with fetch-depth 2, so HEAD^1 is the base branch and
# the difference between them is exactly what the PR changes. Renames count as
# a deletion plus an addition, so renaming code to `.md` still runs the checks.
#
# Readable checks for this script: crates/xtask/tests/ci_changes.rs.
set -euo pipefail

if [[ "${GITHUB_EVENT_NAME:-}" != "pull_request" ]]; then
  echo "Not a pull request, so every Rust check runs."
  echo "rust=true" >> "$GITHUB_OUTPUT"
  echo "libraries=false" >> "$GITHUB_OUTPUT"
  exit 0
fi

# -z gives each path as it is, ended by a NUL byte: without it, git quotes a
# path that isn't plain ASCII ("docs/\305\202\303\263d\305\272.md"), which
# then ends in a quote mark instead of ".md" and doesn't start with "packs/".
# The NULs become line breaks for grep. A path holding a line break splits in
# parts, which never skips a check: its last part ends in ".md" only if the
# path does, and its first starts with "packs/" only if the path does.
changed="$(git diff -z --name-only --no-renames HEAD^1 HEAD | tr '\0' '\n')"
echo "Files this pull request changes:"
sed 's/^/  /' <<< "$changed"

if grep -qvE '\.md$' <<< "$changed"; then
  echo "Something other than docs changed, so the Rust checks run."
  echo "rust=true" >> "$GITHUB_OUTPUT"
elif grep -qE '^packs/' <<< "$changed"; then
  echo "A Pack's Markdown changed (a Feel Test log), which the Pack checker reads, so the Rust checks run."
  echo "rust=true" >> "$GITHUB_OUTPUT"
else
  echo "Only docs changed, so the Rust checks are skipped and report as passed."
  echo "rust=false" >> "$GITHUB_OUTPUT"
fi

if grep -qxE 'Cargo\.lock|deny\.toml' <<< "$changed"; then
  echo "Cargo.lock or deny.toml changed, so known security advisories are checked."
  echo "libraries=true" >> "$GITHUB_OUTPUT"
else
  echo "libraries=false" >> "$GITHUB_OUTPUT"
fi
