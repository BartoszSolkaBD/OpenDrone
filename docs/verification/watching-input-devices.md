# Watching Input Devices

`cargo xtask input-monitor` shows every connected Input Device live, read exactly as the game reads it: SDL 3.4 on its own thread ([ADR-0018](../adr/0018-input-through-sdl3-on-its-own-thread.md)), the built-in Pack's Input Device profiles, and the Channels each device gives. It's how the maintainer checks a real Radio or Gamepad, since CI has no devices. The terms are in the [Input](../context/input.md) deep dive; the profile format is in [Checking a Pack](checking-a-pack.md#an-input-device-profile-input-devicesidtoml).

## Running it

```sh
cargo xtask input-monitor               # runs until Ctrl+C
cargo xtask input-monitor --seconds 30  # stops by itself
```

The first build compiles SDL from source, which needs CMake and a C compiler and takes about 30 s on the M4. On Linux, install `libudev-dev` and `pkg-config` first.

With nothing plugged in it says so and keeps watching:

```text
Watching Input Devices through SDL 3.4.18 on its own thread. Times are seconds on the input thread's clock.
No Input Device is connected. Plug in a Radio or a Gamepad; the monitor keeps watching.
```

## What it shows

- **When a device comes or goes**, one line each, stamped with the input thread's clock: found (its name, USB ids, kind and the profile it got), lost (unplugged, or silent for 1 s for a device that reports at rest) and back. The found line also says whether 1 s of silence counts as lost. A device whose profile says it reports at rest, but whose motion sensors didn't switch on, is lost only when unplugged, and the line says so.
- **Every half second, each device:**
  - its Channels in µs: roll, pitch, throttle and yaw, then AUX1 (Arm), AUX2 (Flight Mode) and AUX3 (Crash Flip). Full stick is 988 and 2012 µs, centre 1500 µs; a switch with no source shows "none";
  - when its Channels last changed, in seconds on the input thread's clock: the stamp the game turns into Simulation Time;
  - its raw values as SDL reports them: a Radio's axes and the button channels held (CH9 and up), or a Gamepad's sticks, triggers and buttons by SDL's position names;
  - how many times a second its Channels changed;
  - for a Radio, whether it seems to still transmit: no two value changes closer than 3 ms while the sticks move means RF is on ([#30](https://github.com/BartoszSolkaBD/OpenDrone/issues/30)).
- **How often the input thread read SDL**: about 3,000 times a second on the dev Mac.

The monitor flies on each profile's starting values: it doesn't calibrate, and it doesn't read the keyboard (it has no window).

## Checking the verified devices

**The Radiomaster Pocket** over USB, in EdgeTX's Classic USB joystick mode, with RF off and the sim model's ADC filter Off:

- It's found as a Radio on `opendrone/radiomaster-pocket`. Note the name SDL gives it: #18 saw "EdgeTX Radiomaster Pocket Joystick", which EdgeTX's USB strings don't explain, so it's worth a recheck.
- Throttle down reads 988 µs and the centred sticks about 1500 µs. Full stick each way reads 988 and 2012 µs.
- CH5 high sets AUX1 to 2012 µs. CH6 low, middle and high set AUX2 to 988, 1500 and 2012 µs (Acro, Horizon, Angle). CH7 high sets AUX3. SE shows as CH9 held.
- While the sticks move it says "not transmitting". With RF on it should say it seems to transmit, with changes no closer than about 4 ms at 250 Hz.
- Left alone it sends nothing and is never lost. Unplugged, it's lost at once, and the Channels don't change in that instant: the false centre SDL sends on CH6 and CH7 at unplug is thrown away.

**The DualSense** over USB:

- It's found as a Gamepad on `opendrone/dualsense`.
- R1 toggles AUX1 on and off; L1 holds AUX3 on only while held.
- Left on the desk, the right stick's flicker shows in roll: about 1522–1526 µs, because the Pack's starting deadband of 5 % is narrower than how far its stick rests off centre. Setup's Calibration removes it.
- It keeps reporting at rest through its motion sensors, so it's never lost while it rests, and its found line says 1 s of silence counts as lost. Its Channels change at most about 250 times a second.

## Recordings instead of devices

CI can't plug devices in, so the readable checks in `crates/input/tests/` replay recordings instead: trimmed copies of the maintainer's own runs from the SDL3 input probe ([#18](https://github.com/BartoszSolkaBD/OpenDrone/issues/18), [#27](https://github.com/BartoszSolkaBD/OpenDrone/issues/27), [#30](https://github.com/BartoszSolkaBD/OpenDrone/issues/30)), in `crates/input/tests/traces/`. Each file's header names the run and the stretch it keeps. They hold the Pocket with RF off (with its unplug, and with the ADC filter on and Off), and the DualSense letting go after fast circles, resting on the desk, and pressing its buttons and triggers.

No recording of a Pocket with RF on exists: that check builds one from a real RF-off recording, sending one report per 4 ms packet as EdgeTX's source says it does. A recording of a real Pocket with RF on would replace it.
