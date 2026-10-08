# Checking a merge-only update

A PR got `Verdict: pass` on a commit, R. Since then, main was merged into the PR's branch, so CI could run on the two together. A **merge-only update** is such a branch where the only changes since R are main's own changes, clashes resolved in `Cargo.lock` or docs text, and Results fingerprints. Every new commit needs a fresh Verdict ([The Reviewer and the Verdict](reviewer.md)). This page is the short check for that case. A fresh agent on the Light tier runs it ([the delegator](delegator.md#model-tiers)).

Your job is small and mechanical: confirm the update brought in main's changes and nothing else. Don't change code, push, merge or edit labels.

## The check

1. **Find R and the PR's latest commit, H.** R is the first line of the newest Verdict from the maintainer's account. Then fetch the PR's branch:

   ```sh
   gh pr view <PR> -R BartoszSolkaBD/OpenDrone --json headRefOid,headRefName --jq '.headRefOid, .headRefName'
   gh pr view <PR> -R BartoszSolkaBD/OpenDrone --json comments --jq '[.comments[] | select(.author.login == "BartoszSolkaBD" and (.body | startswith("Reviewed commit")))] | last // empty | [.body | splits("\r?\n") | select(length > 0)] | "\(first) / \(last)"'
   git fetch origin <headRefName> main
   ```

   If there's no Verdict, or the newest one isn't a pass, stop: this check doesn't apply.
2. **List the commits after R.** First check that R is in H's history, then list the branch's own line of commits. `--first-parent` leaves out the commits that came in from main.

   ```sh
   git merge-base --is-ancestor R H && echo "R is in H's history" || echo "R is NOT in H's history"
   git log --first-parent --format='%H %P %s' R..H
   ```

   If R isn't in H's history, stop. Each listed commit must be one of two kinds:
   - **A merge from main.** It has two parents, and the second is on main:

     ```sh
     git merge-base --is-ancestor <second parent> origin/main && echo "on main" || echo "NOT on main"
     ```
   - **A fingerprint commit.** It changes only `*.results.toml` files, and only inside their `[fingerprints]` tables (step 4).
3. **For each merge commit M, redo the merge and compare.** Git can merge M's two parents by itself. Whatever differs from M is what the author changed by hand. First redo the merge:

   ```sh
   git merge-tree --write-tree --name-only --no-messages M^1 M^2
   ```

   It exits 1 when files conflict. The first line of its output is **T**, the tree git made by itself. The lines after it are the files that conflicted. Then run these two, with T copied from that first line. The first lists the files M changed by hand. The second checks that no value line in a Results file changed:

   ```sh
   git diff --no-renames --name-only T M
   git diff --no-renames T M -- '*.results.toml' | grep -E '^[-+](format|scenario|what|basis|expected|measured|\[\[)'
   ```

   - **Files that conflicted:** they may be only `Cargo.lock`, files under `docs/`, `CONTEXT.md`, and Results files. Anything else conflicting means stop.
   - **Files that differ between T and M:**
     - Each must be a file that conflicted and is in that allowed set. Otherwise the author changed something else, or dropped part of main's change. Stop.
     - Results files may differ only in fingerprints. The last command must print nothing.
   - **Read `git diff T M -- <file>`** for each file that conflicted. The conflict markers are gone, both sides are kept, and nothing is added.
   - **Rewritten Results belong in their own commit,** a fingerprint commit after the merge, as [PR #116](https://github.com/BartoszSolkaBD/OpenDrone/pull/116) did ([Bringing main in](author.md#bringing-main-in)). If the merge commit itself rewrote Results files that didn't conflict, they differ from T, and this step stops.
4. **For each fingerprint commit C,** check that no other kind of file changed, and that no value line changed:

   ```sh
   git diff --no-renames --name-only C^ C | grep -v '\.results\.toml$'
   git diff --no-renames C^ C | grep -E '^[-+](format|scenario|what|basis|expected|measured|\[\[)'
   ```

   Both must print nothing.
5. **Cross-check:** with N the second parent of the newest merge, `git diff R H` may list only files that main changed, plus Results files. This catches a stray file. Step 3 is what catches a dropped change of main's. First find where R and main split:

   ```sh
   git merge-base R N
   ```

   Call its output **B**. Then run these two, with B copied in. The first writes the files main changed to a scratch file named after the PR. The second lists any other file the branch changed:

   ```sh
   git diff --no-renames --name-only B N | sort > /tmp/main-files-<PR>.txt
   git diff --no-renames --name-only R H | sort | comm -13 /tmp/main-files-<PR>.txt - | grep -v '\.results\.toml$'
   ```

   The second must print nothing.
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
