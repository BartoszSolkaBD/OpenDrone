//! A Pack's manifest, `pack.toml` (#16 §1 and §12, ADR-0011).

use crate::document::{self, Document, Problems, Table};

/// What a Pack says about itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    /// The first half of every item's id, such as `opendrone` in
    /// `opendrone/whoop-65`. It never changes once released.
    pub id: String,
    pub name: String,
    pub description: String,
    /// The Pack author's own label for this version.
    pub version: String,
    pub author: String,
    /// The Pack's main licence, as a standard SPDX name, such as `CC0-1.0`.
    pub licence: String,
    /// Files under another licence, by path, from the `[licences]` list.
    pub licences: Vec<LicenceOverride>,
    /// The Quads taken out of this Pack, from the `[retired]` list, in the
    /// order it writes them.
    pub retired: Vec<Retired>,
}

/// One line of `[retired]`: a Quad taken out of the Pack, and why. A change
/// that takes a Quad out must say so here, unless it only renames or moves
/// it, and CI lists every one for the Reviewer (`cargo xtask feel-tests`).
/// The Quad's folder must be gone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Retired {
    /// The Quad's folder name, its id inside the Pack, such as `whoop-65`
    /// from the line `"quads/whoop-65" = "…"`.
    pub quad: String,
    /// Why it was taken out, in a sentence.
    pub why: String,
    /// The line of `pack.toml` that retires it.
    pub line: usize,
}

/// One line of `[licences]`: files under a licence other than the Pack's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LicenceOverride {
    /// A path inside the Pack, where `*` and `**` match any name, such as
    /// `"maps/harbour/textures/**"`.
    pub path: String,
    pub licence: String,
    /// Who to credit. Anything under a CC BY licence needs one.
    pub credit: Option<String>,
}

const KEYS: &[&str] = &[
    "format",
    "id",
    "name",
    "description",
    "version",
    "author",
    "licence",
    "licences",
    "retired",
];

/// The id kept for Test Quads and the built-in Test Maps (#16 §2).
const TEST: &str = "test";

/// Reads `pack.toml`. A broken manifest skips the whole Pack, so every
/// problem is returned at once.
pub fn read_manifest(file: &str, text: &str) -> Result<Manifest, Problems> {
    let text = document::upgraded(file, text, document::PACK_UPGRADES)?;
    let doc = Document::parse(file, &text)?;
    let mut problems = Problems::new();
    doc.check_format(&mut problems);
    if doc.is_newer() {
        return Err(problems);
    }
    let root = doc.root();
    root.refuse_unknown(KEYS, &mut problems);
    let mut field = |key: &str| {
        let (text, item) = root.text(key, &mut problems)?;
        if text.trim().is_empty() {
            problems.push(item.problem(format!("`{key}` can't be empty")));
        }
        Some((text.to_string(), item))
    };
    let id = field("id");
    let name = field("name");
    let description = field("description");
    let version = field("version");
    let author = field("author");
    let licence = field("licence");
    if let Some((id, item)) = &id {
        if !is_an_id(id) {
            problems.push(item.problem(format!(
                "the Pack's id \"{id}\" must be lowercase words joined by dashes, such as \"opendrone\""
            )));
        } else if id == TEST {
            problems.push(item.problem(
                "the id \"test\" is kept for Test Quads and Test Maps, which only Scenarios use; pick another",
            ));
        }
    }
    if let Some((licence, item)) = &licence
        && !is_a_licence(licence)
    {
        problems.push(item.problem(format!(
            "\"{licence}\" isn't a licence's standard (SPDX) name, such as \"CC0-1.0\" or \"CC-BY-4.0\""
        )));
    }
    let mut licences = Vec::new();
    if let Some(item) = root.get("licences")
        && let Some(table) = item.table(&mut problems)
    {
        for (path, item) in table.entries() {
            if path.is_empty()
                || path.starts_with('/')
                || path.contains('\\')
                || path.split('/').any(|part| part == "..")
            {
                problems.push(item.problem(format!(
                    "\"{path}\" isn't a path inside the Pack: write it from the Pack's folder with /, such as \"maps/harbour/textures/**\""
                )));
                continue;
            }
            let Some(entry) = item.table(&mut problems) else {
                continue;
            };
            entry.refuse_unknown(&["licence", "credit"], &mut problems);
            let licence = entry.text("licence", &mut problems);
            let credit = match entry.get("credit") {
                Some(item) => item.text(&mut problems).map(str::to_string),
                None => None,
            };
            let Some((licence, licence_item)) = licence else {
                continue;
            };
            if !is_a_licence(licence) {
                problems.push(licence_item.problem(format!(
                    "\"{licence}\" isn't a licence's standard (SPDX) name, such as \"CC-BY-4.0\""
                )));
            }
            if licence.contains("CC-BY") && credit.as_deref().is_none_or(|c| c.trim().is_empty()) {
                problems.push(entry.problem(format!(
                    "\"{path}\" is under {licence}, so it needs a credit line: add credit = \"…\""
                )));
            }
            licences.push(LicenceOverride {
                path,
                licence: licence.to_string(),
                credit,
            });
        }
    }
    let retired = read_retired(&root, &mut problems);
    let manifest = (|| {
        Some(Manifest {
            id: id?.0,
            name: name?.0,
            description: description?.0,
            version: version?.0,
            author: author?.0,
            licence: licence?.0,
            licences,
            retired,
        })
    })();
    match manifest {
        Some(manifest) => problems.or(manifest),
        None => Err(problems),
    }
}

/// The `[retired]` list: each line names a Quad's folder and says why it was
/// taken out, such as `"quads/whoop-65" = "Replaced by the Whoop 75."`.
fn read_retired(root: &Table<'_, '_>, problems: &mut Problems) -> Vec<Retired> {
    let mut retired = Vec::new();
    let Some(table) = root.get("retired").and_then(|item| item.table(problems)) else {
        return retired;
    };
    for (path, item) in table.entries() {
        let Some(quad) = path.strip_prefix("quads/").filter(|quad| is_an_id(quad)) else {
            problems.push(item.problem(format!(
                "\"{path}\" isn't a Quad of this Pack: write its folder, such as \"quads/whoop-65\"; so far only Quads can be retired"
            )));
            continue;
        };
        let Some(why) = item.text(problems) else {
            continue;
        };
        if why.trim().is_empty() {
            problems.push(item.problem(format!(
                "\"{path}\" needs a why: say in a sentence why it was taken out"
            )));
            continue;
        }
        retired.push(Retired {
            quad: quad.to_string(),
            why: why.to_string(),
            line: item.line(),
        });
    }
    retired
}

/// Lowercase words (letters and digits) joined by single dashes, such as
/// `whoop-65`: the form of every Pack and item id (#16 §2).
pub fn is_an_id(text: &str) -> bool {
    !text.is_empty()
        && text.split('-').all(|word| {
            !word.is_empty()
                && word
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

/// A licence's SPDX name, or names joined by OR, AND or WITH, such as
/// `MIT OR Apache-2.0`.
fn is_a_licence(text: &str) -> bool {
    text.split(' ').enumerate().all(|(i, word)| {
        if i % 2 == 1 {
            matches!(word, "OR" | "AND" | "WITH")
        } else {
            !word.is_empty()
                && word
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'+'))
        }
    }) && text.split(' ').count() % 2 == 1
}
