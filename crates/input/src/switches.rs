//! Virtual Switches: switch Channels driven by a Gamepad button or a keyboard
//! key instead of a physical switch (#19 §5, ADR-0017).
//!
//! - **Arm** toggles: each press flips it. It also turns itself off whenever
//!   the game reports a disarm (Reset, a Failsafe drop, a blocked arm, or
//!   Crash Flip's disarm), so the next press always means "arm now". The
//!   Flight Controller still sees a real off-then-on, so Betaflight's rules
//!   hold.
//! - **Crash Flip** is on only while held.
//! - **Flight Mode**, once a button or key is bound, steps through its modes
//!   on each press.
//!
//! Keys travel with the Flying Input Device: they drive the switches its
//! profile binds to keys, and its Channels carry them.

use crate::profile::{FlightModeSwitch, OnOffSwitch, Press, PressStyle, Switches};

/// The state of one device's Virtual Switches.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VirtualSwitches {
    arm: bool,
    crash_flip: bool,
    flight_mode_step: usize,
    /// Which bound buttons and keys were held at the last update, to tell a
    /// press from a hold.
    held: Vec<Press>,
}

impl VirtualSwitches {
    pub fn new() -> VirtualSwitches {
        VirtualSwitches::default()
    }

    /// Takes the current state of the bound buttons and keys: `is_held`
    /// says whether one is held right now. Returns whether a switch changed.
    pub fn update(&mut self, switches: &Switches, is_held: impl Fn(&Press) -> bool) -> bool {
        let before = (self.arm, self.crash_flip, self.flight_mode_step);
        let pressed = |source: &Press, held: &[Press]| is_held(source) && !held.contains(source);
        if let Some(OnOffSwitch::Virtual { source, style }) = &switches.arm {
            match style {
                PressStyle::Toggle if pressed(source, &self.held) => self.arm = !self.arm,
                PressStyle::Toggle => {}
                PressStyle::Hold => self.arm = is_held(source),
            }
        }
        if let Some(OnOffSwitch::Virtual { source, style }) = &switches.crash_flip {
            match style {
                PressStyle::Toggle if pressed(source, &self.held) => {
                    self.crash_flip = !self.crash_flip;
                }
                PressStyle::Toggle => {}
                PressStyle::Hold => self.crash_flip = is_held(source),
            }
        }
        if let Some(FlightModeSwitch::Steps { source, modes }) = &switches.flight_mode
            && pressed(source, &self.held)
            && !modes.is_empty()
        {
            self.flight_mode_step = (self.flight_mode_step + 1) % modes.len();
        }
        self.held = sources(switches).filter(|s| is_held(s)).collect();
        before != (self.arm, self.crash_flip, self.flight_mode_step)
    }

    /// The game reports a disarm: an Arm toggle turns itself off. Returns
    /// whether it was on.
    pub fn disarmed(&mut self) -> bool {
        std::mem::replace(&mut self.arm, false)
    }

    /// Whether the Arm Virtual Switch is on.
    pub fn arm(&self) -> bool {
        self.arm
    }

    /// Whether the Crash Flip Virtual Switch is on.
    pub fn crash_flip(&self) -> bool {
        self.crash_flip
    }

    /// Which of the Flight Mode switch's modes it has stepped to.
    pub fn flight_mode_step(&self) -> usize {
        self.flight_mode_step
    }
}

/// Every button and key that drives a Virtual Switch.
fn sources(switches: &Switches) -> impl Iterator<Item = Press> + '_ {
    let on_off = |s: &Option<OnOffSwitch>| match s {
        Some(OnOffSwitch::Virtual { source, .. }) => Some(source.clone()),
        _ => None,
    };
    let flight_mode = match &switches.flight_mode {
        Some(FlightModeSwitch::Steps { source, .. }) => Some(source.clone()),
        _ => None,
    };
    [
        on_off(&switches.arm),
        on_off(&switches.crash_flip),
        flight_mode,
    ]
    .into_iter()
    .flatten()
}
