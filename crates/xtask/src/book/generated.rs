//! The parts of book pages made from the repo when the book is built: each
//! Quad's Feel Test log, from its Pack folder, and the list of crates with
//! their rustdoc. Links in what these return start with `/`, meaning the
//! repo's root, and are turned into book links afterwards.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use super::markdown;
use super::paths;

/// Every Quad's Feel Test log (`<packs>/<pack>/quads/<quad>/feel-tests.md`),
/// each under a heading with the Quad's name and id, in Pack and Quad order.
pub fn feel_test_logs(root: &Path, packs: &str) -> String {
    let mut logs = Vec::new();
    for pack in folders(&root.join(packs)) {
        let pack_folder = format!("{packs}/{pack}");
        let pack_id = string_in(root, &format!("{pack_folder}/pack.toml"), "id")
            .unwrap_or_else(|| pack.clone());
        for quad in folders(&root.join(&pack_folder).join("quads")) {
            let quad_folder = format!("{pack_folder}/quads/{quad}");
            let log = format!("{quad_folder}/feel-tests.md");
            if let Ok(text) = fs::read_to_string(root.join(&log)) {
                let name = string_in(root, &format!("{quad_folder}/quad.toml"), "name")
                    .unwrap_or_else(|| quad.clone());
                logs.push((format!("{pack_id}/{quad}"), name, log, text));
            }
        }
    }

    if logs.is_empty() {
        return format!(
            "No Quad has a Feel Test log yet. Each Quad's log is the `feel-tests.md` file in its \
             folder in its Pack (`{packs}/<pack>/quads/<quad>/feel-tests.md`), and it shows up here \
             once it exists.\n"
        );
    }
    let mut page = String::new();
    for (id, name, log, text) in logs {
        let _ = write!(
            page,
            "\n## {name}\n\nQuad `{id}`, from [`{log}`](/{log}).\n\n{}\n",
            rooted_links(&markdown::demote_headings(&text, 2), &log)
        );
    }
    page
}

/// A table of every crate under `crates/`, each linked to its rustdoc in the
/// book's `api/` folder, in the order ADR-0003 describes them: the core first,
/// then the edge crates, the game and the developer tools.
pub fn crate_list(root: &Path, crates: &str, book_source: &str) -> Result<String, String> {
    let core: Vec<String> = toml_file(root, "crates/xtask/walls.toml")
        .and_then(|walls| {
            walls.get("core")?.as_array().map(|names| {
                names
                    .iter()
                    .filter_map(|n| n.as_str().map(str::to_owned))
                    .collect()
            })
        })
        .unwrap_or_default();

    let mut rows = Vec::new();
    for folder in folders(&root.join(crates)) {
        let manifest = format!("{crates}/{folder}/Cargo.toml");
        let Some(package) = toml_file(root, &manifest).and_then(|t| t.get("package").cloned())
        else {
            continue;
        };
        let Some(name) = package.get("name").and_then(toml::Value::as_str) else {
            return Err(format!("`{manifest}` has no package name"));
        };
        let description = package
            .get("description")
            .and_then(toml::Value::as_str)
            .unwrap_or("");
        let (order, kind) = match core.iter().position(|c| c == name) {
            Some(at) => (at, "Core"),
            None if name == "opendrone" => (200, "The game"),
            None if name == "xtask" => (300, "Developer tools"),
            None => (100, "Edge"),
        };
        rows.push((order, name.to_owned(), kind, description.to_owned()));
    }
    if rows.is_empty() {
        return Err(format!("there are no crates in `{crates}/`"));
    }
    rows.sort();

    let mut list = String::from("| Crate | Kind | What it is |\n|---|---|---|\n");
    for (_, name, kind, description) in rows {
        let folder = name.replace('-', "_");
        let _ = writeln!(
            list,
            "| [`{name}`](/{book_source}/api/{folder}/index.html) | {kind} | {} |",
            description.replace('|', "\\|")
        );
    }
    Ok(list)
}

/// Turns the relative links in a page that lives at `file` into links from the
/// repo's root, so the page can be shown inside another one.
pub fn rooted_links(markdown: &str, file: &str) -> String {
    markdown::change_links(markdown, |destination, _| {
        if paths::is_outside_link(destination) || destination.starts_with('/') {
            return None;
        }
        let (path, suffix) = paths::split_suffix(destination);
        if path.is_empty() {
            return None;
        }
        match paths::join(paths::folder(file), path) {
            Some(joined) => Some(format!("/{joined}{suffix}")),
            // It leaves the repo; written this way, the book reports that.
            None => Some(format!("/../{path}{suffix}")),
        }
    })
}

/// The names of the folders inside `folder`, sorted.
fn folders(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn toml_file(root: &Path, file: &str) -> Option<toml::Table> {
    fs::read_to_string(root.join(file)).ok()?.parse().ok()
}

/// A top-level text line of a TOML file, if the file and the line are there.
/// A broken file just isn't used here: the Pack checker reports it.
fn string_in(root: &Path, file: &str, key: &str) -> Option<String> {
    toml_file(root, file)?.get(key)?.as_str().map(str::to_owned)
}
