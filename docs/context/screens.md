# Screens and navigation

The screens a pilot sees and how they move between them. Back to the [map](../../CONTEXT.md).

## Language

**Hub**:
The home screen. It shows the last Map, Quad and Preset next to a FLY button, so a returning pilot gets to the Launch Spot in one press.
_Avoid_: Main menu, lobby, home screen

**First Launch**:
The single pass a new install makes before the Hub: a flashing-images notice with Text size and Reduce motion, the question "Have you flown FPV before?", which picks the starting Preset, and the Input Device setup, where Calibration and switch setup run unless the pilot chooses Set up later.
_Avoid_: Onboarding, tutorial, setup wizard

**Pause Menu**:
The menu the Pause Action opens during Free Flight. From it the pilot can Resume, Reset, swap the Map or Quad, open Settings, go back to the Hub or quit.
_Avoid_: Esc menu, pit stop, in-game menu

**Pre-flight Warning**:
A notice on the Hub that something would stop the pilot flying, or flying well, such as no Input Device, no way to Arm, an Input Device that isn't calibrated, or a Radio still transmitting. It never blocks flying.
_Avoid_: Error, blocker

## Rules

- A returning pilot reaches the Launch Spot in one press from the Hub. A new install takes three when the pilot skips the Input Device setup: answer the question, Set up later, FLY. Setting up there is the selected choice and adds about half a minute of guided steps.
- In single-player, the Pause Menu freezes the simulation. In multiplayer it will have to open over a world that keeps running.
- Losing the Input Device never opens the Pause Menu. The Failsafe handles it, as in Betaflight.
- Settings take effect as soon as they change. Only resolution and window mode ask to be kept, and they change back after 15 seconds.
- Every menu works with a Radio, a Gamepad, or the keyboard and mouse. A Radio moves the cursor with the right stick, and a flick of yaw right or left selects or goes back. Menus never read the throttle or the switches.
- Each setting is one stop in a menu, changed with left and right. There are no drop-down lists.
