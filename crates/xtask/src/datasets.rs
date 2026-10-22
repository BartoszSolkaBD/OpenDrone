//! The outside datasets whose files are in the repo, `assets-src/datasets.toml`
//! (ADR-0014, `docs/data-policy.md`): only CC BY data, committed as trimmed
//! CSV files with an attribution notice beside them. `cargo xtask credits`
//! lists each file in `CREDITS.md` with its SHA-256, so a changed data file
//! makes `CREDITS.md` out of date until it is written again.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use toml::{Table, Value};

/// Where the list is, from the repo's root.
pub const DATASETS: &str = "assets-src/datasets.toml";

/// The licences a committed dataset may carry: CC BY only.
const LICENCES: [&str; 1] = ["CC-BY-4.0"];

/// The notice every dataset's folder holds beside its files.
const NOTICE: &str = "NOTICE.md";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dataset {
    pub name: String,
    pub page: String,
    /// Who made it, as its attribution names them.
    pub by: String,
    pub licence: String,
    /// The folder its files are in, from the repo's root.
    pub folder: String,
    /// Each CSV file in the folder, by name, in name order, with its
    /// SHA-256.
    pub files: Vec<(String, String)>,
}

/// Reads the list and checksums every file it covers. No list means no
/// datasets.
pub fn read(root: &Path) -> Result<Vec<Dataset>, String> {
    let path = root.join(DATASETS);
    let Ok(text) = fs::read_to_string(&path) else {
        return Ok(Vec::new());
    };
    let doc: Table = text
        .parse()
        .map_err(|e| format!("{DATASETS} isn't TOML: {e}"))?;
    let mut problems = Vec::new();
    let mut datasets = Vec::new();
    let Some(Value::Array(list)) = doc.get("dataset") else {
        return Err(format!(
            "{DATASETS} lists each dataset as a [[dataset]] table"
        ));
    };
    for (n, entry) in list.iter().enumerate() {
        let Value::Table(entry) = entry else {
            problems.push(format!("{DATASETS}: dataset {} must be a table", n + 1));
            continue;
        };
        let mut text_of = |key: &str| match entry.get(key) {
            Some(Value::String(s)) => s.clone(),
            _ => {
                problems.push(format!(
                    "{DATASETS}: dataset {} needs `{key}`, in quotes",
                    n + 1
                ));
                String::new()
            }
        };
        let name = text_of("name");
        let page = text_of("page");
        let by = text_of("by");
        let licence = text_of("licence");
        let folder = text_of("folder");
        if !licence.is_empty() && !LICENCES.contains(&licence.as_str()) {
            problems.push(format!(
                "{DATASETS}: \"{name}\" is under {licence}, but only CC BY data enters the repo (ADR-0014, docs/data-policy.md)"
            ));
        }
        if folder.is_empty() {
            continue;
        }
        let dir = root.join(&folder);
        if !dir.join(NOTICE).is_file() {
            problems.push(format!(
                "{DATASETS}: \"{name}\"'s folder {folder} has no {NOTICE}: CC BY data carries its attribution notice beside its files"
            ));
        }
        let mut names: Vec<String> = match fs::read_dir(&dir) {
            Ok(entries) => entries
                .filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|name| name.ends_with(".csv"))
                .collect(),
            Err(e) => {
                problems.push(format!("{DATASETS}: can't read {folder}: {e}"));
                continue;
            }
        };
        names.sort();
        if names.is_empty() {
            problems.push(format!(
                "{DATASETS}: \"{name}\"'s folder {folder} holds no CSV files"
            ));
        }
        let mut files = Vec::new();
        for file in names {
            match fs::read(dir.join(&file)) {
                Ok(bytes) => files.push((file, crate::textures::sha256(&bytes))),
                Err(e) => problems.push(format!("can't read {folder}/{file}: {e}")),
            }
        }
        datasets.push(Dataset {
            name,
            page,
            by,
            licence,
            folder,
            files,
        });
    }
    if problems.is_empty() {
        Ok(datasets)
    } else {
        Err(problems.join("\n"))
    }
}

/// `CREDITS.md`'s section for the datasets, empty when there are none.
pub fn credits(datasets: &[Dataset]) -> String {
    if datasets.is_empty() {
        return String::new();
    }
    let mut out = String::from(
        "\n## Datasets\n\n\
Outside data under CC BY 4.0, committed only as trimmed CSV files, each folder with its attribution notice beside the files ([the data policy](docs/data-policy.md)). Anyone may use them for any purpose, crediting their makers as the notice says. Data with no licence, such as NeuroBEM, is downloaded when it's needed and never committed, so it isn't listed here.\n\n\
| Dataset | By | Licence | File | SHA-256 |\n|---|---|---|---|---|\n",
    );
    for dataset in datasets {
        for (i, (file, sha256)) in dataset.files.iter().enumerate() {
            let (name, by, licence) = if i == 0 {
                (
                    format!("[{}]({})", dataset.name, dataset.page),
                    dataset.by.as_str(),
                    dataset.licence.as_str(),
                )
            } else {
                (String::new(), "", "")
            };
            let _ = writeln!(
                out,
                "| {name} | {by} | {licence} | [`{file}`]({}/{file}) | `{sha256}` |",
                dataset.folder
            );
        }
    }
    out
}
