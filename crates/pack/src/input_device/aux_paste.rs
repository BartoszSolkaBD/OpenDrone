//! "Paste from Betaflight CLI…" for a Radio's switches (#19 §5): pasted
//! `aux` lines become the `[switches]` of a Radio's Input Device profile.
//!
//! The Flight Controller's Betaflight CLI translator reads the lines as
//! Betaflight would (`opendrone_flight_controller::cli::AuxPaste`): for Arm,
//! Flight Mode and Crash Flip, the AUX channel and what each of its three
//! positions means. This turns that into the profile's switches: AUX*n* is
//! CH(*n*+4), Arm and Crash Flip are on in one position, and the Flight Mode
//! switch gives each position its mode. A switch a profile can't hold is
//! refused with a note, such as Arm on in two positions, or a switch on CH9
//! and up, which a Radio sends as buttons (pressed is high).

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
}

/// Reads pasted `aux` lines as the switches of a Radio's Input Device profile, or says, a note
/// each, why they can't be.
pub fn switches_from_aux(text: &str) -> Result<PastedSwitches, Vec<String>> {
    let pasted = AuxPaste::read(text).map_err(|refusal| refusal.0)?;
    let mut notes = Vec::new();
    let arm = pasted.arm.and_then(|s| on_off("Arm", s, &mut notes));
    let crash_flip = pasted
        .crash_flip
        .and_then(|s| on_off("Crash Flip", s, &mut notes));
    let flight_mode = pasted.flight_mode.and_then(|s| flight_mode(s, &mut notes));
    if !notes.is_empty() {
        return Err(notes);
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
    })
}

fn on_off(name: &str, switch: AuxSwitch, notes: &mut Vec<String>) -> Option<OnOffSwitch> {
    let channel = switch.channel();
    let on: Vec<usize> = (0..3).filter(|&i| switch.on[i]).collect();
    if on.len() > 1 {
        let positions: Vec<&str> = on.iter().map(|&i| POSITION_NAMES[i]).collect();
        notes.push(format!(
            "{name} on AUX{} (CH{channel}) is on at {}: a profile's {name} switch is on in one position.",
            switch.aux,
            positions.join(" and ")
        ));
        return None;
    }
    let &position = on.first()?;
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
    notes.push(format!(
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
