//! Telling whether a Radio still transmits (#30, decision 5).
//!
//! With its RF module off, a Pocket runs its mixer at 1 kHz and many of its
//! value changes come exactly 1 ms apart. With RF on, EdgeTX runs the mixer,
//! and sends a USB report, once per ELRS packet: at 250 Hz no two changes
//! come closer than 4 ms. So a Radio counts as still transmitting when, while
//! its sticks move, no two of its value changes come closer than about 3 ms.
//!
//! Counting changes isn't enough: with EdgeTX's ADC filter on, a Pocket with
//! RF off skips many reports. Setup uses this to say "Your radio sends 250
//! updates a second. Turn RF off for 1000", and the Hub for its RF-on
//! Pre-flight Warning: a bound quad nearby would arm with you.

use std::time::Duration;

/// Two value changes closer than this mean the radio isn't transmitting.
pub const CLOSEST_WHILE_TRANSMITTING: Duration = Duration::from_millis(3);

/// Gaps longer than this mean the sticks stopped, so they don't count.
pub const MOVING: Duration = Duration::from_millis(30);

/// How many gaps between changes, while the sticks move, it takes to tell.
pub const GAPS_TO_TELL: usize = 40;

/// Whether a Radio still transmits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transmitting {
    /// No two changes came closer than about 3 ms: RF seems to be on. The
    /// closest two are given.
    Yes { closest: Duration },
    /// Some changes came closer: RF is off.
    No { closest: Duration },
    /// The sticks haven't moved enough to tell.
    CantTellYet,
}

/// Tells, from the moments a Radio's stick channels changed (each poll that
/// brought a change counts once, in time order).
pub fn still_transmitting(changes: &[Duration]) -> Transmitting {
    let gaps: Vec<Duration> = changes
        .windows(2)
        .map(|pair| pair[1].saturating_sub(pair[0]))
        .filter(|gap| !gap.is_zero() && *gap <= MOVING)
        .collect();
    let Some(&closest) = gaps.iter().min() else {
        return Transmitting::CantTellYet;
    };
    if closest < CLOSEST_WHILE_TRANSMITTING {
        // One close pair is proof enough, however few changes came.
        Transmitting::No { closest }
    } else if gaps.len() < GAPS_TO_TELL {
        Transmitting::CantTellYet
    } else {
        Transmitting::Yes { closest }
    }
}
