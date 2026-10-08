//! Auto-arm: the sim-only Assist that arms the Quad on the first throttle
//! raise, for a pilot with no Arm switch (#13, #21, #32 §4).
//!
//! Betaflight has no such feature, so Auto-arm works through Betaflight's own
//! arming, as a radio's logical switch would: it drives the Arm switch
//! (AUX1) in each Radio Link frame, and the Flight Controller's arming checks
//! decide. With Auto-arm on, no Arm switch is bound, so the pilot's AUX1 is
//! never read.
//!
//! - **Arming.** When the throttle rises from below `min_check` and the
//!   Flight Controller's last loop named nothing blocking arming (so the
//!   ESCs have beeped ready, the Quad is tilted no further than
//!   `small_angle`, and the link and Failsafe allow it), Auto-arm turns Arm
//!   on. Betaflight arms only on a frame whose throttle is low, one frame
//!   after the switch goes on, so Auto-arm holds the throttle at the bottom
//!   in that frame and the next: the pilot's throttle reaches the Flight
//!   Controller two frames late at the moment of arming, 8 ms at 250 Hz. If
//!   arming is refused all the same, Auto-arm lets go and waits for the
//!   throttle to go low and rise again.
//! - **Disarming.** Arm then stays on: with no Arm switch, only Reset (which
//!   powers the Flight Controller up afresh) or a Failsafe drop disarms the
//!   Quad. After a drop Arm goes off, and the Quad arms again once Failsafe
//!   allows it and the throttle has gone low and risen again.

use opendrone_flight_controller::{Channel, Channels, FlightController};
use opendrone_maths::Fingerprinter;

/// How many frames Auto-arm holds the throttle low while arming: Betaflight
/// turns Arm on in the modes with the first and arms with the second.
const ARMING_FRAMES: u8 = 2;

/// Auto-arm's state, between two frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AutoArm {
    /// Arm off, waiting for the throttle to rise; true when the last frame's
    /// throttle was low.
    Waiting { throttle_was_low: bool },
    /// Arm on and the throttle held low, until the Flight Controller arms:
    /// how many frames so far.
    Arming { frames: u8 },
    /// Arm on.
    Armed,
}

impl AutoArm {
    /// Auto-arm for a Flight Controller that starts armed (a mid-air start)
    /// or not. Disarmed, it waits for a raise from low, so a throttle already
    /// up must come down first.
    pub(crate) fn new(armed: bool) -> AutoArm {
        if armed {
            AutoArm::Armed
        } else {
            AutoArm::Waiting {
                throttle_was_low: false,
            }
        }
    }

    /// The frame the Flight Controller gets in place of `frame`: Arm (AUX1)
    /// as Auto-arm drives it, and the throttle held low while arming.
    pub(crate) fn shape(&mut self, frame: Channels, fc: &FlightController) -> Channels {
        let low = frame.throttle.micros() < f64::from(fc.tune().min_check);
        let waiting = AutoArm::Waiting {
            throttle_was_low: low,
        };
        let (next, arm, hold_throttle) = match *self {
            AutoArm::Armed if fc.armed() => (AutoArm::Armed, true, false),
            AutoArm::Arming { .. } if fc.armed() => (AutoArm::Armed, true, false),
            AutoArm::Arming { frames } if frames < ARMING_FRAMES => {
                (AutoArm::Arming { frames: frames + 1 }, true, true)
            }
            AutoArm::Armed | AutoArm::Arming { .. } => (waiting, false, false),
            AutoArm::Waiting { throttle_was_low } => {
                let raised = throttle_was_low && !low;
                let allowed = !fc.armed() && !fc.debug().arming_blocks.any();
                if raised && allowed {
                    (AutoArm::Arming { frames: 1 }, true, true)
                } else {
                    (waiting, false, false)
                }
            }
        };
        *self = next;
        Channels {
            arm: Channel::from_switch(arm),
            throttle: if hold_throttle {
                Channel::LOW
            } else {
                frame.throttle
            },
            ..frame
        }
    }

    pub(crate) fn write_fingerprint(&self, f: &mut Fingerprinter) {
        match *self {
            AutoArm::Waiting { throttle_was_low } => {
                f.write_u64(0);
                f.write_u64(u64::from(throttle_was_low));
            }
            AutoArm::Arming { frames } => {
                f.write_u64(1);
                f.write_u64(u64::from(frames));
            }
            AutoArm::Armed => f.write_u64(2),
        }
    }
}
