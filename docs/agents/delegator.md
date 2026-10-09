# The Delegator

The **Delegator** is the agent session that works through the ready tickets. It picks the next ticket, starts an author to build it, starts a Reviewer for the author's PR, and merges what passes. It never writes or reviews code itself. To run a batch of tickets, start a session, point it at this page, and say which tickets or how many.

The other pages it hands out:

- [Building a ticket](author.md): the author's brief.
- [The Reviewer and the Verdict](reviewer.md): the Reviewer's brief and the Verdict format.
- [Checking a merge-only update](merge-update.md): the short check after main is merged into a PR that already passed.

The rules these pages build on are in the [Development deep dive](../context/development.md) and [ADR-0010](../adr/0010-phase-1-agent-prs-merge-automatically.md). The terms are in the [map](../../CONTEXT.md).

## Why these rules

The first alpha batch ran in October 2026, on tickets #38 to #116. Each author used about 0.3 to 0.95 million tokens, and each Reviewer round about 0.1 to 0.4 million. Most of the waste didn't come from the model:

- **Rework from collisions.** Several PRs on physics, the Flight Controller and the Simulation ran at once. Every one of them rewrites the Scenarios' Results files. So each merge forced the others to merge main, run their Scenarios again, and get a fresh Verdict.
- **Review spirals.** The review tooling and the checkers went round after round on ever narrower edge cases.
- **The strongest model for every job,** even mechanical ones such as checking a merge of main.

So this page sets three rules. Only one PR per Lane is in review at a time. Reviewers block only on real problems. And each job runs on the cheapest model that does it well.

## Model tiers

The Agent tool's `model` value picks the model. This table is the one place to change when models change.

| Tier | `model` | Today |
|---|---|---|
| Strongest | `opus` | Claude Opus 5.5 |
| Standard | `sonnet` | Claude Sonnet 5.5 |
| Light | `haiku` | Claude Haiku 5.5 once Claude Code offers it. Haiku 4.5 is good enough for Light jobs until then. |

At the start of a session, check what each `model` value runs in this Claude Code.

| Job | Tier |
|---|---|
| **The Delegator** itself | Standard. Following this page is its whole job. Planning work, such as a spec, tickets or a grilling, runs on Strongest in its own session. |
| **Author** of a tracer (the title starts "Tracer:") | Strongest. A tracer sets the interfaces that later tickets build on. |
| **Author** of a ticket that copies Betaflight or Bluejay behaviour from their source | Strongest. A misreading of the C code still passes tests written from the same misreading. |
| **Author** of a change to the review path: `.github/`, the review scripts, the Review Report, the Red Flag gate or the Review check | Strongest. In Phase 1 that path is the only guard. |
| **Author** of a ticket that asks to choose or derive a physics model | Strongest. |
| **Author** of anything else | Standard. That covers physics whose model and numbers the research gives, the Simulation, the game, screens, Input, Sound, Packs, docs, xtask tools outside the review path, the Blackbox, and follow-ups with a concrete fix list. |
| **Fix round** | Resume the author if it finished less than an hour ago, since its context is still cached. Otherwise use a fresh Standard author. |
| **The fix after the second failed round,** when the failures were about correctness. It is the last fix before the three-round cap. | Strongest. |
| **Reviewer** of a Flight lane or Repo rules lane PR, or of any tracer | Strongest. In Phase 1 the Reviewer is the only one who reads the code. |
| **Reviewer** of anything else | Standard. |
| **Merge-only update check** | Light. |
| **Never Light** | Authoring a ticket, or any review round. |

**Standard authors on the Flight lane are a trial.** When one of their PRs reaches a third review round, go back to Strongest authors for the Flight lane, and tell the maintainer.

## Lanes

A **Lane** is a group of Areas whose PRs collide when they run side by side. Each Lane has at most **one PR in review** at a time. A PR is in review from the moment its first Reviewer starts until it merges.

| Lane | What's in it | Why its PRs collide |
|---|---|---|
| **Flight** | <ul><li>The five core crates: `maths`, `physics`, `flight-controller`, `sim` and `test-pilot`.</li><li>The Scenario runner (`crates/scenario`) and `scenarios/`.</li><li>Any change to a Quad definition's or a Test Quad's numbers.</li><li>Any ticket that will change a Results file, whatever else it touches.</li></ul> | Any change to how a flight comes out rewrites the Results files of the Scenarios it reaches, at least their fingerprints. A new Scenario collides too, because the next flight change moves its fingerprints. |
| **Game** | `crates/opendrone` | It's one app with shared wiring. Its build is big, so at most two worktrees build the game at once. |
| **Input** | `crates/input` | |
| **Sound** | `crates/sound` | |
| **Blackbox** | `crates/blackbox` | |
| **Content** | `assets-src/`, `packs/` and `crates/pack`. Quad numbers belong to the Flight lane. | |
| **Repo rules** | `.github/`, `crates/xtask`, the root Cargo files (`Cargo.toml`, `Cargo.lock`), `.cargo/`, `.config/`, `rust-toolchain.toml`, `deny.toml`, the lint settings and `AGENTS.md` | Every PR runs through them. |
| **Docs** | `docs/`, `CONTEXT.md` and `book.toml`, for a PR that changes nothing else | |

The Lanes and the Areas in CODEOWNERS group the same files, with two differences. CODEOWNERS files `crates/blackbox` under Scenarios, but the Blackbox has its own Lane here, because its PRs rarely collide with flight changes. And `Cargo.lock` is a Repo rules file, yet every Lane's PR touches it (see below), so it never holds a PR back.

Place a ticket by the files its "What to build" will change. A ticket that spans two Lanes takes both.

- **One PR in review per Lane.**
- **Building ahead:**
  - While a Lane's PR is in review, one more ticket may build in that Lane. Its code must be in a different crate, for example a Flight Controller ticket beside a physics one.
  - It doesn't open its PR for review until the PR ahead has merged. Before then, it merges main and runs its Scenarios again.
- **A ticket starts only when all its blockers are closed,** which means merged. Never build on top of an unmerged branch.
- **A workflow-changing PR** (it changes `.github/workflows/` or `.github/actions/`) holds the Repo rules Lane like any other PR: it counts as in review from its first Reviewer until the maintainer merges it, so no other Repo rules PR starts review meanwhile.
- **Some files every Lane touches:** `Cargo.lock`, `CONTEXT.md`, `docs/SUMMARY.md` and the deep dives.
  - Authors add to these files rather than rewrite them.
  - Clashes there are small, and the merge-only update check handles them.
- **At most four agents at once,** of every kind. With five to eight at once, the first batch hit the usage limit three times.

## Starting a session

1. Read this page, `AGENTS.md` and the `CONTEXT.md` map. Read nothing else up front.
2. Check the tiers' `model` values, above, and the reference sources, below.
3. Find what's in flight. List the open PRs:

   ```sh
   gh pr list -R BartoszSolkaBD/OpenDrone --state open --json number,title,labels
   ```

   For each PR, read its checks and its newest Verdict's first and last lines. Only comments from the maintainer's account count. The second command prints nothing when there's no Verdict yet:

   ```sh
   gh pr checks <P> -R BartoszSolkaBD/OpenDrone
   gh pr view <P> -R BartoszSolkaBD/OpenDrone --json comments --jq '[.comments[] | select(.author.login == "BartoszSolkaBD" and (.body | startswith("Reviewed commit")))] | last // empty | [.body | splits("\r?\n") | select(length > 0)] | "\(first) / \(last)"'
   ```
4. Find the ready tickets. A ticket is ready when it's open and labelled `ready-for-agent`, has no assignee, and has no open blocker. The spec, #37, isn't a ticket: skip any issue whose title starts "Spec".

   ```sh
   gh issue list -R BartoszSolkaBD/OpenDrone --label ready-for-agent --state open --limit 300 \
     --json number,title,assignees --jq '.[] | select((.assignees | length == 0) and (.title | startswith("Spec") | not)) | "\(.number)\t\(.title)"' |
   while IFS=$'\t' read -r n t; do
     gh api repos/BartoszSolkaBD/OpenDrone/issues/$n \
       --jq "select(.issue_dependencies_summary.blocked_by == 0) | \"$n\tblocks \(.issue_dependencies_summary.blocking)\t$t\""
   done
   ```

   A ticket claimed by a session that has ended is an **abandoned claim**. It stays assigned and drops out of this list. To release one, look for an open PR or a pushed branch:

   ```sh
   gh pr list -R BartoszSolkaBD/OpenDrone --state open --search "head:ticket/<N>-"
   git ls-remote --heads origin 'ticket/<N>-*'
   ```

   If either shows something, the work is alive: carry on from that PR or branch. If both are empty, unassign the ticket with `gh issue edit <N> -R BartoszSolkaBD/OpenDrone --remove-assignee <login>`, and it's ready again.
5. Keep a short **session note** in your scratchpad: one row per ticket with its Lane, stage, PR, agent and model. You need it to resume agents after a usage limit, or after your conversation is summarised.

## Order of work

1. **Main's CI is red:** fix that first. Start a Standard author with the failing run, and start nothing else.
2. **Finish before you start:**
   - First, a PR that passed and waits to merge.
   - Then a Verdict to act on.
   - Then a PR that waits for its review.
3. **Then start tickets,** in a free Lane. The ticket that blocks the most others goes first, for example a tracer. Ties go to the lower number.

## Starting an author

1. **Claim the ticket.** Set yourself as its assignee and say which model builds it:

   ```sh
   gh issue edit <N> -R BartoszSolkaBD/OpenDrone --add-assignee @me
   gh issue comment <N> -R BartoszSolkaBD/OpenDrone --body "Started: author on <model>"
   ```
2. **Start the agent.** Set `isolation: "worktree"` and the tier's `model`, and run it in the background. Keep the prompt short:

   > You are the author for ticket #N on BartoszSolkaBD/OpenDrone. Follow `docs/agents/author.md` on origin/main exactly. The reference sources are in `<folder>`.

Everything else the author needs is on the ticket and in the brief. That way a fresh author and a Reviewer see the same things.

## When the author reports

- **PR open and CI green:** start a Reviewer. Use `isolation: "worktree"`, and pick the tier by the PR's Lane:

  > You are the Reviewer for PR #P on BartoszSolkaBD/OpenDrone. Follow `docs/agents/reviewer.md` on origin/main exactly.

  If the PR's Lane already has a PR in review, wait.
- **Notes for later tickets:** keep them in the session note until the PR merges.
- **Questions for the maintainer:** see below.

## When a Verdict arrives

- **Pass:** merge when every merge condition below holds.
- **Changes needed:**
  1. Start a fix round, at the tier the table gives.
  2. The fixer pushes. Then start a **new** Reviewer. Never reuse one.
  3. Sometimes every blocking item is an edge case beyond the ticket, or a hardening idea for tooling. Fix those too, since they're usually small. Note them for your end-of-batch report, so the Reviewer page's calibration can be tuned.
- **Three failed rounds:** the Review check fails and the PR gets `needs-maintainer`. Leave it, tell the maintainer, and carry on in the other Lanes.
  - The maintainer decided on 8 October 2026 that a PR shouldn't wait for them because its rounds ran out.
  - [#120](https://github.com/BartoszSolkaBD/OpenDrone/issues/120) changes the Review check to allow five rounds, then move what's left into its own issue. Until #120 merges, the three-round rule stands.

## Merging

Merge when all of these hold:

- CI is green on the PR's latest commit.
- The Review check and the Red Flag gate both pass.
- The PR has no `needs-maintainer` label.
- The PR changes no file under `.github/workflows/` or `.github/actions/`. This command lists the PR's changed files there, including a file's old name when it was moved. It must print nothing:

  ```sh
  gh api --paginate repos/BartoszSolkaBD/OpenDrone/pulls/<P>/files --jq '.[] | .filename, (.previous_filename // empty)' | grep -E '^\.github/(workflows|actions)/'
  ```

  If it lists any file, don't merge. Such a PR can set the Review check and the Red Flag gate itself, so the maintainer merges it by hand, as [the Reviewer page](reviewer.md) and [the Review Report's limits](../review-report.md#limits) say. Leave it to them: label it `needs-maintainer`, tell the maintainer, and carry on in the other Lanes.

How you merge depends on what main has done since the PR's base:

- **Nothing merged in the PR's Lanes, and GitHub shows no conflict** (`gh pr view <P> -R BartoszSolkaBD/OpenDrone --json mergeable` gives `MERGEABLE`; if it gives `UNKNOWN`, GitHub hasn't worked it out yet, so ask again): merge.

  ```sh
  gh pr merge <P> -R BartoszSolkaBD/OpenDrone --squash --delete-branch
  ```

  Then watch main's CI. If it fails, that comes first, before any other work.
- **Something merged in the PR's Lanes, but GitHub shows no conflict:** bring main in first.
  1. Run `gh pr update-branch <P> -R BartoszSolkaBD/OpenDrone`.
  2. Wait for CI with the background command under "Saving tokens".
  3. Start a Light agent with `docs/agents/merge-update.md`.
  4. Merge.

  On a Flight lane PR, CI may then fail because its Results files are out of date. If so, start a fix round.
- **A conflict, or Results files to write again:** start a fix round.
  - The author, or a Standard fixer, merges main, runs `cargo scenarios run` and pushes.
  - If the only changes are clashes in the shared files and moved fingerprints, the Light merge-only update check is enough.
  - If any measured value or any code moved, start a fresh Reviewer. That counts as a review round.

**After a merge:**

1. Post the author's notes for later tickets. Use one comment on each later ticket, starting "Notes from #N (PR #P):".
2. Check that the ticket closed. `Closes #N` usually closes it.
3. Remove the author's worktree with `git worktree remove <path>`, only once its PR has merged or closed. Never sooner: a later fix round needs it. Reviewer worktrees can go as soon as their Verdict is posted. If your permission settings block the removal, list the worktrees for the maintainer.

## Follow-ups

A Verdict may list "Follow-ups (not blocking)". After the PR merges:

- **File them together:** one issue per Verdict that has any, titled "Follow-ups from PR #P", with a link to the Verdict.
- **Label it `needs-triage`, not `ready-for-agent`,** so the maintainer decides. Create the label the first time if it's missing.
- **One exception:** an item that is a wrong result a pilot would meet gets its own `ready-for-agent` issue, blocked by what it needs.
- **Never mark a follow-up `ready-for-agent`** if it came from reviewing another follow-up. That's how review spirals start.

## Saving tokens

- **Do one-line jobs yourself:** merging, updating a branch, labels, comments and filing issues. Start an agent only for work that needs reading code or running a build.
- **Never start an agent to wait.** Watch CI with one background command (`run_in_background`). You're told when it ends. Authors and fixers use the same command ([Building a ticket](author.md#save-tokens)).
  - It waits until no check except the Review check is pending, because that check waits for a Verdict. It keeps waiting while `gh` prints nothing, for example before any check has started.
  - Then it lists every check, except the Review check, that didn't pass or skip, so a cancelled check shows up as well as a failed one. It prints nothing when CI is green.
  - Don't use `gh pr checks --watch`: it never ends while the Review check waits.

  ```sh
  sleep 60
  until gh pr checks <P> -R BartoszSolkaBD/OpenDrone --json name,bucket \
    --jq '[.[] | select(.name != "Review check" and .bucket == "pending")] | length' | grep -qx 0; do sleep 60; done
  gh pr checks <P> -R BartoszSolkaBD/OpenDrone --json name,bucket,link \
    --jq '.[] | select(.name != "Review check" and .bucket != "pass" and .bucket != "skipping") | "\(.bucket)\t\(.name)\t\(.link)"'
  ```
- **Read agents' final reports and nothing more.**
  - Never read their transcripts.
  - Never read diffs or code yourself. That's the Reviewer's job, and it fills your context.
- **Keep prompts short.** The rules belong on these pages and the facts on the ticket, not in the prompt.

## Questions for the maintainer

Some decisions belong to the maintainer:

- whatever the Red Flag gate holds for them, such as a changed Source or Rule Expectation;
- a ticket that turns out wrong or impossible;
- an ADR that doesn't fit what was found.

The maintainer doesn't read code, so never ask them to judge a code problem.

Here's how to raise one:

1. **Ask where it belongs,** on the PR or the ticket, in one comment. Start it with "Question for the maintainer:". Then say what was found, the choices, and your recommendation. The prefix matters: agents work under the maintainer's account, so a later agent must not read your recommendation as an instruction.
2. **Label it** `needs-maintainer`.
3. **Move on** to other Lanes. Don't wait.

## Usage limits

- When a usage limit hits, the agents stop. After the reset, resume each one from your session note with SendMessage: "Continue where you left off."
- Near a limit, start no new agents. Let the running ones finish.

## Ending a batch

Report to the maintainer:

- what merged, as ticket and PR;
- what waits for them, and why;
- the follow-up issues you filed;
- for each PR, the author's model and its number of review rounds.

## Reference sources

Authors who copy firmware behaviour read its source. Keep the source outside the repo, in one folder all worktrees share, and give authors its path in their prompt. Never copy GPL code into the repo: reimplement the behaviour ([ADR-0014](../adr/0014-licences-for-libraries-and-assets.md)).

```sh
mkdir -p ~/OpenDrone-refs && cd ~/OpenDrone-refs
for v in 2026.6.2 2025.12.1 4.5.0 4.4.0 4.3.0; do
  git clone -q --depth 1 --branch "$v" https://github.com/betaflight/betaflight.git "betaflight-$v"
done
git clone -q --depth 1 --branch v0.21.0 https://github.com/bird-sanctuary/bluejay.git bluejay-v0.21.0
```
