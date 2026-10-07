//! Red Flags: changes that could weaken a check or a decision (#15 §5,
//! `docs/context/development.md`). Each one waits for the maintainer, is
//! decided by the Reviewer, or is only listed.

use std::collections::BTreeSet;

use super::areas::{Areas, REPO_RULES};
use super::changes::Changes;
use super::libraries::{self, NewLibrary};
use super::scenarios;

/// What a Red Flag asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// The Red Flag gate fails, and the PR gets the `needs-maintainer` label.
    WaitsForMaintainer,
    /// The Reviewer decides whether it holds up.
    ReviewerDecides,
    /// Only listed in the Review Report.
    ListedOnly,
}

impl Level {
    pub const ALL: [Level; 3] = [
        Level::WaitsForMaintainer,
        Level::ReviewerDecides,
        Level::ListedOnly,
    ];

    /// The key `review.json` uses.
    pub fn key(self) -> &'static str {
        match self {
            Level::WaitsForMaintainer => "waits-for-maintainer",
            Level::ReviewerDecides => "reviewer-decides",
            Level::ListedOnly => "listed-only",
        }
    }
}

/// One Red Flag: a short title and the detail, both plain sentences.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RedFlag {
    pub level: Level,
    pub title: String,
    pub detail: String,
}

impl RedFlag {
    pub fn new(level: Level, title: &str, detail: String) -> RedFlag {
        RedFlag {
            level,
            title: title.to_string(),
            detail,
        }
    }
}

/// The ADRs' folder.
const ADRS: &str = "docs/adr/";
/// The deep dives, where glossary terms live.
const DEEP_DIVES: &str = "docs/context/";
/// The walls between crates name the five core crates.
const WALLS: &str = include_str!("../../walls.toml");

/// Every Red Flag in a set of changes.
pub fn detect(changes: &Changes, areas: &Areas, new_libraries: &[NewLibrary]) -> Vec<RedFlag> {
    let mut flags = scenarios::flags(changes);
    adrs(changes, &mut flags);
    bevy_and_wgpu(changes, &mut flags);
    house_rules(changes, &mut flags);
    unsafe_code(changes, &mut flags);
    repo_rules(changes, areas, &mut flags);
    for library in new_libraries {
        flags.push(RedFlag::new(
            Level::ListedOnly,
            "A new outside library",
            format!(
                "`{}` {} (licence: {}).",
                library.name,
                library.version,
                library.licence.as_deref().unwrap_or("not looked up")
            ),
        ));
    }
    glossary_terms(changes, &mut flags);
    flags.sort_by_key(|flag| flag.level);
    flags
}

/// An existing ADR edited, renamed or deleted waits; a new one is listed.
fn adrs(changes: &Changes, flags: &mut Vec<RedFlag>) {
    for path in changes.under(ADRS).filter(|p| p.ends_with(".md")) {
        match (changes.base.read(path), changes.head.read(path)) {
            (Some(_), Some(_)) => flags.push(RedFlag::new(
                Level::WaitsForMaintainer,
                "An existing ADR edited",
                format!("`{path}`. A decision changes only with the maintainer."),
            )),
            (Some(_), None) => flags.push(RedFlag::new(
                Level::WaitsForMaintainer,
                "An existing ADR deleted or renamed",
                format!("`{path}`. A decision changes only with the maintainer."),
            )),
            (None, Some(_)) => flags.push(RedFlag::new(
                Level::ListedOnly,
                "A new ADR",
                format!("`{path}`."),
            )),
            (None, None) => {}
        }
    }
}

/// Bevy moving to a new 0.N, or wgpu to a new major version (ADR-0021).
fn bevy_and_wgpu(changes: &Changes, flags: &mut Vec<RedFlag>) {
    if !changes.touches("Cargo.lock") {
        return;
    }
    let base = libraries::read_lock(&changes.base.text("Cargo.lock").unwrap_or_default());
    let head = libraries::read_lock(&changes.head.text("Cargo.lock").unwrap_or_default());
    for moved in libraries::series_moves(&base, &head) {
        let name = if moved.name == "bevy" { "Bevy" } else { "wgpu" };
        flags.push(RedFlag::new(
            Level::WaitsForMaintainer,
            &format!("{name} moves to a new version"),
            format!(
                "`{}` goes from {} to {} in `Cargo.lock`. Before it merges, the maintainer runs \
                 the Frame Check and looks at both Maps in both Video Looks (ADR-0021).",
                moved.name,
                moved.from.join(", "),
                moved.to.join(", ")
            ),
        ));
    }
}

/// The five core crates' folders, such as `crates/physics/`, from the walls.
fn core_folders() -> Vec<String> {
    let walls: toml::Table = WALLS.parse().unwrap_or_default();
    walls
        .get("core")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_str)
        .map(|name| format!("crates/{}/", name.trim_start_matches("opendrone-")))
        .collect()
}

/// A house-rule exception in a core crate: allowing one of Clippy's
/// `disallowed_*` lints, which carry the house rules (ADR-0001), or changing a
/// core crate's `clippy.toml`.
fn house_rules(changes: &Changes, flags: &mut Vec<RedFlag>) {
    let mut settings = Vec::new();
    for folder in core_folders() {
        for path in changes.under(&folder) {
            if path.ends_with("/clippy.toml") {
                settings.push(format!("`{path}`"));
                continue;
            }
            if !path.ends_with(".rs") {
                continue;
            }
            let exceptions: Vec<String> = changes
                .added_lines(path)
                .into_iter()
                .filter(|line| {
                    let line = line.replace(' ', "");
                    (line.contains("allow(") || line.contains("expect("))
                        && line.contains("clippy::disallowed_")
                })
                .collect();
            if !exceptions.is_empty() {
                flags.push(RedFlag::new(
                    Level::ReviewerDecides,
                    "A house-rule exception in a core crate",
                    format!("`{path}` adds {}.", quoted_lines(&exceptions)),
                ));
            }
        }
    }
    if !settings.is_empty() {
        flags.push(RedFlag::new(
            Level::ReviewerDecides,
            "A house-rule exception in a core crate",
            format!(
                "the house rules themselves changed, in {}.",
                settings.join(", ")
            ),
        ));
    }
}

/// New `unsafe` code anywhere, or a change to where it's allowed.
fn unsafe_code(changes: &Changes, flags: &mut Vec<RedFlag>) {
    for path in &changes.paths {
        let is_rust = path.ends_with(".rs");
        if !is_rust && !path.ends_with("Cargo.toml") {
            continue;
        }
        let added: Vec<String> = changes
            .added_lines(path)
            .into_iter()
            .filter(|line| {
                let code = code_part(line);
                if is_rust {
                    has_word(&code, "unsafe") || allows_unsafe_code(&code)
                } else {
                    // A lint setting in a manifest, such as `unsafe_code = "allow"`.
                    has_word(&code, "unsafe_code") && code.contains('=')
                }
            })
            .collect();
        if !added.is_empty() {
            flags.push(RedFlag::new(
                Level::ReviewerDecides,
                "New `unsafe` code",
                format!("`{path}` adds {}.", quoted_lines(&added)),
            ));
        }
    }
}

/// A line without its comment and without the insides of its strings, so a
/// word in a comment or a message isn't taken for code.
fn code_part(line: &str) -> String {
    let mut code = String::new();
    let mut in_string = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if in_string => {
                chars.next();
            }
            '"' => {
                in_string = !in_string;
                code.push(c);
            }
            '/' if !in_string && chars.peek() == Some(&'/') => break,
            '#' if !in_string && code.trim().is_empty() && chars.peek() == Some(&' ') => {
                // A TOML comment.
                break;
            }
            _ if in_string => {}
            _ => code.push(c),
        }
    }
    code
}

/// Whether a line of Rust changes the `unsafe_code` lint, such as
/// `#![allow(unsafe_code)]`.
fn allows_unsafe_code(code: &str) -> bool {
    let compact = code.replace(' ', "");
    has_word(code, "unsafe_code")
        && ["allow(", "expect(", "warn("]
            .iter()
            .any(|attribute| compact.contains(attribute))
}

/// Whether `word` appears in `text` on its own, not inside a longer name.
fn has_word(text: &str, word: &str) -> bool {
    let is_name = |c: char| c.is_alphanumeric() || c == '_';
    text.match_indices(word).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        !before.is_some_and(is_name) && !after.is_some_and(is_name)
    })
}

/// A change to the Repo rules Area: CI, lint settings, the licence policy,
/// CODEOWNERS, the Rust version, xtask or AGENTS.md.
fn repo_rules(changes: &Changes, areas: &Areas, flags: &mut Vec<RedFlag>) {
    let files: Vec<&str> = changes
        .paths
        .iter()
        .map(String::as_str)
        .filter(|path| areas.of(path) == Some(REPO_RULES))
        .collect();
    if !files.is_empty() {
        flags.push(RedFlag::new(
            Level::ReviewerDecides,
            "A change to the Repo rules",
            format!(
                "{}. The ticket must ask for it, and a check is never weakened unless the ticket \
                 says so.",
                listed(&files, 10)
            ),
        ));
    }
}

/// A new glossary term: a `**Term**:` line in a deep dive that no changed deep
/// dive had before.
fn glossary_terms(changes: &Changes, flags: &mut Vec<RedFlag>) {
    let terms = |text: Option<String>| -> BTreeSet<String> {
        text.unwrap_or_default()
            .lines()
            .filter_map(|line| line.trim().strip_prefix("**")?.strip_suffix("**:"))
            .map(str::to_string)
            .collect()
    };
    let mut before = BTreeSet::new();
    let mut after = Vec::new();
    for path in changes.under(DEEP_DIVES).filter(|p| p.ends_with(".md")) {
        before.extend(terms(changes.base.text(path)));
        after.extend(
            terms(changes.head.text(path))
                .into_iter()
                .map(|t| (t, path)),
        );
    }
    for (term, path) in after {
        if !before.contains(&term) {
            flags.push(RedFlag::new(
                Level::ListedOnly,
                "A new glossary term",
                format!("**{term}**, in `{path}`."),
            ));
        }
    }
}

/// Up to `most` files, quoted, and how many more there are.
fn listed(files: &[&str], most: usize) -> String {
    let shown: Vec<String> = files.iter().take(most).map(|f| format!("`{f}`")).collect();
    match files.len().saturating_sub(most) {
        0 => shown.join(", "),
        more => format!("{} and {more} more files", shown.join(", ")),
    }
}

/// Up to three lines of code, quoted, and how many more there are.
fn quoted_lines(lines: &[String]) -> String {
    let shown: Vec<String> = lines
        .iter()
        .take(3)
        .map(|line| format!("`{}`", line.trim().replace('`', "'")))
        .collect();
    let more = lines.len().saturating_sub(3);
    if more > 0 {
        format!("{} and {more} more such lines", shown.join(", "))
    } else {
        shown.join(", ")
    }
}
