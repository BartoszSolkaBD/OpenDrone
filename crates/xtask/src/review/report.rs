//! The Review Report's sections from Red Flags to Downloads (#15 §10),
//! worked out from what a pull request changes. The Verdict section on top is
//! added when the comment is posted, because it changes with every Verdict.

use std::fmt::Write as _;

use serde_json::{Value, json};

use super::areas::{Areas, EVERYTHING_ELSE};
use super::changes::Changes;
use super::flags::{self, Level, RedFlag};
use super::libraries::{self, NewLibrary};
use super::results::{self, Moved};

/// Where the Areas are written.
pub const CODEOWNERS: &str = ".github/CODEOWNERS";

/// At most this many rows of What moved; the rest are counted.
const MOST_MOVES: usize = 100;

/// Everything the Review Report says about a pull request's changes.
#[derive(Clone, Debug)]
pub struct Review {
    pub flags: Vec<RedFlag>,
    pub moved: Moved,
    /// Each Area touched, in CODEOWNERS order, with its changed files.
    pub areas: Vec<(String, Vec<String>)>,
    pub new_libraries: Vec<NewLibrary>,
}

impl Review {
    /// Works out the Review Report for a set of changes. `metadata` is the
    /// head's `cargo metadata`, for the new libraries' licences.
    pub fn of(changes: &Changes, metadata: Option<&Value>) -> Review {
        let codeowners = changes
            .base
            .text(CODEOWNERS)
            .or_else(|| changes.head.text(CODEOWNERS))
            .unwrap_or_default();
        let areas = Areas::parse(&codeowners);
        let new_libraries = if changes.touches("Cargo.lock") {
            libraries::new_libraries(
                &libraries::read_lock(&changes.base.text("Cargo.lock").unwrap_or_default()),
                &libraries::read_lock(&changes.head.text("Cargo.lock").unwrap_or_default()),
                metadata,
            )
        } else {
            Vec::new()
        };
        Review {
            flags: flags::detect(changes, &areas, &new_libraries),
            moved: results::moved(changes),
            areas: touched(changes, &areas),
            new_libraries,
        }
    }

    /// Whether a Red Flag waits for the maintainer, so the gate fails.
    pub fn waits_for_maintainer(&self) -> bool {
        self.flags
            .iter()
            .any(|f| f.level == Level::WaitsForMaintainer)
    }

    /// The Areas touched, for PR labels. Files only the catch-all covers have
    /// no Area.
    pub fn area_names(&self) -> Vec<String> {
        self.areas
            .iter()
            .map(|(area, _)| area.clone())
            .filter(|area| area != EVERYTHING_ELSE)
            .collect()
    }

    /// `review.json`: what the privileged workflow acts on.
    pub fn json(&self) -> Value {
        json!({
            "waits_for_maintainer": self.waits_for_maintainer(),
            "areas": self.area_names(),
            "red_flags": self.flags.iter().map(|f| json!({
                "level": f.level.key(),
                "title": f.title,
                "detail": f.detail,
            })).collect::<Vec<_>>(),
        })
    }

    /// The sections from Red Flags to Downloads, in Markdown.
    pub fn markdown(&self) -> String {
        let mut text = String::new();
        self.red_flags(&mut text);
        self.what_moved(&mut text);
        let _ = writeln!(
            text,
            "### Speed\n\nNot measured yet. Work Counts arrive with the physics Work Count gate \
             (#78) and the render Work Count gate (#79).\n"
        );
        self.areas_touched(&mut text);
        let _ = writeln!(
            text,
            "### Renders\n\nNo Map renders yet. Renders of every changed Map arrive with the \
             asset pipeline (#63).\n"
        );
        let _ = writeln!(
            text,
            "### Downloads\n\nNo Blackbox logs or builds yet. Blackbox logs from Scenario runs \
             arrive with #55, and PR builds (the `build` label) with #81."
        );
        text
    }

    fn red_flags(&self, text: &mut String) {
        let _ = writeln!(text, "### Red Flags\n");
        if self.flags.is_empty() {
            let _ = writeln!(text, "None.\n");
            return;
        }
        for level in Level::ALL {
            let flags: Vec<&RedFlag> = self.flags.iter().filter(|f| f.level == level).collect();
            if flags.is_empty() {
                continue;
            }
            let heading = match level {
                Level::WaitsForMaintainer => {
                    "**Waits for the maintainer.** The Red Flag gate fails, and this PR gets the \
                     `needs-maintainer` label."
                }
                Level::ReviewerDecides => {
                    "**The Reviewer decides.** The Reviewer checks that each one holds up."
                }
                Level::ListedOnly => "**Listed only.**",
            };
            let _ = writeln!(text, "{heading}\n");
            for flag in flags {
                let _ = writeln!(text, "- **{}**: {}", flag.title, flag.detail);
            }
            let _ = writeln!(text);
        }
    }

    fn what_moved(&self, text: &mut String) {
        let _ = writeln!(text, "### What moved\n");
        let moved = &self.moved;
        if moved.is_empty() {
            let _ = writeln!(
                text,
                "Nothing moved: no Results file changed, so every measured value is the same as \
                 before.\n"
            );
            return;
        }
        if moved.moves.is_empty() {
            let _ = writeln!(text, "No measured value moved.\n");
        } else {
            let _ = writeln!(
                text,
                "Every Expectation whose measured value moved, biggest move first.\n\n\
                 | Scenario | Expectation | Before | After | Move |\n|---|---|---|---|---|"
            );
            for m in moved.moves.iter().take(MOST_MOVES) {
                let _ = writeln!(
                    text,
                    "| {} | {} | {} | {} | {} |",
                    cell(&m.scenario),
                    cell(&m.what),
                    cell(&m.before),
                    cell(&m.after),
                    m.size.text()
                );
            }
            if moved.moves.len() > MOST_MOVES {
                let _ = writeln!(
                    text,
                    "\n…and {} smaller moves, which the Results files' diff shows.",
                    moved.moves.len() - MOST_MOVES
                );
            }
            let _ = writeln!(text);
        }
        for (heading, lines) in [
            ("New Expectations", &moved.new),
            ("Expectations no longer measured", &moved.gone),
            ("Fingerprints that moved", &moved.fingerprints),
        ] {
            if lines.is_empty() {
                continue;
            }
            let _ = writeln!(text, "{heading}:\n");
            for line in lines.iter().take(MOST_MOVES) {
                let _ = writeln!(text, "- {line}");
            }
            if lines.len() > MOST_MOVES {
                let _ = writeln!(text, "- …and {} more", lines.len() - MOST_MOVES);
            }
            let _ = writeln!(text);
        }
    }

    fn areas_touched(&self, text: &mut String) {
        let _ = writeln!(text, "### Areas touched\n");
        if self.areas.is_empty() {
            let _ = writeln!(text, "None: this PR changes no files.\n");
        }
        for (area, files) in &self.areas {
            let shown: Vec<String> = files.iter().take(5).map(|f| format!("`{f}`")).collect();
            let more = if files.len() > 5 {
                format!(" and {} more", files.len() - 5)
            } else {
                String::new()
            };
            let name = if area == EVERYTHING_ELSE {
                "Everything else (no Area)"
            } else {
                area
            };
            let _ = writeln!(text, "- **{name}**: {}{more}", shown.join(", "));
        }
        if !self.areas.is_empty() {
            let _ = writeln!(text);
        }
        if self.new_libraries.is_empty() {
            let _ = writeln!(text, "No new outside libraries.\n");
        } else {
            let _ = writeln!(
                text,
                "New outside libraries (cargo-deny checks their licences against ADR-0014):\n\n\
                 | Library | Version | Licence |\n|---|---|---|"
            );
            for library in &self.new_libraries {
                let _ = writeln!(
                    text,
                    "| {} | {} | {} |",
                    cell(&library.name),
                    cell(&library.version),
                    cell(library.licence.as_deref().unwrap_or("not looked up"))
                );
            }
            let _ = writeln!(text);
        }
    }
}

/// Each Area touched, in CODEOWNERS order with the catch-all last, and its
/// changed files.
fn touched(changes: &Changes, areas: &Areas) -> Vec<(String, Vec<String>)> {
    let mut order = areas.names();
    order.retain(|a| a != EVERYTHING_ELSE);
    order.push(EVERYTHING_ELSE.to_string());
    order
        .into_iter()
        .filter_map(|area| {
            let files: Vec<String> = changes
                .paths
                .iter()
                .filter(|path| areas.of(path).unwrap_or(EVERYTHING_ELSE) == area)
                .cloned()
                .collect();
            (!files.is_empty()).then_some((area, files))
        })
        .collect()
}

/// Text safe inside a Markdown table cell.
fn cell(text: &str) -> String {
    text.replace('|', "\\|").replace('\n', " ")
}
