# In Phase 1, agent PRs merge automatically on a public repo

The repo is public from the start. In Phase 1 only the maintainer and their agents contribute. Agents work under the maintainer's GitHub account, and their PRs merge by themselves once every required check passes. Nobody reads the code before it merges. The maintainer doesn't read Rust, so a human approval click would add a wait without adding a check.

The protection comes from two places:

- **CI gates:** the Scenario checks, the crate walls, the licence policy, the Work Count gates and a Red Flag gate. Anything that could make a failing check "pass", such as editing a Source Expectation, waits for the maintainer.
- **An independent Reviewer:** a fresh agent with none of the author's conversation. It reads the ticket, the change and the repo's rules. A required Review check passes only when its Verdict covers the latest commit and says pass.

Phase 2 starts when the repo accepts outside contributions, or at the 0.1.0 alpha, whichever comes first. A new grilling designs its approval rules then. Decided in [#15](https://github.com/BartoszSolkaBD/OpenDrone/issues/15).

## Considered options

- **Stay private until the alpha.** Rejected on cost:
  - A private repo on GitHub Free gets 2,000 CI minutes a month, on 2-CPU machines, with no branch protection.
  - A macOS minute costs $0.062 against $0.006 on Linux.
  - Our three-OS Scenario CI comes to roughly $1–2 a run. The free allowance covers fewer than ten runs.

  Public repos get standard runners free and unlimited, on 4-CPU machines, plus branch protection and Pages.
- **The maintainer approves every PR.** Rejected for Phase 1. It's a click without a reading. GitHub also never lets an author approve their own PR, and agents work under the maintainer's account.
- **A separate GitHub identity for agents now,** a GitHub App or a machine account, so GitHub itself would require the maintainer's approval. Deferred to Phase 2, where it's the obvious starting point.
- **Collaborators only, through GitHub's interaction limit.** Not chosen, because the maintainer wants issues and PRs open to everyone. Switching it on later takes a minute.
- **A review bot in CI.** Deferred. The Verdict format is fixed now, so a CI bot can post the same Verdict later.

## Consequences

- **Agents hold the maintainer's admin rights in Phase 1.**
  - "Waits for the maintainer" and "never merge past a failing check" are rules agents follow, not limits GitHub enforces.
  - The next Review Report names any merge that skipped a check.
- **Strangers can't use the automatic path.**
  - Agents act only on text written by the maintainer's account.
  - The Review check fails for any PR not opened by the maintainer's account or by Dependabot from a branch in this repo.
- **Phase 1 keeps the door to Phase 2 open.** CODEOWNERS is written by Area, and branch protection is on, with required checks, squash merges and auto-merge, but no required approvals.
- **Everything is visible from day one:** spec tickets, half-built code and the maintainer's commit email in the existing history. New commits use GitHub's noreply address.
