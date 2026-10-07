//! A Pack's manifest, `pack.toml` (#16 §1, ADR-0011).

use crate::document::{Document, Problems};

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
    /// The Pack's main licence, as a standard SPDX name.
    pub licence: String,
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
];

/// Reads `pack.toml`. A broken manifest skips the whole Pack, so every
/// problem is returned at once.
pub fn read_manifest(file: &str, text: &str) -> Result<Manifest, Problems> {
    let doc = Document::parse(file, text)?;
    let mut problems = Problems::new();
    doc.check_format(&mut problems);
    let root = doc.root();
    root.refuse_unknown(KEYS, &mut problems);
    let mut field = |key: &str| {
        root.text(key, &mut problems)
            .map(|(text, item)| (text.to_string(), item))
    };
    let id = field("id");
    let name = field("name");
    let description = field("description");
    let version = field("version");
    let author = field("author");
    let licence = field("licence");
    if let Some((id, item)) = &id
        && !is_an_id(id)
    {
        problems.push(item.problem(format!(
            "the Pack's id \"{id}\" must be lowercase words joined by dashes, such as \"opendrone\""
        )));
    }
    let manifest = (|| {
        Some(Manifest {
            id: id?.0,
            name: name?.0,
            description: description?.0,
            version: version?.0,
            author: author?.0,
            licence: licence?.0,
        })
    })();
    match manifest {
        Some(manifest) => problems.or(manifest),
        None => Err(problems),
    }
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
