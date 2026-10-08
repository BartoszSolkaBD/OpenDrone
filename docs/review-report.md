# Reading the Review Report

Every pull request gets one **Review Report**: a comment from CI (`github-actions`) that stays up to date, so you can judge the PR in one place. The terms are in the [Development deep dive](context/development.md). The Reviewer's side, with the exact Verdict format, is in [The Reviewer and the Verdict](agents/reviewer.md).

| Section | What it shows |
|---|---|
| **Verdict** | The Review check: whether the newest Verdict covers the latest commit and says pass. It also gives a warning naming any earlier merge that skipped a check. |
| **Red Flags** | Changes that could weaken a check or a decision. Some wait for you, some the Reviewer decides, and some are only listed. |
| **What moved** | Every Expectation whose measured value moved, biggest move first, before and after, from the Scenarios' Results files. |
| **Speed** | Work Counts against main. Not measured yet: they arrive with #78 and #79. |
| **Areas touched** | The Areas whose folders the PR changes, with any new outside library and its licence. |
| **Renders** | Pictures of every changed Map. None yet: they arrive with #63. |
| **Downloads** | Blackbox logs and builds. None yet: they arrive with #55 and #81. |

The whole Report is worked out again after every push, and whenever the maintainer's account comments on the PR.

## Red Flags

The **Red Flag gate** is a required check. It fails when a Red Flag waits for you. CI then adds the `needs-maintainer` label, and the PR can't merge by itself. For a PR that changes CI's workflows, the gate's result can't be trusted; see Limits below.

| Red Flag | Level | How CI spots it |
|---|---|---|
| A Source or Rule Expectation changed, including a loosened tolerance | **Waits for you** | It compares every Scenario file with where the PR branched off main. Any change to such an Expectation counts: its value or tolerance (even a tighter one), its Basis, any other field, or removing it. Expectations are matched by what they measure and when, with the moment read as a time, so "1 s" and "1.0 s" are the same. Reordering them, respacing the file, or adding a new one doesn't count. When a Scenario file is deleted, its Expectations are looked for in every Scenario the PR adds, whatever its file or name: one found unchanged has only moved, and a Source or Rule one found changed, or nowhere, waits. |
| An existing ADR edited | **Waits for you** | Any change to a file in `docs/adr/` that exists on main: an edit, a rename or a deletion. |
| Bevy moving to a new 0.N, with its wgpu major | **Waits for you** | It compares `Cargo.lock` with main. A patch release, a release candidate becoming the release, or Bevy's first arrival doesn't count. |
| A Scenario's setup changed under its Source or Rule Expectations | The Reviewer decides | In a Scenario that holds Source or Rule Expectations, any field other than its Expectations, its name and its format changed: its starting state, its inputs, an `[osd]` option. So did the Test Quad it flies, if the PR changes that Test Quad's file, even when the Scenario itself is untouched. The changed fields are listed. When a deleted Scenario's Expectations turn up in several new files, the setup of each new file holding one of its Source or Rule Expectations is compared with the old one. Those Expectations then check a different flight. It doesn't wait for you, because the format migration tool (#60) adds new starting-state items to every Scenario. |
| A deleted Scenario | The Reviewer decides | A Scenario file is gone, and not every one of its Expectations turns up in a Scenario the PR adds. Its Source and Rule Expectations also wait for you, as above, so this alone applies to a Scenario with only Observed ones. |
| A loosened tolerance on an Observed Expectation | The Reviewer decides | The range it accepts got wider, compared in the same units, so "± 1 mm/s" is the same as "± 0.001 m/s". Removing an Observed Expectation counts here too. |
| A house-rule exception in a core crate | The Reviewer decides | In a core crate: a newly allowed or expected `clippy::disallowed_*` lint, or a group that holds them (`clippy::style`, `clippy::all`, `clippy::restriction`, `clippy`, `warnings`), however the attribute is spread over lines and whatever comes before it on its line; a change to the `[lints]` in its `Cargo.toml`; a change to its `clippy.toml`; or a `.clippy.toml`, which Clippy reads instead of `clippy.toml`, so even an empty one turns the house rules off. The walls check (`cargo xtask walls`) also refuses a `.clippy.toml` in a core crate. |
| New `unsafe` code | The Reviewer decides | The word `unsafe` in a new line of Rust code. Comments and literals are found with rustc's own lexer, in the copy that is Rust 1.99's (`ra-ap-rustc_lexer` 0.174.0). They are read as rustc 1.99 reads a file in its crate's edition (from the crate's `Cargo.toml`), a skipped `#!` first line included. So a comment or a literal hides code, or passes for it, only where it does for rustc itself. What it can't see: a file one crate pulls in from another with `include!` or `#[path]`, which rustc reads in the other crate's edition, and an edition set on a single target such as `[lib]`. The same reading finds the house-rule exceptions above. It also covers a newly allowed `unsafe_code` lint, a crate's own `unsafe_code` setting, a crate other than `opendrone-input` and the game that stops taking the workspace's lints (which forbid `unsafe`), and a change to the workspace's `unsafe_code = "forbid"`. |
| A change to CI workflows | The Reviewer decides | Any file in `.github/workflows/` or `.github/actions/`. The Report says plainly that, for this PR, the Red Flag gate's and the Review check's own results can't be trusted (see Limits). The Reviewer checks that no workflow asks for `statuses: write` or `permissions: write-all`, or sets a status. |
| A change to the Repo rules | The Reviewer decides | Any file in the Repo rules Area of main's `.github/CODEOWNERS`: CI, the root Cargo files, `.cargo/`, the Rust version (`rust-toolchain.toml` or the older `rust-toolchain`), lint and format settings (a crate's `clippy.toml` or `.clippy.toml` included), the licence policy, CODEOWNERS, xtask and `AGENTS.md`. |
| An Observed Expectation updated | Listed | Its value changed. The new basis line is its one-line reason. |
| A new outside library | Listed | A library in the PR's `Cargo.lock` whose name main's lacks, with its licence as crates.io gives it (at most 30 are looked up). cargo-deny checks the licence. |
| A new ADR, or a new glossary term | Listed | A new file in `docs/adr/`, or a new `**Term**:` line in a deep dive. |

**When a Red Flag waits for you:** read it, then either merge the PR by hand or ask for changes. A merge by hand goes past a failing check, so the Review Reports after it name that merge (see below).

## What moved

The Results files hold each measured value to 3 significant figures, and they're identical on every computer. So any value that moved shows here, even inside its tolerance. The **Move** column gives the change as a share of the value before. A change from zero, or one that isn't a number (such as a broken number), sorts first. New Expectations, ones no longer measured, and Scenarios whose fingerprints moved are listed below the table. A fingerprint shows a move too small to reach 3 significant figures.

## Areas and labels

The Areas come from the comment above each block in main's `.github/CODEOWNERS`, and a file belongs to the last block that matches it, as in CODEOWNERS. Each Area the PR touches becomes a label such as `area: Physics`. A label for an Area the PR no longer touches comes off. Files only the catch-all `*` covers have no Area.

## The Review check

The Review check is a commit status on the PR's latest commit, set only for PRs into main (see below). It **passes** only when all three hold:

- the newest Verdict covers that commit and says pass;
- the PR was opened by the maintainer's account or by Dependabot, from a branch in this repo;
- fewer than 3 review rounds have failed.

It **waits** while no Verdict covers the latest commit. It **fails** otherwise. After 3 failed rounds the PR also gets the `needs-maintainer` label. Only comments from the maintainer's account count as Verdicts.

## Merges that skipped a check

After every merge into main, CI checks that the merged PR's last commit had passed every check main requires. A check missing from its last commit counts as skipped. If one hadn't passed, the PR gets the `skipped-a-check` label and a comment naming each one. Every Review Report then warns about it, under its Verdict, until you take that label off. Agents never merge past a failing check, but in Phase 1 they hold your rights, so this is how a slip shows ([ADR-0010](adr/0010-phase-1-agent-prs-merge-automatically.md)).

## How it's built, and why it's safe

The workflow that posts the Report can write to the PR, so it must never run anything the PR controls. Otherwise a PR could change how it is judged, or use that write access. So everything happens in one workflow, **`.github/workflows/review.yml`**, which only ever runs main's code.

**It runs:**

- after every push to a PR, when a PR is opened, reopened or closed, and when a PR's base branch changes (`pull_request_target`). A closed PR's own Report stays as it is; only the PRs that had its commit are judged again (see below);
- whenever the maintainer's account comments on a PR (`issue_comment`);
- after a merge into main (`push`).

For all three, GitHub runs the workflow file as it is on main, even when the PR changes it.

**Trusted, because it comes from main:**

- the workflow and its script, `.github/scripts/review.sh`;
- xtask, built from main's checkout with main's Rust version;
- main's `.github/CODEOWNERS`, which decides the Areas, the labels and the Repo rules flag;
- the rules for every Red Flag.

**Data only, never run, built or checked out:**

- **The PR's latest commit.** It is fetched by its SHA as git objects and read only with `git show`, `git diff` and `git ls-tree`, against the commit where the PR branched off main. No `cargo`, `rustup` or other tool ever runs on the PR's files, so its `rust-toolchain`, `.cargo/config.toml` and build scripts have no effect on this workflow.
- **The PR's comments and details,** from GitHub's API. Only the maintainer's account's comments count as Verdicts.
- **The new libraries' licences,** from crates.io's API. The names and versions come from the PR's `Cargo.lock`, so they are checked before they go into a web address: a name of letters, digits, `-` and `_`, and a version that starts with a digit and holds no `..`.

**Every piece of text from the PR or crates.io is escaped before it reaches the Report.** That covers file names, Scenario text and reasons, lines of code, library names and licences. So it can't start a heading, open HTML, forge the Report's hidden marker, fake a Verdict section or mention anyone. Invisible formatting characters, such as U+202E, which shows the text after it right to left, appear as their codes.

**What the workflow posts:**

- **The Red Flag gate,** as a commit status on the PR's latest commit: failure when a Red Flag waits for you.
- **The Review check,** as a commit status on the same commit.
- **Both go pending first.** As soon as a run has read GitHub's record of the PR and knows it merges into main, before it fetches anything, it sets both statuses to pending, and to error if the run then fails. A run that times out or is cancelled leaves them pending. So neither passes by accident, and no older result stays on the commit.
- **If GitHub's record of the PR can't be read,** a run started by a PR event marks the commit that event named with error, when the event says the PR merges into main. A run started by a comment names no commit, so it changes no status, and its failed run is the trace.
- **The Review Report comment.** It is found by its hidden marker, and only if CI posted it.
- **The labels.** Only Areas in main's CODEOWNERS become labels.

**Which PRs set the two statuses:**

- A status belongs to a commit, not to a PR. So only a PR into main sets them.
- A PR into any other branch still gets a Report, which says it isn't judged, but it never sets either status. It is judged afresh when its base changes to main.
- If two open PRs into main have the same latest commit, both statuses fail. A PR built on another PR's branch doesn't count: its latest commit is a later one.
- When one of those PRs closes, gets a new commit or moves off main, the others are judged again at once, each in its own run. So is an open PR into main when another PR into main arrives with its latest commit. If GitHub can't say which PRs have that commit, the job that asks fails and shows on the PR, rather than judging nobody quietly.

No job is named "Red Flag gate" or "Review check", so a job in this workflow can't pass for either status.

Branch protection (#83) should require **Red Flag gate** and **Review check**, beside the CI checks in [`CONTRIBUTING.md`](../CONTRIBUTING.md).

You can work out the Report locally: `cargo xtask review-report --base origin/main --head HEAD --out report` prints the Red Flags, fails if one waits for you, and writes the Report to `report/report.md`.

## Limits

**A PR that changes CI's workflows can set both statuses itself.**

- Any workflow with `statuses: write`, or with `permissions: write-all`, which grants it, can set any commit status, the Red Flag gate and the Review check included.
- A PR from a branch in this repo can change `ci.yml`, or add a workflow, that asks for that permission and sets both statuses to success after this workflow has run. That would get past the gate and a Reviewer's changes needed, and the merge check couldn't tell.
- Forks and Dependabot can't do this: their workflows get a read-only token.
- Nothing inside the repo can fully stop it. So the Report flags every change to `.github/workflows/` or `.github/actions/` and says the PR's statuses can't be trusted. The Reviewer judges such a PR from its diff, and the maintainer should merge it by hand.
- In Phase 1, agents hold the maintainer's admin rights anyway, so this is a rule agents follow, like "never merge past a failing check" (ADR-0010).

**Guards GitHub can add, for #83:**

- **Statuses only from one source.** Post the two statuses with a dedicated GitHub App, whose key lives in an environment that only `main` may use. Then set branch protection to require each status from that App. A PR's own workflow then can't make a status that counts. This is the strongest guard.
- **Code Owner review for workflows.** Require it for `/.github/workflows/` and `/.github/actions/` only. GitHub never lets an author approve their own PR, so such PRs then merge only by the maintainer's hand. This fits Phase 2, when CODEOWNERS starts to be enforced.
- **A ruleset that limits who may change workflow files,** if the plan allows it.

**Other limits:**

- A change to the review workflow, its script or xtask takes effect only once it is on main. Until then the PR is judged by main's copy. That is why such a change is a Repo rules change: the Reviewer judges it from the diff, not from the Report.
- CI (`ci.yml`) runs the PR's code, as it must. Unchanged, it has a read-only token, and nothing it produces reaches the Red Flag gate or the Report.
- If a run stops early, for example if GitHub doesn't have the PR's latest commit yet, that commit keeps its pending or error statuses until the next push or comment. That fails closed: the PR waits. A comment's run that can't read the PR's record at all changes nothing; its failed run is the trace.
- Dependabot's own runs get a read-only token. So on a Dependabot PR, the Report and both statuses appear once the maintainer's account comments.
