#!/usr/bin/env bash
# The Review Report, the Red Flag gate and the Review check (#77), run by
# .github/workflows/review.yml with write access, from main's copy. It moves
# data between git, GitHub's API, crates.io's API and main's xtask, which
# makes every decision:
#
#   review.sh update   For one PR: fetch its commits as git objects (never
#                      checked out, never built, never run), work out the
#                      Review Report and its Red Flags, and the Review check
#                      from the PR's comments; then post the Review Report
#                      comment, the labels, and the Red Flag gate's and the
#                      Review check's commit statuses.
#                      Needs REPO, XTASK and PR_NUMBER; RUN_URL is optional.
#   review.sh merged   After a push to main: if the merged PR hadn't passed
#                      every required check, label it skipped-a-check and say
#                      so on it. Needs REPO, XTASK, SHA and BRANCH.
#
# GH_TOKEN is the workflow's token. Readable checks for xtask's side:
# crates/xtask/tests/review_report.rs and review_check.rs.
#
# To try it by hand without posting anything, set DRY_RUN=1: every call that
# would change something on GitHub is printed instead. It still fetches the
# PR's commits into refs/review/ in the local repository.
set -Eeuo pipefail

: "${REPO:?}" "${XTASK:?}"
work="${RUNNER_TEMP:-/tmp}/review-post"
rm -rf "$work"
mkdir -p "$work"

api() {
  gh api -H "X-GitHub-Api-Version: 2022-11-28" "$@"
}

# Every call that changes something on GitHub goes through here.
change() {
  if [[ -n "${DRY_RUN:-}" ]]; then
    echo "DRY_RUN: would call gh api $*, with:" >&2
    cat >&2
    echo >&2
    echo "(dry run)"
    return 0
  fi
  api "$@"
}

# Sets a commit status: post_status <sha> <context> <state> <description> [<url>]
post_status() {
  jq -n --arg context "$2" --arg state "$3" --arg description "$4" --arg url "${5:-}" \
    '{context: $context, state: $state, description: $description}
      + (if $url == "" then {} else {target_url: $url} end)' \
    | change --method POST "repos/$REPO/statuses/$1" --input - > /dev/null
  echo "$2 on $1: $3 ($4)"
}

# Makes sure a label exists (xtask gives its name, colour and description),
# then puts it on the PR.
add_label() {
  local pr="$1" label="$2" name
  name="$(jq -r .name <<< "$label")"
  if ! api "repos/$REPO/labels/$(jq -rn --arg n "$name" '$n | @uri')" > /dev/null 2>&1; then
    jq '{name, color, description}' <<< "$label" \
      | change --method POST "repos/$REPO/labels" --input - > /dev/null
  fi
  jq -n --arg name "$name" '{labels: [$name]}' \
    | change --method POST "repos/$REPO/issues/$pr/labels" --input - > /dev/null
  echo "Added the label \`$name\` to #$pr."
}

# The commit the gate is judging, once known, so a failure can say so on it.
judged=""
on_error() {
  if [[ -n "$judged" ]]; then
    post_status "$judged" "Red Flag gate" error \
      "The review workflow failed: see its run" "${RUN_URL:-}" || true
  fi
}
trap on_error ERR

# Licences of the outside libraries a PR adds, from crates.io, shaped like
# `cargo metadata`'s output. Names and versions come from the PR's Cargo.lock,
# so they are checked before they go into a web address; crates.io's answers
# are data, which xtask escapes. At most 30 are looked up, a second apart, as
# crates.io asks.
licences() {
  local base="$1" head="$2" name version count=0
  local packages="$work/packages.jsonl"
  : > "$packages"
  while read -r name version; do
    [[ "$name" =~ ^[A-Za-z0-9_-]{1,64}$ && "$version" =~ ^[0-9A-Za-z.+-]{1,64}$ ]] || continue
    if (( count >= 30 )); then
      break
    fi
    count=$((count + 1))
    sleep 1
    curl -fsS --max-time 15 -A "OpenDrone review workflow (https://github.com/$REPO)" \
      "https://crates.io/api/v1/crates/$name/$(jq -rn --arg v "$version" '$v | @uri')" \
      | jq -c --arg name "$name" --arg version "$version" \
        '{name: $name, version: $version,
          license: (.version.license // null | if type == "string" then . else null end)}' \
      >> "$packages" || true
  done < <("$XTASK" new-libraries --base "$base" --head "$head")
  jq -s '{packages: .}' "$packages"
}

update() {
  local pr="${PR_NUMBER:-}"
  if [[ ! "$pr" =~ ^[0-9]+$ ]]; then
    echo "No pull request to update."
    return 0
  fi

  api "repos/$REPO/pulls/$pr" > "$work/pr.json"
  if [[ "$(jq -r .state "$work/pr.json")" != "open" ]]; then
    echo "#$pr is closed, so its Review Report stays as it is."
    return 0
  fi
  local head base_ref
  head="$(jq -r .head.sha "$work/pr.json")"
  base_ref="$(jq -r .base.ref "$work/pr.json")"
  if [[ ! "$head" =~ ^[0-9a-f]{40}$ || ! "$base_ref" =~ ^[A-Za-z0-9._/-]{1,100}$ ]]; then
    echo "#$pr's latest commit or base branch looks wrong: $head, $base_ref" >&2
    return 1
  fi

  # The PR's commits, as git objects only: nothing is checked out or run.
  git fetch --no-tags --quiet origin \
    "+refs/pull/$pr/head:refs/review/head" "+refs/heads/$base_ref:refs/review/base"
  if [[ "$(git rev-parse refs/review/head)" != "$head" ]]; then
    echo "#$pr moved on while this ran; the run for its newest push updates it."
    return 0
  fi
  local base
  base="$(git merge-base refs/review/base refs/review/head)"
  judged="$head"
  post_status "$head" "Red Flag gate" pending "Working out the Red Flags" "${RUN_URL:-}"

  api --paginate "repos/$REPO/issues/$pr/comments?per_page=100" > "$work/comments.json"
  api "search/issues?q=repo:$REPO+is:pr+is:merged+label:skipped-a-check&per_page=20" \
    > "$work/skipped.json" || echo '{"items": []}' > "$work/skipped.json"

  licences "$base" "$head" > "$work/metadata.json"

  # Exit code 1 means a Red Flag waits for the maintainer; the gate status
  # below says so. Anything else but 0 means the Report couldn't be made.
  local report=() code=0
  "$XTASK" review-report --base "$base" --head "$head" \
    --metadata "$work/metadata.json" --out "$work/report" || code=$?
  if (( code == 0 || code == 1 )); then
    jq -n --argjson number "$pr" --arg head_sha "$head" '{number: $number, head_sha: $head_sha}' \
      > "$work/report/pr.json"
    report=(--report "$work/report")
  else
    echo "The Review Report couldn't be worked out for $head." >&2
  fi

  "$XTASK" review-update \
    --pr "$work/pr.json" \
    --comments "$work/comments.json" \
    --skipped-merges "$work/skipped.json" \
    --codeowners .github/CODEOWNERS \
    --out "$work/out" \
    ${report[@]+"${report[@]}"}

  jq -c '.[]' "$work/out/labels-add.json" | while read -r label; do
    add_label "$pr" "$label"
  done
  jq -r '.[]' "$work/out/labels-remove.json" | while read -r name; do
    change --method DELETE "repos/$REPO/issues/$pr/labels/$(jq -rn --arg n "$name" '$n | @uri')" \
      < /dev/null > /dev/null || true
    echo "Took the label \`$name\` off #$pr."
  done

  local id url
  id="$(cat "$work/out/comment-id")"
  if [[ "$id" =~ ^[0-9]+$ ]]; then
    url="$(jq -n --rawfile body "$work/out/comment.md" '{body: $body}' \
      | change --method PATCH "repos/$REPO/issues/comments/$id" --input - --jq .html_url)"
    echo "Updated the Review Report: $url"
  else
    url="$(jq -n --rawfile body "$work/out/comment.md" '{body: $body}' \
      | change --method POST "repos/$REPO/issues/$pr/comments" --input - --jq .html_url)"
    echo "Posted the Review Report: $url"
  fi

  local gate_url="$url"
  if [[ "$(jq -r .state "$work/out/gate.json")" == "error" ]]; then
    gate_url="${RUN_URL:-$url}"
  fi
  post_status "$head" "Red Flag gate" "$(jq -r .state "$work/out/gate.json")" \
    "$(jq -r .description "$work/out/gate.json")" "$gate_url"
  post_status "$head" "Review check" "$(jq -r .state "$work/out/status.json")" \
    "$(jq -r .description "$work/out/status.json")" "$url"
  judged=""
}

merged() {
  : "${SHA:?}" "${BRANCH:?}"
  local merged_pr pr head
  merged_pr="$(api "repos/$REPO/commits/$SHA/pulls" \
    --jq '[.[] | select(.merged_at != null)][0] // empty')"
  if [[ -z "$merged_pr" ]]; then
    echo "$SHA didn't come from a pull request."
    return 0
  fi
  pr="$(jq -r .number <<< "$merged_pr")"
  head="$(jq -r .head.sha <<< "$merged_pr")"

  api "repos/$REPO/branches/$BRANCH" > "$work/branch.json"
  api "repos/$REPO/rules/branches/$BRANCH" > "$work/rules.json" || echo '[]' > "$work/rules.json"
  api --paginate "repos/$REPO/commits/$head/check-runs?per_page=100" > "$work/runs.json"
  api "repos/$REPO/commits/$head/status" > "$work/status.json"

  local skipped
  skipped="$("$XTASK" merge-check \
    --branch "$work/branch.json" \
    --rules "$work/rules.json" \
    --check-runs "$work/runs.json" \
    --status "$work/status.json" \
    --out "$work/out")"
  if [[ -z "$skipped" ]]; then
    echo "#$pr passed every required check before it merged."
    return 0
  fi
  echo "#$pr merged before these checks passed:"
  echo "$skipped"
  add_label "$pr" "$(cat "$work/out/label.json")"
  jq -n --rawfile body "$work/out/comment.md" '{body: $body}' \
    | change --method POST "repos/$REPO/issues/$pr/comments" --input - > /dev/null
}

case "${1:-}" in
  update) update ;;
  merged) merged ;;
  *)
    echo "Usage: review.sh update|merged" >&2
    exit 2
    ;;
esac
