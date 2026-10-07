//! Red Flags: changes that could weaken a check or a decision (#15 §5,
//! `docs/context/development.md`). Each one waits for the maintainer, is
//! decided by the Reviewer, or is only listed.
//!
//! Every detail is Markdown that is safe to post: text from the pull request
//! goes through [`code`] or [`plain`].

use std::collections::BTreeSet;

use super::areas::{Areas, REPO_RULES};
use super::changes::Changes;
use super::libraries::{self, NewLibrary};
use super::markdown::{code, plain};
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
/// The crates that may use `unsafe` (#15 §4).
const MAY_USE_UNSAFE: [&str; 2] = ["crates/input/Cargo.toml", "crates/opendrone/Cargo.toml"];

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
                "{} {} (licence: {}).",
                code(&library.name),
                plain(&library.version),
                plain(library.licence.as_deref().unwrap_or("not looked up"))
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
                format!(
                    "{}. A decision changes only with the maintainer.",
                    code(path)
                ),
            )),
            (Some(_), None) => flags.push(RedFlag::new(
                Level::WaitsForMaintainer,
                "An existing ADR deleted or renamed",
                format!(
                    "{}. A decision changes only with the maintainer.",
                    code(path)
                ),
            )),
            (None, Some(_)) => flags.push(RedFlag::new(
                Level::ListedOnly,
                "A new ADR",
                format!("{}.", code(path)),
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
                "{} goes from {} to {} in `Cargo.lock`. Before it merges, the maintainer runs \
                 the Frame Check and looks at both Maps in both Video Looks (ADR-0021).",
                code(&moved.name),
                plain(&moved.from.join(", ")),
                plain(&moved.to.join(", "))
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

/// Whether allowing a lint turns a house rule off. Clippy's `disallowed_*`
/// lints carry the house rules (ADR-0001); `clippy::style` holds them,
/// `clippy::all` holds that, and `warnings` covers everything CI denies.
fn turns_off_a_house_rule(lint: &str) -> bool {
    lint.starts_with("clippy::disallowed_")
        || matches!(
            lint,
            "clippy" | "clippy::all" | "clippy::style" | "clippy::restriction" | "warnings"
        )
}

/// Every lint a Rust file allows or expects, by name: in `#[allow(…)]`,
/// `#![allow(…)]`, `#[expect(…)]` or inside a `cfg_attr`, however the
/// attribute is spread over lines. Comments and strings don't count.
fn allowed_lints(text: &str) -> Vec<String> {
    let code: String = text.lines().map(code_part).collect::<Vec<_>>().join(" ");
    let compact: String = code.chars().filter(|c| !c.is_whitespace()).collect();
    let mut lints = Vec::new();
    for keyword in ["allow(", "expect("] {
        for (at, _) in compact.match_indices(keyword) {
            let before = compact[..at].chars().next_back();
            if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                continue;
            }
            let inside = &compact[at + keyword.len()..];
            let end = inside.find(')').unwrap_or(inside.len());
            lints.extend(
                inside[..end]
                    .split(',')
                    .filter(|lint| !lint.is_empty())
                    .map(str::to_string),
            );
        }
    }
    lints
}

/// A house-rule exception in a core crate: newly allowing a lint that carries
/// a house rule, a change to a core crate's lint settings in its `Cargo.toml`,
/// or a change to its `clippy.toml`, which holds the house rules.
fn house_rules(changes: &Changes, flags: &mut Vec<RedFlag>) {
    let mut settings = Vec::new();
    for folder in core_folders() {
        for path in changes.under(&folder) {
            if path.ends_with("/clippy.toml") {
                settings.push(code(path));
            } else if path == format!("{folder}Cargo.toml") {
                if lints_of(changes.base.text(path)) != lints_of(changes.head.text(path)) {
                    flags.push(RedFlag::new(
                        Level::ReviewerDecides,
                        "A house-rule exception in a core crate",
                        format!("{} changes the crate's lint settings.", code(path)),
                    ));
                }
            } else if path.ends_with(".rs") {
                let mut before: Vec<String> = changes
                    .base
                    .text(path)
                    .map(|t| allowed_lints(&t))
                    .unwrap_or_default();
                let mut new = Vec::new();
                for lint in changes
                    .head
                    .text(path)
                    .map(|t| allowed_lints(&t))
                    .unwrap_or_default()
                {
                    match before.iter().position(|b| *b == lint) {
                        Some(at) => {
                            before.remove(at);
                        }
                        None if turns_off_a_house_rule(&lint) => new.push(code(&lint)),
                        None => {}
                    }
                }
                if !new.is_empty() {
                    flags.push(RedFlag::new(
                        Level::ReviewerDecides,
                        "A house-rule exception in a core crate",
                        format!("{} now allows {}.", code(path), new.join(", ")),
                    ));
                }
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

/// A manifest's `[lints]` table, if it can be read.
fn lints_of(manifest: Option<String>) -> Option<toml::Value> {
    manifest?.parse::<toml::Table>().ok()?.get("lints").cloned()
}

/// Whether a crate's manifest takes the workspace's lints, which forbid
/// `unsafe` code.
fn takes_workspace_lints(manifest: Option<String>) -> bool {
    lints_of(manifest)
        .and_then(|lints| lints.get("workspace").and_then(toml::Value::as_bool))
        .unwrap_or(false)
}

/// The workspace's `unsafe_code` setting in the root `Cargo.toml`.
fn workspace_unsafe_setting(manifest: Option<String>) -> Option<toml::Value> {
    manifest?
        .parse::<toml::Table>()
        .ok()?
        .get("workspace")?
        .get("lints")?
        .get("rust")?
        .get("unsafe_code")
        .cloned()
}

/// New `unsafe` code anywhere, or a change to where it's allowed: a line that
/// allows it, a crate that stops taking the workspace's lints, or a change to
/// the workspace's `unsafe_code = "forbid"`.
fn unsafe_code(changes: &Changes, flags: &mut Vec<RedFlag>) {
    for path in &changes.paths {
        let is_rust = path.ends_with(".rs");
        let is_manifest = path.ends_with("Cargo.toml");
        if !is_rust && !is_manifest {
            continue;
        }
        let mut found: Vec<String> = changes
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
            .map(|line| code(&line))
            .collect();
        if is_rust
            && let Some(text) = changes.head.text(path)
            && allowed_lints(&text).iter().any(|l| l == "unsafe_code")
            && !changes
                .base
                .text(path)
                .is_some_and(|t| allowed_lints(&t).iter().any(|l| l == "unsafe_code"))
            && found.is_empty()
        {
            found.push("an allowance of `unsafe_code`".to_string());
        }
        if is_manifest
            && path.starts_with("crates/")
            && !MAY_USE_UNSAFE.contains(&path.as_str())
            && takes_workspace_lints(changes.base.text(path))
            && !takes_workspace_lints(changes.head.text(path))
        {
            found.push(
                "no `[lints] workspace = true` any more, so `unsafe` isn't forbidden there"
                    .to_string(),
            );
        }
        if path == "Cargo.toml"
            && workspace_unsafe_setting(changes.base.text(path))
                != workspace_unsafe_setting(changes.head.text(path))
        {
            found.push("a change to the workspace's `unsafe_code = \"forbid\"`".to_string());
        }
        if !found.is_empty() {
            flags.push(RedFlag::new(
                Level::ReviewerDecides,
                "New `unsafe` code",
                format!("{} adds {}.", code(path), listed(&found, 3, "such lines")),
            ));
        }
    }
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
    let files: Vec<String> = changes
        .paths
        .iter()
        .filter(|path| areas.of(path) == Some(REPO_RULES))
        .map(|path| code(path))
        .collect();
    if !files.is_empty() {
        flags.push(RedFlag::new(
            Level::ReviewerDecides,
            "A change to the Repo rules",
            format!(
                "{}. The ticket must ask for it, and a check is never weakened unless the ticket \
                 says so.",
                listed(&files, 10, "files")
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
                format!("**{}**, in {}.", plain(&term), code(path)),
            ));
        }
    }
}

/// Up to `most` items, and how many more there are.
fn listed(items: &[String], most: usize, more_of: &str) -> String {
    let shown = items
        .iter()
        .take(most)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    match items.len().saturating_sub(most) {
        0 => shown,
        more => format!("{shown} and {more} more {more_of}"),
    }
}
