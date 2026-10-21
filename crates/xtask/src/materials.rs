//! The material library's manifest, `assets-src/materials.toml` (ADR-0014,
//! #63): which materials Maps use and the CC0 textures behind them, each with
//! where it comes from, its licence and the SHA-256 of its file. `cargo xtask
//! textures` fetches and checks the files; `cargo xtask credits` writes
//! `CREDITS.md` from it. The Blender scripts read the same file, so a
//! material's name and colour can't drift apart from it.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use toml::{Table, Value};

/// Where the manifest is, from the repo's root.
pub const MANIFEST: &str = "assets-src/materials.toml";

/// Where fetched pictures go, from the repo's root. Git ignores the folder:
/// pictures are fetched, never committed.
pub const TEXTURES_FOLDER: &str = "assets-src/textures";

/// The places textures may come from. Both are CC0.
const SOURCES: [&str; 2] = ["Poly Haven", "ambientCG"];

/// One picture of a texture: a colour map or a normal map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextureFile {
    /// `diffuse` or `normal`.
    pub kind: String,
    pub url: String,
    /// The name inside the zip the URL gives, if it gives one (ambientCG).
    pub in_zip: Option<String>,
    /// The SHA-256 of the file, 64 lowercase hex digits; empty until pinned.
    pub sha256: String,
}

impl TextureFile {
    /// Where the fetched file lives, below [`TEXTURES_FOLDER`], such as
    /// `plywood/diffuse.jpg`.
    pub fn cached_as(&self, texture: &str) -> String {
        format!("{texture}/{}.jpg", self.kind)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Texture {
    /// The key in the manifest, the source's own id, such as `plywood`.
    pub name: String,
    pub source: String,
    pub page: String,
    pub author: String,
    pub licence: String,
    /// What one repeat of the picture covers, such as `3 m`.
    pub size: String,
    pub files: Vec<TextureFile>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    /// Each material's name and the texture it uses, if any.
    pub materials: Vec<(String, Option<String>)>,
    pub textures: Vec<Texture>,
}

pub fn read(root: &Path) -> Result<Manifest, String> {
    let path = root.join(MANIFEST);
    let text = fs::read_to_string(&path).map_err(|e| format!("{MANIFEST}: can't be read: {e}"))?;
    parse(&text)
}

pub fn parse(text: &str) -> Result<Manifest, String> {
    let doc: Table = text
        .parse()
        .map_err(|e| format!("{MANIFEST} isn't TOML: {e}"))?;
    let mut problems = Vec::new();
    let mut textures = Vec::new();
    let table = |value: Option<&Value>, what: &str, problems: &mut Vec<String>| match value {
        Some(Value::Table(t)) => Some(t.clone()),
        _ => {
            problems.push(format!("{MANIFEST} has no [{what}] table"));
            None
        }
    };
    let text_of = |t: &Table, key: &str, whose: &str, problems: &mut Vec<String>| match t.get(key) {
        Some(Value::String(s)) => s.clone(),
        _ => {
            problems.push(format!("{whose} needs `{key}`, in quotes"));
            String::new()
        }
    };
    if let Some(all) = table(doc.get("textures"), "textures", &mut problems) {
        for (name, entry) in &all {
            let whose = format!("the texture \"{name}\"");
            let Value::Table(entry) = entry else {
                problems.push(format!("{whose} must be a table"));
                continue;
            };
            let source = text_of(entry, "source", &whose, &mut problems);
            if !source.is_empty() && !SOURCES.contains(&source.as_str()) {
                problems.push(format!(
                    "{whose} comes from \"{source}\", but textures come only from {}",
                    SOURCES.join(" and ")
                ));
            }
            let licence = text_of(entry, "licence", &whose, &mut problems);
            if !licence.is_empty() && licence != "CC0-1.0" {
                problems.push(format!(
                    "{whose} is under {licence}, but only CC0 pictures enter the repo (ADR-0014)"
                ));
            }
            let mut files = Vec::new();
            for kind in ["diffuse", "normal"] {
                let Some(Value::Table(file)) = entry.get(kind) else {
                    problems.push(format!(
                        "{whose} needs a `{kind}` file, written {{ url = \"…\", sha256 = \"…\" }}"
                    ));
                    continue;
                };
                let sha256 = text_of(file, "sha256", &format!("{whose}'s {kind}"), &mut problems);
                if !sha256.is_empty()
                    && !(sha256.len() == 64
                        && sha256.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')))
                {
                    problems.push(format!(
                        "{whose}'s {kind} checksum isn't a SHA-256: write 64 lowercase hex digits, or leave it empty for `cargo xtask textures --pin` to fill in"
                    ));
                }
                files.push(TextureFile {
                    kind: kind.to_string(),
                    url: text_of(file, "url", &format!("{whose}'s {kind}"), &mut problems),
                    in_zip: file
                        .get("in_zip")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    sha256,
                });
            }
            textures.push(Texture {
                name: name.clone(),
                source,
                page: text_of(entry, "page", &whose, &mut problems),
                author: text_of(entry, "author", &whose, &mut problems),
                licence,
                size: text_of(entry, "size", &whose, &mut problems),
                files,
            });
        }
    }
    let mut materials = Vec::new();
    if let Some(all) = table(doc.get("materials"), "materials", &mut problems) {
        for (name, entry) in &all {
            let Value::Table(entry) = entry else {
                problems.push(format!("the material \"{name}\" must be a table"));
                continue;
            };
            let texture = entry
                .get("texture")
                .and_then(Value::as_str)
                .map(str::to_string);
            if let Some(texture) = &texture
                && !textures.iter().any(|t| &t.name == texture)
            {
                problems.push(format!(
                    "the material \"{name}\" uses the texture \"{texture}\", which [textures] doesn't list"
                ));
            }
            materials.push((name.clone(), texture));
        }
    }
    for texture in &textures {
        if !materials
            .iter()
            .any(|(_, t)| t.as_deref() == Some(&texture.name))
        {
            problems.push(format!(
                "the texture \"{}\" is listed but no material uses it: take it out, so nothing is credited for nothing",
                texture.name
            ));
        }
    }
    if problems.is_empty() {
        Ok(Manifest {
            materials,
            textures,
        })
    } else {
        Err(problems.join("\n"))
    }
}

/// The repo's root: the nearest folder above this one that holds `assets-src/`.
pub fn repo() -> Result<PathBuf, String> {
    let here = std::env::current_dir().map_err(|e| format!("can't tell where this is: {e}"))?;
    here.ancestors()
        .find(|folder| folder.join("assets-src").is_dir())
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            "Run this inside the OpenDrone repo: no folder here or above holds assets-src/.".into()
        })
}

/// `CREDITS.md`, generated from the manifest and the list of datasets.
pub fn credits(manifest: &Manifest, datasets: &[crate::datasets::Dataset]) -> String {
    let mut out = String::new();
    out.push_str(
        "# Credits\n\n\
This file lists every outside asset and dataset in OpenDrone, each with its source, its licence and a checksum ([ADR-0014](docs/adr/0014-licences-for-libraries-and-assets.md)). **It is generated** from the texture manifest, [`assets-src/materials.toml`](assets-src/materials.toml), and the list of datasets, [`assets-src/datasets.toml`](assets-src/datasets.toml), by `cargo xtask credits`; don't edit it by hand. CI checks it is up to date. Only CC0 assets and CC BY data enter the repo.\n\n\
## Textures\n\n\
Poly Haven and ambientCG pictures, all CC0 1.0: anyone may use them for any purpose, with no credit needed, and they are credited here all the same. They are not in the repo: `cargo xtask textures` downloads each into `assets-src/textures/`, and refuses a file whose SHA-256 differs from the one below. A Map's `.glb` carries only a material's name; the game swaps the pictures in by name.\n\n\
| Texture | Source | By | Licence | File | SHA-256 |\n|---|---|---|---|---|---|\n",
    );
    for texture in &manifest.textures {
        for (i, file) in texture.files.iter().enumerate() {
            let name = if i == 0 {
                format!("[{}]({})", texture.name, texture.page)
            } else {
                String::new()
            };
            let (source, author, licence) = if i == 0 {
                (
                    texture.source.as_str(),
                    texture.author.as_str(),
                    texture.licence.as_str(),
                )
            } else {
                ("", "", "")
            };
            let _ = writeln!(
                out,
                "| {name} | {source} | {author} | {licence} | {} | `{}` |",
                file.kind, file.sha256
            );
        }
    }
    out.push_str(&crate::datasets::credits(datasets));
    out.push_str(
        "\n## Not yet\n\n\
- Sound files: the CC0 hit and menu clips, and each Map's Background Sound, are listed here the same way once they are in the repo.\n\
- Outside fonts (OFL-1.1), if any are used. The OSD fonts are OpenDrone's own, under CC0.\n",
    );
    if datasets.is_empty() {
        out.push_str("- Reference datasets under CC BY.\n");
    }
    out.push_str(
        "\n\
Nothing else from outside is in the repo.\n\n\
The Rust libraries OpenDrone uses aren't listed here: their licences are checked by cargo-deny against [`deny.toml`](deny.toml).\n",
    );
    out
}
