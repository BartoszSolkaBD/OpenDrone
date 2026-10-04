# The Flight Controller copies Betaflight 2026.6 and translates older Tunes

Real quads run many Betaflight versions; the maintainer's run 4.3 and 4.4. Between 4.3 and 2026.6, several things changed: Angle and Horizon, feedforward, anti-gravity, I-term windup and the Dynamic D defaults. We implement 2026.6 behaviour only. A `diff all` from Betaflight 4.3 or newer is translated into 2026.6 settings:

- Renamed settings are mapped to their new names.
- Settings missing from the diff take their own version's defaults.
- Settings the old version didn't have are set to whatever reproduces the old behaviour, where such a value exists: feedforward averaging off, yaw hold off, low-throttle TPA off, and Angle's earth reference off.

That gives one Flight Controller to spec, test and keep current, and today's Betaflight diffs import cleanly. Decided in [#21](https://github.com/BartoszSolkaBD/OpenDrone/issues/21).

## Considered options

- **Fill missing settings with 2026.6 defaults.** The 4.3 Meteor would get 5 to 11 times more feedforward lag than the real one.
- **Emulate each Tune's own version for the parts that changed.** Closest to real quads, but it roughly doubles those parts and their Scenarios.
- **Copy 4.3 only for the alpha.** Matches the maintainer's whoop, but the project would start on 2022 firmware.

## Consequences

- A translated 4.3 or 4.4 Tune still differs from the real quad in three ways:
  - Angle and Horizon: 2026's are softer around centre.
  - 4.3's anti-gravity: 2026's punch-out boost is weaker.
  - Feedforward jitter attenuation is slightly stronger.

  If feel tests show this matters, 4.3's anti-gravity can be added later.
- The importer accepts Betaflight 4.3 and newer only.
