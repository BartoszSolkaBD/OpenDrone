//! `cargo xtask book`: builds the docs site and checks every link in it
//! (#15 §11).
//!
//! 1. rustdoc for every crate, internal items included, with its warnings as
//!    errors, so a broken link in the code's own docs stops it too;
//! 2. the book, with mdBook, from `docs/` (`book.toml` at the repo's root,
//!    which also runs `cargo xtask book-preprocessor` on every page);
//! 3. rustdoc copied into the book's `api/` folder;
//! 4. the link check on the whole book.
//!
//! CI runs this on every PR, and the Pages workflow publishes what it builds.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::{env, fs};

pub mod generated;
pub mod link_check;
pub mod markdown;
pub mod paths;
pub mod preprocessor;

/// The book page that lists every crate; `api/index.html` sends readers there.
const CRATE_LIST_PAGE: &str = "book/the-code.html";

pub fn run(args: &[String]) -> ExitCode {
    if !args.is_empty() {
        eprintln!("Usage: cargo xtask book");
        return ExitCode::from(2);
    }
    match build() {
        Ok(()) => ExitCode::SUCCESS,
        Err(problem) => {
            eprintln!("{problem}");
            ExitCode::FAILURE
        }
    }
}

/// `cargo xtask check-links <folder>`: the link check alone, on a book that's
/// already built.
pub fn run_link_check(args: &[String]) -> ExitCode {
    let [folder] = args else {
        eprintln!("Usage: cargo xtask check-links <folder of a built book>");
        return ExitCode::from(2);
    };
    match check_links(Path::new(folder)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(problem) => {
            eprintln!("{problem}");
            ExitCode::FAILURE
        }
    }
}

fn build() -> Result<(), String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let root = root.canonicalize().unwrap_or(root);
    let book_toml = read_toml(&root.join("book.toml"))?;
    check_mdbook_version(&book_toml)?;
    let target = target_folder(&root)?;

    println!("Building rustdoc for every crate, internal items included…");
    let mut flags = env::var("RUSTDOCFLAGS").unwrap_or_default();
    flags.push_str(" -D warnings");
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let rustdoc = Command::new(cargo)
        .args([
            "doc",
            "--workspace",
            "--no-deps",
            "--document-private-items",
            "--locked",
        ])
        .env("RUSTDOCFLAGS", flags.trim())
        .current_dir(&root)
        .status()
        .map_err(|error| format!("Could not run cargo doc: {error}"))?;
    if !rustdoc.success() {
        return Err(
            "rustdoc found problems in the code's docs (above). The book wasn't built.".to_owned(),
        );
    }

    println!("Building the book from docs/ with mdBook…");
    let book = root.join(
        book_toml
            .get("build")
            .and_then(|build| build.get("build-dir"))
            .and_then(toml::Value::as_str)
            .unwrap_or("book"),
    );
    let mdbook = Command::new("mdbook")
        .arg("build")
        .arg(&root)
        .status()
        .map_err(|error| {
            format!(
                "Could not run mdbook: {error}. {}",
                install_mdbook(&book_toml)
            )
        })?;
    if !mdbook.success() {
        return Err("mdBook couldn't build the book (above).".to_owned());
    }

    let api = book.join("api");
    if api.exists() {
        fs::remove_dir_all(&api)
            .map_err(|error| format!("Could not clear {}: {error}", api.display()))?;
    }
    copy_folder(&target.join("doc"), &api)?;
    fs::write(api.join("index.html"), api_index()).map_err(|error| {
        format!(
            "Could not write {}: {error}",
            api.join("index.html").display()
        )
    })?;

    check_links(&book)?;
    println!(
        "The book is built, with rustdoc under api/: open {}",
        book.join("index.html").display()
    );
    Ok(())
}

fn check_links(book: &Path) -> Result<(), String> {
    let checked = link_check::check(book);
    if checked.pages == 0 {
        return Err(format!("There is no book to check in {}.", book.display()));
    }
    if checked.problems.is_empty() {
        println!(
            "Every link works: {} links on {} pages checked (links to other websites aren't followed).",
            checked.links, checked.pages
        );
        Ok(())
    } else {
        let count = checked.problems.len();
        let mut report = format!(
            "The book has {count} broken {}:\n",
            if count == 1 { "link" } else { "links" }
        );
        for problem in checked.problems {
            report.push_str(&format!("- {problem}\n"));
        }
        Err(report)
    }
}

/// `book.toml` names the mdBook version the book is built with. CI installs
/// exactly that one; elsewhere a different one only gets a warning.
fn check_mdbook_version(book_toml: &toml::Table) -> Result<(), String> {
    let Some(wanted) = wanted_mdbook(book_toml) else {
        return Err(
            "book.toml should name its mdBook version as `mdbook-version` under \
                    [preprocessor.opendrone]."
                .to_owned(),
        );
    };
    let output = Command::new("mdbook")
        .arg("--version")
        .output()
        .map_err(|_| format!("mdBook isn't installed. {}", install_mdbook(book_toml)))?;
    let found = String::from_utf8_lossy(&output.stdout);
    let found = found
        .trim()
        .trim_start_matches("mdbook")
        .trim()
        .trim_start_matches('v');
    if found == wanted {
        return Ok(());
    }
    let message = format!(
        "The book is built with mdBook {wanted} (book.toml), but this mdbook is {found}. {}",
        install_mdbook(book_toml)
    );
    if env::var_os("CI").is_some() {
        Err(message)
    } else {
        eprintln!("Warning: {message}");
        Ok(())
    }
}

fn wanted_mdbook(book_toml: &toml::Table) -> Option<&str> {
    book_toml
        .get("preprocessor")?
        .get("opendrone")?
        .get("mdbook-version")?
        .as_str()
}

fn install_mdbook(book_toml: &toml::Table) -> String {
    let version = wanted_mdbook(book_toml).unwrap_or("<version>");
    format!(
        "Install it with `cargo install mdbook --version {version} --locked`, or from its prebuilt downloads."
    )
}

/// Where cargo puts what it builds (normally `target/`).
fn target_folder(root: &Path) -> Result<PathBuf, String> {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(root)
        .output()
        .map_err(|error| format!("Could not run cargo metadata: {error}"))?;
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("cargo metadata gave unreadable output: {error}"))?;
    metadata["target_directory"]
        .as_str()
        .map(PathBuf::from)
        .ok_or_else(|| "cargo metadata didn't say where `target` is.".to_owned())
}

fn read_toml(file: &Path) -> Result<toml::Table, String> {
    fs::read_to_string(file)
        .map_err(|error| format!("Could not read {}: {error}", file.display()))?
        .parse()
        .map_err(|error| format!("{} isn't valid TOML: {error}", file.display()))
}

fn copy_folder(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|error| format!("Could not make {}: {error}", to.display()))?;
    let entries = fs::read_dir(from)
        .map_err(|error| format!("Could not read {}: {error}", from.display()))?;
    for entry in entries.flatten() {
        let (source, destination) = (entry.path(), to.join(entry.file_name()));
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            copy_folder(&source, &destination)?;
        } else {
            fs::copy(&source, &destination)
                .map_err(|error| format!("Could not copy {}: {error}", source.display()))?;
        }
    }
    Ok(())
}

/// rustdoc makes no page for `api/` itself, so this one sends readers to the
/// book's list of crates.
fn api_index() -> String {
    format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <title>OpenDrone's code</title>\n\
         <meta http-equiv=\"refresh\" content=\"0; url=../{CRATE_LIST_PAGE}\">\n</head>\n<body>\n\
         <p>The list of OpenDrone's crates is on <a href=\"../{CRATE_LIST_PAGE}\">the book's page \
         about the code</a>.</p>\n</body>\n</html>\n"
    )
}
