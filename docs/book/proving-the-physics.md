# Proving the physics

OpenDrone's behaviour can be reviewed without reading its code. Each behaviour is pinned by **Scenarios**: readable TOML files in which every number carries its unit and sticks are in percent ([Verification](../context/verification.md)).

- **A Scenario** is a starting state that spells out everything ([ADR-0002](../adr/0002-scenario-starting-state-spells-out-everything.md)), inputs from a Timeline or an Input Track, and Expectations about what happens. It's one of four kinds: Flight, Thrust Stand, Flight Controller or Physics.
- **Every Expectation has a Basis:** a cited **Source**, a **Rule** worked out from physics with the working shown, or what the sim was **Observed** to do when it was written. Source and Rule Expectations are locked: if the sim disagrees, the sim gets fixed. Observed ones may be updated, each with a one-line reason.
- **The Results file** beside each Scenario holds the measured value of every Expectation. It's committed, so a pull request shows every value that moved, even inside its tolerance.
- **The same flight everywhere.** The Simulation is bit-exact on every computer ([ADR-0001](../adr/0001-bit-exact-determinism-with-ordinary-floats.md)), and CI runs every Scenario on macOS, Windows and Linux and checks that all three agree.
- **Feel Tests** judge what numbers can't. The maintainer flies the Whoop 65 against the real quad it's modelled on, following [the checklist](../feel-test-checklist.md), and moves only Estimates, inside their ranges, logging every change.

This part holds [the catalogue of every Scenario](scenario-catalogue.md), [the Feel Test checklist](../feel-test-checklist.md) and [each Quad's Feel Test log](feel-test-logs.md). How numbers and tolerances are written is in [the unit list](../units.md).
