# Switches reach the Flight Controller with fixed meanings

Arm, Flight Mode and Crash Flip reach the Flight Controller as switch Channels with one fixed layout:

| Channel | Meaning |
|---|---|
| AUX1 | Arm: high is armed |
| AUX2 | Flight Mode: low Acro, middle Horizon, high Angle |
| AUX3 | Crash Flip: high is on |

The Flight Controller keeps one built-in Modes table for that layout and runs Betaflight's own mode logic on it. Each pilot's Input Device profile turns whatever they flip, press or type into these three Channels. Examples:
- an Arm switch on CH7 that arms when low
- a Flight Mode switch with Angle at the bottom, as on the maintainer's Cetus X
- a Gamepad button
- a keyboard key

Betaflight does it differently: a pilot can put any mode on any AUX channel, in any range, through its Modes tab. We chose fixed meanings because they keep Scenarios and Gamepads simple:
- A Scenario keeps saying "armed" and "Flight Mode Angle".
- A Gamepad button doesn't need a µs range.

The layout follows ExpressLRS's rule that Arm sits on AUX1, with high meaning armed, and it matches how both of the maintainer's quads arm and Crash Flip. Decided in [#19](https://github.com/BartoszSolkaBD/OpenDrone/issues/19).

## Considered options

- **Copy Betaflight's Modes tab.** The Flight Controller would get the radio's raw AUX1–AUXn channels plus a Modes table (a channel and a 900–2100 µs range in 25 µs steps) from the pilot's profile. Rejected:
  - Every Scenario's starting state would have to spell out a Modes table ([ADR-0002](0002-scenario-starting-state-spells-out-everything.md)).
  - Every Gamepad profile would need µs ranges for its buttons.

## Consequences

- **The Input Device profile does all the translating.** Its `[switches]` section names, for each of Arm, Flight Mode and Crash Flip, the device control and which of its positions means what. Setup confirms each one by asking the pilot to flip it.
- **Pasting `aux` lines from a Betaflight `diff all` still works.** The paste tests each switch position (988, 1500 and 2012 µs) against the pasted ranges, as Betaflight would, and writes the result into the profile. Modes other than Arm, Angle, Horizon and Flip Over After Crash are listed as ignored.
- **What's lost from the raw values:**
  - A Blackbox log shows the fixed AUX values, not the radio's raw ones.
  - The Flight Controller never sees AUX4 and up.
- **One source per switch.** Each switch Channel has exactly one source at a time: a Radio switch, a Gamepad button or a key. A key or button drives a Virtual Switch, and its frames still travel in the Flying Input Device's Radio Link, so Failsafe works the same.
- **A new switch-driven mode later** gets the next fixed AUX channel, and the PR that adds it updates every profile and Scenario.
