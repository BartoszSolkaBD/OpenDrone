# The OSD is worked out beside the Simulation, from the Flight Controller's state

The goggles show Betaflight 2026.6's OSD: battery, timers, Flight Mode, warnings and the statistics page, copied from Betaflight's own logic. That logic lives with our Flight Controller code. It runs 12 times a second in Simulation Time, as Betaflight's OSD does, but beside the Simulation rather than inside it. It reads each tick's Flight Controller state and the pilot's OSD layout, and it hands out the OSD screen: which character sits in which cell, and what each element reads. Nothing it does feeds back into a flight. The game and the Scenario runner both run it the same way.

We chose this so that Scenarios can check what the pilot sees, for example that `FAIL SAFE` appears 1.5 s after the Radio Link is lost. That keeps behaviour reviewable without reading code, and an agent PR that breaks a warning fails a Scenario. Keeping the OSD outside the Simulation means the OSD layout, a pilot setting that never changes a flight, doesn't join the starting state that every Scenario spells out ([ADR-0002](0002-scenario-starting-state-spells-out-everything.md)). In multiplayer, each pilot works out only their own OSD. Decided in [#31](https://github.com/BartoszSolkaBD/OpenDrone/issues/31).

## Considered options

- **Inside the Simulation.** The OSD layout would join the pilot's settings handed to the Simulation, like Rates. Rejected: every Scenario's starting state would have to spell out an OSD layout that never changes a flight. In multiplayer, every machine would also work out every pilot's OSD.
- **In the game, from the Flight Controller's debug record.** Rejected: only plain readable tests could check it, never Scenarios. Betaflight's warning order and timers would also have to be copied into the game, away from the Flight Controller code they belong with.

## Consequences

- **The Flight Controller's output each tick gains what Betaflight's OSD reads.** That's why arming is blocked, the Failsafe phase, the battery state (the filtered voltage, cell count, and LOW or CRITICAL), Crash Flip state and the beeper. These are added to the Flight Controller's output in [ADR-0003](0003-crate-split-and-flight-inputs.md)'s table.
- **Scenarios.** A Scenario that expects OSD text adds an `[osd]` section spelling out the OSD options its Expectations depend on: the timers, which warnings and statistics are on, and the alarms. Its Expectations name elements, such as "the warnings element reads `FAIL SAFE`", never grid cells, so it needs no layout. Other Scenarios never mention the OSD.
- **Timers count Simulation Time.** They stop while the Pause Menu is open, and start from zero when Reset, a new Map or a new Quad powers the Flight Controller up fresh.
- **The OSD stays deterministic.** It's worked out from Simulation Time and tick states, never from the computer's clock, so a replayed flight shows the same OSD.
- **Drawing it is the game's job.** Analog draws the screen inside the merged camera pass, before grain and Breakup. Digital draws it crisp, as late as its picture. The game keeps the last few OSD screens for that.
