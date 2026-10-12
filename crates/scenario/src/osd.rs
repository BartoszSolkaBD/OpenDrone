//! A Scenario's `[osd]` section (ADR-0022): the OSD options its
//! Expectations depend on, and those Expectations, which read OSD elements by
//! name, never grid cells. The runner works the OSD out beside the run, from
//! each Flight Controller loop, as the game does.
//!
//! ```toml
//! [osd]
//! timer_1    = "on time, whole seconds, alarm at 10 min"
//! timer_2    = "total armed time, whole seconds, alarm at 10 min"
//! warnings   = ["arming disabled", "fail safe", "crash flip", "battery critical", "low battery", "battery not full", "visual beeper"]
//! statistics = ["timer 2", "min battery", "max current", "used mah", "blackbox log number"]
//! alarms     = { rssi = "20%", link_quality = "80%", rssi_dbm = "-60 dBm", rsnr = "4 dB", capacity = "2200 mAh" }
//!
//! [[osd.expect]]
//! what  = "warnings"
//! at    = "2.6 s"
//! value = "FAIL SAFE"
//! basis = "source: …"
//! ```

use opendrone_flight_controller::DebugRecord;
use opendrone_flight_controller::Tune;
use opendrone_flight_controller::osd::{
    Osd, OsdLayout, OsdReading, Statistic, Timer, TimerPrecision, TimerSource, Warning,
};
use opendrone_pack::units::{self, Expected};

use crate::read::Basis;
use crate::run::Measured;

/// A Scenario's OSD: the options and the Expectations.
#[derive(Clone, Debug)]
pub struct OsdSection {
    /// OpenDrone's default layout with every element shown, and the
    /// section's options.
    pub layout: OsdLayout,
    pub expectations: Vec<OsdExpectation>,
}

/// One OSD Expectation.
#[derive(Clone, Debug)]
pub struct OsdExpectation {
    pub reading: OsdReading,
    pub when: OsdWhen,
    pub expected: OsdExpected,
    pub basis: Basis,
    /// The line its `[[osd.expect]]` starts on.
    pub line: usize,
    /// What it checks in words, such as "warnings at 2.6 s".
    pub description: String,
}

/// When an OSD Expectation reads the screen, in steps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OsdWhen {
    /// The screen after this step.
    At(u64),
    /// How long after `from` the reading first is `reads`, up to `to`.
    First { from: u64, to: u64, reads: String },
}

/// What an OSD Expectation expects.
#[derive(Clone, Debug, PartialEq)]
pub enum OsdExpected {
    /// This text, or blank for "(blank)".
    Text(String),
    /// When it first reads its text: a time with its tolerance.
    Time(Expected),
    /// That it never does.
    Never,
}

/// How a Scenario writes a blank reading.
pub const BLANK: &str = "(blank)";

/// A reading as a Scenario writes it: blank as "(blank)".
pub fn shown(text: &str) -> String {
    if text.is_empty() {
        BLANK.to_string()
    } else {
        text.to_string()
    }
}

/// A timer as a Scenario writes it: "on time, whole seconds, alarm at 10
/// min" (or "no alarm").
pub fn timer(text: &str) -> Result<Timer, String> {
    let help = "a timer is its source, its precision and its alarm, such as \"total armed time, whole seconds, alarm at 10 min\": the source is on time, total armed time, last armed time, on or armed time or launch time; the precision whole seconds, tenths or hundredths; the alarm \"alarm at <n> min\" or \"no alarm\"";
    let parts: Vec<&str> = text.split(',').map(str::trim).collect();
    let [source, precision, alarm] = parts.as_slice() else {
        return Err(help.to_string());
    };
    let source = TimerSource::ALL
        .into_iter()
        .find(|s| s.words() == *source)
        .ok_or(help)?;
    let precision = TimerPrecision::ALL
        .into_iter()
        .find(|p| p.words() == *precision)
        .ok_or(help)?;
    let alarm_minutes = match *alarm {
        "no alarm" => 0,
        other => other
            .strip_prefix("alarm at ")
            .and_then(|rest| rest.strip_suffix(" min"))
            .and_then(|n| n.trim().parse::<u8>().ok())
            .ok_or(help)?,
    };
    Ok(Timer {
        source,
        precision,
        alarm_minutes,
    })
}

/// A warning by its words, such as "fail safe".
pub fn warning(words: &str) -> Result<Warning, String> {
    Warning::ALL
        .into_iter()
        .find(|w| w.words() == words.trim())
        .ok_or_else(|| {
            let all: Vec<&str> = Warning::ALL.iter().map(|w| w.words()).collect();
            format!(
                "\"{words}\" isn't one of Betaflight's OSD warnings: {}",
                all.join(", ")
            )
        })
}

/// A statistic by its words, such as "min battery".
pub fn statistic(words: &str) -> Result<Statistic, String> {
    Statistic::DISPLAY_ORDER
        .into_iter()
        .find(|s| s.words() == words.trim())
        .ok_or_else(|| {
            let all: Vec<&str> = Statistic::DISPLAY_ORDER.iter().map(|s| s.words()).collect();
            format!(
                "\"{words}\" isn't one of Betaflight's OSD statistics: {}",
                all.join(", ")
            )
        })
}

/// An alarm's whole number, written with its unit, such as "-60 dBm".
pub fn alarm(text: &str, unit: &str, lowest: i64, highest: i64) -> Result<i64, String> {
    text.trim()
        .strip_suffix(unit)
        .and_then(|n| n.trim().parse::<i64>().ok())
        .filter(|n| (lowest..=highest).contains(n))
        .ok_or_else(|| {
            format!("write it as a whole number from {lowest} to {highest} with its unit, such as \"{highest}{}{unit}\"", if unit == "%" { "" } else { " " })
        })
}

/// The OSD worked out beside one run, with its Expectations' tallies.
pub(crate) struct OsdRun<'s> {
    osd: Osd,
    tallies: Vec<OsdTally<'s>>,
}

struct OsdTally<'s> {
    expectation: &'s OsdExpectation,
    /// What it read at its moment.
    read: Option<String>,
    /// For `first`: how long after the stretch's start, in seconds.
    first: Option<f64>,
}

impl<'s> OsdRun<'s> {
    /// The OSD for a run of a Scenario with an `[osd]` section, for a Quad
    /// named `quad_name` on `tune`.
    pub fn new(section: &'s OsdSection, quad_name: &str, tune: &Tune) -> OsdRun<'s> {
        OsdRun {
            osd: Osd::new(section.layout.clone(), quad_name, tune),
            tallies: section
                .expectations
                .iter()
                .map(|expectation| OsdTally {
                    expectation,
                    read: None,
                    first: None,
                })
                .collect(),
        }
    }

    /// After a step: the Flight Controller's loop, if one ran, reaches the
    /// OSD, and each Expectation reads the screen if it's due.
    pub fn see(&mut self, tick: u64, record: Option<&DebugRecord>, step: f64) {
        if let Some(record) = record {
            self.osd.see(record);
        }
        let screen = self.osd.screen();
        for tally in &mut self.tallies {
            match &tally.expectation.when {
                OsdWhen::At(at) if *at == tick => {
                    tally.read = Some(tally.expectation.reading.read(screen));
                }
                OsdWhen::First { from, to, reads }
                    if *from < tick && tick <= *to && tally.first.is_none() =>
                {
                    let now = tally.expectation.reading.read(screen);
                    if shown(&now) == *reads {
                        tally.first = Some((tick - from) as f64 * step);
                    }
                }
                _ => {}
            }
        }
    }

    /// Every Expectation's measured value and verdict.
    pub fn finish(self) -> Vec<Measured> {
        self.tallies
            .into_iter()
            .map(|tally| {
                let e = tally.expectation;
                let (expected, measured, passed) = match &e.expected {
                    OsdExpected::Text(text) => {
                        let read = tally.read.map(|r| shown(&r));
                        let passed = read.as_deref() == Some(text.as_str());
                        (
                            text.clone(),
                            read.unwrap_or_else(|| "nothing read".to_string()),
                            passed,
                        )
                    }
                    OsdExpected::Never => match tally.first {
                        None => ("never".to_string(), "never".to_string(), true),
                        Some(after) => (
                            "never".to_string(),
                            format!("after {} s", units::significant_figures(after, 3)),
                            false,
                        ),
                    },
                    OsdExpected::Time(expected) => match tally.first {
                        None => (expected.text(), "never".to_string(), false),
                        Some(after) => (
                            expected.text(),
                            expected.unit().write(after),
                            expected.accepts(after),
                        ),
                    },
                };
                Measured {
                    description: e.description.clone(),
                    basis: e.basis.kind,
                    expected,
                    measured,
                    passed,
                    line: e.line,
                }
            })
            .collect()
    }
}
