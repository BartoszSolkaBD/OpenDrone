//! A Quad's Tune, `tune.txt`: Betaflight CLI `set` lines with 2026.6 names
//! and Betaflight's own units, each marked with where its value came from
//! ([ADR-0015]).
//!
//! The file stays Betaflight CLI text, so it could be pasted into a real quad,
//! and it has no `format = N` line: its header comment says which Betaflight
//! it follows. The Flight Controller tickets read its values; the Pack checker
//! checks its shape, its marks, and the two settings that must agree with the
//! Quad definition (`motor_poles` and `yaw_motors_reversed`).
//!
//! [ADR-0015]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0015-tune-is-betaflight-cli-text-spelling-out-every-setting.md

use std::collections::BTreeMap;

use crate::document::{Problem, Problems};

/// A read Tune: every setting by its Betaflight name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tune {
    /// The file it was read from, for problems.
    pub file: String,
    pub settings: BTreeMap<String, TuneSetting>,
}

/// One `set` line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TuneSetting {
    /// The value as Betaflight writes it, such as `12` or `OFF`.
    pub value: String,
    /// Where the value came from, such as `diff` or `4.3 default`.
    pub mark: String,
    pub line: usize,
}

impl Tune {
    /// A problem on the line of `setting`, or on the whole file when the Tune
    /// doesn't set it.
    pub fn problem(&self, setting: &str, sentence: impl Into<String>) -> Problem {
        let line = self.settings.get(setting).map_or(0, |s| s.line);
        Problem::of(&self.file, line, sentence)
    }
}

const MARKS: &str = "`diff`, a version's default such as `4.3 default`, `ADR-0008` or `hand-set: <reason>`, optionally followed by `(was <old name>)`";

/// Reads `tune.txt`, listing every problem at once.
pub fn read_tune(file: &str, text: &str) -> Result<Tune, Problems> {
    let (tune, problems) = read_tune_as_far_as_it_goes(file, text);
    problems.or(tune)
}

/// Reads every `set` line that is fine, and lists the problems with the rest.
pub(crate) fn read_tune_as_far_as_it_goes(file: &str, text: &str) -> (Tune, Problems) {
    let mut problems = Problems::new();
    let mut settings = BTreeMap::new();
    let problem = |line: usize, sentence: String| Problem::of(file, line, sentence);
    let mut seen_anything = false;
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !seen_anything && !trimmed.starts_with('#') {
            problems.push(problem(
                line,
                "a Tune starts with comment lines (#) saying where it came from: the Betaflight version, the quad and the date".into(),
            ));
        }
        seen_anything = true;
        if trimmed.starts_with('#') {
            continue;
        }
        let Some(rest) = trimmed.strip_prefix("set ") else {
            let command = trimmed.split_whitespace().next().unwrap_or(trimmed);
            problems.push(problem(
                line,
                format!(
                    "`{command}` isn't a `set` line: a Tune holds only `set` lines and comments, and Betaflight's other commands, such as `aux` lines and rate profiles, belong to the pilot (ADR-0015)"
                ),
            ));
            continue;
        };
        let (setting, mark) = match rest.split_once('#') {
            Some((setting, mark)) => (setting, Some(mark.trim())),
            None => (rest, None),
        };
        let Some((name, value)) = setting.split_once('=') else {
            problems.push(problem(
                line,
                "a `set` line is written `set <name> = <value>`, such as `set motor_poles = 12`"
                    .into(),
            ));
            continue;
        };
        let (name, value) = (name.trim(), value.trim());
        if name.is_empty()
            || !name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        {
            problems.push(problem(
                line,
                format!(
                    "\"{name}\" isn't a Betaflight setting name: they're lowercase words joined by underscores, such as `motor_poles`"
                ),
            ));
            continue;
        }
        if value.is_empty() {
            problems.push(problem(line, format!("`{name}` has no value")));
            continue;
        }
        let Some(mark) = mark.filter(|mark| !mark.is_empty()) else {
            problems.push(problem(
                line,
                format!(
                    "`{name}` has no mark saying where its value came from: end the line with # and {MARKS}"
                ),
            ));
            continue;
        };
        if let Some(sentence) = refuse_mark(mark) {
            problems.push(problem(line, format!("`{name}`'s mark {sentence}")));
            continue;
        }
        if let Some(earlier) = settings.get(name).map(|s: &TuneSetting| s.line) {
            problems.push(problem(
                line,
                format!("`{name}` is set twice, here and on line {earlier}"),
            ));
            continue;
        }
        settings.insert(
            name.to_string(),
            TuneSetting {
                value: value.to_string(),
                mark: mark.to_string(),
                line,
            },
        );
    }
    let tune = Tune {
        file: file.to_string(),
        settings,
    };
    (tune, problems)
}

/// `None` for a known mark (the text after `#`); otherwise what's wrong. A
/// mark may go on after a `;` with a note, such as `diff; must match the
/// Quad's motor poles`.
fn refuse_mark(mark: &str) -> Option<String> {
    let head = mark.split(';').next().unwrap_or("").trim();
    let head = match head.split_once("(was ") {
        Some((before, old)) => {
            let old = old.trim_end_matches(')').trim();
            if old.is_empty() || !head.ends_with(')') {
                return Some(format!(
                    "\"{mark}\" names its old setting name as `(was <old name>)`"
                ));
            }
            before.trim()
        }
        None => head,
    };
    let known = head == "diff"
        || head == "ADR-0008"
        || head
            .strip_prefix("hand-set:")
            .is_some_and(|reason| !reason.trim().is_empty())
        || head.strip_suffix(" default").is_some_and(|version| {
            !version.is_empty()
                && version.split('.').count() >= 2
                && version
                    .split('.')
                    .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
        });
    if known {
        None
    } else {
        Some(format!(
            "\"{mark}\" isn't one OpenDrone knows: write {MARKS}"
        ))
    }
}
