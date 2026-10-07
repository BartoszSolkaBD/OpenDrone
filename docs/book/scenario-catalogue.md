# The Scenario catalogue

Every Scenario in the repo, made from the files in `scenarios/` each time the book is built, so it's never out of date. Each Scenario shows its kind, the Quad and the Map it names, and each Expectation with its Basis: **Source** (a cited outside reference), **Rule** (worked out from physics, with the working shown) or **Observed** (what the sim did when the Expectation was written). Source and Rule Expectations are locked: if the sim disagrees, the sim gets fixed.

The catalogue reads each Scenario with the Scenario runner's own code, so it lists exactly what the runner runs. The runner runs them in CI on macOS, Windows and Linux, and writes each one's measured values to the Results file beside it ([how to read them](../verification/reading-a-scenario.md)). To print this page without building the book, run `cargo xtask scenario-catalogue`.

{{#scenario-catalogue ../../scenarios}}
