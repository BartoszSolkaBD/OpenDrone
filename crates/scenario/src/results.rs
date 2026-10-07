//! The Results file written beside each Scenario, and the per-step
//! fingerprint files CI compares across computers.

use std::fmt::Write as _;

use opendrone_maths::Fingerprint;

use crate::read::{BasisKind, Scenario};
use crate::run::Outcome;

/// How many checkpoints a Results file lists: the whole state's fingerprint
/// after each tenth of the run.
const CHECKPOINTS: u64 = 10;

/// The Results file's text: every Expectation's measured value to 3
/// significant figures, the automatic "no broken numbers" check, and the
/// fingerprints of the Quad, the Map and the run. Everything in it comes out
/// the same on every computer.
pub fn results_text(scenario: &Scenario, outcome: &Outcome) -> String {
    let mut text = String::new();
    let file_name = scenario.file.rsplit('/').next().unwrap_or(&scenario.file);
    let _ = writeln!(
        text,
        "# Results of {file_name}, written by the Scenario runner (`cargo scenarios run`).\n\
         # Don't edit this file by hand: CI checks that it is up to date. Each value is\n\
         # what this run measured, to 3 significant figures."
    );
    let _ = writeln!(text, "format   = 1");
    let _ = writeln!(text, "scenario = {}", quoted(&scenario.name));
    for e in &outcome.expectations {
        let _ = writeln!(text);
        let _ = writeln!(text, "[[expect]]");
        let _ = writeln!(text, "what     = {}", quoted(&e.description));
        let _ = writeln!(text, "basis    = {}", quoted(e.basis.word()));
        let _ = writeln!(text, "expected = {}", quoted(&e.expected));
        let _ = writeln!(text, "measured = {}", quoted(&e.measured));
    }
    let _ = writeln!(text);
    let _ = writeln!(text, "[[expect]]");
    let _ = writeln!(
        text,
        "what     = {}",
        quoted("no broken numbers: every number in the state is a real number after every step")
    );
    let _ = writeln!(text, "basis    = {}", quoted(BasisKind::Rule.word()));
    let _ = writeln!(text, "expected = \"none broken\"");
    let measured = outcome
        .first_broken_number
        .clone()
        .unwrap_or_else(|| "none broken".to_string());
    let _ = writeln!(text, "measured = {}", quoted(&measured));

    let steps = outcome.step_fingerprints.len().saturating_sub(1) as u64;
    let _ = writeln!(text);
    let _ = writeln!(text, "[fingerprints]");
    let _ = writeln!(
        text,
        "quad = \"{}\"   # {}: what the Simulation receives from it",
        outcome.quad.1, outcome.quad.0
    );
    let _ = writeln!(text, "map  = \"{}\"   # {}", outcome.map.1, outcome.map.0);
    let _ = writeln!(
        text,
        "run  = \"{}\"   # the whole state after each of {steps} steps at {} Hz, in order",
        outcome.run_fingerprint(),
        outcome.physics_rate
    );
    let _ = writeln!(text);
    let _ = writeln!(
        text,
        "[fingerprints.checkpoints]   # the whole state after each tenth of the run"
    );
    let mut last = None;
    for k in 1..=CHECKPOINTS {
        let tick = k * steps / CHECKPOINTS;
        if tick == 0 || last == Some(tick) {
            continue;
        }
        last = Some(tick);
        let seconds = tick as f64 / f64::from(outcome.physics_rate);
        let _ = writeln!(
            text,
            "\"{seconds} s\" = \"{}\"",
            outcome.step_fingerprints[tick as usize]
        );
    }
    text
}

/// The per-step fingerprint file for the agreement check: one line per step,
/// the step's number and the whole state's fingerprint after it.
pub fn fingerprints_text(relative: &str, outcome: &Outcome) -> String {
    let mut text = String::with_capacity(outcome.step_fingerprints.len() * 24 + 200);
    let _ = writeln!(
        text,
        "# Fingerprints of the Scenario {relative}: one line per step, the step's\n\
         # number and the whole state's fingerprint after it (0 is the start).\n\
         # physics rate: {} Hz",
        outcome.physics_rate
    );
    for (tick, fingerprint) in outcome.step_fingerprints.iter().enumerate() {
        let _ = writeln!(text, "{tick} {fingerprint}");
    }
    text
}

/// Reads a fingerprint file back: the physics rate and every step's
/// fingerprint, in order.
pub fn read_fingerprints(text: &str) -> Option<(u32, Vec<Fingerprint>)> {
    let mut rate = None;
    let mut steps = Vec::new();
    for line in text.lines() {
        if let Some(comment) = line.strip_prefix('#') {
            if let Some(hz) = comment.trim().strip_prefix("physics rate:") {
                rate = hz.trim().strip_suffix(" Hz").and_then(|hz| hz.parse().ok());
            }
            continue;
        }
        let (tick, fingerprint) = line.split_once(' ')?;
        if tick.parse::<usize>().ok()? != steps.len() {
            return None;
        }
        steps.push(Fingerprint(u64::from_str_radix(fingerprint, 16).ok()?));
    }
    Some((rate?, steps))
}

/// Text in TOML quotes.
fn quoted(text: &str) -> String {
    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('"');
    for c in text.chars() {
        match c {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\n' => quoted.push_str("\\n"),
            c if c.is_control() => {
                let _ = write!(quoted, "\\u{:04X}", c as u32);
            }
            c => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted
}
