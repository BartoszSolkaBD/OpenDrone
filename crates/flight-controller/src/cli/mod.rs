//! The Betaflight CLI translator: Betaflight CLI text in, settings out. It
//! opens no files; the caller hands it the text.
//!
//! - [`import_tune`] imports a `diff`, `diff all` or `dump` from Betaflight
//!   4.3 or newer as a 2026.6 Tune ([ADR-0008], [ADR-0015]), the way a Quad
//!   definition's `tune.txt` is made (`cargo xtask import-tune`).
//! - [`RatesPaste`] reads the pilot's Rates from the active rate profile of a
//!   pasted `diff`, `diff all` or `dump` (#13).
//! - [`AuxPaste`] reads the Arm, Flight Mode and Crash Flip switches from
//!   pasted `aux` lines (#19 §5, [ADR-0017]).
//!
//! The settings it knows, their names and their defaults in each Betaflight
//! version, are one table, [`table`]: one row a setting. Names, numbers and
//! defaults are facts read from each version's source (`src/main/cli/settings.c`
//! and the parameter groups' reset values); nothing of Betaflight's code is
//! copied (the Betaflight research §8.4).
//!
//! [ADR-0008]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0008-copy-betaflight-2026-6-translate-older-tunes.md
//! [ADR-0015]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0015-tune-is-betaflight-cli-text-spelling-out-every-setting.md
//! [ADR-0017]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0017-switches-reach-the-flight-controller-with-fixed-meanings.md

pub mod aux;
mod import;
mod rates;
pub mod table;

use core::fmt;

pub use aux::AuxPaste;
pub use import::{ImportedSetting, LeftOut, TuneImport, Where, Why, import_tune};
pub use rates::{NowAfter, RatesPaste};

/// A Betaflight version, as the CLI prints it on its `# version` line, such
/// as 4.3.0 or 2026.6.2.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl Version {
    /// Reads `4.3.0`, `2026.6.2` or `4.3`; a release candidate's suffix, as
    /// in `4.5.0-RC1`, is left aside.
    pub fn parse(text: &str) -> Option<Version> {
        let text = text.split('-').next().unwrap_or(text);
        let mut parts = text.split('.');
        let number = |part: Option<&str>| -> Option<u16> {
            let part = part?;
            if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            part.parse().ok()
        };
        let major = number(parts.next())?;
        let minor = number(parts.next())?;
        let patch = match parts.next() {
            Some(part) => number(Some(part))?,
            None => 0,
        };
        if parts.next().is_some() {
            return None;
        }
        Some(Version {
            major,
            minor,
            patch,
        })
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// The Betaflight versions whose settings the translator knows, oldest first:
/// 4.3, the oldest it imports ([ADR-0008]), up to 2026.6, the one our Flight
/// Controller copies. A version's patch releases share its defaults.
///
/// [ADR-0008]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0008-copy-betaflight-2026-6-translate-older-tunes.md
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Family {
    V4_3,
    V4_4,
    V2026_6,
}

impl Family {
    pub const ALL: [Family; 3] = [Family::V4_3, Family::V4_4, Family::V2026_6];

    /// How the version is named, such as `4.3`: marks say `4.3 default`.
    pub fn name(self) -> &'static str {
        match self {
            Family::V4_3 => "4.3",
            Family::V4_4 => "4.4",
            Family::V2026_6 => "2026.6",
        }
    }

    /// The release whose source the table's defaults were read from.
    pub fn defaults_from(self) -> &'static str {
        match self {
            Family::V4_3 => "4.3.0",
            Family::V4_4 => "4.4.0",
            Family::V2026_6 => "2026.6.2",
        }
    }

    /// Where this version sits in each row of the table.
    pub(crate) fn index(self) -> usize {
        match self {
            Family::V4_3 => 0,
            Family::V4_4 => 1,
            Family::V2026_6 => 2,
        }
    }

    /// The version a Betaflight release belongs to, or why the translator
    /// can't read it.
    pub fn of(version: Version) -> Result<Family, String> {
        match (version.major, version.minor) {
            (4, 3) => Ok(Family::V4_3),
            (4, 4) => Ok(Family::V4_4),
            (2026, 6) => Ok(Family::V2026_6),
            (major, minor) if (major, minor) < (4, 3) => Err(format!(
                "This is Betaflight {version}, which is older than 4.3: OpenDrone imports Betaflight 4.3 or newer (ADR-0008)."
            )),
            (major, minor) if (major, minor) > (2026, 6) => Err(format!(
                "This is Betaflight {version}, which is newer than 2026.6, the Betaflight OpenDrone copies: OpenDrone imports 4.3, 4.4 and 2026.6."
            )),
            _ => Err(format!(
                "This is Betaflight {version}. OpenDrone knows the settings and defaults of Betaflight 4.3, 4.4 and 2026.6 so far, not yet of {}.{}, so it can't translate it.",
                version.major, version.minor
            )),
        }
    }
}

/// Why a paste or an import is refused: one plain sentence or more.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal(pub Vec<String>);

impl Refusal {
    pub(crate) fn one(sentence: impl Into<String>) -> Refusal {
        Refusal(vec![sentence.into()])
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, sentence) in self.0.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            write!(f, "{sentence}")?;
        }
        Ok(())
    }
}

/// Where a line of CLI text sits: before any profile, or in a PID profile or
/// a rate profile, by number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Section {
    Master,
    Profile(u8),
    RateProfile(u8),
}

/// What one line of CLI text says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Command<'a> {
    /// `set <name> = <value>`.
    Set { name: &'a str, value: &'a str },
    /// `profile <n>`: from here on, PID profile n.
    Profile(u8),
    /// `rateprofile <n>`: from here on, rate profile n.
    RateProfile(u8),
    /// Any other command, by its first word, such as `aux` or `feature`.
    Other(&'a str),
}

/// One line of CLI text that isn't a comment or blank.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Line<'a> {
    /// Its line number, from 1.
    pub number: usize,
    /// The line, trimmed.
    pub text: &'a str,
    pub section: Section,
    pub command: Command<'a>,
}

/// Betaflight CLI text, read line by line: what `diff`, `diff all` and
/// `dump` print.
#[derive(Clone, Debug)]
pub(crate) struct CliText<'a> {
    /// The `# version` line's version, if it has one.
    pub version: Option<Version>,
    /// The firmware the `# version` line names, such as `Betaflight`.
    pub firmware: Option<&'a str>,
    /// The command the CLI echoed at the top, such as `diff all`.
    pub echo: Option<&'a str>,
    /// The day the configuration was saved, which 4.3 prints, such as
    /// `2023-01-16`.
    pub configured: Option<String>,
    /// The day the firmware was built, such as `Jun 14 2022`.
    pub built: Option<String>,
    /// The craft name on the `# name:` line, or its `set name` or
    /// `set craft_name`.
    pub craft_name: Option<&'a str>,
    pub lines: Vec<Line<'a>>,
}

impl<'a> CliText<'a> {
    pub fn read(text: &'a str) -> CliText<'a> {
        let mut cli = CliText {
            version: None,
            firmware: None,
            echo: None,
            configured: None,
            built: None,
            craft_name: None,
            lines: Vec::new(),
        };
        let mut section = Section::Master;
        let mut set_name = None;
        for (index, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(comment) = line.strip_prefix('#') {
                let comment = comment.trim();
                if let Some((firmware, version, built)) = version_line(comment) {
                    cli.firmware.get_or_insert(firmware);
                    if cli.version.is_none() {
                        cli.version = version;
                        cli.built = built;
                    }
                } else if let Some(name) = comment.strip_prefix("name:") {
                    cli.craft_name.get_or_insert(name.trim());
                } else if let Some((_, after)) = comment.split_once("date: ") {
                    let day = after.split('T').next().unwrap_or(after).trim();
                    cli.configured.get_or_insert(day.to_string());
                } else if cli.echo.is_none()
                    && ["diff", "dump"]
                        .iter()
                        .any(|c| comment == *c || comment.starts_with(&format!("{c} ")))
                {
                    cli.echo = Some(comment);
                }
                continue;
            }
            let command = command(line);
            match command {
                Command::Profile(n) => section = Section::Profile(n),
                Command::RateProfile(n) => section = Section::RateProfile(n),
                Command::Set { name, value }
                    if section == Section::Master
                        && matches!(name, "name" | "craft_name")
                        && !value.is_empty()
                        && value != "-" =>
                {
                    set_name.get_or_insert(value);
                }
                _ => {}
            }
            cli.lines.push(Line {
                number: index + 1,
                text: line,
                section,
                command,
            });
        }
        if cli.craft_name.is_none() {
            cli.craft_name = set_name;
        }
        cli
    }

    /// The active PID profile: the last `profile` line's.
    pub fn active_profile(&self) -> Option<u8> {
        self.lines.iter().rev().find_map(|line| match line.command {
            Command::Profile(n) => Some(n),
            _ => None,
        })
    }

    /// The active rate profile: the last `rateprofile` line's.
    pub fn active_rate_profile(&self) -> Option<u8> {
        self.lines.iter().rev().find_map(|line| match line.command {
            Command::RateProfile(n) => Some(n),
            _ => None,
        })
    }

    /// The Betaflight version, or why there isn't one the translator reads.
    pub fn family(&self) -> Result<(Version, Family), Refusal> {
        match (self.firmware, self.version) {
            (Some(firmware), _) if !firmware.eq_ignore_ascii_case("betaflight") => {
                Err(Refusal::one(format!(
                    "This is {firmware}'s CLI output, not Betaflight's: OpenDrone reads Betaflight 4.3 or newer."
                )))
            }
            (_, Some(version)) => Family::of(version)
                .map(|family| (version, family))
                .map_err(Refusal::one),
            _ => Err(Refusal::one(
                "There's no `# version` line, so the Betaflight version is unknown: paste the whole output of `diff all`, from its first line.",
            )),
        }
    }
}

/// The firmware, version and build date on a `# version` line, such as
/// `Betaflight / STM32F411 (S411) 4.3.0 Jun 14 2022 / 00:48:04 (229ac66) MSP
/// API: 1.44`.
fn version_line(comment: &str) -> Option<(&str, Option<Version>, Option<String>)> {
    let (firmware, rest) = comment.split_once(" / ")?;
    if firmware.contains(' ') || firmware.is_empty() {
        return None;
    }
    // The version follows the board, in brackets.
    let after_board = rest.split_once(") ").map_or(rest, |(_, after)| after);
    let mut words = after_board.split_whitespace();
    let version = words.next().and_then(Version::parse);
    let built: Vec<&str> = words.take_while(|word| *word != "/").collect();
    let built = (!built.is_empty()).then(|| built.join(" "));
    Some((firmware, version, built))
}

/// What a line that isn't a comment says.
fn command(line: &str) -> Command<'_> {
    let word = line.split_whitespace().next().unwrap_or(line);
    let rest = line[word.len()..].trim();
    match word {
        "set" => match rest.split_once('=') {
            Some((name, value)) => Command::Set {
                name: name.trim(),
                value: value.trim(),
            },
            None => Command::Other(word),
        },
        "profile" => rest.parse().map_or(Command::Other(word), Command::Profile),
        "rateprofile" => rest
            .parse()
            .map_or(Command::Other(word), Command::RateProfile),
        _ => Command::Other(word),
    }
}
