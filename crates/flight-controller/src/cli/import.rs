//! The `diff all` importer: a `diff`, `diff all` or `dump` from Betaflight
//! 4.3 or newer, imported as a Betaflight 2026.6 Tune ([ADR-0008],
//! [ADR-0015], #21's resolution).
//!
//! 1. The export's own settings are gathered: those before any profile, and
//!    the active PID profile's (the last `profile` line's), plus, for 4.3,
//!    TPA from the active rate profile ([`table::TUNE_IN_RATE_PROFILE`]).
//!    Other PID profiles, the rate profiles (Rates belong to the pilot), the
//!    `simplified_*` sliders, hardware-only settings, settings 2026.6 has no
//!    counterpart for, and every command other than `set` are left out, each
//!    with its reason.
//! 2. Every row of [`table::SETTINGS`] is worked out: renamed settings take
//!    their new name, settings the export doesn't set take their own
//!    version's default, and settings the version lacked take ADR-0008's
//!    value. Each carries its mark.
//! 3. The Tune spells out every setting the Flight Controller reads
//!    ([`crate::Tune::settings`]), under its tab. The other settings the
//!    export sets stay, under "Not simulated yet": the table's in its order,
//!    then the rest in the export's.
//!
//! [ADR-0008]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0008-copy-betaflight-2026-6-translate-older-tunes.md
//! [ADR-0015]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0015-tune-is-betaflight-cli-text-spelling-out-every-setting.md

use std::collections::BTreeMap;

use super::table::{self, Place, Rule, Setting, Source};
use super::{CliText, Command, Family, Refusal, Section, Version};
use crate::Tune;

/// A Betaflight export imported as a 2026.6 Tune.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TuneImport {
    pub version: Version,
    pub family: Family,
    /// The command the export came from, as the CLI echoed it, such as
    /// `diff all`.
    pub command: Option<String>,
    /// The quad's craft name in the export.
    pub craft_name: Option<String>,
    /// The day the configuration was saved, where the export says (4.3 does).
    pub configured: Option<String>,
    /// The day the firmware was built.
    pub built: Option<String>,
    /// The active PID profile, the one read.
    pub profile: u8,
    /// Every setting worked out: each row of the table, then the export's
    /// settings the table doesn't know, in the export's order.
    pub settings: Vec<ImportedSetting>,
    /// The export's lines that don't reach the Tune, each with its reason.
    pub left_out: Vec<LeftOut>,
    /// Values of settings the Flight Controller reads that it can't read, as
    /// [`Tune::check`] says. The Tune is still written; the Pack checker
    /// refuses it until they're fixed by hand (`hand-set: <reason>`).
    pub problems: Vec<String>,
}

/// One setting of the imported Tune.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportedSetting {
    /// Its Betaflight 2026.6 name.
    pub name: String,
    pub value: String,
    /// Where its value came from (ADR-0015), such as `diff (was d_min_roll)`.
    pub mark: String,
    /// The table's note for it, such as its unit, or "".
    pub note: &'static str,
    /// Its place in the Tune, if the table knows it.
    pub place: Option<Place>,
    /// The export's line it came from, if one.
    pub line: Option<usize>,
    pub written: Where,
}

/// Whether and where a setting is written into the Tune.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Where {
    /// Under its tab: the Flight Controller reads it, or the Pack checker
    /// compares it with the Quad definition.
    UnderItsTab,
    /// Under "Not simulated yet": the export sets it, and nothing reads it
    /// yet.
    NotSimulatedYet,
    /// Not written: nothing reads it yet, and the export doesn't set it.
    NotWritten,
}

/// A line of the export that doesn't reach the Tune.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeftOut {
    /// Its line number, from 1.
    pub line: usize,
    pub text: String,
    pub why: Why,
}

/// Why a line doesn't reach the Tune.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// It describes the board or its other hardware, or names the quad.
    HardwareOnly,
    /// A `simplified_*` slider: the importer reads the numbers it set.
    Slider,
    /// A setting of a PID profile that isn't the active one.
    OtherProfile,
    /// A rate profile's setting: Rates belong to the pilot (ADR-0015).
    RateProfile,
    /// 2026.6 has no counterpart, for this reason.
    Retired(&'static str),
    /// A command other than `set`, such as `aux`, `feature` or `serial`.
    NotASetting,
}

/// The old version's value of one setting: the export's, or its default.
#[derive(Clone, Copy, Debug)]
struct Old<'a> {
    name: &'a str,
    value: &'a str,
    /// The export's line that set it, if it did.
    line: Option<usize>,
}

impl Old<'_> {
    /// `diff`, or the version's default, such as `4.3 default`.
    fn mark(&self, family: Family) -> String {
        match self.line {
            Some(_) => "diff".into(),
            None => format!("{} default", family.name()),
        }
    }

    fn number(&self) -> Result<u32, String> {
        self.value
            .parse()
            .map_err(|_| format!("`{}` is a whole number, not \"{}\"", self.name, self.value))
    }
}

/// Imports a `diff`, `diff all` or `dump` from Betaflight 4.3 or newer as a
/// 2026.6 Tune. Refused, with plain sentences, when the version is older
/// than 4.3 or one the table doesn't know, or the export holds no PID
/// profile.
pub fn import_tune<'t>(text: &'t str) -> Result<TuneImport, Refusal> {
    let cli = CliText::read(text);
    let (version, family) = cli.family()?;
    let Some(profile) = cli.active_profile() else {
        return Err(Refusal::one(
            "There's no `profile` line, so this export holds no PID profile: export the quad's settings with `diff all`.",
        ));
    };
    let rate_profile = cli.active_rate_profile();
    let read_in_rate_profile = |name: &str| {
        table::TUNE_IN_RATE_PROFILE
            .iter()
            .any(|(f, n)| *f == family && *n == name)
    };

    // 1. The export's own settings, by their names in its version.
    let mut given: BTreeMap<&'t str, (&'t str, usize)> = BTreeMap::new();
    let mut order: Vec<&'t str> = Vec::new();
    let mut left_out = Vec::new();
    for line in &cli.lines {
        let leave = |why| LeftOut {
            line: line.number,
            text: line.text.to_string(),
            why,
        };
        let (name, value) = match line.command {
            Command::Set { name, value } => (name, value),
            Command::Profile(_) | Command::RateProfile(_) => continue,
            Command::Other(_) => {
                left_out.push(leave(Why::NotASetting));
                continue;
            }
        };
        let in_tune = match line.section {
            Section::Master => true,
            Section::Profile(n) => n == profile,
            Section::RateProfile(n) => Some(n) == rate_profile && read_in_rate_profile(name),
        };
        let why = if !in_tune {
            Some(match line.section {
                Section::Profile(_) => Why::OtherProfile,
                _ => Why::RateProfile,
            })
        } else if table::is_slider(name) {
            Some(Why::Slider)
        } else if table::is_hardware_only(name) {
            Some(Why::HardwareOnly)
        } else {
            table::RETIRED
                .iter()
                .find(|r| r.name == name && r.versions.contains(&family))
                .map(|r| Why::Retired(r.why))
        };
        match why {
            Some(why) => left_out.push(leave(why)),
            None => {
                if given.insert(name, (value, line.number)).is_none() {
                    order.push(name);
                }
            }
        }
    }

    // 2. Every row of the table, worked out.
    let reads = Tune::settings();
    let mut settings = Vec::new();
    let mut problems = Vec::new();
    let mut used: Vec<&'t str> = Vec::new();
    for setting in table::SETTINGS {
        let mut take = |name: &'static str, default: &'static str| -> Old<'t> {
            used.push(name);
            match given.get(name) {
                Some(&(value, line)) => Old {
                    name,
                    value,
                    line: Some(line),
                },
                None => Old {
                    name,
                    value: default,
                    line: None,
                },
            }
        };
        let worked_out = match setting.source(family) {
            Source::Same(default) => {
                let value = take(setting.name, default);
                Ok((value.value.to_string(), value.mark(family), value.line))
            }
            Source::Was(was, default) => {
                let value = take(was, default);
                Ok((
                    value.value.to_string(),
                    format!("{} (was {was})", value.mark(family)),
                    value.line,
                ))
            }
            Source::Adr0008(value) => Ok((value.to_string(), "ADR-0008".to_string(), None)),
            Source::Newer => Ok((
                setting.default_2026().to_string(),
                format!("{} default", Family::V2026_6.name()),
                None,
            )),
            Source::Rule(rule, inputs) => {
                let inputs: Vec<Old<'t>> = inputs
                    .iter()
                    .map(|&(name, default)| take(name, default))
                    .collect();
                let line = inputs.iter().find_map(|input| input.line);
                apply(rule, family, &inputs).map(|(value, mark)| (value, mark, line))
            }
        };
        let (value, mark, line) = match worked_out {
            Ok(found) => found,
            Err(sentence) => {
                problems.push(sentence);
                continue;
            }
        };
        let written = if reads.contains(&setting.name)
            || table::CHECKED_AGAINST_THE_QUAD.contains(&setting.name)
        {
            Where::UnderItsTab
        } else if line.is_some() {
            Where::NotSimulatedYet
        } else {
            Where::NotWritten
        };
        if written == Where::UnderItsTab
            && let Err(sentence) = Tune::check(setting.name, &value)
        {
            problems.push(sentence);
        }
        settings.push(imported(setting, value, mark, line, written));
    }

    // 3. The export's settings the table doesn't know: kept, not simulated.
    for name in order {
        if used.contains(&name) {
            continue;
        }
        let (value, line) = given[name];
        settings.push(ImportedSetting {
            name: name.to_string(),
            value: value.to_string(),
            mark: "diff".into(),
            note: "",
            place: None,
            line: Some(line),
            written: Where::NotSimulatedYet,
        });
    }

    Ok(TuneImport {
        version,
        family,
        command: cli.echo.map(str::to_string),
        craft_name: cli.craft_name.map(str::to_string),
        configured: cli.configured.clone(),
        built: cli.built.clone(),
        profile,
        settings,
        left_out,
        problems,
    })
}

fn imported(
    setting: &Setting,
    value: String,
    mark: String,
    line: Option<usize>,
    written: Where,
) -> ImportedSetting {
    ImportedSetting {
        name: setting.name.to_string(),
        value,
        mark,
        note: setting.note,
        place: Some(setting.place),
        line,
        written,
    }
}

/// Works out a setting whose meaning changed, from the old version's
/// settings: its value and mark.
fn apply(rule: Rule, family: Family, inputs: &[Old<'_>]) -> Result<(String, String), String> {
    let version = family.name();
    match rule {
        Rule::DBase | Rule::DPeak => {
            let (d, d_min) = (inputs[0], inputs[1]);
            let (d_value, d_min_value) = (d.number()?, d_min.number()?);
            let dynamic = d_min_value > 0 && d_min_value < d_value;
            let was = |old: &Old<'_>| format!("{} (was {})", old.mark(family), old.name);
            Ok(match (rule, dynamic) {
                (Rule::DBase, true) => (d_min.value.into(), was(&d_min)),
                (Rule::DBase, false) => (d.value.into(), d.mark(family)),
                (_, true) => (d.value.into(), was(&d)),
                (_, false) if d_min_value <= d_value => (d_min.value.into(), was(&d_min)),
                (_, false) => (
                    "0".into(),
                    format!(
                        "ADR-0008; Dynamic D was off in {version}: {} {d_min_value} isn't below {} {d_value}",
                        d_min.name, d.name
                    ),
                ),
            })
        }
        Rule::DMaxAdvance => {
            let (gain, advance) = (inputs[0].number()?, inputs[1].number()?);
            let value = ((gain * advance + 50) / 100).min(200);
            Ok((
                value.to_string(),
                format!(
                    "ADR-0008; {version}'s stick boost: {} {gain} × {} {advance} ÷ 100",
                    inputs[0].name, inputs[1].name
                ),
            ))
        }
        Rule::ItermWindup => {
            let (limit, pidsum) = (inputs[0].number()?, inputs[1].number()?);
            if pidsum == 0 {
                return Err(format!("`{}` can't be 0", inputs[1].name));
            }
            let percent = (limit * 100 + pidsum / 2) / pidsum;
            let held = percent.clamp(20, 100);
            let within = if held == percent {
                String::new()
            } else {
                format!(", held within 2026.6's 20 to 100 from {percent}")
            };
            Ok((
                held.to_string(),
                format!(
                    "ADR-0008; {version}'s {} {limit} is {percent}% of {} {pidsum}{within}",
                    inputs[0].name, inputs[1].name
                ),
            ))
        }
    }
}

/// Marks are written from this column on, as in the built-in Tunes.
const MARK_COLUMN: usize = 39;

impl TuneImport {
    /// The setting by its 2026.6 name.
    pub fn setting(&self, name: &str) -> Option<&ImportedSetting> {
        self.settings.iter().find(|s| s.name == name)
    }

    /// `diff` or `dump`, as the export's own word.
    fn export_word(&self) -> &'static str {
        match &self.command {
            Some(command) if command.starts_with("dump") => "dump",
            _ => "diff",
        }
    }

    /// The Tune, as `tune.txt` holds it: the header saying where it came
    /// from, then every setting written, one `set` line each with its mark.
    /// `quad` is the Quad's on-screen name and `source` names the file the
    /// export was read from.
    pub fn tune_txt(&self, quad: &str, source: &str) -> String {
        let mut out = String::new();
        let mut line = |text: &str| {
            out.push_str(text);
            out.push('\n');
        };
        let version = self.family.name();
        let command = self.command.as_deref().unwrap_or("CLI export");
        let quad_name = match &self.craft_name {
            Some(name) => format!(" of the quad named \"{name}\""),
            None => String::new(),
        };
        let day = match (&self.configured, &self.built) {
            (Some(day), _) => format!("configured {day} "),
            (None, Some(day)) => format!("firmware built {day} "),
            (None, None) => String::new(),
        };
        line(&format!(
            "# {quad} Tune: Betaflight 2026.6 names and units."
        ));
        line(&format!(
            "# Imported from the {command}{quad_name}: Betaflight {},",
            self.version
        ));
        line(&format!("# {day}({source}),"));
        line("# by `cargo xtask import-tune`.");
        line("# Each line's mark says where its value came from:");
        let written: Vec<&ImportedSetting> = self
            .settings
            .iter()
            .filter(|s| s.written != Where::NotWritten)
            .collect();
        let uses = |test: &dyn Fn(&str) -> bool| written.iter().any(|s| test(&s.mark));
        let own_default = format!("{version} default");
        let newer_default = format!("{} default", Family::V2026_6.name());
        let export = self.export_word();
        let mut legend: Vec<(String, String)> = Vec::new();
        if uses(&|m| m.starts_with("diff")) {
            legend.push(("diff".into(), "set on the real quad".into()));
        }
        if uses(&|m| m.starts_with(&own_default)) {
            legend.push((
                own_default.clone(),
                format!(
                    "not in the {export}, so Betaflight {}'s default",
                    self.family.defaults_from()
                ),
            ));
        }
        if uses(&|m| m.starts_with("ADR-0008")) {
            legend.push((
                "ADR-0008".into(),
                format!("{version} had no such setting (or it meant something else); set to behave like {version}"),
            ));
        }
        if self.family != Family::V2026_6 && uses(&|m| m.starts_with(&newer_default)) {
            legend.push((
                newer_default.clone(),
                format!(
                    "{version} had no such setting and no value behaves like {version}: Betaflight {}'s default",
                    Family::V2026_6.defaults_from()
                ),
            ));
        }
        if uses(&|m| m.contains("(was ")) {
            legend.push(("(was …)".into(), format!("the setting's {version} name")));
        }
        legend.push((
            "hand-set: …".into(),
            "changed by hand, with the reason".into(),
        ));
        let width = legend
            .iter()
            .map(|(m, _)| m.chars().count())
            .max()
            .unwrap_or(0);
        for (mark, meaning) in &legend {
            let pad = width - mark.chars().count();
            line(&format!("#   {mark}{}  {meaning}", " ".repeat(pad)));
        }
        line("#");
        line("# It spells out every setting the Flight Controller reads so far, grouped by");
        line("# the Betaflight App's tabs in their order, with each tab's CLI-only settings");
        line(&format!(
            "# after the ones the tab shows (ADR-0015). The settings the {export} sets that"
        ));
        line("# OpenDrone doesn't simulate yet come last, under their own heading. As the");
        line("# Flight Controller reads more settings, run the importer again.");

        // A blank line before each tab; its CLI-only settings follow at once.
        let mut heading: Option<&str> = None;
        for setting in written.iter().filter(|s| s.written == Where::UnderItsTab) {
            let place = setting.place.map_or("", Place::heading);
            if Some(place) != heading {
                let same_tab = heading.is_some_and(|tab| place == format!("{tab}, CLI only"));
                if !same_tab {
                    line("");
                }
                line(&format!("# {place}"));
                heading = Some(place);
            }
            line(&set_line(setting));
        }
        let not_simulated: Vec<&&ImportedSetting> = written
            .iter()
            .filter(|s| s.written == Where::NotSimulatedYet)
            .collect();
        if !not_simulated.is_empty() {
            line("");
            line("# Not simulated yet");
            for setting in not_simulated {
                line(&set_line(setting));
            }
        }
        out
    }

    /// What the import did, in plain sentences, for the person running it.
    pub fn report(&self) -> String {
        let mut out = String::new();
        let quad = match &self.craft_name {
            Some(name) => format!(" of \"{name}\""),
            None => String::new(),
        };
        out.push_str(&format!(
            "Imported the {}{quad}: Betaflight {}, PID profile {}.\n",
            self.command.as_deref().unwrap_or("CLI export"),
            self.version,
            self.profile
        ));
        let count = |w: Where| self.settings.iter().filter(|s| s.written == w).count();
        out.push_str(&format!(
            "- {} settings the Flight Controller reads (or the Pack checker compares with the Quad), under their tabs.\n",
            count(Where::UnderItsTab)
        ));
        let renamed: Vec<String> = self
            .settings
            .iter()
            .filter(|s| s.written != Where::NotWritten)
            .filter_map(|s| {
                let (_, was) = s.mark.split_once("(was ")?;
                let was = was.split(')').next()?;
                Some(format!("{was} → {}", s.name))
            })
            .collect();
        if !renamed.is_empty() {
            out.push_str(&format!("- Renamed: {}.\n", renamed.join(", ")));
        }
        let not_simulated: Vec<&str> = self
            .settings
            .iter()
            .filter(|s| s.written == Where::NotSimulatedYet)
            .map(|s| s.name.as_str())
            .collect();
        if !not_simulated.is_empty() {
            out.push_str(&format!(
                "- Not simulated yet, kept under their own heading: {}.\n",
                not_simulated.join(", ")
            ));
        }
        let names = |why: &dyn Fn(Why) -> bool| -> Vec<String> {
            self.left_out
                .iter()
                .filter(|l| why(l.why))
                .map(|l| {
                    l.text
                        .strip_prefix("set ")
                        .and_then(|rest| rest.split('=').next())
                        .unwrap_or_else(|| l.text.split_whitespace().next().unwrap_or(""))
                        .trim()
                        .to_string()
                })
                .collect()
        };
        let mut list = |label: &str, items: Vec<String>| {
            if !items.is_empty() {
                let mut unique: Vec<String> = Vec::new();
                for item in items {
                    if !unique.contains(&item) {
                        unique.push(item);
                    }
                }
                out.push_str(&format!("- Left out, {label}: {}.\n", unique.join(", ")));
            }
        };
        list("hardware only", names(&|w| w == Why::HardwareOnly));
        list(
            "the simplified_* sliders (their numbers are read instead)",
            names(&|w| w == Why::Slider),
        );
        list(
            "other PID profiles' settings",
            names(&|w| w == Why::OtherProfile),
        );
        list(
            "rate profiles' settings (Rates belong to the pilot)",
            names(&|w| w == Why::RateProfile),
        );
        list(
            "commands other than `set`",
            names(&|w| w == Why::NotASetting),
        );
        for left in &self.left_out {
            if let Why::Retired(why) = left.why {
                out.push_str(&format!(
                    "- Left out, line {}: `{}`: {why}.\n",
                    left.line, left.text
                ));
            }
        }
        for problem in &self.problems {
            out.push_str(&format!(
                "- The Flight Controller can't read this yet: {problem}.\n"
            ));
        }
        out
    }
}

/// `set <name> = <value>`, with its mark (and note) from the mark column on.
fn set_line(setting: &ImportedSetting) -> String {
    let set = format!("set {} = {}", setting.name, setting.value);
    let pad = MARK_COLUMN.saturating_sub(set.chars().count()).max(1);
    let note = if setting.note.is_empty() {
        String::new()
    } else {
        format!("; {}", setting.note)
    };
    format!("{set}{}# {}{note}", " ".repeat(pad), setting.mark)
}
