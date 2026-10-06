# A Tune is Betaflight CLI text that spells out every setting

A Quad's Tune lives in a `tune.txt` beside its `quad.toml`. It's written in Betaflight's own CLI form (`set p_roll = 40`), with 2026.6 names and Betaflight's own units. Every setting our Flight Controller reads is written out every time, about 150 to 200 lines (Betaflight 2026.6.2 has 149 PID-profile settings alone). Each line carries a mark saying where its value came from:
- `diff`: set on the real quad
- `4.3 default`: missing from the diff, so it takes its own version's default
- `ADR-0008`: a setting the old version didn't have, set to behave like the old version
- `(was …)`: the setting's old name
- `hand-set: <reason>`: changed by hand

We chose CLI text because every Betaflight pilot already reads it, the importer's output then looks like its input, and a Tune could even be pasted into a real quad running 2026.6. We chose to spell out every setting because a Tune that lists only its differences hides its meaning in defaults inside our code. Those defaults change whenever we copy a newer Betaflight. [ADR-0008](0008-copy-betaflight-2026-6-translate-older-tunes.md) showed why that matters: filling the Meteor's gaps with 2026.6 defaults would have given it 5 to 11 times the real quad's feedforward lag. Spelling everything out is the same choice [ADR-0002](0002-scenario-starting-state-spells-out-everything.md) made for Scenarios. Decided in [#16](https://github.com/BartoszSolkaBD/OpenDrone/issues/16).

## Considered options

- **A `[tune]` section inside the Quad's TOML file.** One file and one syntax per Quad, but it reads less like what pilots know, and the importer would translate between two forms.
- **Only the settings that differ from 2026.6's defaults, like a `diff`.** That's about 25 to 40 lines for the Meteor, but every Tune would change silently when the defaults do.

## Consequences

- **The one exception to "units on every number":** Tune values keep Betaflight's own units with no unit text. For example, `failsafe_delay = 15` means 1.5 s, because Betaflight counts tenths of a second. A value always means exactly what it means in Betaflight.
- **The top of the file records where the Tune came from:** the Betaflight version, the quad and the date.
- **Layout:** settings are grouped by topic, in the order of Betaflight Configurator's tabs. Settings we don't simulate yet stay, under their own heading.
- **What's dropped:** the Modes tab lines (`aux`) and the rate profiles. Which switch does what belongs to the pilot's Input Device profile, and Rates belong to the pilot.
- **Hardware cross-checks:** `motor_poles` must match the Quad's pole count, and `yaw_motors_reversed` must match its prop direction. The checker refuses a mismatch.
- **No Confidence:** a Tune is copied exactly from a real quad or from Betaflight's defaults, so its lines carry no Confidence. Their marks say where each value came from.
