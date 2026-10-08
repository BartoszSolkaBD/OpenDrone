//! "Paste from Betaflight CLI…" for a Radio's switches (#19 §5): pasted
//! `aux` lines become the `[switches]` of a Radio's Input Device profile.
//!
//! The Flight Controller's Betaflight CLI translator reads the lines as
//! Betaflight would (`opendrone_flight_controller::cli::AuxPaste`): for Arm,
//! Flight Mode and Crash Flip, the AUX channel and what each of its three
//! positions means. This turns that into the profile's switches: AUX*n* is
//! CH(*n*+4), Arm and Crash Flip are on in one position, and the Flight Mode
//! switch gives each position its mode.
//!
//! A profile's on/off switch is on in one position (#19 §8), but a pasted
//! range often covers two: ARM at 1500–2100 µs, say, on a 2-position switch,
//! which only ever sends low or high. On at middle and high is read as on at
//! high, and on at low and middle as on at low, each with a note. A switch
//! no profile can hold is refused with a note: on at every position, on at
//! both ends but not the middle, or on anywhere but high on CH9 and up,
//! which a Radio sends as buttons (pressed is high).

use opendrone_flight_controller::cli::AuxPaste;
use opendrone_flight_controller::cli::aux_paste::{
    AuxFlightModeSwitch, AuxSwitch, FlightMode as PastedMode, POSITION_NAMES,
};

use super::controls::{Position, RADIO_AXIS_CHANNELS};
use super::{FlightMode, FlightModeSwitch, OnOffSwitch, Switches};

/// The switches pasted `aux` lines give.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PastedSwitches {
    /// A switch the paste doesn't set is left without a source.
    pub switches: Switches,
    /// The modes the paste sets that OpenDrone ignores, each as Betaflight
    /// names it with its `aux` line, such as `BEEPER (aux 3 13 3 1700 2100 0
    /// 0)`.
    pub ignored: Vec<String>,
    /// What the paste read differently from Betaflight, in plain sentences,
    /// such as a switch on in two positions read as on in one.
    pub notes: Vec<String>,
}

/// Reads pasted `aux` lines as the switches of a Radio's Input Device profile, or says, a note
/// each, why they can't be.
pub fn switches_from_aux(text: &str) -> Result<PastedSwitches, Vec<String>> {
    let pasted = AuxPaste::read(text).map_err(|refusal| refusal.0)?;
    let mut refused = Vec::new();
    let mut notes = Vec::new();
    let arm = pasted
        .arm
        .and_then(|s| on_off("Arm", s, &mut refused, &mut notes));
    let crash_flip = pasted
        .crash_flip
        .and_then(|s| on_off("Crash Flip", s, &mut refused, &mut notes));
    let flight_mode = pasted
        .flight_mode
        .and_then(|s| flight_mode(s, &mut refused));
    if !refused.is_empty() {
        return Err(refused);
    }
    Ok(PastedSwitches {
        switches: Switches {
            arm,
            flight_mode,
            crash_flip,
        },
        ignored: pasted
            .ignored
            .iter()
            .map(|i| format!("{} ({})", i.mode, i.text))
            .collect(),
        notes,
    })
}

fn on_off(
    name: &str,
    switch: AuxSwitch,
    refused: &mut Vec<String>,
    notes: &mut Vec<String>,
) -> Option<OnOffSwitch> {
    let channel = switch.channel();
    let on: Vec<usize> = (0..3).filter(|&i| switch.on[i]).collect();
    let named = |positions: &[usize]| {
        positions
            .iter()
            .map(|&i| POSITION_NAMES[i])
            .collect::<Vec<_>>()
            .join(" and ")
    };
    // The one position a profile's switch is on in: two neighbouring
    // positions read as the end one, as a 2-position switch sends only its
    // ends.
    let position = match on.as_slice() {
        [one] => *one,
        [1, 2] | [0, 1] => {
            let end = if on == [1, 2] { 2 } else { 0 };
            notes.push(format!(
                "{name} on AUX{} (CH{channel}) is on at {}: a profile's switch is on in one position, so OpenDrone reads it as on at {}, as a 2-position switch sends.",
                switch.aux,
                named(&on),
                POSITION_NAMES[end]
            ));
            end
        }
        _ => {
            refused.push(format!(
                "{name} on AUX{} (CH{channel}) is on at {}: a profile's switch is on in one position, and no switch reads this as one.",
                switch.aux,
                named(&on)
            ));
            return None;
        }
    };
    if channel <= RADIO_AXIS_CHANNELS {
        let on = [Position::Low, Position::Middle, Position::High][position];
        return Some(OnOffSwitch::Channel { channel, on });
    }
    if position == 2 {
        return Some(OnOffSwitch::Channel {
            channel,
            on: Position::Pressed,
        });
    }
    refused.push(format!(
        "{name} on AUX{} is CH{channel}, which a Radio sends as a button, so it can only be on when pressed (high), not {}.",
        switch.aux, POSITION_NAMES[position]
    ));
    None
}

fn flight_mode(switch: AuxFlightModeSwitch, notes: &mut Vec<String>) -> Option<FlightModeSwitch> {
    let channel = switch.channel();
    if channel > RADIO_AXIS_CHANNELS {
        notes.push(format!(
            "The Flight Mode switch on AUX{} is CH{channel}, a button, but a Flight Mode switch needs a switch on CH1–CH8.",
            switch.aux
        ));
        return None;
    }
    let mode = |m: PastedMode| match m {
        PastedMode::Acro => FlightMode::Acro,
        PastedMode::Horizon => FlightMode::Horizon,
        PastedMode::Angle => FlightMode::Angle,
    };
    let [low, middle, high] = switch.modes.map(mode);
    Some(FlightModeSwitch::Channel {
        channel,
        low,
        middle,
        high,
    })
}
