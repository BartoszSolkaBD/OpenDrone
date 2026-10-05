# Stick input reaches the Flight Controller through an emulated Radio Link

Betaflight tunes RC smoothing and feedforward to how often stick frames arrive. So if the Flight Controller got raw Input Device samples, each device would feel like a different radio link. A model of Betaflight 2026.6 showed the Pocket at 1 kHz behaving like an F1000 link with about 40% of the usual feedforward, and the DualSense over Bluetooth giving undersized, noisy feedforward. So every Input Device is resampled into a Radio Link at the pilot's Packet Rate. The default is 250 Hz, which is ELRS's default and the maintainer's own link. The Flight Controller sees it as an ELRS receiver on CRSF. Every device then feels like a real ELRS link, and Scenarios and future multiplayer get one steady input stream. Decided in [#21](https://github.com/BartoszSolkaBD/OpenDrone/issues/21).

## Considered options

- **Raw device samples.** Faithful to the device, not to the pilot's quad. The feel would change with the device, and feedforward is noisy over Bluetooth and on a Pocket with its RF module on.
- **A fixed 250 Hz link with no setting.** Betaflight gives about 35% less feedforward at 500 Hz than at 250 Hz on medium stick moves, so a pilot who flies 500 Hz couldn't match their setup.

## Consequences

- A Packet Rate faster than the device reports causes duplicate frames and pulsing feedforward, for example 500 Hz with a DualSense on USB (250 Hz). The settings screen warns about it.
- Failsafe runs on the Radio Link. Unplugging the Input Device stops its frames.
- How the Radio Link handles the DualSense's 8-bit sticks and its 250 Hz USB clock was decided in [#27](https://github.com/BartoszSolkaBD/OpenDrone/issues/27): it locks to the device's report beat ([ADR-0020](0020-radio-link-locks-to-the-device-report-beat.md)), and the 8-bit steps get no special treatment.
- How it handles a Radio's own filtering was decided in [#30](https://github.com/BartoszSolkaBD/OpenDrone/issues/30). The real Pocket's uneven value updates come from EdgeTX's ADC filter. The Radio Link gets no Radio-specific treatment and copies nothing from radio firmware; the Pocket's setup turns the filter off on the sim model instead. #21 had called a Pocket through a 250 Hz Radio Link "indistinguishable" from real ELRS 250. With the filter on, the two actually differ by Blackbox-level feedforward ripple, and the real Pocket on ELRS is no closer to an ideal radio.
