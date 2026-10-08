//! What the input thread hands over: every change SDL reported in one poll,
//! stamped with the computer's clock as the thread read it, and each
//! device's current values.
//!
//! The thread reads SDL about 3,000 times a second (ADR-0018). Everything
//! SDL reported in one poll arrives together, as one [`Batch`] with one
//! stamp. Recorded traces are replayed as the same batches, so every check
//! runs the code the game runs.

use std::time::Duration;

use crate::controls::{PadAxis, PadButton};
use crate::device::DeviceInfo;

/// SDL's number for one plugged-in device. A device that is unplugged and
/// plugged back in gets a new one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SdlId(pub u32);

/// One change SDL reported.
#[derive(Clone, Debug, PartialEq)]
pub enum Raw {
    /// A device was plugged in and opened, with its values at that moment.
    /// SDL reports only changes, so these are where every control starts.
    Added {
        device: SdlId,
        info: DeviceInfo,
        state: DeviceState,
    },
    /// The operating system removed the device.
    Removed { device: SdlId },
    /// A joystick axis moved: a Radio's CH1–CH8 are axes 0–7.
    Axis { device: SdlId, axis: u8, value: i16 },
    /// A joystick button: a Radio's CH9–CH32 are buttons 0–23.
    Button {
        device: SdlId,
        button: u8,
        down: bool,
    },
    /// A gamepad stick or trigger moved, by SDL's position.
    PadAxis {
        device: SdlId,
        axis: PadAxis,
        value: i16,
    },
    /// A gamepad button, by SDL's position.
    PadButton {
        device: SdlId,
        button: PadButton,
        down: bool,
    },
    /// A report that changed nothing we read, such as a DualSense's
    /// motion-sensor reading: proof the device is still there.
    Report { device: SdlId },
}

impl Raw {
    pub fn device(&self) -> SdlId {
        match self {
            Raw::Added { device, .. }
            | Raw::Removed { device }
            | Raw::Axis { device, .. }
            | Raw::Button { device, .. }
            | Raw::PadAxis { device, .. }
            | Raw::PadButton { device, .. }
            | Raw::Report { device } => *device,
        }
    }

    /// Whether this is a control's value, as opposed to a device coming,
    /// going or reporting.
    pub fn is_value(&self) -> bool {
        matches!(
            self,
            Raw::Axis { .. } | Raw::Button { .. } | Raw::PadAxis { .. } | Raw::PadButton { .. }
        )
    }
}

/// Everything SDL reported in one poll of the input thread, stamped with the
/// computer's clock when the thread read it: the time since the thread
/// started.
#[derive(Clone, Debug, PartialEq)]
pub struct Batch {
    pub at: Duration,
    pub events: Vec<Raw>,
}

/// Throws away every value that arrives in the same instant as its device's
/// removal (ADR-0003). At unplug, SDL sets each axis back to its first value,
/// so a Pocket's switch channels jump to centre just as it goes, which could
/// flip a switch (#18). Returns how many values were dropped.
pub fn drop_values_arriving_with_removals(batch: &mut Batch) -> usize {
    let removed: Vec<SdlId> = batch
        .events
        .iter()
        .filter_map(|raw| match raw {
            Raw::Removed { device } => Some(*device),
            _ => None,
        })
        .collect();
    let before = batch.events.len();
    batch
        .events
        .retain(|raw| !(raw.is_value() && removed.contains(&raw.device())));
    before - batch.events.len()
}

/// Every control's current value on one device. SDL reports only changes, so
/// this is where they're kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceState {
    /// Joystick axes, in SDL's units (−32768…32767).
    pub axes: Vec<i16>,
    /// Joystick buttons.
    pub buttons: Vec<bool>,
    /// The gamepad view, when SDL reads the device as a gamepad.
    pub pad: Option<PadState>,
}

/// A gamepad's sticks, triggers (0…32767) and buttons, by SDL's position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PadState {
    pub axes: [i16; PadAxis::COUNT],
    pub buttons: [bool; PadButton::COUNT],
}

impl PadState {
    /// Everything at rest: sticks centred, triggers released, no button
    /// down.
    pub fn at_rest() -> PadState {
        PadState {
            axes: [0; PadAxis::COUNT],
            buttons: [false; PadButton::COUNT],
        }
    }

    pub fn axis(&self, axis: PadAxis) -> i16 {
        self.axes[axis.index()]
    }

    pub fn button(&self, button: PadButton) -> bool {
        self.buttons[button.index()]
    }
}

impl DeviceState {
    /// A device with these many joystick axes and buttons, all at 0 and up,
    /// and a gamepad view if SDL reads it as a gamepad.
    pub fn new(axes: usize, buttons: usize, gamepad: bool) -> DeviceState {
        DeviceState {
            axes: vec![0; axes],
            buttons: vec![false; buttons],
            pad: gamepad.then(PadState::at_rest),
        }
    }

    /// Takes one change. Returns whether a value changed.
    pub fn apply(&mut self, raw: &Raw) -> bool {
        fn set<T: PartialEq + Copy>(slot: Option<&mut T>, value: T) -> bool {
            match slot {
                Some(old) if *old != value => {
                    *old = value;
                    true
                }
                _ => false,
            }
        }
        match raw {
            Raw::Axis { axis, value, .. } => set(self.axes.get_mut(usize::from(*axis)), *value),
            Raw::Button { button, down, .. } => {
                set(self.buttons.get_mut(usize::from(*button)), *down)
            }
            Raw::PadAxis { axis, value, .. } => set(
                self.pad.as_mut().map(|pad| &mut pad.axes[axis.index()]),
                *value,
            ),
            Raw::PadButton { button, down, .. } => set(
                self.pad
                    .as_mut()
                    .map(|pad| &mut pad.buttons[button.index()]),
                *down,
            ),
            Raw::Added { .. } | Raw::Removed { .. } | Raw::Report { .. } => false,
        }
    }

    /// A Radio channel's axis (CH1–CH8), if the device has it.
    pub fn channel_axis(&self, channel: u8) -> Option<i16> {
        let index = usize::from(channel.checked_sub(1)?);
        self.axes.get(index).copied()
    }

    /// A Radio button channel (CH9–CH32), if the device has it.
    pub fn channel_button(&self, channel: u8) -> Option<bool> {
        let index = usize::from(channel.checked_sub(9)?);
        self.buttons.get(index).copied()
    }
}
