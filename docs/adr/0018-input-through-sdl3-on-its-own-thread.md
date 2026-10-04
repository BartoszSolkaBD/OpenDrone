# Input Devices are read through SDL 3.4 on their own thread, not Bevy's gamepad input

`opendrone-input` reads every Input Device through SDL 3.4 on a thread of its own, and stamps each sample with the computer's clock as it reads it. SDL comes through `sdl3-sys`, built from source and statically linked with only its joystick and HIDAPI parts. Bevy's built-in gamepad input isn't used for flying.

We chose this because Bevy reads gamepads once per frame, filters out small stick moves, and on macOS drops the Pocket's yaw and CH5–CH8. SDL is the only stack that:
- reads both verified devices on all three desktop OSes,
- has no 125 Hz cap or window-focus requirement on Windows,
- also runs on iOS and Android.

A prototype confirmed it on the dev Mac beside a Bevy window: the Pocket at 1 kHz and 11 bits, the DualSense over USB at 250 Hz and 8 bits, and no macOS permission prompt. Researched in [#3](https://github.com/BartoszSolkaBD/OpenDrone/issues/3), decided in [#18](https://github.com/BartoszSolkaBD/OpenDrone/issues/18).

## Considered options

- **Bevy's gamepad input (`bevy_gilrs`).** Rejected. It reads once per frame, its default settings filter the sticks, and Bevy 0.19 loses Pocket axes it can't name.
- **gilrs on our own thread.** The fallback. Rejected: on Windows it polls at 125 Hz, needs window focus and can't read a DualSense over Bluetooth, and it has no mobile support.
- **Raw HID (`hidapi`).** Rejected. We would have to parse every device's HID descriptor ourselves, and it has no iOS support.

## Consequences

- **Build.**
  - Building needs CMake and a C compiler. SDL adds about 30 s to a clean build on the M4, and about a minute on CI runners. CI built it on macOS, Windows and Linux.
  - On Linux, SDL is built as a no-window build (`sdl-unix-console-build`), so no X11 or Wayland headers are needed for it.
- **Timing.**
  - Samples are stamped when the input thread reads them, not with the device's or the OS's own time. So the thread polls at about 3,000 times a second, giving stamps accurate to about 0.3 ms.
  - A plain sleep may be too coarse on Windows: a CI runner managed only about 1,240 polls a second. That needs checking on real Windows hardware.
  - Plugging a device in or out stalls the thread for a few milliseconds.
- **Losing a device.** SDL reports changes only, and a resting Pocket sends nothing. So a device counts as lost only when the operating system removes it ([ADR-0003](0003-crate-split-and-flight-inputs.md)).
- **Bluetooth.** The DualSense over Bluetooth hasn't been measured. It gets a check later, if Bluetooth support needs changes.
