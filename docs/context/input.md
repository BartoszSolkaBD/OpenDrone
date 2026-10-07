# Input

The physical devices pilots fly with, and the Actions they trigger. Back to the [map](../../CONTEXT.md).

What an Input Device profile holds and what the Pack checker refuses: [Checking a Pack](../verification/checking-a-pack.md#an-input-device-profile-input-devicesidtoml). Seeing a real device's Channels live: [Watching Input Devices](../verification/watching-input-devices.md).

## Language

**Input Device**:
Any physical device a pilot flies with: a Radio or a Gamepad.
_Avoid_: Controller (ambiguous)

**Radio**:
An RC transmitter, such as the Radiomaster Pocket, that the computer sees as a USB joystick.
_Avoid_: Transmitter, TX, drone controller

**Gamepad**:
A console-style controller, such as a DualSense or an Xbox controller.
_Avoid_: Joypad, controller

**Flying Input Device**:
The one Input Device whose Channels fly the Quad. It's the device that pressed FLY, unless the pilot picks one in Settings, and it never changes in flight.
_Avoid_: Active controller, main device, primary device

**Channel**:
One stick or switch value, as a receiver hands it to the Flight Controller, such as roll, throttle or the arm switch. Arm, Flight Mode and Crash Flip always arrive on AUX1, AUX2 and AUX3, whatever the pilot flips or presses.
_Avoid_: Axis (that's the raw device side), RC input

**Virtual Switch**:
A switch Channel driven by a Gamepad button or a keyboard key instead of a physical switch. An Arm Virtual Switch toggles, and turns itself off whenever the Quad disarms. A Crash Flip Virtual Switch is on only while held.
_Avoid_: Fake switch, button switch

**Action**:
A command that isn't flying, such as Reset, Camera Tilt up or Pause, bound to a keyboard key, a Gamepad button or a Radio switch.
_Avoid_: Hotkey, shortcut, command

**Calibration**:
Measuring one Input Device's sticks so that their ends and centre mean full stick and centre stick: each stick's min, centre and max, plus a deadband against flicker at rest. Every Input Device is calibrated, Radios included.
_Avoid_: Trim, stick setup

**Radio Link**:
The simulated radio connection that carries a pilot's Channels from their Input Device to the Quad's Flight Controller at a set Packet Rate, like an ELRS link. Every Input Device flies through it, Gamepads included. When the Packet Rate divides the device's Report Rate evenly, the Radio Link follows the device's report beat.
_Avoid_: RC link, ELRS link, receiver

**Packet Rate**:
How many times a second the Radio Link delivers a fresh set of Channels, as in ELRS. The pilot picks it; the default is 250 Hz.
_Avoid_: Link rate, refresh rate, polling rate

**Report Rate**:
How many times a second an Input Device sends its values to the computer, on a beat of its own: 250 Hz for the DualSense over USB, 1000 Hz for the Radiomaster Pocket with its RF module off. With its RF module on, a Pocket reports once per ELRS packet instead.
_Avoid_: Polling rate, refresh rate, sample rate

**Input Device profile**:
The setup that turns one Input Device's axes, buttons and switches into Channels and Actions: its channel mapping, switches, Actions and Calibration. A Pack carries a starting profile for each device model, along with the device's facts, such as its Report Rate. The pilot's own copy holds what setup measured or the pilot chose, and is kept with their settings.
_Avoid_: Input profile, controller config, mapping file

## Rules

- Support is meant to be general. Verified devices are only the ones the maintainer can test, currently the DualSense over USB and the Radiomaster Pocket over USB with its RF module off. The DualSense over Bluetooth stays unverified until a later check.
- An Input Device that goes quiet because its sticks rest isn't lost. A resting Pocket can send nothing at all.
- A device that keeps reporting at rest counts as lost after 1 s of silence, exactly as if it were unplugged. The DualSense over USB keeps its motion sensors on as that heartbeat. Devices that report only changes, like the Pocket, are lost only when unplugged.
- The keyboard can't fly a Quad. Its keys trigger Actions and can drive Virtual Switches, which travel in the Flying Input Device's Radio Link frames.
- Every Action can be rebound. An Action fires once, when its key, button or switch moves into the bound position, and it works from every connected device.
- Only the Flying Input Device's Channels fly. Losing it runs Failsafe even when another Input Device is connected.
- Channels keep the Input Device's full resolution, after its Calibration and channel mapping.
- At the Simulation's front door a Channel is a whole number: the step an ELRS receiver hands the Flight Controller over CRSF, from 172 (-100%, 988 µs) through 992 (centre, 1500 µs) to 1811 (+100%, 2012 µs), about 0.625 µs a step. Each value from an Input Device, and each stick a Scenario writes in percent, is rounded to the nearest step. Betaflight reads step s as 0.62477 × s + 881 µs, so a centred stick reads 1500.77 µs, as on a real quad on ELRS.
- Arm, Flight Mode and Crash Flip reach the Flight Controller as switch Channels with fixed meanings: Arm on AUX1 (high is armed), Flight Mode on AUX2 (low Acro, middle Horizon, high Angle) and Crash Flip on AUX3 (high is on). The Input Device profile turns whatever the pilot flips, presses or types into those ([ADR-0017](../adr/0017-switches-reach-the-flight-controller-with-fixed-meanings.md)).
- Each switch Channel has one source at a time: a Radio switch, a Gamepad button or a key.
- Calibrated full stick lands at 988 and 2012 µs and centre at 1500 µs, where ELRS puts ±100% and centre. A Radio at EdgeTX defaults gives the numbers a real quad gets, calibrated or not.
- An Input Device that hasn't been calibrated flies on its Pack profile's starting values.
- A deadband belongs to the Input Device profile, never to the Tune. It acts on roll, pitch and yaw only, and works like Betaflight's: full stick stays full stick.
- Stick input reaches the Flight Controller only through the Radio Link, never straight from the Input Device.
- When the Packet Rate divides the Input Device's Report Rate evenly, the Radio Link locks to the device's report beat: each frame leaves a fixed margin after a report is due and carries exactly one fresh report ([ADR-0020](../adr/0020-radio-link-locks-to-the-device-report-beat.md)). At other Packet Rates it runs on its own clock.
- The Packet Rate setting warns when the rate is faster than the device's Report Rate, and adds a gentler note when the rate doesn't divide it evenly (150 and 100 Hz on a DualSense, 150 Hz on a Pocket).
- A Gamepad's 8-bit stick steps get no special treatment on the way to the Radio Link. Calibration removes the flicker at rest, and Input smoothing is there for pilots who want it smoother.
- A Radio's values get no special treatment on the way to the Radio Link either: no low-pass, and nothing copied from the radio's firmware. Through the Radio Link, a Radio should feel like an ideal radio on a real ELRS link ([#30](https://github.com/BartoszSolkaBD/OpenDrone/issues/30)).
- Losing the Input Device, whether unplugged or silent, loses the Radio Link, and the Flight Controller runs Failsafe.
- A device is matched to a profile by its USB ids plus its product name, ignoring case, and never by firmware version. Every EdgeTX and OpenTX radio (USB 1209:4F54) is a Radio. Devices without a profile of their own fall back to "Any Radio" or "Any Gamepad".
- EdgeTX 2.10 and later are supported, in EdgeTX's Classic USB joystick mode.
- The Pocket's setup asks for RF off and for the sim model's ADC filter set to Off. EdgeTX's ADC filter is on by default, and with it on the values creep and then jump by about 10 counts instead of following the stick.
- A Radio counts as still transmitting when, while its sticks move, no two of its value changes come closer than about 3 ms. With RF off, many come exactly 1 ms apart. Counting changes isn't enough, because the ADC filter skips many reports.
- A Pack's Input Device profile is only a starting point. The pilot's calibrated copy is never written back into a Pack, and no Pack can change it. The device's facts (its name, match and Report Rate) always come from the current Pack.
