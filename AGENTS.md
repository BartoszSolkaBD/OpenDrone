# AGENTS.md

## Agent skills

### Issue tracker

Issues are tracked in GitHub Issues on `BartoszSolkaBD/OpenDrone`, via the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

Default vocabulary: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context. `CONTEXT.md` is a map that links to topic deep dives in `docs/context/`; ADRs live in `docs/adr/`. See `docs/agents/domain.md`.

### Docs site

`docs/` is also an mdBook, published to GitHub Pages. Every Markdown page under `docs/` must be listed in `docs/SUMMARY.md`, and every link must lead somewhere real; `cargo xtask book` builds the book with rustdoc and checks both. See `CONTRIBUTING.md`.

### Pull requests and review

One PR per ticket. Every PR gets a Reviewer, a fresh agent with none of the author's conversation, before it merges, and CI keeps one Review Report comment up to date on it. `docs/agents/reviewer.md` says how to start a Reviewer, what it reads and checks, and the exact Verdict format: one PR comment whose first line is `Reviewed commit <full SHA>` and whose last line is exactly `Verdict: pass` or `Verdict: changes needed`. How to read the Review Report and its Red Flags: `docs/review-report.md`.

### Delegating tickets

A **delegator** session works through the ready tickets. It starts an author for each ticket and a Reviewer for each PR, each on the model its job needs, keeps one PR in review per Lane, and merges what passes. To run a batch, start a session and point it at `docs/agents/delegator.md`. Authors follow `docs/agents/author.md`. After main is merged into a PR that already passed, a short check is enough: `docs/agents/merge-update.md`.
