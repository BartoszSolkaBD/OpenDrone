//! Red Flags: changes that could weaken a check or a decision (#15 §5,
//! `docs/context/development.md`). Each one waits for the maintainer, is
//! decided by the Reviewer, or is only listed.
//!
//! Every detail is Markdown that is safe to post: text from the pull request
//! goes through [`code`] or [`plain`].

use std::collections::BTreeSet;

use super::areas::{Areas, REPO_RULES};
use super::changes::{Changes, Side};
use super::libraries::{self, NewLibrary};
use super::markdown::{code, plain};
use super::rust::{code_only, edition_of};
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
    other_files(changes, &mut flags);
    workflows(changes, &mut flags);
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
/// `clippy::all` holds that, and `warnings` covers everything CI denies. A
/// lint, or a lint attribute, that a macro's argument names (`$lint`) could be
/// any of them.
fn turns_off_a_house_rule(lint: &str) -> bool {
    lint.contains('$')
        || lint.starts_with("clippy::disallowed_")
        || matches!(
            lint,
            "clippy" | "clippy::all" | "clippy::style" | "clippy::restriction" | "warnings"
        )
}

/// A Rust file's code on one side, with comments and literals blanked out,
/// read as rustc 1.99 reads it in its crate's edition ([`super::rust`]).
fn code_of(side: &Side, path: &str) -> Option<String> {
    let text = side.text(path)?;
    Some(code_only(&text, edition_of(path, |file| side.text(file))))
}

/// Every lint a Rust file's code allows or expects, by name: in `#[allow(…)]`,
/// `#![allow(…)]`, `#[expect(…)]` or inside a `cfg_attr`, however the
/// attribute is spread over lines. `code` comes from [`code_of`], so
/// comments, strings and char literals don't count, and hide one only
/// where they would hide it from rustc. Inside a macro, a lint named by an
/// argument (`#[allow($lint)]`) comes back as `$lint`, and an attribute named
/// by one (`#[$level(…)]`) as the whole attribute.
fn allowed_lints(code: &str) -> Vec<String> {
    let compact: String = code.chars().filter(|c| !c.is_whitespace()).collect();
    let mut lints = Vec::new();
    for opening in ["#[$", "#![$"] {
        for (at, _) in compact.match_indices(opening) {
            let inside = &compact[at + opening.len() - 1..];
            let end = inside.find(']').unwrap_or(inside.len());
            lints.push(inside[..end].to_string());
        }
    }
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
/// a change to its `clippy.toml`, which holds the house rules, or a
/// `.clippy.toml`, which Clippy reads instead of the `clippy.toml` beside it.
fn house_rules(changes: &Changes, flags: &mut Vec<RedFlag>) {
    let mut settings = Vec::new();
    let mut dotted = Vec::new();
    for folder in core_folders() {
        for path in changes.under(&folder) {
            if path.ends_with("/.clippy.toml") {
                dotted.push(code(path));
            } else if path.ends_with("/clippy.toml") {
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
                let mut before: Vec<String> = code_of(&changes.base, path)
                    .map(|code| allowed_lints(&code))
                    .unwrap_or_default();
                let mut new = Vec::new();
                for lint in code_of(&changes.head, path)
                    .map(|code| allowed_lints(&code))
                    .unwrap_or_default()
                {
                    match before.iter().position(|b| *b == lint) {
                        Some(at) => {
                            before.remove(at);
                        }
                        None if lint.contains('$') => {
                            new.push(format!("{} (named by a macro's argument)", code(&lint)))
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
    if !dotted.is_empty() {
        flags.push(RedFlag::new(
            Level::ReviewerDecides,
            "A house-rule exception in a core crate",
            format!(
                "{}. Clippy reads a `.clippy.toml` instead of the `clippy.toml` beside it, which \
                 holds the house rules, so even an empty one turns them off. The walls check \
                 refuses one in a core crate.",
                dotted.join(", ")
            ),
        ));
    }
}

/// Every Rust edition a manifest sets, with where: the workspace's, the
/// package's (or that it takes the workspace's), and each target's.
fn editions(manifest: Option<String>) -> Vec<(String, toml::Value)> {
    let Some(table) = manifest.and_then(|m| m.parse::<toml::Table>().ok()) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    let mut add = |place: String, table: Option<&toml::Value>| {
        if let Some(edition) = table.and_then(|t| t.get("edition")) {
            found.push((place, edition.clone()));
        }
    };
    add(
        "workspace.package".to_string(),
        table.get("workspace").and_then(|w| w.get("package")),
    );
    add("package".to_string(), table.get("package"));
    add("lib".to_string(), table.get("lib"));
    for kind in ["bin", "example", "test", "bench"] {
        for (i, target) in table
            .get(kind)
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            add(format!("{kind}[{i}]"), Some(target));
        }
    }
    found
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

/// The lines of Rust a pull request adds, as written, judged by their code
/// alone: a line counts only if its code, without comments, strings and char
/// literals, is new.
fn added_code_lines(changes: &Changes, path: &str) -> Vec<(String, String)> {
    let base = code_of(&changes.base, path).unwrap_or_default();
    let head_text = changes.head.text(path).unwrap_or_default();
    let head = code_of(&changes.head, path).unwrap_or_default();
    let mut base_lines: Vec<&str> = base.lines().map(str::trim_end).collect();
    base_lines.sort_unstable();
    let mut added = Vec::new();
    for (code, line) in head.lines().zip(head_text.lines()) {
        match base_lines.binary_search(&code.trim_end()) {
            Ok(at) => {
                base_lines.remove(at);
            }
            Err(_) => added.push((code.to_string(), line.to_string())),
        }
    }
    added
}

/// New `unsafe` code anywhere, or a change to where it's allowed: a line of
/// Rust with `unsafe` in its code, a newly allowed `unsafe_code` lint, a
/// crate's own `unsafe_code` setting, a crate that stops taking the
/// workspace's lints, or a change to the workspace's `unsafe_code = "forbid"`.
fn unsafe_code(changes: &Changes, flags: &mut Vec<RedFlag>) {
    for path in &changes.paths {
        let mut found: Vec<String> = Vec::new();
        if path.ends_with(".rs") {
            found.extend(
                added_code_lines(changes, path)
                    .into_iter()
                    .filter(|(code, _)| has_word(code, "unsafe"))
                    .map(|(_, line)| code(&line)),
            );
            let allows = |side: &Side| {
                code_of(side, path)
                    .map(|code| allowed_lints(&code))
                    .unwrap_or_default()
                    .iter()
                    .filter(|lint| lint.as_str() == "unsafe_code")
                    .count()
            };
            if allows(&changes.head) > allows(&changes.base) {
                found.push("an allowance of the `unsafe_code` lint".to_string());
            }
        } else if path == "Cargo.toml" {
            if workspace_unsafe_setting(changes.base.text(path))
                != workspace_unsafe_setting(changes.head.text(path))
            {
                found.push("a change to the workspace's `unsafe_code = \"forbid\"`".to_string());
            }
        } else if path.starts_with("crates/") && path.ends_with("/Cargo.toml") {
            let setting = |text: Option<String>| {
                lints_of(text).and_then(|lints| lints.get("rust")?.get("unsafe_code").cloned())
            };
            if let Some(now) = setting(changes.head.text(path))
                && setting(changes.base.text(path)).as_ref() != Some(&now)
            {
                found.push(format!(
                    "its own `unsafe_code` setting, {}",
                    code(&now.to_string())
                ));
            }
            if !MAY_USE_UNSAFE.contains(&path.as_str())
                && takes_workspace_lints(changes.base.text(path))
                && !takes_workspace_lints(changes.head.text(path))
            {
                found.push(
                    "no `[lints] workspace = true` any more, so `unsafe` isn't forbidden there"
                        .to_string(),
                );
            }
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

/// How many times a Rust file's code compiles another file's text as Rust:
/// `include!(…)`, or a `path = "…"` attribute (`#[path = "…"] mod m;`, also
/// inside a `cfg_attr`).
fn other_file_uses(code: &str) -> usize {
    let compact: String = code.chars().filter(|c| !c.is_whitespace()).collect();
    let is_name = |c: char| c.is_alphanumeric() || c == '_';
    let includes = compact
        .match_indices("include!")
        .filter(|(at, _)| !compact[..*at].chars().next_back().is_some_and(is_name))
        .count();
    let paths = compact
        .match_indices("path=")
        .filter(|(at, _)| {
            matches!(compact[..*at].chars().next_back(), Some('[' | '(' | ','))
                && !compact[at + 5..].starts_with('=')
        })
        .count();
    includes + paths
}

/// Rust that compiles another file's text: a new `include!` or `#[path]` in a
/// Rust file. The Report reads only `.rs` files, so the file it names, which
/// may end in anything, is for the Reviewer to read.
fn other_files(changes: &Changes, flags: &mut Vec<RedFlag>) {
    let files: Vec<String> = changes
        .paths
        .iter()
        .filter(|path| path.ends_with(".rs"))
        .filter(|path| {
            let uses = |side: &Side| code_of(side, path).map_or(0, |c| other_file_uses(&c));
            uses(&changes.head) > uses(&changes.base)
        })
        .map(|path| code(path))
        .collect();
    if !files.is_empty() {
        flags.push(RedFlag::new(
            Level::ReviewerDecides,
            "Code read from another file",
            format!(
                "{} adds `include!` or `#[path]`, which compile another file's text as Rust. \
                 The Red Flags read only `.rs` files, so read the file it names for new \
                 `unsafe` code or a house-rule exception.",
                listed(&files, 10, "files")
            ),
        ));
    }
}

/// The folders whose files GitHub runs as workflows, or as their actions.
const WORKFLOWS: [&str; 2] = [".github/workflows/", ".github/actions/"];

/// A change to CI's workflows. Any workflow can set any commit status, so a
/// PR that changes one could set the Red Flag gate and the Review check
/// itself: the Report says so plainly.
fn workflows(changes: &Changes, flags: &mut Vec<RedFlag>) {
    let files: Vec<String> = changes
        .paths
        .iter()
        .filter(|path| WORKFLOWS.iter().any(|folder| path.starts_with(folder)))
        .map(|path| code(path))
        .collect();
    if !files.is_empty() {
        flags.push(RedFlag::new(
            Level::ReviewerDecides,
            "A change to CI workflows",
            format!(
                "{}. A workflow can set any commit status, so for this PR the Red Flag gate's and \
                 the Review check's own results can't be trusted. Check that no workflow it \
                 changes or adds asks for `statuses: write` or `permissions: write-all`, or sets \
                 a status, and the maintainer should merge it by hand.",
                listed(&files, 10, "files")
            ),
        ));
    }
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
    let edition_changes = |path: &str| {
        (path == "Cargo.toml" || path.ends_with("/Cargo.toml"))
            && editions(changes.base.text(path)) != editions(changes.head.text(path))
    };
    // A changed Rust edition, in any manifest, first: it changes how every
    // file of the crate reads.
    let files: Vec<String> = changes
        .paths
        .iter()
        .filter(|path| edition_changes(path))
        .map(|path| format!("{} (a Rust edition changes)", code(path)))
        .chain(
            changes
                .paths
                .iter()
                .filter(|path| !edition_changes(path) && areas.of(path) == Some(REPO_RULES))
                .map(|path| code(path)),
        )
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
