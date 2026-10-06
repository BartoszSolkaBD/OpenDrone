# The goggles OSD: Betaflight's OSD, analog and digital drawing, fonts and other sims

Research for [#31](https://github.com/BartoszSolkaBD/OpenDrone/issues/31), to decide what the pilot sees over the FPV picture. All sources were read on 2026-10-05.

Citation prefixes:
- **BF26** = `https://github.com/betaflight/betaflight/blob/2026.6.2`
- **BF44** = `https://github.com/betaflight/betaflight/blob/4.4.0`
- **BF43** = `https://github.com/betaflight/betaflight/blob/4.3.0`
- **APP** = `https://github.com/betaflight/betaflight-configurator/blob/2026.6.2`

## 1. Betaflight 2026.6's OSD

**Elements.** 2026.6.2 has 98 elements, each with an `osd_<name>_pos` setting (enum at BF26/src/main/osd/osd.h#L122-L233, CLI names at BF26/src/main/cli/settings.c#L1552-L1716). Only `osd_warnings_pos` is visible by default. Every other element has a default position but no visibility bit (BF26/src/main/osd/osd.c#L438-L472).

How the ones a Free Flight sim can use are drawn (BF26/src/main/osd/osd_elements.c):

| Element | Drawn as | Blinks |
|---|---|---|
| `vbat` | battery-level glyph + volts; 2 decimals below 10 V, 1 above | when the battery isn't OK |
| `avg_cell_voltage` | battery-level glyph + volts, 2 decimals | same |
| `current`, `mah_drawn` | amps; mAh | mAh at `osd_cap_alarm` (2200) |
| `tim_1`, `tim_2` | clock glyph + `MM:SS` | when past its alarm |
| `flymode` | 4 letters (below) | no |
| `throttle` | throttle glyph + percent | no |
| `warnings` | the first active warning (below) | when that warning blinks |
| `crosshairs` | 3 glyphs; positionable | no |
| `ah`, `ah_sbar` | artificial horizon 9 × up to 10 glyphs; sidebars ±7 columns | no |
| `craft_name` | upper-cased name | no |
| `disarmed` | `DISARMED` while disarmed | no |
| `link_quality` | CRSF: LQ glyph + `rfMode:LQ`, e.g. `7:100` | below 80 |
| `rssi_dbm`, `rsnr` | dBm; SNR | dBm below −60; SNR never |
| `flip_arrow` | arrow, in Crash Flip or when disarmed and not upright | no |
| `log_status` | log glyph + log number, only while a Blackbox mode switch is on | no |

**Timers.** Sources are ON TIME, TOTAL ARM, LAST ARM, ON/ARM and (new in 2026.6) LAUNCH TIME. The settings are `osd_tim1` and `osd_tim2`, packed as `source | precision<<4 | alarm minutes<<8`. Both default to seconds with a 10-minute alarm: timer 1 ON TIME, timer 2 TOTAL ARM. That's the same in 4.3, 4.4 and 2026.6 (BF26/src/main/osd/osd.h#L103-L108, osd.c#L326-L329).

**Position encoding** (BF43/src/main/osd/osd.h#L56-L80, BF44 #L56-L83, BF26 #L67-L94):

| Bits | 4.3 | 4.4 and 2026.6 |
|---|---|---|
| 0–4 | column | column, low bits |
| 5–9 | row | row |
| 10 | unused | column bit 5, for HD grids up to 63 columns |
| 11, 12, 13 | visible in OSD profile 1, 2, 3 | same |
| 14–15 | variant | same |

**The maintainer's quads, decoded** (visible elements only, column,row):
- **Meteor65 Pro, 4.3.0:**
  - RSSI dBm 1,9; LQ 1,10; battery voltage 1,12
  - throttle 22,10; Flight Mode 22,11; timer 2 22,12
  - crosshair 13,6; craft name 11,12; warnings 10,10
- **Cetus X, 4.4.0:**
  - throttle 2,9; RSSI dBm 2,10; LQ 2,11; battery voltage 2,12
  - Flight Mode 24,10; timer 2 23,12
  - crosshair 13,7; craft name 11,12; warnings 9,9

The Cetus X's `osd_canvas_width = 30` and `osd_canvas_height = 13` appear in its diff because 4.4 writes the real display size into memory at every boot, and `diff` compares memory with defaults. 4.4's default canvas is 53 × 20 whenever HD is built in (BF44/src/main/osd/osd.c#L404-L411, #L501-L546).

**Grids.**
- SD: 30 columns, 13 rows on NTSC and 16 on PAL (BF26/src/main/drivers/display.h#L27-L30).
- HD: 53 × 20 unless the goggles say otherwise (BF26/src/main/osd/osd.h#L99-L101).
- A 2026.6 build with both SD and HD defaults to HD. An element below the last row is moved to that row at boot (BF26/src/main/osd/osd.c#L566-L587).

**Warnings** (BF26/src/main/osd/osd_warnings.c#L172-L533). The first active one wins, in this order (the ones that can fire in the alpha):

| Text | When | On by default |
|---|---|---|
| arming-block reason, e.g. `THROTTLE`, `ANGLE`, `RXLOSS`, `FAILSAFE`, `FLIP_SWITCH` | Arm switch on and arming blocked; cycles every 0.5 s | yes |
| `FAIL SAFE` (blinks) | Failsafe stage 2 | yes |
| `>CRASH FLIP<` | Crash Flip active | yes |
| `CRASHFLIP SW` | Crash Flip switch on while disarmed | yes |
| `LINK QUALITY`, `RSSI DBM`, `RSNR LOW` | below their alarms | **no** |
| ` LAND NOW` (blinks) | battery critical | yes |
| `LOW BATTERY` (blinks) | battery warning | yes |
| `BATT < FULL` | never armed since power-up and cells below 4.10 V | yes |
| `  * * * *` | the beeper sounds (visual beeper) | yes |

- The arming-block names are at BF26/src/main/fc/runtime_config.c#L38-L69.
- `FLIP_SWITCH` is set when the Crash Flip switch is turned off mid-flip with automatic re-arm off, and is cleared by a manual disarm (BF26/src/main/fc/core.c#L296-L355).
- **There is no in-flight `RXLOSS` warning.** `RXLOSS` is only an arming-block reason.
- 4.3 and 4.4 spell the Crash Flip texts `> CRASH FLIP <` and `CRASH FLIP SWITCH` (BF44/src/main/osd/osd_warnings.c#L67-L148).
- The warning switches are 4.3's separate `osd_warn_*` settings, then `osd_warn_bitmask` from 4.4. The bit order differs between 4.4 and 2026.6 (BF44/src/main/osd/osd.h#L258-L277 against BF26 #L312-L334).

**Flight Mode texts**, first match wins (BF26/src/main/osd/osd_elements.c#L1174-L1212): `!FS!`, `RESC`, `HEAD`, `PASS`, `POSH`, `ALTH`, `ANGL`, `HOR `, `ATRN`, `CHIR`, `AIR `, `ACRO`. With Airmode on, Acro reads `AIR`. There's no Crash Flip text.

**Link elements with ELRS.**
- Betaflight copies LQ, RSSI dBm and SNR from CRSF link statistics.
- When they stop for 250 ms, LQ drops to 0 and RSSI dBm to its minimum (BF26/src/main/rx/crsf.c#L230-L328).
- ELRS reports its packet-rate code as `rf_Mode`; 250 Hz LoRa is 7 (ExpressLRS 3.6.4 `src/include/common.h`, `rx_main.cpp`).

**Battery** (BF26/src/main/sensors/battery.c#L112-L148, #L203-L312):
- LOW BATTERY at `vbat_warning_cell_voltage` 3.50 V a cell.
- LAND NOW at `vbat_min_cell_voltage` 3.30 V.
- `BATT < FULL` below `vbat_full_cell_voltage` 4.10 V.
- Cell count = floor(volts ÷ `vbat_max_cell_voltage`) + 1.
- `current_meter` defaults to VIRTUAL (a throttle estimate) unless the board sets it (BF26/src/main/sensors/battery.c#L89-L97, common_pre.h#L368). The Meteor's board, BETAFPVF411, sets `current_meter = ADC` ([unified target](https://github.com/betaflight/unified-targets/blob/master/configs/default/BEFH-BETAFPVF411.config)).

**Other behaviour** (BF26/src/main/osd/osd.c):
- **Refresh and blink:** the OSD redraws at `osd_framerate_hz`, default 12. Blinking is 2 Hz: 250 ms on, 250 ms off.
- **Boot screen:** a 24 × 4-character logo for 4 s at power-up (#L491-L542).
- **`ARMED`:** shown centred for 0.5 s at arming, with `>CRASH FLIP<` under it in Crash Flip (#L1205-L1228).
- **Statistics page** (#L1233-L1336):
  - shown on disarm for up to 60 s
  - dismissed by throttle or pitch above 1750 µs, the Crash Flip switch, or arming
  - not shown after a Failsafe
  - default list (`osd_stat_bitmask` 14124): timer 2, max speed, min battery, min RSSI, max current, mAh used, Blackbox, Blackbox log number
- **OSD profiles:** `osd_profile` 1–3; an element shows if its bit for the active profile is set.
- **Betaflight's own SITL build has no OSD** (BF26/src/platform/SIMULATOR/target/SITL/target.h#L138).

## 2. Analog: the MAX7456 OSD chip

From the [MAX7456 datasheet](https://www.analog.com/media/en/technical-documentation/data-sheets/MAX7456.pdf) and Betaflight's driver (BF26/src/main/drivers/max7456.c):
- Characters are 12 × 18 pixels, 2 bits a pixel: black, white or transparent. The chip stores 256 characters.
- The grid is 360 pixels across, and 13 × 18 = 234 lines a field on NTSC (16 rows on PAL), so it covers about the whole active picture. Betaflight's own Pico OSD spans 48 µs of the 52.6 µs line ([BF26/src/platform/PICO/osd/osd_tx.pio](https://github.com/betaflight/betaflight/blob/2026.6.2/src/platform/PICO/osd/osd_tx.pio)).
- Betaflight's docs say edge characters may be cut off by the goggles, and the App's 15 preset spots keep a one-cell margin.
- The chip switches between camera video and OSD pixel by pixel, making one composite signal for the VTX. So the OSD travels the radio link with the picture: noise and sync loss hit both. That's our inference from the signal path; no source states it outright.
- Betaflight doesn't use the chip's hardware blink. It skips blinking elements in alternate 250 ms halves.
- The overlay is redrawn every field (59.94 Hz), while the values change at 12 Hz.

**Analog 16:9.** Analog FPV is natively 4:3 ([Oscar Liang](https://oscarliang.com/fpv-camera/)). 16:9 goggles stretch a 4:3 signal, the OSD with it.

## 3. Digital systems

| System | OSD grid | Fonts |
|---|---|---|
| DJI O3/O4 with Goggles 2, 3, N3 | 53 × 20 (Betaflight's HD default); goggle setting "HD OSD" or "Normal" (large font) | DJI's own; can't be changed ([O4 manual](https://terra-1-g.djicdn.com/6189933d30024fc1b331bffe4fe41837/o4-air-unit/UM/DJI_O4_Air_Unit_Series_User_Manual_v1.0_en.pdf)) |
| Walksnail Avatar | 53 × 20 | built-in or SD-card fonts, 24 × 36 / 36 × 54 px tiles ([Caddx wiki](https://wiki.caddxfpv.com/en/Avatar-HD-Series/manual/System-Function)) |
| HDZero | 50 × 18; a 30 × 16 grid can be centred | BMP fonts, 24 × 36 / 36 × 54 px ([hdzero-vtx](https://github.com/hd-zero/hdzero-vtx), [hdzero-goggle](https://github.com/hd-zero/hdzero-goggle)) |
| DJI V1/V2 with WTFOS | 60 × 22; SD 30 × 15 centred | PNG fonts ([msp-osd](https://github.com/fpv-wtf/msp-osd)) |

- **How the OSD gets there:** Betaflight sends it over MSP DisplayPort at 12 Hz, alongside the video on the same air link ([Betaflight VTX docs](https://betaflight.com/docs/wiki/getting-started/hardware/vtx)).
- **On link loss:**
  - WTFOS keeps the last screen.
  - HDZero keeps the last values for about a second, then clears them.
  - Walksnail's release notes fix OSD flicker under weak signal.
  - DJI doesn't document it.
- **Goggle icons:** the goggles add their own beside Betaflight's OSD.
  - DJI shows signal bars, bitrate, goggle battery and storage.
  - Walksnail shows voltages, bitrate, latency, channel with 0–4 bars, and a recording dot, all hidden by one "Goggles Icons" switch.
  - HDZero shows channel, link quality, antenna bars, recording icon and temperatures.

## 4. Fonts and licences

ADR-0014 allows CC0, OFL and permissive licences.

| Font | Licence | Shippable |
|---|---|---|
| Betaflight App `.mcm` fonts (default, bold, large, …) | GPL-3.0 by inclusion; no separate licence. Most of `default.mcm`'s letters match MWOSD's GPL font | no |
| INAV, EmuFlight, ArduPilot, MWOSD, WTFOS fonts | GPL-3.0 | no |
| SNEAKY_FPV HD fonts | personal use only | no |
| Conthrax | commercial; app embedding excluded | no |
| "OSD Fonts CC0" (`Over9kfpv/osd-fonts`) | CC0, but a week old and built against Betaflight's GPL font | risky |
| Terminus, Spleen, Unscii, Unifont; Roboto Mono, Share Tech Mono | OFL, BSD, MIT or public domain | yes |

So the alpha draws its own CC0 OSD fonts on Betaflight's 256-character map. The character codes themselves are facts from `osd_symbols.h`, unchanged from 4.3 to 2026.6 apart from new additions.

## 5. Other sims

First-party patch notes, manuals and guides:
- [Liftoff](https://store.steampowered.com/news/app/410340)
- [Liftoff: Micro Drones](https://store.steampowered.com/news/app/1432320)
- [VelociDrone manual](https://www.velocidrone.com/desktop_manual)
- [Uncrashed](https://store.steampowered.com/news/app/1682970)
- [TRYP FPV](https://store.steampowered.com/news/app/1881200)
- [DRL](https://store.steampowered.com/news/app/641780)
- [FPV SkyDive](https://store.steampowered.com/news/app/1278060)

- **No sim imports a Betaflight OSD layout or a `diff`.** That includes VelociDrone and TRYP, which run Betaflight code.
- **All draw their own font on free placement.** None imitates the analog character grid.
- **Layout editors are recent:**
  - Liftoff (September 2025): drag, anchor, snap grid, colour, Workshop sharing
  - TRYP (June 2026, work in progress)
  - Uncrashed (July 2026)
- **Voltage sag became common in 2024–2026,** mostly opt-in. Per-cell voltage, mAh and current arrived in 2026 (Liftoff, TRYP). VelociDrone shows a battery percentage.
- **None has** a post-flight statistics page, a crosshair offered as a motion-sickness aid, or HUD text sizes.
- **Whether analog noise hits the OSD** isn't documented anywhere. FPV SkyDive's screenshots show crisp text over a broken-up picture.

## 6. Drawing it inside the camera's 2 ms

These are estimates from the [#28](https://github.com/BartoszSolkaBD/OpenDrone/issues/28) prototype's merged pass (Analog 0.8–1.0 ms, Digital 0.4–0.5 ms at native 1440p on the M4); the build measures them.

- **What the pass gets:** two inputs.
  - the font image: 256 characters of 12 × 18 for Analog, an HD size for Digital
  - the OSD screen: 30 × 13 character codes plus blink bits, uploaded when the OSD redraws (12 times a second)
- **Analog** reads the OSD in its luma step, at the same tear and roll offset as the picture, then adds grain and static. 2–4 extra small-texture reads a pixel: about +0.1–0.2 ms.
- **Digital** reads it after the picture is made, crisp: about +0.05–0.1 ms.
- **Work Counts:** no new pass and no new shader pipeline; two small textures.
- **Bevy 0.20:**
  - The merged pass's shader moves from Bevy's old WGSL dialect to WESL (`import …;` instead of `#import`, `@if(…)` instead of `#ifdef`, `.wesl` files). The OSD's additions are plain bindings and functions, with nothing 0.20-specific.
  - The system that hands the OSD screen to the renderer must be ordered explicitly after the one that works it out, because 0.20's schedule randomisation exposes accidental orderings.
  - In the render world, the grid upload happens in the pass's prepare step, before the pass runs.
- **The sim's notices and Digital's goggle icons** are Bevy UI on top. They use 0.20's `em`/`rem` sizes so Text size scales them.
