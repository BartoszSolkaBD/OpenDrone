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

The Verdict section updates whenever the maintainer's account comments on the PR. The rest updates after every push.

## Red Flags

The **Red Flag gate** is a required check. It fails when a Red Flag waits for you. CI then adds the `needs-maintainer` label, and the PR can't merge by itself.

| Red Flag | Level | How CI spots it |
|---|---|---|
| A Source or Rule Expectation changed, including a loosened tolerance | **Waits for you** | It compares every Scenario file with main. Any change to such an Expectation counts: its value or tolerance (even a tighter one), its Basis, any other field, or removing it. Expectations are matched by what they measure and when, so reordering them, respacing the file or moving it to another folder doesn't count, and neither does adding a new one. |
| An existing ADR edited | **Waits for you** | Any change to a file in `docs/adr/` that exists on main: an edit, a rename or a deletion. |
| Bevy moving to a new 0.N, with its wgpu major | **Waits for you** | It compares `Cargo.lock` with main. A patch release, a release candidate becoming the release, or Bevy's first arrival doesn't count. |
| A deleted Scenario | The Reviewer decides | A Scenario file is gone, and no new file has its name. A Scenario that moved isn't deleted. |
| A loosened tolerance on an Observed Expectation | The Reviewer decides | The range it accepts got wider, compared in the same units, so "± 1 mm/s" is the same as "± 0.001 m/s". Removing an Observed Expectation counts here too. |
| A house-rule exception in a core crate | The Reviewer decides | A new `allow` or `expect` of a `clippy::disallowed_*` lint in a core crate, or a change to a core crate's `clippy.toml`. |
| New `unsafe` code | The Reviewer decides | The word `unsafe` in a new line of Rust, outside comments and quoted text, or a change to the `unsafe_code` setting. |
| A change to the Repo rules | The Reviewer decides | Any file in the Repo rules Area of `.github/CODEOWNERS`: CI, lint settings, the licence policy, CODEOWNERS, the Rust version, xtask and `AGENTS.md`. |
| An Observed Expectation updated | Listed | Its value changed. The new basis line is its one-line reason. |
| A new outside library | Listed | A library in the PR's `Cargo.lock` whose name main's lacks, with its licence. cargo-deny checks the licence. |
| A new ADR, or a new glossary term | Listed | A new file in `docs/adr/`, or a new `**Term**:` line in a deep dive. |

**When a Red Flag waits for you:** read it, then either merge the PR by hand or ask for changes. A merge by hand goes past a failing check, so the Review Reports after it name that merge (see below).

## What moved

The Results files hold each measured value to 3 significant figures, and they're identical on every computer. So any value that moved shows here, even inside its tolerance. The **Move** column gives the change as a share of the value before. A change from zero, or one that isn't a number (such as a broken number), sorts first. New Expectations, ones no longer measured, and Scenarios whose fingerprints moved are listed below the table. A fingerprint shows a move too small to reach 3 significant figures.

## Areas and labels

The Areas come from the comment above each block in `.github/CODEOWNERS`, and a file belongs to the last block that matches it, as in CODEOWNERS. Each Area the PR touches becomes a label such as `area: Physics`. A label for an Area the PR no longer touches comes off. Files only the catch-all `*` covers have no Area.

## The Review check

The Review check is a commit status on the PR's latest commit. It **passes** only when all three hold:

- the newest Verdict covers that commit and says pass;
- the PR was opened by the maintainer's account or by Dependabot, from a branch in this repo;
- fewer than 3 review rounds have failed.

It **waits** while no Verdict covers the latest commit. It **fails** otherwise. After 3 failed rounds the PR also gets the `needs-maintainer` label. Only comments from the maintainer's account count as Verdicts.

## Merges that skipped a check

After every merge into main, CI checks that the merged PR's last commit had passed every check main requires. A check missing from its last commit counts as skipped. If one hadn't passed, the PR gets the `skipped-a-check` label and a comment naming each one. Every Review Report then warns about it, under its Verdict, until you take that label off. Agents never merge past a failing check, but in Phase 1 they hold your rights, so this is how a slip shows ([ADR-0010](adr/0010-phase-1-agent-prs-merge-automatically.md)).

## How it's built, and why it's safe

A workflow that can write to a PR must never run the PR's own code, or a PR could use that access. So the work is split in two:

- **`.github/workflows/review-report.yml`** runs on every PR with a read-only token and no secrets. It reads the PR's files as data, with `git show`, and runs `cargo xtask review-report`. That writes the Report's sections from Red Flags down, and fails when a Red Flag waits for you: this job is the **Red Flag gate**. It builds the gate from main's code, so a PR can't change how it is judged. Only while main has no gate yet does the PR's own run. It leaves the Report as a download.
- **`.github/workflows/review.yml`** has write access. It runs only as it is on main: after the Review Report workflow finishes, when the maintainer's account comments, and after a merge. It checks out main, builds main's xtask, and reads the PR through GitHub's API. The download is the one thing a PR can shape, so xtask treats it as text only. It must be for the PR's latest commit, it is cut short if it's long, and anything that could pass for the Report's hidden marker is stripped. Only Areas in main's CODEOWNERS become labels. The workflow then sets the Review check, posts or edits the Report (found by its hidden marker, and only if CI posted it), and adds the labels.

Branch protection (#83) should require **Red Flag gate** and **Review check**, beside the CI checks in [`CONTRIBUTING.md`](../CONTRIBUTING.md).

You can work out the Report locally: `cargo xtask review-report --base origin/main --head HEAD --out report` prints the Red Flags, fails if one waits for you, and writes the Report to `report/report.md`.

**Limits:**

- A PR can still edit `review-report.yml` itself. That is a change to the Repo rules, and the Reviewer decides on it.
- Dependabot's own runs get a read-only token. So on a Dependabot PR, the Report and the Review check appear once the maintainer's account comments.
