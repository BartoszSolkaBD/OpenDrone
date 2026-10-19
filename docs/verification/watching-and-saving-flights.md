# Watching a Scenario and saving a flight

Two developer tools in the game, with no menu entry ([#23](https://github.com/BartoszSolkaBD/OpenDrone/issues/23)): one opens a Scenario in the game so you can watch what it does, and one, **Save Flight**, turns a flight you flew into a Scenario. The terms are in the [Verification](../context/verification.md) deep dive; the files are explained in [Reading a Scenario and its Results](reading-a-scenario.md).

## Watching a Scenario

```sh
cargo run -p opendrone -- --scenario scenarios/quads/whoop-65/crash-flips-upright-on-a-hard-floor.toml
```

The game opens in its normal FPV view, on the Scenario's Quad and Map, and the Scenario runs: nobody flies it. A Test Map shows its plain grid floor, as in every flight.

- **Which Scenarios.** A Flight Scenario or a Thrust Stand Scenario, from a file inside `scenarios/`, with its Input Track beside it if it plays one. A Physics or Flight Controller Scenario is refused, saying so: those two have no Quad flying in a Map to watch.
- **Pause.** Esc (or the Options button on a DualSense) stops the Simulation, and a second press goes on, as in any flight. The camera keys work as always: up and down for the Camera Tilt, right and left for the FOV.
- **Nothing else flies it.** No Input Device starts a flight, and Reset does nothing. The window's title says the Scenario's name and how far into it the Simulation is.
- **It's the flight the runner measures.** The game sets the Scenario up exactly as the headless runner does and steps it with its own stepping, so what you watch ends on the fingerprint in the Scenario's Results file. When the Scenario's last moment comes, the Quad stays where it was.
- **Other options.** `--look`, `--aspect`, `--tilt`, `--fov` and `--chase` work as in a normal flight, and `--screenshot FILE.bmp` saves a picture after a while, for a machine nobody is watching. `--scenario` names its own Quad, so it doesn't go with `--quad`.

## Save Flight

Save Flight is an Action, like Reset and Pause, with **no default key**: you bind it to a button or a switch in your Input Device profile, with the name `save_flight` in `[actions]` ([Checking a Pack](checking-a-pack.md#an-input-device-profile-input-devicesidtoml)). The keyboard has no key for it.

When it fires, the game writes two files into `scenarios/saved/`, named after the Quad, such as `whoop-65-saved-flight-1.toml` and `whoop-65-saved-flight-1.csv`. It never overwrites a saved flight: the next one is number 2. It says what it did in the window's title for a few seconds, and in the log.

- **The Scenario** is a Flight Scenario that starts landed at the Launch Spot, as Reset leaves the Quad, with every item of its starting state spelled out from the flight's own settings ([ADR-0002](../adr/0002-scenario-starting-state-spells-out-everything.md)): the Quad, the Map, the Rates, the Assists, the Packet Rate, the physics rate and the random seed.
- **The Input Track** holds every Flight Input the game handed the Simulation, each at its Simulation Time: the Channels, the Flying Input Device lost and back, and Reset. How the file is written is in [An Input Track](reading-a-scenario.md#an-input-track). The Scenario's `input_device` says the Flying Input Device's Report Rate and whether it reports at rest, so the Radio Link behaves as it did in the game.
- **The same fingerprint as in the game.** The headless runner flies the saved Scenario to the same flight, bit for bit, so every step's fingerprint is the game's ([ADR-0001](../adr/0001-bit-exact-determinism-with-ordinary-floats.md)). Readable checks prove it, for a flight with a Reset and an unplug in it, on both Quads. The game itself fingerprints only the Simulation's whole state after the last step, because fingerprinting every step would cost more than taking it, so the Scenario's comment at the top says that one. To see it, run `cargo scenarios check --fingerprints some-folder`: the file it writes for the saved Scenario has one line for each step, and the line for the last step must say the same. If it doesn't, that is a bug: say so in an issue.
- **Expectations are added afterwards.** The Scenario reader refuses a Scenario with no Expectation, so the saved one carries a single Observed Expectation: the Quad's height at the moment you saved. It also ends the run there. Replace it with what this flight should prove, each with its Basis. Then `cargo scenarios run scenarios/saved/whoop-65-saved-flight-1.toml` writes its Results file, which CI wants beside every committed Scenario. Until you do, `cargo scenarios check` says the Results file is missing.
- **From when.** The ticket says "since the last Reset or arming". A saved flight can only match the game's fingerprint from a start that really is a fresh start, and neither of those is one: Reset puts the Quad back, but the Simulation Time, the Radio Link's beat and the air's flicker go on through it, and arming changes none of the Quad's state. So the saved flight is the whole flight, from the moment an Input Device started it, with every Reset in it as a `reset` row. The Scenario's comment lists when Reset was pressed and when the Arm switch went on and off, so an Expectation can name the stretch since then.
- **Before a flight has started** (no Input Device has moved a stick yet) there is nothing to save, and it says so.
- **Flight Mode.** The Scenario format flies Acro only until [#51](https://github.com/BartoszSolkaBD/OpenDrone/issues/51). A flight saved with another Flight Mode setting is written as it was, and the runner refuses it until then, saying so.

A saved flight can be watched: `cargo run -p opendrone -- --scenario scenarios/saved/whoop-65-saved-flight-1.toml`.
