#!/usr/bin/env bash
# The privileged half of the Review Report (#77), run by
# .github/workflows/review.yml with write access. It only moves data between
# GitHub's API and main's xtask, which makes every decision:
#
#   review.sh update   For one PR: work out the Review check from its
#                      comments, and post the Review Report comment, the
#                      labels and the Review check's commit status.
#                      Needs REPO, XTASK, and PR_NUMBER or RUN_HEAD_SHA.
#   review.sh merged   After a push to main: if the merged PR hadn't passed
#                      every required check, label it skipped-a-check and say
#                      so on it. Needs REPO, XTASK, SHA and BRANCH.
#
# GH_TOKEN is the workflow's token. Nothing here runs code from a pull
# request. Readable checks for xtask's side: crates/xtask/tests/review_check.rs.
#
# To try it by hand without posting anything, set DRY_RUN=1: every call that
# would change something on GitHub is printed instead.
set -euo pipefail

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

update() {
  local pr="${PR_NUMBER:-}"
  if [[ -z "$pr" && -n "${RUN_HEAD_SHA:-}" ]]; then
    # A fork's PR isn't named in the workflow_run event, so find it by commit.
    pr="$(api "repos/$REPO/commits/$RUN_HEAD_SHA/pulls" \
      --jq '[.[] | select(.state == "open")][0].number // empty' || true)"
  fi
  if [[ ! "$pr" =~ ^[0-9]+$ ]]; then
    echo "No open pull request to update."
    return 0
  fi

  api "repos/$REPO/pulls/$pr" > "$work/pr.json"
  if [[ "$(jq -r .state "$work/pr.json")" != "open" ]]; then
    echo "#$pr is closed, so its Review Report stays as it is."
    return 0
  fi
  local head
  head="$(jq -r .head.sha "$work/pr.json")"
  if [[ ! "$head" =~ ^[0-9a-f]{40}$ ]]; then
    echo "#$pr's latest commit isn't a full SHA: $head" >&2
    return 1
  fi

  api --paginate "repos/$REPO/issues/$pr/comments?per_page=100" > "$work/comments.json"
  api "search/issues?q=repo:$REPO+is:pr+is:merged+label:skipped-a-check&per_page=20" \
    > "$work/skipped.json" || echo '{"items": []}' > "$work/skipped.json"

  # The newest finished Review Report run for the PR's latest commit, if any.
  local report=() run_id
  run_id="$(api "repos/$REPO/actions/workflows/review-report.yml/runs?head_sha=$head&status=completed&per_page=1" \
    --jq '.workflow_runs[0].id // empty' || true)"
  if [[ "$run_id" =~ ^[0-9]+$ ]] \
    && gh run download "$run_id" --repo "$REPO" --name review-report --dir "$work/download"; then
    report=(--report "$work/download")
  else
    echo "No finished Review Report for $head yet."
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

  jq --arg url "$url" '{state, description, context: "Review check", target_url: $url}' \
    "$work/out/status.json" \
    | change --method POST "repos/$REPO/statuses/$head" --input - > /dev/null
  echo "Review check on $head: $(jq -r '.state + " (" + .description + ")"' "$work/out/status.json")"
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
