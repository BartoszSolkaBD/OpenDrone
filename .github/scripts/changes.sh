#!/usr/bin/env bash
# Tells a CI job what this run needs to check, as these step outputs:
#
#   rust=true       anything other than Markdown changed, or anything under
#                   packs/, so the Rust checks run. A PR that only touches
#                   docs skips them, and the job still reports as passed, so
#                   required checks never hang (#15 §4). Warning: this skip
#                   assumes no Rust check reads a `.md` file outside packs/;
#                   the Pack checker reads each Quad's feel-tests.md, which is
#                   why any change under packs/ runs the checks, and the
#                   readable checks read .github/bevy-upgrade-issue.md, the
#                   Bevy upgrade ticket's template (#81), which is why any
#                   change under .github/ does too.
#                   The Red Flag gate reads ADRs and deep dives, so it runs in
#                   its own workflow (review.yml) with no skip.
#   libraries=true  Cargo.lock or deny.toml changed: the PR adds or upgrades a
#                   library, or changes the policy, so known security
#                   advisories are checked (ADR-0014).
#   maps=true       Anything under assets-src/ (the Blender scripts, the
#                   material library, the pinned Blender version) or a Pack's
#                   maps/ changed, so the Maps are rebuilt with Blender on
#                   Linux and must match the committed `.glb` (#63).
#   flight=true     Something that can change a flight result changed, so
#                   the Scenarios run on macOS and Windows too and the
#                   agreement check compares the three OSes (#240). That is a
#                   change, other than to Markdown, to one of the flight
#                   paths, or any change under packs/. The flight paths are
#                   the `flight_paths` list just below this header, each with
#                   its reason: it is the one place to edit them.
#                   CONTRIBUTING.md and the readable checks point to it.
#   all_checks=true This job runs every Rust check: the Rust checks run, and
#                   it runs on Linux or for a push to main. A pull request
#                   runs every check on Linux only (#240).
#   scenarios=true  This job runs the Scenarios and keeps the fingerprints
#                   for the agreement check: it runs every check, or it runs
#                   on macOS or Windows for a pull request where flight=true.
#                   The job reads the OS from RUNNER_OS, which GitHub sets.
#
# Pushes to main always run every Rust check on every OS, the Scenarios and
# the agreement check included, and rebuild the Maps. So a Windows- or
# macOS-only break in code that can't move a flight (the SDL build, the
# window, the Input crate) is found on main after the merge, not on the pull
# request: the maintainer chose that cost (#240).
#
# A pull request is checked out as GitHub's merge commit with fetch-depth 2,
# so HEAD^1 is the base branch and the difference between them is exactly
# what the PR changes. Renames count as a deletion plus an addition, so
# renaming code to `.md` still runs the checks.
#
# Readable checks for this script: crates/xtask/tests/ci_changes.rs.
set -euo pipefail

# The flight paths: what a pull request can change to move a flight result.
# This is the one list to edit. A path ending in "/" is a folder, and counts
# with everything under it except Markdown; any other path is one file. A
# path may hold only letters, digits, ".", "_", "-" and "/".
#
# The Scenario runner is built from the crates here only (see
# crates/scenario/Cargo.toml and what those crates use), so a change to the
# game, the Input crate, the sound or the xtask tools can't move a flight. A
# new crate the runner is built with must join this list: a readable check
# reads them from `cargo metadata` and fails if one is missing.
#
# packs/, the Quad definitions and the Maps the Scenarios fly, counts too,
# Markdown included; it is checked on its own below, where flight is decided.
flight_paths=(
  # The core.
  crates/maths/
  crates/physics/
  crates/flight-controller/
  crates/sim/
  crates/test-pilot/
  # The Scenario runner and the two edge crates it is built with. The Pack
  # reader holds the built-in Test Maps and turns Quad definitions and Maps
  # into the Simulation's numbers; the Blackbox writer is empty for now, but
  # the runner is built with it.
  crates/scenario/
  crates/pack/
  crates/blackbox/
  # The Scenarios, their Results and the Test Quads.
  scenarios/
  # The libraries' versions, the build settings and Rust itself.
  Cargo.lock
  Cargo.toml
  rust-toolchain.toml
  .cargo/
  .config/
  # How CI runs and compares the Scenarios on each OS.
  .github/workflows/ci.yml
  .github/scripts/changes.sh
)

# The flight paths as one grep pattern: a folder matches what starts with it,
# a file matches only itself. A "." is escaped, so it matches only a ".".
flight_pattern=""
for path in "${flight_paths[@]}"; do
  if [[ ! "$path" =~ ^[A-Za-z0-9._/-]+$ ]]; then
    echo "::error::The flight path \"$path\" holds a character other than letters, digits, \".\", \"_\", \"-\" and \"/\"."
    exit 1
  fi
  path="${path//./\\.}"
  if [[ "$path" == */ ]]; then
    flight_pattern+="|^$path"
  else
    flight_pattern+="|^$path\$"
  fi
done
flight_pattern="${flight_pattern#|}"

if [[ "${GITHUB_EVENT_NAME:-}" != "pull_request" ]]; then
  echo "Not a pull request, so every Rust check runs."
  echo "rust=true" >> "$GITHUB_OUTPUT"
  echo "libraries=false" >> "$GITHUB_OUTPUT"
  echo "maps=true" >> "$GITHUB_OUTPUT"
  echo "flight=true" >> "$GITHUB_OUTPUT"
  echo "all_checks=true" >> "$GITHUB_OUTPUT"
  echo "scenarios=true" >> "$GITHUB_OUTPUT"
  exit 0
fi

# -z gives each path as it is, ended by a NUL byte: without it, git quotes a
# path that isn't plain ASCII ("docs/\305\202\303\263d\305\272.md"), which
# then ends in a quote mark instead of ".md" and doesn't start with "packs/".
# The NULs become line breaks for grep. A path holding a line break splits in
# parts, which never skips a check: its last part ends in ".md" only if the
# path does, and its first starts with "packs/" only if the path does.
# `$(…)` drops every line break at the very end of what it reads, so a name
# ending in one ("notes.md⏎"), sorted last, would read as Markdown. The "x"
# keeps them: it comes off, then exactly one line break, the one after the
# last path. Such a name then leaves an empty line, which isn't Markdown. The
# `&&` adds the "x" only if git worked, so a failed `git diff` still stops the
# script (pipefail) instead of reading as "nothing changed".
changed="$(git diff -z --name-only --no-renames HEAD^1 HEAD | tr '\0' '\n' && echo x)"
changed="${changed%x}"
changed="${changed%$'\n'}"
echo "Files this pull request changes:"
sed 's/^/  /' <<< "$changed"

rust=false
if grep -qvE '\.md$' <<< "$changed"; then
  echo "Something other than docs changed, so the Rust checks run."
  rust=true
elif grep -qE '^packs/' <<< "$changed"; then
  echo "A Pack's Markdown changed (a Feel Test log), which the Pack checker reads, so the Rust checks run."
  rust=true
elif grep -qE '^\.github/' <<< "$changed"; then
  echo "Markdown under .github/ changed (such as the Bevy upgrade ticket's template), which the readable checks read, so the Rust checks run."
  rust=true
else
  echo "Only docs changed, so the Rust checks are skipped and report as passed."
fi
echo "rust=$rust" >> "$GITHUB_OUTPUT"

# Markdown outside packs/ never reaches a flight, so it is left out first.
# It is kept in a variable, never piped into `grep -q`: that stops at its
# first match, the first grep then dies writing the rest (SIGPIPE), and
# pipefail counts the whole test as failed (Reviewer's case on #250).
# `|| true`: grep -v finds nothing to keep when every path is Markdown.
not_markdown="$(grep -vE '\.md$' <<< "$changed" || true)"
flight=false
if grep -qE "$flight_pattern" <<< "$not_markdown" \
  || grep -qE '^packs/' <<< "$changed"; then
  echo "Something that can change a flight result changed, so the Scenarios run on every OS and the agreement check compares them."
  flight=true
else
  echo "Nothing that can change a flight result changed, so macOS and Windows skip the Scenarios and the agreement check is skipped."
fi
echo "flight=$flight" >> "$GITHUB_OUTPUT"

# A pull request runs every Rust check on Linux only; on macOS and Windows it
# runs only the Scenarios, and only when they can move.
all_checks=false
scenarios=false
if [[ "$rust" == true && "${RUNNER_OS:-Linux}" == Linux ]]; then
  echo "This job runs on Linux, so it runs every Rust check."
  all_checks=true
  scenarios=true
elif [[ "$rust" == true && "$flight" == true ]]; then
  echo "This job runs on ${RUNNER_OS}, so for a pull request it runs only the Scenarios, for the agreement check. Main's CI runs every check here."
  scenarios=true
elif [[ "$rust" == true ]]; then
  echo "This job runs on ${RUNNER_OS}, and nothing here can change a flight result, so it skips everything and reports as passed. Main's CI runs every check here."
fi
echo "all_checks=$all_checks" >> "$GITHUB_OUTPUT"
echo "scenarios=$scenarios" >> "$GITHUB_OUTPUT"

if grep -qxE 'Cargo\.lock|deny\.toml' <<< "$changed"; then
  echo "Cargo.lock or deny.toml changed, so known security advisories are checked."
  echo "libraries=true" >> "$GITHUB_OUTPUT"
else
  echo "libraries=false" >> "$GITHUB_OUTPUT"
fi

if grep -qE '^(assets-src/|packs/[^/]+/maps/)' <<< "$changed"; then
  echo "assets-src/ or a Pack's maps/ changed, so the Maps are rebuilt with Blender."
  echo "maps=true" >> "$GITHUB_OUTPUT"
else
  echo "maps=false" >> "$GITHUB_OUTPUT"
fi
