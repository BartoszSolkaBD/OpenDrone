# The Delegator

The **Delegator** is the agent session that works through the ready tickets. It picks the next ticket, starts an author to build it, starts a Reviewer for the author's PR, and merges what passes. It never writes or reviews code itself, and it doesn't decide changes to these agent pages: it may propose one, in its report or as a question for the maintainer. Once the maintainer approves, it may write the change itself and open the PR, and a Reviewer reviews it as usual. To run a batch of tickets, start a session, point it at this page, and say which tickets or how many.

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
| **The Delegator** itself | On trial: the maintainer is comparing Strongest and Standard, so it runs on whichever model its session was started on. Following this page is its whole job. Planning work, such as a spec, tickets or a grilling, runs on Strongest in its own session. |
| **Author** of a tracer (the title starts "Tracer:") | Strongest. A tracer sets the interfaces that later tickets build on. |
| **Author** of a ticket that copies Betaflight or Bluejay behaviour from their source | Strongest. A misreading of the C code still passes tests written from the same misreading. |
| **Author** of a change to the review path: `.github/`, the review scripts, the Review Report, the Red Flag gate or the Review check | Strongest. In Phase 1 that path is the only guard. |
| **Author** of a ticket that asks to choose or derive a physics model | Strongest. |
| **Author** of anything else | Standard. That covers physics whose model and numbers the research gives, the Simulation, the game, screens, Input, Sound, Packs, docs, xtask tools outside the review path, the Blackbox, and follow-ups with a concrete fix list. |
| **Fix round** | Resume the author if it finished less than an hour ago, since its context is still cached. Otherwise use a fresh Standard author. |
| **The fix after the fourth failed round,** when the failures were about correctness. It is the last fix before the five-round cap. | Strongest. |
| **Reviewer** of a Flight lane or Repo rules lane PR, or of any tracer | Strongest. In Phase 1 the Reviewer is the only one who reads the code. |
| **Reviewer** of anything else | Standard. |
| **Merge-only update check** | Light. |
| **Never Light** | Authoring a ticket, or any review round. |

**Standard authors on the Flight lane are a trial.** When one of their PRs reaches a third review round, go back to Strongest authors for the Flight lane, and tell the maintainer.

## Lanes

A **Lane** is a group of Areas whose PRs collide when they run side by side. Each Lane has at most **one PR in review** at a time. A PR is in review from the moment its first Reviewer starts until it merges, or until it steps out of its Lane to wait for the maintainer ([When a PR waits for the maintainer](#when-a-pr-waits-for-the-maintainer)).

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

**Example: a ticket in two Lanes.** Ticket [#47](https://github.com/BartoszSolkaBD/OpenDrone/issues/47) checked the Freestyle 5″ against real flight logs ([PR #238](https://github.com/BartoszSolkaBD/OpenDrone/pull/238)). It changed the Flight lane's crates and `crates/xtask`, so it took the Flight Lane and the Repo rules Lane. It waited for a free slot in both, and while its PR was in review it held the one slot in each.

**Lane labels.** A ticket carries one `lane: <Lane>` label for each Lane it takes, such as `lane: Flight` or `lane: Repo rules`. Whoever files the ticket adds them: a planning session, or the Delegator for an issue it files. Before you start a ticket, check its labels against the files its "What to build" will change. If one is missing or wrong, fix it, and list the fix in your end-of-batch report.

- **One PR in review per Lane.** A PR that waits for the maintainer doesn't count: it steps out of its Lane ([below](#when-a-pr-waits-for-the-maintainer)).
- **Building ahead:**
  - While a Lane's PR is in review, one more ticket may build in that Lane when **none of the files it will change are files the PR in review changes**, and neither changes a Results file. Files decide it, not crates. Check with `gh pr diff <P> --name-only` against the ticket's "What to build". Five Game tickets built ahead in `crates/opendrone` on 21 October 2026, and all five merged without a conflict.
  - It doesn't open its PR for review until the PR ahead has merged. Before then, it merges main and runs its Scenarios again.
- **A ticket starts only when all its blockers are closed,** which means merged. Never build on top of an unmerged branch.
- **A workflow-changing PR** (it changes `.github/workflows/` or `.github/actions/`) holds the Repo rules Lane while it's being reviewed. Once it passes, it waits for the maintainer to merge it by hand, so it steps out of the Lane like any PR that waits for them.
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
4. Find the ready tickets. A ticket is ready when it's open and labelled `ready-for-agent`, has no assignee, has no open blocker, and doesn't wait for the maintainer (no `needs-maintainer` label). The spec, #37, isn't a ticket: skip any issue whose title starts "Spec". The command prints each ready ticket's number, its Lanes from its Lane labels, how many tickets it blocks, and its title:

   ```sh
   gh issue list -R BartoszSolkaBD/OpenDrone --label ready-for-agent --state open --limit 300 \
     --json number,title,assignees,labels --jq '.[] | select((.assignees | length == 0) and (.title | startswith("Spec") | not) and (any(.labels[]; .name == "needs-maintainer") | not)) | "\(.number)\t\([.labels[].name | select(startswith("lane: ")) | ltrimstr("lane: ")] | join(" + ") | if . == "" then "no Lane label" else . end)\t\(.title)"' |
   while IFS=$'\t' read -r n l t; do
     gh api repos/BartoszSolkaBD/OpenDrone/issues/$n \
       --jq "select(.issue_dependencies_summary.blocked_by == 0) | \"$n\t$l\tblocks \(.issue_dependencies_summary.blocking)\t$t\""
   done
   ```

   A ticket that shows "no Lane label" needs one before it starts ([Lane labels](#lanes)).

   A ticket claimed by a session that has ended is an **abandoned claim**. It stays assigned and drops out of this list. To release one, look for an open PR or a pushed branch:

   ```sh
   gh pr list -R BartoszSolkaBD/OpenDrone --state open --search "head:ticket/<N>-"
   git ls-remote --heads origin 'ticket/<N>-*'
   ```

   If either shows something, the work is alive: carry on from that PR or branch. If both are empty, unassign the ticket with `gh issue edit <N> -R BartoszSolkaBD/OpenDrone --remove-assignee <login>`, and it's ready again.
5. Finish any hand-off left by an earlier session ([Hand-offs](#hand-offs)).
6. Keep a short **session note** in your scratchpad: one row per ticket with its Lane, stage, PR, agent and model. You need it to resume agents after a usage limit, or after your conversation is summarised.

## Order of work

1. **Main's CI is red:** fix that first. Start a Standard author with the failing run, and start nothing else. A PR runs every check on Linux only, and on macOS and Windows at most the Scenarios, so main's CI is the first to run everything there: a failure only on macOS or Windows is likely the last merged PR's ([#240](https://github.com/BartoszSolkaBD/OpenDrone/issues/240)).
2. **Finish before you start:**
   - First, a PR that passed and waits to merge, or one the maintainer has just answered.
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
- **Notes for later tickets:** keep them in the session note until the PR merges. If the session ends first, they go in a hand-off ([Hand-offs](#hand-offs)).
- **Questions for the maintainer:** see below.

## When a Verdict arrives

- **Pass:** merge when every merge condition below holds.
- **Changes needed:**
  1. Start a fix round, at the tier the table gives.
  2. The fixer pushes. Then start a **new** Reviewer. Never reuse one.
  3. Sometimes every blocking item is an edge case beyond the ticket, or a hardening idea for tooling. Fix those too, since they're usually small. Note them for your end-of-batch report, so the Reviewer page's calibration can be tuned.
- **Five failed rounds:** the Review check fails and the PR gets `needs-triage`. The maintainer decided on 8 October 2026 that a PR shouldn't wait for them because its rounds ran out ([#120](https://github.com/BartoszSolkaBD/OpenDrone/issues/120)). So don't start a sixth round. Triage it instead:
  1. **File what still blocks** as one issue, to be fixed in a later run. List the fifth Verdict's blocking problems, each with its concrete fix, and link the PR and that Verdict. Title it "Left from PR #P", and label it `ready-for-agent`.
  2. **Post the triage comment** on the PR. Its first line is exactly `Triaged to #<issue>`, naming that issue:

     ```sh
     gh pr comment <P> -R BartoszSolkaBD/OpenDrone --body "Triaged to #<issue>"
     ```

     CI then passes the Review check on the commit the fifth Verdict reviewed, takes `needs-triage` off, and links the issue in the Review Report.
  3. **Merge** when every merge condition below holds. A later commit, such as main merged in, needs a fresh Verdict as usual. A failed round after the triage needs a triage of its own. The commit a triage covers counts as reviewed, so for main merged in, the Light merge-only update check is enough, as after a pass. A full Reviewer after a triage doesn't block again on what the triage issue holds.

  The Red Flag gate is separate: what it holds still waits for the maintainer.

## Merging

Merge when all of these hold:

- CI is green on the PR's latest commit.
- The Review check and the Red Flag gate both pass. The one exception: the maintainer has cleared what the Red Flag gate holds ([When a PR waits for the maintainer](#when-a-pr-waits-for-the-maintainer)).
- The PR has no `needs-maintainer` label.
- The PR changes no file under `.github/workflows/` or `.github/actions/`. This command lists the PR's changed files there, including a file's old name when it was moved. It must print nothing:

  ```sh
  gh api --paginate repos/BartoszSolkaBD/OpenDrone/pulls/<P>/files --jq '.[] | .filename, (.previous_filename // empty)' | grep -E '^\.github/(workflows|actions)/'
  ```

  If it lists any file, don't merge. Such a PR can set the Review check and the Red Flag gate itself, so the maintainer merges it by hand, as [the Reviewer page](reviewer.md) and [the Review Report's limits](../review-report.md#limits) say. Leave it to them: label it `needs-maintainer`, tell the maintainer, and carry on in the other Lanes.

**Wait for the Review check after a pass.** The Review check turns green a minute or two after a pass Verdict, once the Review workflow has run on the comment. After a changes-needed round, the Review check still shows that round's failure, not "pending", until then. So after a pass, check the conditions with this background command (`run_in_background`) rather than at once. It waits up to 10 minutes for the Review check to pass and for no check to be pending. Then it lists every check that didn't pass or skip. It prints nothing when the PR may merge:

```sh
for i in $(seq 20); do
  gh pr checks <P> -R BartoszSolkaBD/OpenDrone --json name,bucket \
    --jq '[.[] | select(.bucket == "pending" or (.name == "Review check" and .bucket != "pass"))] | length' | grep -qx 0 && break
  sleep 30
done
gh pr checks <P> -R BartoszSolkaBD/OpenDrone --json name,bucket,link \
  --jq '.[] | select(.bucket != "pass" and .bucket != "skipping") | "\(.bucket)\t\(.name)\t\(.link)"'
```

If the Review check still hasn't passed after that, the Verdict didn't count, for example because it doesn't cover the latest commit: compare its first line with `gh pr view <P> -R BartoszSolkaBD/OpenDrone --json headRefOid`. A Red Flag gate the maintainer has cleared still shows as failing, as expected.

How you merge depends on what main has done since the PR's base:

- **Nothing merged in the PR's Lanes, and GitHub shows no conflict** (`gh pr view <P> -R BartoszSolkaBD/OpenDrone --json mergeable` gives `MERGEABLE`; if it gives `UNKNOWN`, GitHub hasn't worked it out yet, so ask again): merge.

  ```sh
  gh pr merge <P> -R BartoszSolkaBD/OpenDrone --squash --delete-branch
  ```

  Then watch main's CI. If it fails, that comes first, before any other work.
- **Something merged in the PR's Lanes, but GitHub shows no conflict:** bring main in first. For a PR whose cleared Red Flag gate still fails, skip the Light check and use a fresh Reviewer instead ([Clearing the Red Flag gate](#when-a-pr-waits-for-the-maintainer)).
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

1. Post the author's notes for later tickets. Use one comment on each later ticket, starting "Notes from #N (PR #P):". If the PR has a "Delegator's hand-off:" comment, take the notes and follow-ups from it, and finish as under [Hand-offs](#hand-offs).
2. Check that the ticket closed. `Closes #N` usually closes it.
3. Remove the author's worktree, only once its PR has merged or closed. Never sooner: a later fix round needs it. Reviewer worktrees can go as soon as their Verdict is posted. If your permission settings block the removal, list the worktrees for the maintainer. Authors and Reviewers build in a shared target folder ([Setting up a build](author.md#setting-up-a-build)), but a worktree may still hold a `target/` folder of its own, 17 to 18 GB, so clear it first:
   1. Delete the worktree's `target/` folder if it has one: `rm -rf <path>/target`.
   2. Run `git worktree remove --force <path>`.
   3. If that still says "Directory not empty", run `rm -rf <path>`, then `git worktree prune`.
4. Check the shared target folder with `du -sh ~/.cache/opendrone-target`. If it is over 40 GB, delete it between batches, when no agent is building: `rm -rf ~/.cache/opendrone-target`. The next build makes it again.

## Follow-ups

A Verdict may list "Follow-ups (not blocking)". After the PR merges:

- **File them together:** one issue per PR that has any, titled "Follow-ups from PR #P". Link every Verdict the items come from. A later round often repeats an earlier round's items: list each item once.
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
3. **Move on.** Don't wait, and don't let the question hold a Lane: see below.

When the maintainer answers, in the session or on GitHub:

1. **Record the decision** where the question is, in one comment starting "Maintainer's decision (<date>):", with the choice and anything it leads to.
2. **Act on it.** Take `needs-maintainer` off, file any ticket the decision leads to, and set its blockers.

## When a PR waits for the maintainer

A PR that waits for the maintainer doesn't hold its Lane. Development carries on without it. A PR waits when the Red Flag gate holds something for the maintainer, or when a "Question for the maintainer:" on it is open.

- **A question about the ticket doesn't stop the PR.** The author has done the closest faithful thing and said so in the PR ([Building a ticket](author.md)). Move the question to its own issue, labelled `needs-maintainer` and linked from the PR. The PR then goes on through review and merge. If the maintainer later wants it done differently, that becomes a new ticket.
- **When only part of the PR waits, split it off.** Start a fix round that takes that part out, if the rest stands on its own. Examples are an edit to an existing ADR, or a Source Expectation the ticket doesn't need changed. File what came out as its own `ready-for-agent` issue, labelled `needs-maintainer`, with the question moved there. The rest goes on through review and merge.
- **When the whole PR waits,** for example on Red Flags it can't do without, it steps out of its Lane. Leave it open with `needs-maintainer`, and let the Lane's next PR go into review.
  - When the maintainer answers, it takes the Lane again as soon as the Lane is free, ahead of any new ticket.
  - You then bring main in, as under [Merging](#merging): `gh pr update-branch` and the Light merge-only update check, or a fix round and a fresh Reviewer when Results must be written again or code moved. For a PR whose cleared Red Flag gate still fails, skip the Light check and use a fresh Reviewer instead, as under [Clearing the Red Flag gate](#when-a-pr-waits-for-the-maintainer) below.
  - If the Red Flags after that are the same ones the maintainer cleared, the clearance still holds.
- **Clearing the Red Flag gate.** Nothing in the repo clears the gate: it keeps failing until the PR merges. The maintainer can merge the PR by hand, or tell you to. When they tell you, in the session or on the PR, that they clear what it holds:
  1. Record the decision on the PR.
  2. If main must come in first, bring it in and start a fresh Reviewer. That counts as a review round. Never use the Light merge-only check here: [it](merge-update.md) needs every check but the Review check to pass, so it always stops on the cleared gate.
  3. Take `needs-maintainer` off right before merging, after the last push. CI puts it back each time the Review Report runs again while the gate fails, so check it's gone just before you merge.
  4. Merge past the failing Red Flag gate. Do it only when every other condition under [Merging](#merging) holds, and only for the flags they cleared. The wait command there lists the failing gate: that row is expected.
  5. CI then gives the PR the `skipped-a-check` label and a comment naming each check that hadn't passed ([the Review Report](../review-report.md#merges-that-skipped-a-check)). Check that the comment names only the Red Flag gate. Leave the label for the maintainer to take off, and list the PR in your end-of-batch report. If the comment names any other check, tell the maintainer at once.

## When a PR can't get a green check for a CI reason

Sometimes a PR has passed review, but a check can't go green because of CI itself, not the PR's code: a job that times out or is cancelled, for example "Rust on Windows" on [#233](https://github.com/BartoszSolkaBD/OpenDrone/pull/233). The maintainer decided how this goes on 22 October 2026 ([#245](https://github.com/BartoszSolkaBD/OpenDrone/issues/245)).

1. **Find out once whether the PR is at fault.** You may rerun the failed job one time, to learn whether it fails again. Never loop reruns, and never decide that a failure is "just a flake".
2. **File the cause as its own ticket,** labelled `ready-for-agent` and with its Lane label. Put the evidence in it: the run link, and what was seen (which job, how long it ran, how it ended). Set blocked-by where it applies.
3. **Step the PR out of its Lane.** Label it `needs-maintainer`, link the cause ticket from it, and treat it as under [When a PR waits for the maintainer](#when-a-pr-waits-for-the-maintainer). The Lane's next PR goes into review.
4. **When the cause ticket has merged,** bring main in, as under [Merging](#merging). The PR takes its Lane again as soon as the Lane is free, ahead of any new ticket.

When the cause is a workflow change, the PR that fixes it changes `.github/workflows/` or `.github/actions/`, so the maintainer merges it by hand ([Merging](#merging)).

This is not the clearance of the Red Flag gate ([Clearing the Red Flag gate](#when-a-pr-waits-for-the-maintainer)): that is only for what the gate holds.

**Which checks a PR runs.** From [#240](https://github.com/BartoszSolkaBD/OpenDrone/issues/240): a PR runs Linux in full, and macOS and Windows only when the PR can change a flight result; main runs all three. #240 isn't on main yet, so its PR confirms the exact wording.

## Usage limits

- When a usage limit hits, the agents stop. After the reset, resume each one from your session note with SendMessage: "Continue where you left off."
- Near a limit, start no new agents. Let the running ones finish.

## Hand-offs

The session note dies with its session, but a PR that waits for the maintainer can outlive it. So its notes for later tickets and its follow-ups go on the PR.

- **When a session ends,** post a comment on each open PR that has passed review and waits for the maintainer. Start it "Delegator's hand-off:", and give:
  - the author's notes for later tickets, grouped by ticket;
  - the follow-ups to file after the merge, each with its Verdict's link.
- **When the PR merges,** whoever merged it, post the notes and file the follow-ups as [After a merge](#merging) and [Follow-ups](#follow-ups) say. Then comment "Hand-off done:", naming the tickets that got notes and the follow-up issue.
- **When a session starts,** finish any hand-off whose PR merged meanwhile, for example one the maintainer merged by hand. This lists merged PRs with a hand-off and no "Hand-off done" yet:

  ```sh
  gh pr list -R BartoszSolkaBD/OpenDrone --state merged --search "\"Delegator's hand-off\" in:comments" --limit 50 \
    --json number,comments --jq '.[] | select(([.comments[].body | startswith("Delegator'"'"'s hand-off")] | any) and ([.comments[].body | startswith("Hand-off done")] | any | not)) | .number'
  ```

## Ending a batch

Report to the maintainer:

- what merged, as ticket and PR;
- what waits for them, and why, including each PR that stepped out of its Lane, and each PR merged past a cleared Red Flag gate whose `skipped-a-check` label they should take off;
- the follow-up issues you filed, and each issue you triaged a PR to;
- each PR you left a hand-off on;
- for each PR, the author's model and its number of review rounds;
- the model the Delegator itself ran on, for the trial;
- any Lane labels you added or fixed.

## Reference sources

Authors who copy firmware behaviour read its source. Keep the source outside the repo, in one folder all worktrees share, and give authors its path in their prompt. Never copy GPL code into the repo: reimplement the behaviour ([ADR-0014](../adr/0014-licences-for-libraries-and-assets.md)).

```sh
mkdir -p ~/OpenDrone-refs && cd ~/OpenDrone-refs
for v in 2026.6.2 2025.12.1 4.5.0 4.4.0 4.3.0; do
  git clone -q --depth 1 --branch "$v" https://github.com/betaflight/betaflight.git "betaflight-$v"
done
git clone -q --depth 1 --branch v0.21.0 https://github.com/bird-sanctuary/bluejay.git bluejay-v0.21.0
```
