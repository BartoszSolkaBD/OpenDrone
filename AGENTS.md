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
