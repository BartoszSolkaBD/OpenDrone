#!/usr/bin/env bash
# The Review Report, the Red Flag gate and the Review check (#77), run by
# .github/workflows/review.yml with write access, from main's copy. It moves
# data between git, GitHub's API, crates.io's API and main's xtask, which
# makes every decision:
#
#   review.sh update   For one PR: fetch its commits as git objects (never
#                      checked out, never built, never run), work out the
#                      Review Report and its Red Flags, and the Review check
#                      from the PR's comments (and the issues its triage
#                      comments name, #120); then post the Review Report
#                      comment, the labels, and the Red Flag gate's and the
#                      Review check's commit statuses.
#                      Needs REPO, XTASK and PR_NUMBER; RUN_URL is optional,
#                      and so are EVENT_HEAD_SHA, EVENT_BASE_REF and
#                      DEFAULT_BRANCH, the commit a PR event names, marked
#                      with error if the PR's record can't be read.
#   review.sh others   After a PR closes, gets a new commit or moves onto or
#                      off the default branch: list the other open PRs into
#                      the default branch whose latest commit is (or, before a
#                      push, was) this PR's, as the step output `prs`, so each
#                      is judged again. Needs REPO, PR_NUMBER, DEFAULT_BRANCH
#                      and HEAD_SHA; BEFORE_SHA is the commit before a push.
#   review.sh merged   After a push to main: if the merged PR hadn't passed
#                      every required check, label it skipped-a-check and say
#                      so on it. Needs REPO, XTASK, SHA and BRANCH.
#
# GH_TOKEN is the workflow's token. Readable checks: crates/xtask/tests/
# review_report.rs and review_check.rs for xtask's side, review_script.rs for
# this script.
#
# To try it by hand without posting anything, set DRY_RUN=1: every call that
# would change something on GitHub is printed instead. It still fetches the
# PR's commits into refs/review/ in the local repository.
set -Eeuo pipefail

: "${REPO:?}"
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

# The commit being judged, once known, so a failure can say so on it. The
# script's own output is kept as file 3: a failure inside a call whose output
# goes to a file still logs what it posted.
judged=""
exec 3>&1
on_error() {
  if [[ -n "$judged" ]]; then
    post_status "$judged" "Red Flag gate" error \
      "The review workflow failed: see its run" "${RUN_URL:-}" >&3 || true
    post_status "$judged" "Review check" error \
      "The review workflow failed: see its run" "${RUN_URL:-}" >&3 || true
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
    [[ "$name" =~ ^[A-Za-z0-9_-]{1,64}$ && "$version" =~ ^[0-9][0-9A-Za-z.+-]{0,63}$ ]] || continue
    [[ "$version" != *..* ]] || continue
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
  : "${XTASK:?}"
  local pr="${PR_NUMBER:-}"
  if [[ ! "$pr" =~ ^[0-9]+$ ]]; then
    echo "No pull request to update."
    return 0
  fi

  # Until GitHub's record of the PR is read, a failure marks the commit the
  # event named, if it named one on a PR into the default branch. A comment
  # names no commit: its failed run is the only trace.
  if [[ "${EVENT_HEAD_SHA:-}" =~ ^[0-9a-f]{40}$ && -n "${DEFAULT_BRANCH:-}" \
    && "${EVENT_BASE_REF:-}" == "${DEFAULT_BRANCH:-}" ]]; then
    judged="$EVENT_HEAD_SHA"
  fi

  api "repos/$REPO/pulls/$pr" > "$work/pr.json"
  local head base_ref default_branch
  head="$(jq -r .head.sha "$work/pr.json")"
  base_ref="$(jq -r .base.ref "$work/pr.json")"
  default_branch="$(jq -r .base.repo.default_branch "$work/pr.json")"
  if [[ ! "$head" =~ ^[0-9a-f]{40}$ || ! "$base_ref" =~ ^[A-Za-z0-9._/-]{1,100}$ ]] \
    || [[ "$base_ref" == *..* ]]; then
    echo "#$pr's latest commit or base branch looks wrong: $head, $base_ref" >&2
    return 1
  fi
  # From here on, GitHub's record decides which commit is judged.
  judged=""
  if [[ "$(jq -r .state "$work/pr.json")" != "open" ]]; then
    echo "#$pr is closed, so its Review Report stays as it is."
    return 0
  fi

  # Only a PR into the default branch sets the statuses (xtask decides; this
  # only says "pending" first). Both go pending before anything is fetched,
  # so a run that stops early never leaves an older result on the commit.
  if [[ "$base_ref" == "$default_branch" ]]; then
    judged="$head"
    post_status "$head" "Red Flag gate" pending "Working out the Red Flags" "${RUN_URL:-}"
    post_status "$head" "Review check" pending "Working out the Review check" "${RUN_URL:-}"
  fi

  # The PR's latest commit, by its SHA, and its base branch, as git objects
  # only: nothing is checked out or run.
  git -c protocol.version=2 fetch --no-tags --quiet origin \
    "+$head:refs/review/head" "+refs/heads/$base_ref:refs/review/base"
  local base
  base="$(git merge-base refs/review/base refs/review/head)"

  api --paginate "repos/$REPO/issues/$pr/comments?per_page=100" > "$work/comments.json"
  # Every PR holding this same commit: if another open one into the default
  # branch has it as its latest commit, the commit can't carry this PR's
  # results alone (xtask decides).
  api "repos/$REPO/commits/$head/pulls?per_page=100" > "$work/head-pulls.json"
  api "search/issues?q=repo:$REPO+is:pr+is:merged+label:skipped-a-check&per_page=20" \
    > "$work/skipped.json" || echo '{"items": []}' > "$work/skipped.json"

  # Once the PR's review rounds have run out, the issues its "Triaged to #N"
  # comments name (xtask lists them, and decides which count). A missing one
  # is left out, so it counts for nothing; any other error fails the run, so
  # it shows instead of failing the Review check quietly.
  "$XTASK" triage-issues --comments "$work/comments.json" > "$work/triage-issues.txt"
  : > "$work/issues.json"
  local issue
  while read -r issue; do
    [[ "$issue" =~ ^[1-9][0-9]{0,9}$ ]] || continue
    if api "repos/$REPO/issues/$issue" > "$work/issue.json" 2> "$work/issue.err"; then
      cat "$work/issue.json" >> "$work/issues.json"
      echo >> "$work/issues.json"
    elif grep -qE '\(HTTP (404|410)\)' "$work/issue.err"; then
      echo "GitHub has no issue #$issue, so a triage comment naming it counts for nothing."
    else
      cat "$work/issue.err" >&2
      echo "GitHub couldn't say whether issue #$issue is open." >&2
      return 1
    fi
  done < "$work/triage-issues.txt"

  licences "$base" "$head" > "$work/metadata.json"

  # Exit code 1 means a Red Flag waits for the maintainer; the gate status
  # below says so. Anything else but 0 means the Report couldn't be made.
  local report=() code=0
  "$XTASK" review-report --base "$base" --head "$head" \
    --metadata "$work/metadata.json" --codeowners .github/CODEOWNERS \
    --out "$work/report" || code=$?
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
    --head-pulls "$work/head-pulls.json" \
    --issues "$work/issues.json" \
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

  # The statuses xtask decided: none for a PR into another branch.
  local context state description status_url
  while IFS=$'\t' read -r context state description; do
    status_url="$url"
    if [[ "$state" == "error" ]]; then
      status_url="${RUN_URL:-$url}"
    fi
    post_status "$head" "$context" "$state" "$description" "$status_url"
  done < <(jq -r '.[] | [.context, .state, .description] | @tsv' "$work/out/statuses.json")
  if [[ "$base_ref" != "$default_branch" ]]; then
    echo "#$pr merges into $base_ref, not $default_branch, so it sets no statuses."
  fi
  judged=""
}

# Statuses belong to a commit, so while two open PRs into the default branch
# have the same latest commit, both statuses fail on it. Once one of them
# closes, gets a new commit or moves off the default branch, the others must
# be judged again, or that failure stays. Which PRs to run again is no
# judgment: each run works everything out afresh, and xtask decides there.
others() {
  local pr="${PR_NUMBER:-}" main="${DEFAULT_BRANCH:-}" sha found='[]'
  if [[ ! "$pr" =~ ^[0-9]+$ || ! "$main" =~ ^[A-Za-z0-9._/-]{1,100}$ ]] || [[ "$main" == *..* ]]; then
    echo "The PR number or the default branch looks wrong: $pr, $main" >&2
    return 1
  fi
  for sha in "${BEFORE_SHA:-}" "${HEAD_SHA:-}"; do
    [[ "$sha" =~ ^[0-9a-f]{40}$ ]] || continue
    if ! api "repos/$REPO/commits/$sha/pulls?per_page=100" > "$work/pulls.json" \
      2> "$work/pulls.err"; then
      # A commit GitHub doesn't have (after a force-push) is in no PR. Any
      # other error fails this job, so it shows instead of passing quietly.
      if grep -qE '\(HTTP (404|422)\)' "$work/pulls.err"; then
        echo "GitHub has no commit $sha, so no PR has it."
        echo '[]' > "$work/pulls.json"
      else
        cat "$work/pulls.err" >&2
        echo "GitHub couldn't say which PRs have $sha, so none is judged again." >&2
        return 1
      fi
    fi
    found="$(jq -c --argjson found "$found" --argjson pr "$pr" --arg sha "$sha" --arg main "$main" \
      '$found + [.[] | select(.state == "open" and .base.ref == $main
                         and .head.sha == $sha and .number != $pr) | .number]
       | unique | .[:20]' "$work/pulls.json")"
  done
  if [[ "$found" == '[]' ]]; then
    echo "No other open PR into $main has #$pr's commit as its latest one."
  else
    echo "These open PRs into $main had #$pr's commit as their latest one, so they are judged again: $found"
  fi
  echo "prs=$found" >> "${GITHUB_OUTPUT:-/dev/stdout}"
}

merged() {
  : "${SHA:?}" "${BRANCH:?}" "${XTASK:?}"
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
  others) others ;;
  merged) merged ;;
  *)
    echo "Usage: review.sh update|others|merged" >&2
    exit 2
    ;;
esac
