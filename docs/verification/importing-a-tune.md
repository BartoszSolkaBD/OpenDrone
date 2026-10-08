# Importing a Tune from Betaflight

A Quad's Tune is its Flight Controller settings, written as Betaflight CLI text in `tune.txt` ([ADR-0015](../adr/0015-tune-is-betaflight-cli-text-spelling-out-every-setting.md)). Our Flight Controller copies Betaflight 2026.6, so a Tune uses 2026.6's names. A real quad usually runs an older Betaflight, so its settings are **translated, never emulated** ([ADR-0008](../adr/0008-copy-betaflight-2026-6-translate-older-tunes.md)). The Betaflight CLI translator does the translating. It lives in the Flight Controller crate ([`crates/flight-controller/src/cli/`](../../crates/flight-controller/src/cli/)) and works on text alone; it opens no files.

The Whoop 65's Tune is made this way, from the maintainer's Meteor65 Pro.

## Import a quad's Tune

1. In the Betaflight App's CLI tab, type `diff all` and save everything it prints to a text file. The maintainer's two quads are in [`docs/research/quad-settings/`](../research/quad-settings/README.md).
2. Run the importer with that file and the Quad's folder:

   ```sh
   cargo xtask import-tune docs/research/quad-settings/meteor65-pro.diff-all.txt packs/opendrone/quads/whoop-65
   ```

   It writes the folder's `tune.txt` and lists what it did: the renames, the settings kept as "not simulated yet", and every line it left out, with the reason. To see a Tune without writing it, put `--print` in place of the folder.
3. Run `cargo xtask packs`. The Pack checker reads the new Tune and compares `motor_poles` and `yaw_motors_reversed` with the Quad definition.

Each new Flight Controller ticket reads more settings, so the Whoop 65's Tune is imported again in that ticket's PR. A readable check holds the committed file to exactly what the importer writes, so the Tune is never edited by hand. A real `hand-set` change would need that check changed too.

## What it accepts

- `diff`, `diff all` and `dump`, from Betaflight **4.3 or newer**, pasted whole: the importer needs the `# version` line to know the version and the `profile` lines to find the PID profile. `diff all` is the best choice, because it lists only what the quad changed.
- It knows the settings and defaults of every Betaflight release from 4.3 to 2026.6: **4.3, 4.4, 4.5, 2025.12 and 2026.6**, each read from its source (4.3.0, 4.4.0, 4.5.0, 2025.12.1 and 2026.6.2). A version's patch releases share its defaults. Older than 4.3, and newer than 2026.6, are refused with a sentence saying so.

## What it reads

- The settings before any profile, and the **active PID profile's**: the one the last `profile` line selects. Other PID profiles are left out.
- **The final numbers**, never the `simplified_*` sliders. The firmware never re-applies the sliders at boot, and both of the maintainer's diffs match theirs ([#21](https://github.com/BartoszSolkaBD/OpenDrone/issues/21)).
- **No rate profiles.** Rates belong to the pilot. The one exception is 4.3's TPA (`tpa_mode`, `tpa_rate`, `tpa_breakpoint`), which 4.3 kept in its rate profile and 4.4 moved to the PID profile: for 4.3 it's read from the active rate profile.

## How each line is marked

Every `set` line ends with a mark saying where its value came from, then perhaps a note after a `;`, such as its unit.

| Mark | Meaning |
|---|---|
| `diff` | set on the real quad |
| `4.3 default` | not in the export, so that version's own default |
| `ADR-0008` | the old version had no such setting, or it meant something else; set to behave like the old version |
| `2026.6 default` | the old version had no such setting, and no value behaves like it: Betaflight 2026.6.2's default |
| `(was …)` | the setting's old name, after one of the marks above |
| `hand-set: <reason>` | changed by hand (the importer never writes it) |

## What it translates

| Before | 2026.6 | Why |
|---|---|---|
| `d_min_roll`, the base D (4.3 to 4.5) | `d_roll` | 2025.12 renamed Dynamic D's two numbers: the base is now `d_roll` and the peak `d_max_roll`. The same for pitch and yaw. |
| `d_roll`, the peak D (4.3 to 4.5) | `d_max_roll` | When Dynamic D was off (`d_min` at 0, or not below `d`), D stays `d_roll`, and `d_max_roll` keeps it off. |
| `dshot_idle_value` (4.3 to 4.5) | `motor_idle` | Renamed; the same hundredths of a percent. |
| `level_limit`, `angle_level_strength`, `horizon_transition` (4.3 and 4.4) | `angle_limit`, `angle_p_gain`, `horizon_limit_sticks` | Renamed when 4.5 rebuilt Angle and Horizon. |
| `iterm_limit`, the I limit, 400 (4.3 to 4.5) | `iterm_windup`, 80 | From 2025.12 the I limit is `iterm_windup` percent of `pidsum_limit`: 400 is 80% of 500. The old `iterm_windup` meant something else, so it's left out. |
| `d_max_gain` × `d_max_advance` ÷ 100 (4.3 to 4.5) | `d_max_advance`, 7 | Before 2025.12 the stick-driven D boost was the two multiplied; now it's `d_max_advance` alone: 37 × 20 ÷ 100 ≈ 7. |

Settings the old version lacked take the value that behaves like it (ADR-0008): yaw hold off before 2025.12 (`feedforward_yaw_hold_gain = 0`), low-throttle TPA off before 4.5 (`tpa_low_rate = 0`), Angle's earth reference off before 4.5 (`angle_earth_ref = 0`) and no Crash Flip rate fade before 2025.12 (`crashflip_rate = 0`). Where no value behaves like the old version, the setting takes 2026.6.2's default. One example is 4.3's anti-gravity, which worked differently: ADR-0008 accepts that 2026.6's punch-out boost is weaker.

A setting the export doesn't set takes **its own version's** default, never 2026.6's. That matters for the Meteor: 2026.6's feedforward defaults would give it 5 to 11 times the real quad's feedforward lag (ADR-0008).

## What it leaves out

- **Hardware-only settings:** the OSD, VTX, LEDs, ports and the receiver protocol, sensors and their alignment, the beeper, `expresslrs_*`, the PID loop's rate (`pid_process_denom`: our Flight Controller runs once per physics step), and the quad's name.
- **Settings 2026.6 has no counterpart for**, each with its reason, such as 4.3's `min_throttle` (for analog ESC protocols) and `crashflip_expo`.
- **Every command that isn't `set`:** `aux` lines (the pilot's switches belong to their Input Device profile), `feature`, `serial`, `vtxtable` and the rest.

## Not simulated yet

The settings the export sets that our Flight Controller doesn't read yet stay in the Tune, under a "Not simulated yet" heading at the end: the RPM filter, the dynamic notch, thrust linearisation and more, plus settings that later Flight Controller tickets read, such as feedforward (#49) and Dynamic D's peak (#50). Once a ticket reads one, importing again moves it under its tab, and settings the export doesn't set start being spelled out with their defaults.

## What still differs from the real quad

ADR-0008 lists them: 2026's Angle and Horizon are softer around the centre, 4.3's anti-gravity punch-out boost was stronger, and 2026's feedforward jitter attenuation is slightly stronger.

## Pasting Rates and switches

The same translator reads two pastes a pilot makes from their own quad's CLI. The settings screens show what each would change, Now and After, before the pilot applies it.

- **Rates** ([#13](https://github.com/BartoszSolkaBD/OpenDrone/issues/13)): from a pasted `diff`, `diff all` or `dump`, only the **active rate profile's** rate settings are read, the one the last `rateprofile` line selects. Every other line is ignored, and the paste says so. For the Cetus X's `diff all`: "Used rate profile 0 (3 rate settings); ignored 104 lines." A rate setting the profile doesn't set keeps Betaflight's default, which has been the same since 4.3: Actual, 70 °/s at centre and 670 °/s at full stick.
- **Switches** ([#19](https://github.com/BartoszSolkaBD/OpenDrone/issues/19) §5): from pasted `aux` lines, ARM, ANGLE, HORIZON and FLIP OVER AFTER CRASH are read. AUX*n* is the radio's CH(*n*+4). Each switch position a radio sends (988, 1500 and 2012 µs) is tested against the pasted ranges as Betaflight does, so any ranges work. Angle beats Horizon, and Acro is wherever neither is on. Other modes, such as the Cetus X's BEEPER, are listed as ignored. AND logic and linked modes are refused with a note, as is anything a profile can't hold, such as Arm on two switches. The result is the Input Device profile's `[switches]`. The Meteor65 Pro's lines give exactly the Radiomaster Pocket's starting layout.

## Adding a setting

The translator knows each setting from one row of a table: [`crates/flight-controller/src/cli/table.rs`](../../crates/flight-controller/src/cli/table.rs). A row gives the 2026.6 name, its tab in the Tune, a short note, and where its value comes from in each version, oldest first: the same name with that version's default, an old name, an ADR-0008 value, 2026.6's default, or a rule for a setting whose meaning changed. The table already holds rows for the settings the next Flight Controller tickets read. A Flight Controller ticket that reads a new setting checks its row, adds the setting to the 5″'s Tune, and imports the whoop's again.
