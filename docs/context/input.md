# Input

The physical devices pilots fly with, and the Actions they trigger. Back to the [map](../../CONTEXT.md).

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

**Channel**:
One stick or switch value, as a receiver hands it to the Flight Controller, such as roll, throttle or the arm switch. A Gamepad button can drive a virtual switch Channel.
_Avoid_: Axis (that's the raw device side), RC input

**Action**:
A command that isn't flying, such as Reset, Camera Tilt up or Pause, bound to a keyboard key, a Gamepad button or a Radio switch.
_Avoid_: Hotkey, shortcut, command

**Radio Link**:
The simulated radio connection that carries a pilot's Channels from their Input Device to the Quad's Flight Controller at a set Packet Rate, like an ELRS link. Every Input Device flies through it, Gamepads included. When the Packet Rate divides the device's Report Rate evenly, the Radio Link follows the device's report beat.
_Avoid_: RC link, ELRS link, receiver

**Packet Rate**:
How many times a second the Radio Link delivers a fresh set of Channels, as in ELRS. The pilot picks it; the default is 250 Hz.
_Avoid_: Link rate, refresh rate, polling rate

**Report Rate**:
How many times a second an Input Device sends its values to the computer, on a beat of its own: 250 Hz for the DualSense over USB, 1000 Hz for the Radiomaster Pocket with its RF module off.
_Avoid_: Polling rate, refresh rate, sample rate

**Input Device profile**:
The setup that turns one Input Device's axes and buttons into Channels: its channel mapping, switches and calibration. A Pack carries a starting profile for each device model; the pilot's own copy, made by setup and calibration, is kept with their settings.
_Avoid_: Input profile, controller config, mapping file

## Rules

- Support is meant to be general. Verified devices are only the ones the maintainer can test, currently the DualSense over USB and the Radiomaster Pocket over USB with its RF module off. The DualSense over Bluetooth stays unverified until a later check.
- An Input Device that goes quiet because its sticks rest isn't lost. A resting Pocket sends nothing at all.
- A device that keeps reporting at rest counts as lost after 1 s of silence, exactly as if it were unplugged. The DualSense over USB keeps its motion sensors on as that heartbeat. Devices that report only changes, like the Pocket, are lost only when unplugged.
- The keyboard triggers Actions only. It isn't an Input Device and can't fly a Quad.
- Every Action can be rebound.
- Channels keep the Input Device's full resolution, after its calibration and channel mapping.
- Arm, the Flight Mode switch and the Crash Flip switch reach the Flight Controller as switch Channels, whatever they're bound to, as in Betaflight's Modes tab.
- Stick input reaches the Flight Controller only through the Radio Link, never straight from the Input Device.
- When the Packet Rate divides the Input Device's Report Rate evenly, the Radio Link locks to the device's report beat: each frame leaves a fixed margin after a report is due and carries exactly one fresh report ([ADR-0020](../adr/0020-radio-link-locks-to-the-device-report-beat.md)). At other Packet Rates it runs on its own clock.
- The Packet Rate setting warns when the rate is faster than the device's Report Rate, and adds a gentler note when the rate doesn't divide it evenly (150 and 100 Hz on a DualSense, 150 Hz on a Pocket).
- A Gamepad's 8-bit stick steps get no special treatment on the way to the Radio Link. Calibration removes the flicker at rest, and Input smoothing is there for pilots who want it smoother.
- Losing the Input Device, whether unplugged or silent, loses the Radio Link, and the Flight Controller runs Failsafe.
- A Pack's Input Device profile is only a starting point. The pilot's calibrated copy is never written back into a Pack, and no Pack can change it.
