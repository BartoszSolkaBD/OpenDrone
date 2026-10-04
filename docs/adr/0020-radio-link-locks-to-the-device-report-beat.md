# The Radio Link locks to the Input Device's report beat

When the Packet Rate divides an Input Device's Report Rate evenly, the Radio Link doesn't run on a clock of its own. It learns the device's report beat from the timestamps on its Flight Inputs, and sends each frame a fixed margin after a report is due: 0.75 ms on the dev Mac, just past the measured arrival wobble. Every frame then carries exactly one fresh report, always the same age. At other Packet Rates the Radio Link runs on its own clock.

**Why.** We measured a DualSense over USB in [#18](https://github.com/BartoszSolkaBD/OpenDrone/issues/18) and [#27](https://github.com/BartoszSolkaBD/OpenDrone/issues/27):
- It reports on a steady 4 ms beat, 4.5–6 ppm slower than the Mac's clock.
- Each report reaches our input thread 0–0.5 ms after it's due.

A free-running 250 Hz clock that lands inside that window catches some reports and misses others. Frames then alternate between "no change" and "double change", and in a model of Betaflight 2026.6 this made feedforward ripple about 7–10× larger:
- on real circles, peaks of up to 23% of motor range on the 5" and over 100% on the whoop, against about 2% and 6% when locked;
- because the clocks drift, a free-running link lands in that window for 1–1.5 minutes every 11–15 minutes, at random, and every Pause redraws where it starts.

Real ELRS avoids the same problem the other way round: the module tells the radio when to run its mixer, so each packet carries one fresh sample (ExpressLRS 3.6.4, `CRSFHandset.cpp`). We can't steer a DualSense, so the Radio Link follows it instead.

## Considered options

- **Free-running clock.** Rejected. It's fine 9 times in 10, but the bad stretches come and go at random, and a frame's sample is 0.2–4.2 ms old (2.2 ms on average).
- **Clean timestamps in the input layer, free-running Radio Link.** Rejected. It removes the bad window, apart from a one-frame blip whenever the drifting clocks cross. But each frame's sample age would still vary from 0.75 to 4.75 ms, and change at every Pause.

## Consequences

- **Which rates lock.** The DualSense over USB (250 Hz) locks at Packet Rates of 250 and 50 Hz. The Pocket (1 kHz) locks at 1000, 500, 333, 250, 100 and 50 Hz. Other Packet Rates run free, and the settings screen adds a gentle note: frames carry uneven steps there, most visibly at 150 Hz.
- **Delay.** Frames are about 1.5 ms fresher on average than with a free-running clock, and always equally fresh.
- **Replays.** The beat is learnt only from Flight Input timestamps, so Input Tracks and Scenarios replay exactly. Scenarios with scripted sticks have no device and get plain regular frames.
- **Between reports.** While the sticks rest, the beat carries on by itself; it is steady to a few ppm. The DualSense's motion sensors stay on as a heartbeat, so the Radio Link hears every report. The same heartbeat lets a silent DualSense count as lost after 1 s.
- **The margin follows the measured wobble.** A machine whose input thread polls more coarsely, possibly Windows, needs a wider margin.
- **A late report.** A report that arrives after its frame has left (1 in 19,000 in the sensors run) costs one repeated frame, like one lost ELRS packet.
- **The Pocket gains nothing measurable.** Its values move in uneven jumps every 1–4 ms whatever its USB beat does. That's a separate follow-up.
