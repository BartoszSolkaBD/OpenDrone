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

**Action**:
A command that isn't flying, such as Reset, Camera Tilt up or Pause, bound to a keyboard key, a Gamepad button or a Radio switch.
_Avoid_: Hotkey, shortcut, command

**Radio Link**:
The simulated radio connection that carries a pilot's Channels from their Input Device to the Quad's Flight Controller at a set Packet Rate, like an ELRS link. Every Input Device flies through it, Gamepads included.
_Avoid_: RC link, ELRS link, receiver

**Packet Rate**:
How many times a second the Radio Link delivers a fresh set of Channels, as in ELRS. The pilot picks it; the default is 250 Hz.
_Avoid_: Link rate, refresh rate, polling rate

## Rules

- Support is meant to be general. Verified devices are only the ones the maintainer can test, currently the DualSense and the Radiomaster Pocket.
- The keyboard triggers Actions only. It isn't an Input Device and can't fly a Quad.
- Every Action can be rebound.
- Stick input reaches the Flight Controller only through the Radio Link, never straight from the Input Device.
- Unplugging the Input Device loses the Radio Link, and the Flight Controller runs Failsafe.
