//! The `aux` paste: a Radio pilot pastes the `aux` lines of their quad's
//! `diff`, `diff all` or `dump`, and the Arm, Flight Mode and Crash Flip
//! switches are read from them (#19 §5, [ADR-0017]).
//!
//! An `aux` line is one of Betaflight's mode activation conditions: `aux
//! <slot> <mode> <AUX channel> <from µs> <to µs> <logic> <linked mode>`, the
//! mode by its permanent id and the AUX channel counted from 0, so AUX*n* is
//! the radio's CH(*n*+4). Each switch position a radio sends (988, 1500 and
//! 2012 µs, ExpressLRS's low, middle and high) is tested against the pasted
//! ranges as Betaflight does: a mode is on when its channel's value is at
//! least the range's start and below its end (`isRangeActive` in
//! `src/main/fc/rc_modes.c`), after Betaflight has rounded both to its 25 µs
//! steps. For the Flight Mode, Angle beats Horizon, and Acro is wherever
//! neither is on.
//!
//! ARM, ANGLE, HORIZON and FLIP OVER AFTER CRASH are read; every other mode
//! is listed as ignored, such as the Cetus X's BEEPER. AND logic and linked
//! modes on those four are refused with a note, as is anything else the
//! fixed switch layout can't hold, such as Arm on two switches.
//!
//! What comes out is each switch's AUX channel and what each of its three
//! positions means. `opendrone-pack` turns it into an Input Device
//! profile's `[switches]`.
//!
//! [ADR-0017]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0017-switches-reach-the-flight-controller-with-fixed-meanings.md

use super::{CliText, Command, Refusal};

/// The three switch positions every switch is tested in, in µs: a radio's
/// low, middle and high (ExpressLRS's ±100% and centre).
pub const POSITIONS: [u16; 3] = [988, 1500, 2012];

/// The position names, in [`POSITIONS`]' order.
pub const POSITION_NAMES: [&str; 3] = ["low", "middle", "high"];

/// What one switch position picks on the Flight Mode switch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlightMode {
    Acro,
    Horizon,
    Angle,
}

/// An on/off switch read from the paste: Arm or Crash Flip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuxSwitch {
    /// Its AUX channel: 1 is AUX1.
    pub aux: u8,
    /// Whether the mode is on at each of [`POSITIONS`].
    pub on: [bool; 3],
}

impl AuxSwitch {
    /// The radio channel: AUX*n* is CH(*n*+4).
    pub fn channel(&self) -> u8 {
        self.aux + 4
    }
}

/// The Flight Mode switch read from the paste.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuxFlightModeSwitch {
    /// Its AUX channel: 1 is AUX1.
    pub aux: u8,
    /// The Flight Mode at each of [`POSITIONS`].
    pub modes: [FlightMode; 3],
}

impl AuxFlightModeSwitch {
    /// The radio channel: AUX*n* is CH(*n*+4).
    pub fn channel(&self) -> u8 {
        self.aux + 4
    }
}

/// A mode the paste sets that OpenDrone's fixed layout doesn't carry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IgnoredMode {
    /// Betaflight's name for it, such as `BEEPER`.
    pub mode: String,
    /// The `aux` line, and its line number from 1.
    pub text: String,
    pub line: usize,
}

/// The switches read from pasted `aux` lines. A switch the paste doesn't set
/// is `None`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuxPaste {
    pub arm: Option<AuxSwitch>,
    pub flight_mode: Option<AuxFlightModeSwitch>,
    pub crash_flip: Option<AuxSwitch>,
    /// The other modes set, in the paste's order.
    pub ignored: Vec<IgnoredMode>,
}

/// Betaflight's permanent mode ids and names (`src/main/msp/msp_box.c`).
const MODES: &[(u8, &str)] = &[
    (0, "ARM"),
    (1, "ANGLE"),
    (2, "HORIZON"),
    (3, "ALTHOLD"),
    (4, "ANTI GRAVITY"),
    (5, "MAG"),
    (6, "HEADFREE"),
    (7, "HEADADJ"),
    (8, "CAMSTAB"),
    (11, "POS HOLD"),
    (12, "PASSTHRU"),
    (13, "BEEPER"),
    (15, "LEDLOW"),
    (17, "CALIB"),
    (19, "OSD DISABLE"),
    (20, "TELEMETRY"),
    (23, "SERVO1"),
    (24, "SERVO2"),
    (25, "SERVO3"),
    (26, "BLACKBOX"),
    (27, "FAILSAFE"),
    (28, "AIR MODE"),
    (29, "3D DISABLE"),
    (30, "FPV ANGLE MIX"),
    (31, "BLACKBOX ERASE"),
    (32, "CAMERA CONTROL 1"),
    (33, "CAMERA CONTROL 2"),
    (34, "CAMERA CONTROL 3"),
    (35, "FLIP OVER AFTER CRASH"),
    (36, "PREARM"),
    (37, "GPS BEEP SATELLITE COUNT"),
    (39, "VTX PIT MODE"),
    (40, "USER1"),
    (41, "USER2"),
    (42, "USER3"),
    (43, "USER4"),
    (44, "PID AUDIO"),
    (45, "PARALYZE"),
    (46, "GPS RESCUE"),
    (47, "ACRO TRAINER"),
    (48, "VTX CONTROL DISABLE"),
    (49, "LAUNCH CONTROL"),
    (50, "MSP OVERRIDE"),
    (51, "STICK COMMANDS DISABLE"),
    (52, "BEEPER MUTE"),
    (53, "READY"),
    (54, "LAP TIMER RESET"),
    (55, "CHIRP"),
    (56, "AUTOPILOT"),
    (57, "WP CAPTURE"),
];

const ARM: u8 = 0;
const ANGLE: u8 = 1;
const HORIZON: u8 = 2;
const CRASH_FLIP: u8 = 35;

/// Betaflight's name for a mode, or `mode <id>`.
pub fn mode_name(id: u8) -> String {
    MODES
        .iter()
        .find(|(i, _)| *i == id)
        .map_or_else(|| format!("mode {id}"), |(_, name)| (*name).to_string())
}

/// One `aux` line: a mode on while an AUX channel is in a range.
#[derive(Clone, Copy, Debug)]
struct Condition {
    mode: u8,
    /// The AUX channel, 0 for AUX1.
    aux: u8,
    /// The range in Betaflight's 25 µs steps from 900 µs.
    start: u16,
    end: u16,
    and: bool,
    linked: u8,
}

impl Condition {
    /// Whether Betaflight would count the mode on at `micros` on its
    /// channel.
    fn on_at(&self, micros: u16) -> bool {
        self.start < self.end && micros >= 900 + self.start * 25 && micros < 900 + self.end * 25
    }
}

/// A range end as Betaflight stores it: in 25 µs steps from 900 µs.
fn step(micros: u16) -> u16 {
    (micros.clamp(900, 2100) - 900) / 25
}

impl AuxPaste {
    /// Reads the switches from pasted CLI text holding `aux` lines. Refused,
    /// with a note for each, when the paste holds no usable `aux` line or
    /// sets Arm, Angle, Horizon or Flip Over After Crash in a way the fixed
    /// switch layout can't hold.
    pub fn read(text: &str) -> Result<AuxPaste, Refusal> {
        let cli = CliText::read(text);
        let mut conditions = Vec::new();
        let mut ignored = Vec::new();
        let mut notes = Vec::new();
        let mut any_aux = false;
        for line in &cli.lines {
            if line.command != Command::Other("aux") {
                continue;
            }
            any_aux = true;
            let numbers: Vec<Option<u16>> = line
                .text
                .split_whitespace()
                .skip(1)
                .map(|word| word.parse().ok())
                .collect();
            let fields: Option<Vec<u16>> = numbers.into_iter().collect();
            let condition = fields.filter(|f| (5..=7).contains(&f.len())).and_then(|f| {
                Some(Condition {
                    mode: u8::try_from(f[1]).ok()?,
                    aux: u8::try_from(f[2]).ok().filter(|aux| *aux < 28)?,
                    start: step(f[3]),
                    end: step(f[4]),
                    and: f.get(5).copied().unwrap_or(0) == 1,
                    linked: u8::try_from(f.get(6).copied().unwrap_or(0)).ok()?,
                })
            });
            let Some(condition) = condition else {
                notes.push(format!(
                    "Line {}: `{}` isn't an `aux` line as Betaflight prints it: `aux <slot> <mode> <AUX channel from 0> <from µs> <to µs> <logic> <linked mode>`.",
                    line.number, line.text
                ));
                continue;
            };
            // An unused slot: a dump lists all twenty, most of them empty.
            if condition.start >= condition.end && condition.linked == 0 {
                continue;
            }
            if [ARM, ANGLE, HORIZON, CRASH_FLIP].contains(&condition.mode) {
                let name = mode_name(condition.mode);
                if condition.and {
                    notes.push(format!(
                        "Line {}: {name} uses AND logic, which OpenDrone can't copy: each of Arm, Flight Mode and Crash Flip comes from one switch (ADR-0017).",
                        line.number
                    ));
                } else if condition.linked != 0 {
                    notes.push(format!(
                        "Line {}: {name} is linked to {}, which OpenDrone can't copy: each of Arm, Flight Mode and Crash Flip comes from one switch (ADR-0017).",
                        line.number,
                        mode_name(condition.linked)
                    ));
                } else {
                    conditions.push(condition);
                }
            } else {
                ignored.push(IgnoredMode {
                    mode: mode_name(condition.mode),
                    text: line.text.to_string(),
                    line: line.number,
                });
            }
        }
        if !any_aux {
            return Err(Refusal::one(
                "There are no `aux` lines here: paste the `aux` lines of your quad's `diff all`, `diff` or `dump`.",
            ));
        }
        let arm = on_off(&conditions, ARM, &mut notes);
        let crash_flip = on_off(&conditions, CRASH_FLIP, &mut notes);
        let flight_mode = flight_mode(&conditions, &mut notes);
        if !notes.is_empty() {
            return Err(Refusal(notes));
        }
        Ok(AuxPaste {
            arm,
            flight_mode,
            crash_flip,
            ignored,
        })
    }
}

/// The one AUX channel a mode's conditions use, or a note when they use more.
fn channel_of(conditions: &[&Condition], notes: &mut Vec<String>) -> Option<u8> {
    let first = conditions.first()?;
    let mut channels: Vec<u8> = conditions.iter().map(|c| c.aux).collect();
    channels.sort_unstable();
    channels.dedup();
    if channels.len() > 1 {
        let list: Vec<String> = channels
            .iter()
            .map(|aux| format!("AUX{}", aux + 1))
            .collect();
        notes.push(format!(
            "{} is on more than one switch ({}): OpenDrone reads each of Arm, Flight Mode and Crash Flip from one switch (ADR-0017).",
            mode_name(first.mode),
            list.join(" and ")
        ));
        return None;
    }
    Some(first.aux)
}

/// Whether a mode is on at each switch position: any of its conditions on.
fn on_at_positions(conditions: &[&Condition]) -> [bool; 3] {
    POSITIONS.map(|micros| conditions.iter().any(|c| c.on_at(micros)))
}

fn on_off(conditions: &[Condition], mode: u8, notes: &mut Vec<String>) -> Option<AuxSwitch> {
    let mine: Vec<&Condition> = conditions.iter().filter(|c| c.mode == mode).collect();
    let aux = channel_of(&mine, notes)?;
    let on = on_at_positions(&mine);
    if on == [false; 3] {
        notes.push(format!(
            "{} on AUX{} is on at none of 988, 1500 and 2012 µs, the positions a radio's switch sends.",
            mode_name(mode),
            aux + 1
        ));
        return None;
    }
    Some(AuxSwitch { aux: aux + 1, on })
}

fn flight_mode(conditions: &[Condition], notes: &mut Vec<String>) -> Option<AuxFlightModeSwitch> {
    let angle: Vec<&Condition> = conditions.iter().filter(|c| c.mode == ANGLE).collect();
    let horizon: Vec<&Condition> = conditions.iter().filter(|c| c.mode == HORIZON).collect();
    let both: Vec<&Condition> = angle.iter().chain(&horizon).copied().collect();
    if both.is_empty() {
        return None;
    }
    let mut channels: Vec<u8> = both.iter().map(|c| c.aux).collect();
    channels.sort_unstable();
    channels.dedup();
    if channels.len() > 1 {
        let list: Vec<String> = channels
            .iter()
            .map(|aux| format!("AUX{}", aux + 1))
            .collect();
        notes.push(format!(
            "Angle and Horizon are on more than one switch ({}): OpenDrone reads the Flight Mode from one switch (ADR-0017).",
            list.join(" and ")
        ));
        return None;
    }
    let (angle_on, horizon_on) = (on_at_positions(&angle), on_at_positions(&horizon));
    let modes = [0, 1, 2].map(|i| {
        if angle_on[i] {
            FlightMode::Angle
        } else if horizon_on[i] {
            FlightMode::Horizon
        } else {
            FlightMode::Acro
        }
    });
    if modes == [FlightMode::Acro; 3] {
        notes.push(format!(
            "Angle and Horizon on AUX{} are on at none of 988, 1500 and 2012 µs, the positions a radio's switch sends.",
            channels[0] + 1
        ));
        return None;
    }
    Some(AuxFlightModeSwitch {
        aux: channels[0] + 1,
        modes,
    })
}
