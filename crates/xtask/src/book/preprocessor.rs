//! `cargo xtask book-preprocessor`: the step mdBook runs on every page before
//! it draws the book. `book.toml` names it, so `mdbook build` and
//! `mdbook serve` run it on their own.
//!
//! The docs are written to be read in the repo, on GitHub or by an agent, so
//! their links point at files of the repo. This step makes the same links work
//! in the book, and keeps one copy of every page (#15 §11):
//!
//! - A link to a page of the book lands on that page, wherever the book puts
//!   it. A link to any other file of the repo, such as `deny.toml` or a
//!   Scenario, goes to that file on GitHub.
//! - A page may show a Markdown file from outside `docs/` with
//!   `{{#show <file>}}` on a line of its own, such as the map, `CONTEXT.md`.
//!   Links to that file then land on that page.
//! - `{{#scenario-catalogue <folder>}}` becomes the catalogue of every
//!   Scenario in that folder, `{{#feel-test-logs <folder>}}` each Quad's
//!   Feel Test log from the Packs in that folder, and `{{#crate-list <folder>}}`
//!   the list of crates with links to their rustdoc.
//!
//! Paths after a `{{#…}}` are relative to the page, as in mdBook's own
//! `{{#include}}`.
//!
//! It stops the book, with a plain sentence for each, when a link points to a
//! file that doesn't exist or to a Markdown page under `docs/` the book leaves
//! out, or when a page under `docs/` isn't in the book's table of contents.
//! Headings and rustdoc pages can only be checked once the book is drawn, so
//! `cargo xtask book` checks those links afterwards.

use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::{Map, Value};

use super::{generated, markdown, paths};
use crate::scenario_catalogue;

/// The branch whose files links outside the book point to on GitHub.
const BRANCH: &str = "main";

pub fn run(args: &[String]) -> ExitCode {
    match args {
        // mdBook asks first whether this step works with a renderer. It only
        // changes Markdown, so it works with every one.
        [supports, _renderer] if supports == "supports" => return ExitCode::SUCCESS,
        [] => {}
        _ => {
            eprintln!(
                "Usage: cargo xtask book-preprocessor [supports <renderer>]\n\
                 mdBook runs this itself, with the book on standard input (see book.toml)."
            );
            return ExitCode::from(2);
        }
    }

    let mut input = String::new();
    if let Err(error) = std::io::stdin().read_to_string(&mut input) {
        eprintln!("Could not read the book from mdBook: {error}");
        return ExitCode::from(2);
    }
    let input: Value = match serde_json::from_str(&input) {
        Ok(input) => input,
        Err(error) => {
            eprintln!("mdBook sent something that isn't a book: {error}");
            return ExitCode::from(2);
        }
    };
    match preprocess(input) {
        Ok(book) => {
            let mut stdout = std::io::stdout().lock();
            if serde_json::to_writer(&mut stdout, &book).is_err() || stdout.flush().is_err() {
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
        Err(problems) => {
            eprintln!("The book can't be built:");
            for problem in problems {
                eprintln!("- {problem}");
            }
            ExitCode::FAILURE
        }
    }
}

/// Takes mdBook's `[context, book]` and gives back the changed book.
fn preprocess(input: Value) -> Result<Value, Vec<String>> {
    let Value::Array(mut input) = input else {
        return Err(vec![
            "mdBook sent something other than [context, book].".to_owned(),
        ]);
    };
    let (Some(mut book), Some(context)) = (input.pop(), input.pop()) else {
        return Err(vec![
            "mdBook sent something other than [context, book].".to_owned(),
        ]);
    };
    let settings = Settings::read(&context).map_err(|problem| vec![problem])?;

    let mut pages = Vec::new();
    for_each_chapter(&mut book["items"], &mut |chapter| {
        if let Some(page) = Page::read(chapter, &settings) {
            pages.push(page);
        }
    });

    let mut problems = Vec::new();
    let book_paths = book_paths(&pages, &mut problems);
    let links = Links {
        settings: &settings,
        book_paths: &book_paths,
    };

    let mut next = 0;
    for_each_chapter(&mut book["items"], &mut |chapter| {
        if Page::source_of(chapter).is_none() {
            return;
        }
        let page = &pages[next];
        next += 1;
        let content = chapter.get("content").and_then(Value::as_str).unwrap_or("");
        let expanded = expand(content, page, &settings, &mut problems);
        let changed = markdown::change_links(&expanded, |destination, is_image| {
            match links.change(page, destination, is_image) {
                Ok(link) => link,
                Err(problem) => {
                    problems.push(problem);
                    None
                }
            }
        });
        chapter.insert("content".to_owned(), Value::String(changed));
    });

    let sources: Vec<&str> = pages.iter().map(|page| page.source.as_str()).collect();
    for file in markdown_files(&settings.root, &settings.source) {
        if !sources.contains(&file.as_str()) && file != format!("{}/SUMMARY.md", settings.source) {
            problems.push(format!(
                "`{file}` isn't in the book. Add it to `{}/SUMMARY.md`, in the part it belongs \
                 to, so the book shows every page.",
                settings.source
            ));
        }
    }

    if problems.is_empty() {
        Ok(book)
    } else {
        Err(problems)
    }
}

/// What the book's settings say, from `book.toml`.
struct Settings {
    /// The repo's root: the folder with `book.toml`.
    root: PathBuf,
    /// The folder the book's pages come from, `docs`.
    source: String,
    /// The repo on GitHub, where links to files outside the book go.
    repository: String,
}

impl Settings {
    fn read(context: &Value) -> Result<Settings, String> {
        let root = context["root"]
            .as_str()
            .ok_or("mdBook didn't say where the book is.")?;
        let source = context["config"]["book"]["src"]
            .as_str()
            .unwrap_or("src")
            .replace('\\', "/")
            .trim_matches('/')
            .to_owned();
        let repository = context["config"]["output"]["html"]["git-repository-url"]
            .as_str()
            .ok_or(
                "`book.toml` needs `git-repository-url` under `[output.html]`: links to files \
                 outside the book go to that repo on GitHub.",
            )?
            .trim_end_matches('/')
            .to_owned();
        Ok(Settings {
            root: PathBuf::from(root),
            source,
            repository,
        })
    }

    /// Where a file of the repo is on GitHub.
    fn on_github(&self, file: &str, is_folder: bool, is_image: bool) -> String {
        if file.is_empty() {
            return self.repository.clone();
        }
        let view = if is_folder {
            "tree"
        } else if is_image {
            "raw"
        } else {
            "blob"
        };
        format!("{}/{view}/{BRANCH}/{file}", self.repository)
    }
}

/// One page of the book.
struct Page {
    /// Its file, from the repo's root: `docs/context/flying.md`.
    source: String,
    /// Where the book puts it, from the book's root: `context/flying.md`, or
    /// `research/index.md` for `docs/research/README.md`.
    book_path: String,
    /// The file from outside `docs/` it shows with `{{#show …}}`, from the
    /// repo's root.
    shows: Option<String>,
}

impl Page {
    fn read(chapter: &Map<String, Value>, settings: &Settings) -> Option<Page> {
        let source = format!("{}/{}", settings.source, Page::source_of(chapter)?);
        let book_path = chapter
            .get("path")
            .and_then(Value::as_str)
            .map_or_else(|| source.clone(), |path| path.replace('\\', "/"));
        let content = chapter.get("content").and_then(Value::as_str).unwrap_or("");
        let shows = directives(content)
            .find(|(name, _)| *name == "show")
            .map(|(_, file)| {
                paths::join(paths::folder(&source), file).unwrap_or_else(|| format!("../{file}"))
            });
        Some(Page {
            source,
            book_path,
            shows,
        })
    }

    /// The page's file, from the book's source folder. Draft chapters, which
    /// have no file, have none.
    fn source_of(chapter: &Map<String, Value>) -> Option<String> {
        chapter
            .get("source_path")
            .and_then(Value::as_str)
            .map(|path| path.replace('\\', "/"))
    }

    /// How problems name the page.
    fn named(&self) -> String {
        match &self.shows {
            Some(file) => format!("`{}` (showing `{file}`)", self.source),
            None => format!("`{}`", self.source),
        }
    }
}

/// Where each file the book shows ends up in the book: every page's own file,
/// and every file a page shows.
fn book_paths(pages: &[Page], problems: &mut Vec<String>) -> BTreeMap<String, String> {
    let mut book_paths = BTreeMap::new();
    for page in pages {
        book_paths.insert(page.source.clone(), page.book_path.clone());
    }
    let mut shown_by: BTreeMap<&str, &str> = BTreeMap::new();
    for page in pages {
        let Some(file) = &page.shows else {
            continue;
        };
        if let Some(other) = shown_by.insert(file, &page.source) {
            problems.push(format!(
                "`{file}` is shown by both `{other}` and `{}`. The book shows each file once.",
                page.source
            ));
        } else if book_paths.contains_key(file) {
            problems.push(format!(
                "`{}` shows `{file}`, which is already a page of the book. Show only files from \
                 outside the book's folder.",
                page.source
            ));
        } else {
            book_paths.insert(file.clone(), page.book_path.clone());
        }
    }
    book_paths
}

/// Replaces each `{{#…}}` line this step knows with what it stands for.
fn expand(content: &str, page: &Page, settings: &Settings, problems: &mut Vec<String>) -> String {
    let mut expanded = String::with_capacity(content.len());
    for line in content.split_inclusive('\n') {
        let Some((name, argument)) = directive(line) else {
            expanded.push_str(line);
            continue;
        };
        let target = paths::join(paths::folder(&page.source), argument);
        let replacement = match (name, &target) {
            (_, None) => Err(format!(
                "`{}` names `{argument}` in `{{{{#{name}}}}}`, which is outside the repo.",
                page.source
            )),
            ("show", Some(file)) => match fs::read_to_string(settings.root.join(file)) {
                Ok(text) => Ok(generated::rooted_links(&text, file)),
                Err(_) => Err(format!(
                    "`{}` shows `{argument}`, but there is no `{file}`.",
                    page.source
                )),
            },
            ("scenario-catalogue", Some(folder)) => {
                scenario_catalogue::page(&settings.root, folder).map_err(|found| found.join("\n- "))
            }
            ("feel-test-logs", Some(folder)) => {
                Ok(generated::feel_test_logs(&settings.root, folder))
            }
            ("crate-list", Some(folder)) => {
                generated::crate_list(&settings.root, folder, &settings.source)
            }
            _ => {
                expanded.push_str(line);
                continue;
            }
        };
        match replacement {
            Ok(text) => {
                expanded.push_str(&text);
                if !text.ends_with('\n') {
                    expanded.push('\n');
                }
            }
            Err(problem) => problems.push(problem),
        }
    }
    expanded
}

/// The `{{#name argument}}` lines in a page.
fn directives(content: &str) -> impl Iterator<Item = (&str, &str)> {
    content.lines().filter_map(directive)
}

/// `("show", "../../CONTEXT.md")` from a line `{{#show ../../CONTEXT.md}}`.
fn directive(line: &str) -> Option<(&str, &str)> {
    let inside = line.trim().strip_prefix("{{#")?.strip_suffix("}}")?;
    let (name, argument) = inside.split_once(char::is_whitespace)?;
    Some((name, argument.trim()))
}

/// Turns the links of a page into links that work in the book.
struct Links<'a> {
    settings: &'a Settings,
    book_paths: &'a BTreeMap<String, String>,
}

impl Links<'_> {
    /// The link to write instead of `destination`, `None` to keep it, or the
    /// problem with it.
    fn change(
        &self,
        page: &Page,
        destination: &str,
        is_image: bool,
    ) -> Result<Option<String>, String> {
        if paths::is_outside_link(destination) {
            return Ok(None);
        }
        let (path, suffix) = paths::split_suffix(destination);
        if path.is_empty() {
            return Ok(None);
        }
        let path = paths::percent_decoded(path);
        let file = match path.strip_prefix('/') {
            Some(from_root) => paths::join("", from_root),
            None => paths::join(paths::folder(&page.source), &path),
        }
        .ok_or_else(|| {
            format!(
                "{} links to `{destination}`, which is outside the repo.",
                page.named()
            )
        })?;

        let source = &self.settings.source;
        if let Some(book_path) = self.book_paths.get(&file) {
            return Ok(Some(format!(
                "{}{suffix}",
                paths::relative(&page.book_path, book_path)
            )));
        }
        // The rustdoc pages, which `cargo xtask book` puts in the book's
        // `api/` folder after mdBook has drawn it, and checks then.
        if let Some(in_book) = file.strip_prefix(&format!("{source}/"))
            && (in_book == "api" || in_book.starts_with("api/"))
        {
            return Ok(Some(format!(
                "{}{suffix}",
                paths::relative(&page.book_path, in_book)
            )));
        }

        let Ok(found) = fs::metadata(self.settings.root.join(&file)) else {
            return Err(format!(
                "{} links to `{destination}`, but there is no `{file}`.",
                page.named()
            ));
        };
        if found.is_dir()
            && let Some(book_path) = self.book_paths.get(&format!("{file}/README.md"))
        {
            return Ok(Some(format!(
                "{}{suffix}",
                paths::relative(&page.book_path, book_path)
            )));
        }
        // The table of contents isn't a page of the book; it's on GitHub.
        let is_summary = file == format!("{source}/SUMMARY.md");
        match file.strip_prefix(&format!("{source}/")) {
            Some(_) if file.ends_with(".md") && !is_summary => Err(format!(
                "{} links to `{file}`, which isn't in the book. Add it to `{source}/SUMMARY.md`.",
                page.named()
            )),
            // Other files in the book's folder, such as the `diff all`
            // exports, are copied into the book as they are.
            Some(in_book) if !found.is_dir() && !is_summary => Ok(Some(format!(
                "{}{suffix}",
                paths::relative(&page.book_path, in_book)
            ))),
            _ => Ok(Some(format!(
                "{}{suffix}",
                self.settings.on_github(&file, found.is_dir(), is_image)
            ))),
        }
    }
}

/// Calls `visit` on every chapter of the book, in order: each chapter before
/// the chapters inside it.
fn for_each_chapter(items: &mut Value, visit: &mut dyn FnMut(&mut Map<String, Value>)) {
    let Some(items) = items.as_array_mut() else {
        return;
    };
    for item in items {
        if let Some(Value::Object(chapter)) = item.get_mut("Chapter") {
            visit(chapter);
            if let Some(sub_items) = chapter.get_mut("sub_items") {
                for_each_chapter(sub_items, visit);
            }
        }
    }
}

/// Every Markdown file under `folder`, from the repo's root, sorted.
fn markdown_files(root: &Path, folder: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut folders = vec![folder.to_owned()];
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(root.join(&folder))
            .into_iter()
            .flatten()
            .flatten()
        {
            let path = format!("{folder}/{}", entry.file_name().to_string_lossy());
            if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                folders.push(path);
            } else if path.ends_with(".md") {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}
