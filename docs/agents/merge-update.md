# Checking a merge-only update

A PR got `Verdict: pass` on a commit, R. Since then, main was merged into the PR's branch, so CI could run on the two together. Every new commit needs a fresh Verdict ([The Reviewer and the Verdict](reviewer.md)). This page is the short check for that case. A fresh agent on the Light tier runs it ([the delegator](delegator.md#model-tiers)).

Your job is small and mechanical: confirm the update brought in main's changes and nothing else. Don't change code, push, merge or edit labels.

## The check

1. **Find R and the PR's latest commit, H.** R is the first line of the newest pass Verdict. Then fetch the PR's branch:

   ```sh
   gh pr view <PR> -R BartoszSolkaBD/OpenDrone --json headRefOid,headRefName,comments \
     --jq '.headRefOid, .headRefName, ([.comments[] | select(.body | startswith("Reviewed commit"))] | last | .body | split("\n") | "\(first) / \(last)")'
   git fetch origin <headRefName> main
   ```

   If the newest Verdict isn't a pass, stop: this check doesn't apply.
2. **List the commits after R:** `git log --format='%H %P %s' R..H`. Each one must be one of two kinds:
   - **A merge from main.** It has two parents, and the second is on main: `git merge-base --is-ancestor <second parent> origin/main`.
   - **A fingerprint commit.** It changes only `*.results.toml` files, and only inside their `[fingerprints]` tables.
3. **For each merge commit M, read `git show --cc M`.**
   - **Empty:** the merge was clean.
   - **Anything shown is a conflict the author resolved.** That's allowed only in `Cargo.lock`, in docs text (`docs/` and `CONTEXT.md`), and in Results fingerprints.
   - **Read each hunk.** It must keep both sides, main's and the PR's, and add nothing else.
4. **For each fingerprint commit C,** check that no other kind of file changed, and that no value line changed:

   ```sh
   git diff --name-only C^ C | grep -v '\.results\.toml$'
   git diff C^ C | grep -E '^[-+](format|scenario|what|basis|expected|measured|\[\[)'
   ```

   Both must print nothing.
5. **Cross-check:** `git diff --stat R H` lists only files that main changed, plus Results files.
6. **Check CI:** `gh pr checks <PR> -R BartoszSolkaBD/OpenDrone`. Every check passes, except that the Review check may wait.

## The result

**If every step holds,** post one comment with `gh pr comment <PR> -R BartoszSolkaBD/OpenDrone --body-file verdict.md`:

```text
Reviewed commit <H, all 40 characters>

Merge-only update after the pass on <R>: main merged in, <no conflicts | conflicts resolved only in …, both sides kept>, <no fingerprint commits | fingerprint commits with no value changed>, CI green.

Verdict: pass
```

**If any step fails,** post nothing. Never post `Verdict: changes needed` from this check: that would count as a failed review round for a problem you weren't asked to judge. Tell the delegator which step failed and why. The PR then needs a full Reviewer.

Then report to the delegator. Say what you posted, and the Review check's status a minute or two later.
