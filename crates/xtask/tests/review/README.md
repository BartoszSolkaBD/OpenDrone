# Fixtures for the Review Report's readable checks

`repo/` is a small copy of the repo as it stands before a pull request: three
Scenarios (two with their Results), an ADR, a deep dive, a few crates and a
`Cargo.lock`. The checks in `crates/xtask/tests/review_report.rs` copy it,
swap in files from `changes/` the way a pull request would, and run
`cargo xtask review-report` on the two copies. The real `.github/CODEOWNERS`
is copied in too, so the Areas are the real ones.

Code, Markdown and manifest fixtures end in `.rs.txt`, `.md.txt` and
`Cargo.toml.txt`, and the checks drop the `.txt`. So the Red Flag gate never takes them for this repo's own
code, and a change to one is never a docs-only change that would skip the
checks (see `.github/scripts/changes.sh`).
