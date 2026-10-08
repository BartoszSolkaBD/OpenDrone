//! Format migration: the named steps that bring a file from one format to
//! the next (ADR-0002, ADR-0011, #60).
//!
//! Every Scenario and every Pack file starts with `format = N`. Scenarios
//! and Pack files are numbered apart, so a new starting-state item never
//! touches a Pack, and a Pack change never touches a Scenario. When a new
//! item arrives, the pull request that brings it adds one [`Step`]:
//!
//! - **The repo's own files.** `cargo xtask migrate <step>` runs the step over
//!   every Scenario, or over every Pack file, Test Quad and Test Map, writing the new
//!   item with the value that keeps today's behaviour and bumping the
//!   `format` line, and keeps every comment and the layout ([`migrate`]).
//! - **A pilot's older Pack.** The Pack reader runs the same steps in memory,
//!   one format at a time, before it reads the file ([`upgraded`]). Scenarios
//!   are never upgraded in memory: their starting state spells out
//!   everything in the file itself, so an older Scenario is refused, naming
//!   the step that brings it up to date.
//!
//! The steps rewrite the file's text with `toml_edit`, which keeps comments,
//! blank lines and spacing as they are. [`add_after`] and
//! [`add_to_inline_table`] add an item lined up with its neighbours.

use std::borrow::Cow;
use std::fs;
use std::path::{Path, PathBuf};

pub use toml_edit;
use toml_edit::{Decor, DocumentMut, Item, Key, Table, Value};

use crate::document::{Document, FORMAT, Problem, Problems};

/// The steps that upgrade Pack files, one format each, in order: the first
/// upgrades format 1, the next format 2, and so on. The pull request that
/// changes a Pack file's format adds its step here; [`FORMAT`] follows.
pub const PACK_STEPS: &[Step] = &[];

/// The kinds of file that carry a format number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    /// A Scenario, `scenarios/<topic>/<name>.toml`.
    Scenario,
    /// A Pack's manifest, `pack.toml`.
    Manifest,
    /// A Quad definition, `quads/<id>/quad.toml`.
    Quad,
    /// A Test Quad, `scenarios/test-quads/<id>.toml`: a Pack file that lists
    /// only what it changes in a real Quad.
    TestQuad,
    /// A Map's file, `maps/<id>/map.toml`.
    Map,
    /// An Input Device profile, `input-devices/<id>.toml`.
    InputDevice,
}

impl FileKind {
    /// Which files share its format number.
    pub fn family(self) -> Family {
        match self {
            FileKind::Scenario => Family::Scenarios,
            FileKind::Manifest
            | FileKind::Quad
            | FileKind::TestQuad
            | FileKind::Map
            | FileKind::InputDevice => Family::Packs,
        }
    }

    /// The kind in words, such as "a Quad definition".
    pub fn words(self) -> &'static str {
        match self {
            FileKind::Scenario => "a Scenario",
            FileKind::Manifest => "a Pack's manifest",
            FileKind::Quad => "a Quad definition",
            FileKind::TestQuad => "a Test Quad",
            FileKind::Map => "a Map's file",
            FileKind::InputDevice => "an Input Device profile",
        }
    }
}

/// The files that share one format number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// Every Scenario.
    Scenarios,
    /// Every Pack file: manifests, Quad definitions, Maps' files, Input
    /// Device profiles, and the Test Quads and built-in Test Maps.
    Packs,
}

impl Family {
    /// The family in words.
    pub fn words(self) -> &'static str {
        match self {
            Family::Scenarios => "every Scenario",
            Family::Packs => "every Pack file, Test Quad and Test Map",
        }
    }
}

/// One step from format `from` to format `from + 1`, for one family of files.
#[derive(Clone, Copy, Debug)]
pub struct Step {
    /// The name `cargo xtask migrate` takes, such as `add-wind-assist`:
    /// lowercase words joined by dashes.
    pub name: &'static str,
    /// The files it rewrites.
    pub family: Family,
    /// The format it upgrades; files come out one format newer.
    pub from: i64,
    /// What it adds or changes, and why that keeps today's behaviour, in a
    /// plain sentence.
    pub says: &'static str,
    /// The change itself, on one file of the family. It never touches the
    /// `format` line: [`apply`] bumps that. A kind of file the step doesn't
    /// change is left as it is, apart from its `format` line.
    pub rewrite: fn(FileKind, &mut DocumentMut) -> Result<(), String>,
}

/// One step applied to one file's text: the new text, with every comment and
/// the layout kept, and the `format` line one newer. It refuses a file of
/// another family, or one that isn't written in the format the step upgrades.
pub fn apply(step: &Step, kind: FileKind, text: &str) -> Result<String, String> {
    if kind.family() != step.family {
        return Err(format!(
            "`{}` rewrites {}, and this is {}",
            step.name,
            step.family.words(),
            kind.words()
        ));
    }
    let mut doc: DocumentMut = text.parse().map_err(|error: toml_edit::TomlError| {
        format!(
            "this isn't readable TOML: {}",
            error.message().trim().trim_end_matches('.')
        )
    })?;
    let format = doc.get("format").and_then(Item::as_value);
    match format.and_then(Value::as_integer) {
        Some(n) if n == step.from => {}
        Some(n) => {
            return Err(format!(
                "this file is format {n}, and `{}` upgrades format {} to {}",
                step.name,
                step.from,
                step.from + 1
            ));
        }
        None => return Err("it has no `format = N` line".to_string()),
    }
    (step.rewrite)(kind, &mut doc)?;
    if let Some(format) = doc.get_mut("format").and_then(Item::as_value_mut) {
        let decor = format.decor().clone();
        *format = Value::from(step.from + 1);
        *format.decor_mut() = decor;
    }
    let rewritten = doc.to_string();
    // toml_edit ends the lines it writes with "\n". A file written with
    // Windows line endings keeps them.
    Ok(if text.contains("\r\n") {
        rewritten.replace("\r\n", "\n").replace('\n', "\r\n")
    } else {
        rewritten
    })
}

/// Brings `text`, a `kind` file in format `from`, up to format `to`, one step
/// at a time, or says which step is missing or what went wrong.
pub fn upgrade(
    text: &str,
    kind: FileKind,
    from: i64,
    to: i64,
    steps: &[Step],
) -> Result<String, String> {
    let mut text = text.to_string();
    for format in from..to {
        let step = steps
            .iter()
            .find(|step| step.from == format && step.family == kind.family())
            .ok_or_else(|| {
                format!(
                    "this file is format {format}, and this OpenDrone has no step that upgrades it"
                )
            })?;
        text = apply(step, kind, &text).map_err(|error| {
            format!(
                "upgrading this file from format {format} to {}: {error}",
                format + 1
            )
        })?;
    }
    Ok(text)
}

/// The text the Pack reader reads: `text` itself when its `format` is the
/// newest or can't be read (then [`Document::check_format`] says why), or
/// `text` upgraded in memory by [`PACK_STEPS`] when it is older.
pub fn upgraded<'t>(file: &str, text: &'t str, kind: FileKind) -> Result<Cow<'t, str>, Problems> {
    upgraded_with(file, text, kind, FORMAT, PACK_STEPS)
}

/// [`upgraded`] with other steps, which make `newest` the newest format.
pub fn upgraded_with<'t>(
    file: &str,
    text: &'t str,
    kind: FileKind,
    newest: i64,
    steps: &[Step],
) -> Result<Cow<'t, str>, Problems> {
    let doc = Document::parse(file, text)?;
    let root = doc.root();
    let Some(item) = root.get("format") else {
        return Ok(text.into());
    };
    match item.integer() {
        Some(n) if n < newest && (n >= 1 || steps.iter().any(|step| step.from == n)) => {
            upgrade(text, kind, n, newest, steps)
                .map(Into::into)
                .map_err(|sentence| Problems(vec![item.problem(sentence)]))
        }
        _ => Ok(text.into()),
    }
}

// Writing steps

/// Adds `key = value` to the table `table` (such as `"motors"` or
/// `"start.rates"`, or `""` for the top of the file) on the line after the
/// key `after`, lined up with it: the same indent, and the `=` in the same
/// column when the new key fits. A comment above the next key stays with that
/// key.
pub fn add_after(
    doc: &mut DocumentMut,
    table: &str,
    after: &str,
    key: &str,
    value: impl Into<Value>,
) -> Result<(), String> {
    let name = table_name(table);
    let target = table_mut(doc, table)?;
    if target.contains_key(key) {
        return Err(format!("{name} already has `{key}`"));
    }
    let Some((anchor, anchor_item)) = target.get_key_value(after) else {
        return Err(format!("{name} has no `{after}` to add `{key}` after"));
    };
    let indent = raw(anchor.leaf_decor().prefix(), "")
        .rsplit('\n')
        .next()
        .unwrap_or_default()
        .to_string();
    let column = anchor.display_repr().chars().count()
        + raw(anchor.leaf_decor().suffix(), " ").chars().count();
    let padding = column.saturating_sub(key.chars().count()).max(1);
    let value_prefix = anchor_item
        .as_value()
        .map_or(" ".to_string(), |value| raw(value.decor().prefix(), " "));
    let mut value = value.into();
    *value.decor_mut() = Decor::new(value_prefix, "");
    let new_key = Key::new(key).with_leaf_decor(Decor::new(indent, " ".repeat(padding)));
    target.insert_formatted(&new_key, Item::Value(value));

    let order: Vec<String> = target.iter().map(|(k, _)| k.to_string()).collect();
    let place = |k: &str| {
        let lookup = if k == key { after } else { k };
        let index = order
            .iter()
            .position(|o| o == lookup)
            .unwrap_or(order.len());
        2 * index + usize::from(k == key)
    };
    target.sort_values_by(|a, _, b, _| place(a.get()).cmp(&place(b.get())));
    Ok(())
}

/// Adds `key = value` at the end of the inline table `inline` in the table
/// `table`, such as `assists = { … }` in `[start]`, spaced like the item
/// before it. A table written over several lines puts it on a line of its
/// own and keeps its trailing comma; a comment on the last item's line stays
/// on that line.
pub fn add_to_inline_table(
    doc: &mut DocumentMut,
    table: &str,
    inline: &str,
    key: &str,
    value: impl Into<Value>,
) -> Result<(), String> {
    let name = table_name(table);
    let target = table_mut(doc, table)?
        .get_mut(inline)
        .and_then(Item::as_inline_table_mut)
        .ok_or_else(|| format!("{name} has no `{inline} = {{ … }}`"))?;
    if target.contains_key(key) {
        return Err(format!("`{inline}` in {name} already has `{key}`"));
    }
    let mut value = value.into();
    let Some(last) = target.iter().last().map(|(key, _)| key.to_string()) else {
        *value.decor_mut() = Decor::new(" ", " ");
        target.insert_formatted(&Key::new(key).with_leaf_decor(Decor::new(" ", " ")), value);
        return Ok(());
    };
    let trailing_comma = target.trailing_comma();
    let trailing = raw(Some(target.trailing()), "");
    let (last_key, last_item) = target
        .get_key_value_mut(&last)
        .ok_or_else(|| format!("`{inline}` in {name} can't be read"))?;
    let last_value = last_item
        .as_value_mut()
        .ok_or_else(|| format!("`{inline}` in {name} can't be read"))?;
    // The new item starts as the last one does: on a new line with its
    // indent (leaving out any comment above the last item), or after a space.
    let last_prefix = raw(last_key.leaf_decor().prefix(), " ");
    let start = match last_prefix.rfind('\n') {
        Some(i) => last_prefix[i..].to_string(),
        None => last_prefix,
    };
    let key_suffix = raw(last_key.leaf_decor().suffix(), " ");
    let value_prefix = raw(last_value.decor().prefix(), " ");
    // What follows the last item, up to the closing brace: after its comma
    // when it has one. Whatever is on the last item's own line, such as a
    // comment, stays there, after the comma that now follows it; the rest
    // moves after the new item.
    let after = if trailing_comma {
        trailing
    } else {
        raw(last_value.decor().suffix(), " ")
    };
    let (same_line, rest) = match after.find('\n') {
        Some(i) => after.split_at(i),
        None => ("", after.as_str()),
    };
    let new_key =
        Key::new(key).with_leaf_decor(Decor::new(format!("{same_line}{start}"), key_suffix));
    if trailing_comma {
        *value.decor_mut() = Decor::new(value_prefix, "");
        target.set_trailing(rest);
    } else {
        last_value.decor_mut().set_suffix("");
        *value.decor_mut() = Decor::new(value_prefix, rest);
    }
    target.insert_formatted(&new_key, value);
    Ok(())
}

fn table_mut<'d>(doc: &'d mut DocumentMut, path: &str) -> Result<&'d mut Table, String> {
    let mut table = doc.as_table_mut();
    if path.is_empty() {
        return Ok(table);
    }
    for part in path.split('.') {
        table = table
            .get_mut(part)
            .and_then(Item::as_table_mut)
            .ok_or_else(|| format!("there's no {}", table_name(path)))?;
    }
    Ok(table)
}

fn table_name(path: &str) -> String {
    if path.is_empty() {
        "the top of the file".to_string()
    } else {
        format!("[{path}]")
    }
}

/// A piece of a file's spacing as written, or toml_edit's `default` when
/// the item was made by a step and has none of its own.
fn raw(text: Option<&toml_edit::RawString>, default: &str) -> String {
    text.map_or(default, |text| text.as_str().unwrap_or(default))
        .to_string()
}

// The repo's own files

/// One file a step may rewrite.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MigrationFile {
    pub path: PathBuf,
    /// How reports name it, such as `packs/opendrone/pack.toml`.
    pub label: String,
    pub kind: FileKind,
}

/// What a step does to a set of files, before anything is written.
#[derive(Clone, Debug)]
pub struct Migration {
    pub step: Step,
    /// Every file the step rewrites, with its new text.
    pub rewritten: Vec<(MigrationFile, String)>,
    /// Files already in the format the step writes, left as they are.
    pub already: Vec<MigrationFile>,
}

impl Migration {
    /// Writes every rewritten file, all or nothing as far as the computer
    /// allows: each new text goes first to a hidden file beside its file,
    /// and only when every one is written do they replace the files. So a
    /// file that can't be written leaves every file as it was. Only a
    /// failure while replacing them, which is rare, can leave some files
    /// rewritten and others not; the problems then name each one.
    pub fn write(&self) -> Result<(), Problems> {
        let mut problems = Problems::new();
        let mut ready = Vec::new();
        for (file, text) in &self.rewritten {
            let hidden = hidden_beside(&file.path);
            match fs::write(&hidden, text) {
                Ok(()) => ready.push((file, hidden)),
                Err(error) => problems.push(Problem::of(
                    &file.label,
                    0,
                    format!("can't be written ({error}), so no file was changed"),
                )),
            }
        }
        if !problems.is_empty() {
            for (_, hidden) in &ready {
                let _ = fs::remove_file(hidden);
            }
            return Err(problems);
        }
        let mut replaced = Vec::new();
        for (file, hidden) in &ready {
            match fs::rename(hidden, &file.path) {
                Ok(()) => replaced.push(*file),
                Err(error) => {
                    let _ = fs::remove_file(hidden);
                    problems.push(Problem::of(
                        &file.label,
                        0,
                        format!("can't be replaced, so it was left as it was: {error}"),
                    ));
                }
            }
        }
        if !problems.is_empty() {
            for file in replaced {
                problems.push(Problem::of(
                    &file.label,
                    0,
                    "was rewritten before another file failed",
                ));
            }
        }
        problems.or(())
    }
}

/// The hidden file a new text is written to before it replaces `path`:
/// `.<name>.migrating`, beside it.
fn hidden_beside(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    path.with_file_name(format!(".{name}.migrating"))
}

/// Works out what `step` does to each of `files`, writing nothing: a file in
/// the format the step upgrades is rewritten, and one already in the format
/// it writes is left alone. Any other file, such as one that needs an
/// earlier step first, is a problem, and then nothing should be written.
pub fn migrate(step: &Step, files: &[MigrationFile]) -> Result<Migration, Problems> {
    let mut migration = Migration {
        step: *step,
        rewritten: Vec::new(),
        already: Vec::new(),
    };
    let mut problems = Problems::new();
    for file in files {
        let text = match fs::read_to_string(&file.path) {
            Ok(text) => text,
            Err(error) => {
                problems.push(Problem::of(
                    &file.label,
                    0,
                    format!("can't be read: {error}"),
                ));
                continue;
            }
        };
        let doc = match Document::parse(&file.label, &text) {
            Ok(doc) => doc,
            Err(found) => {
                problems.extend(found);
                continue;
            }
        };
        let root = doc.root();
        let Some(item) = root.get("format") else {
            problems.push(Problem::of(
                &file.label,
                1,
                "every file starts with `format = N`, and this one has none",
            ));
            continue;
        };
        let to = step.from + 1;
        match item.integer() {
            Some(n) if n == step.from => match apply(step, file.kind, &text) {
                Ok(new) => migration.rewritten.push((file.clone(), new)),
                Err(sentence) => problems.push(Problem::of(&file.label, 0, sentence)),
            },
            Some(n) if n == to => migration.already.push(file.clone()),
            Some(n) if n < step.from => problems.push(item.problem(format!(
                "this file is format {n}: it needs the steps before `{}` first, which upgrades format {} to {to}",
                step.name, step.from
            ))),
            Some(n) => problems.push(item.problem(format!(
                "this file is format {n}, newer than the format {to} `{}` writes",
                step.name
            ))),
            None => problems.push(item.problem("`format` must be a whole number")),
        }
    }
    problems.or(migration)
}

/// Every Pack file in the Packs in `packs`, in folder order: each Pack's
/// `pack.toml`, every `quads/<id>/quad.toml`, `maps/<id>/map.toml` and
/// `input-devices/<id>.toml`. Problems name files starting with
/// `packs_label`, such as `packs`.
pub fn pack_files(packs: &Path, packs_label: &str) -> Result<Vec<MigrationFile>, Problems> {
    let mut files = Vec::new();
    let file = |path: PathBuf, label: String, kind| MigrationFile { path, label, kind };
    for pack in names(packs, packs_label)? {
        let folder = packs.join(&pack);
        let label = format!("{packs_label}/{pack}");
        if !folder.is_dir() {
            continue;
        }
        if folder.join("pack.toml").is_file() {
            files.push(file(
                folder.join("pack.toml"),
                format!("{label}/pack.toml"),
                FileKind::Manifest,
            ));
        }
        for (kind_folder, file_name, kind) in [
            ("quads", "quad.toml", FileKind::Quad),
            ("maps", "map.toml", FileKind::Map),
        ] {
            let items = folder.join(kind_folder);
            if !items.is_dir() {
                continue;
            }
            for item in names(&items, &format!("{label}/{kind_folder}"))? {
                let path = items.join(&item).join(file_name);
                if path.is_file() {
                    files.push(file(
                        path,
                        format!("{label}/{kind_folder}/{item}/{file_name}"),
                        kind,
                    ));
                }
            }
        }
        let devices = folder.join("input-devices");
        if devices.is_dir() {
            for name in names(&devices, &format!("{label}/input-devices"))? {
                if name.ends_with(".toml") {
                    files.push(file(
                        devices.join(&name),
                        format!("{label}/input-devices/{name}"),
                        FileKind::InputDevice,
                    ));
                }
            }
        }
    }
    Ok(files)
}

/// Every `<id>.toml` in `folder`, in order, each a `kind` file, such as the
/// Test Quads in `scenarios/test-quads/`. A folder that isn't there has none.
pub fn toml_files(
    folder: &Path,
    label: &str,
    kind: FileKind,
) -> Result<Vec<MigrationFile>, Problems> {
    if !folder.is_dir() {
        return Ok(Vec::new());
    }
    Ok(names(folder, label)?
        .into_iter()
        .filter(|name| name.ends_with(".toml"))
        .map(|name| MigrationFile {
            path: folder.join(&name),
            label: format!("{label}/{name}"),
            kind,
        })
        .collect())
}

/// The names in a folder, in order, leaving out hidden ones.
fn names(folder: &Path, label: &str) -> Result<Vec<String>, Problems> {
    let mut names: Vec<String> = fs::read_dir(folder)
        .map_err(|error| Problems::of_file(label, format!("can't be read: {error}")))?
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| !name.starts_with('.'))
        .collect();
    names.sort();
    Ok(names)
}
