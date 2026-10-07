# OpenDrone

OpenDrone is a free, open-source FPV drone flight simulator. Its flight physics stay faithful to real-world quads, and easier flying comes only from optional Assists layered on top, never from changing the physics underneath.

This book explains OpenDrone to pilots, to its maintainer and to the agents that build it. It's made from the `docs/` folder of the [repo](https://github.com/BartoszSolkaBD/OpenDrone), so each page here is the same file the agents read. It has seven parts:

1. **[Pilot guide](pilot-guide.md):** what OpenDrone is for a pilot, and what the first playable version will have.
2. **[How OpenDrone works](book/map.md):** the map of how OpenDrone is described and built, a deep dive for each topic with its terms and rules, and the unit list every number in OpenDrone's files uses.
3. **[Decisions](book/decisions.md):** each decision that's hard to undo, and why it was made.
4. **[Proving the physics](book/proving-the-physics.md):** how Scenarios pin down what the simulator does, the catalogue of every Scenario, and the Feel Tests.
5. **[Research](research/README.md):** the research behind the decisions, with its sources.
6. **[Working on OpenDrone](book/working-on-opendrone.md):** how a change reaches the main branch, setting up, CI, and what data may enter the repo.
7. **[The code](book/the-code.md):** the reference for every crate, made by rustdoc.

OpenDrone is being built towards its first playable version, the 0.1.0 alpha, planned in [the spec](https://github.com/BartoszSolkaBD/OpenDrone/issues/37). There's nothing to fly yet.
