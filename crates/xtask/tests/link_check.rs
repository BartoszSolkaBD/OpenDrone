//! Readable checks for the docs site's link check, `cargo xtask check-links`,
//! which `cargo xtask book` runs on the drawn book (#15 §11: a broken link
//! blocks).
//!
//! Each check writes a small drawn book, a few HTML pages like the ones mdBook
//! and rustdoc write, and checks it.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn a_book_whose_links_all_work_passes_the_link_check() {
    let book = Book::whose_links_all_work("all-links-work");
    let outcome = book.check();
    assert!(outcome.passed, "{}", outcome.output);
    outcome.says("Every link works");
}

#[test]
fn a_link_to_a_missing_page_breaks_the_book() {
    let book = Book::whose_links_all_work("missing-page");
    book.write("context/flying.html", "<a href=\"../units.html\">units</a>");
    let outcome = book.check();
    assert!(!outcome.passed, "{}", outcome.output);
    outcome
        .says("`context/flying.html` links to `../units.html`, but the book has no `units.html`.");
}

#[test]
fn a_link_to_a_missing_heading_breaks_the_book() {
    let book = Book::whose_links_all_work("missing-heading");
    book.write(
        "context/flying.html",
        "<a href=\"../book/map.html#gone\">gone</a>",
    );
    let outcome = book.check();
    assert!(!outcome.passed, "{}", outcome.output);
    outcome.says(
        "`context/flying.html` links to `../book/map.html#gone`, but `book/map.html` has no \
         heading or anchor `gone`.",
    );
}

#[test]
fn a_link_to_a_missing_heading_on_the_same_page_breaks_the_book() {
    let book = Book::whose_links_all_work("missing-heading-same-page");
    book.write(
        "units.html",
        "<h1 id=\"the-unit-list\">The unit list</h1><a href=\"#tolerances\">below</a>",
    );
    let outcome = book.check();
    assert!(!outcome.passed, "{}", outcome.output);
    outcome.says("`units.html` links to `#tolerances`, but `units.html` has no heading or anchor `tolerances`.");
}

#[test]
fn a_link_to_a_crate_without_rustdoc_breaks_the_book() {
    let book = Book::whose_links_all_work("missing-rustdoc");
    book.write(
        "book/the-code.html",
        "<a href=\"../api/opendrone_physics/index.html\">physics</a>",
    );
    let outcome = book.check();
    assert!(!outcome.passed, "{}", outcome.output);
    outcome.says(
        "`book/the-code.html` links to `../api/opendrone_physics/index.html`, but the book has no \
         `api/opendrone_physics/index.html`.",
    );
}

#[test]
fn a_link_from_the_web_servers_root_breaks_the_book() {
    let book = Book::whose_links_all_work("link-from-the-root");
    book.write("units.html", "<a href=\"/units.html\">here</a>");
    let outcome = book.check();
    assert!(!outcome.passed, "{}", outcome.output);
    outcome.says("`units.html` links to `/units.html`, which starts at the web server's root.");
}

/// A small drawn book in a scratch folder.
struct Book {
    root: PathBuf,
}

impl Book {
    /// A book whose links all work: links between pages, to headings, to a
    /// folder's index page, to a picture and to rustdoc, plus the kinds of
    /// links the check leaves alone.
    fn whose_links_all_work(name: &str) -> Book {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("link-check")
            .join(name);
        if root.exists() {
            fs::remove_dir_all(&root).expect("clear the scratch book");
        }
        let book = Book { root };
        book.write(
            "index.html",
            "<link rel=\"stylesheet\" href=\"css/general.css\">\n\
             <h1 id=\"opendrone\">OpenDrone</h1>\n\
             <a href=\"book/map.html\">the map</a> <a href=\"book/map.html#topics\">its topics</a>\n\
             <a href=\"research/\">research</a> <a href=\"#opendrone\">top</a>\n\
             <img src=\"skate-park.png\" alt=\"the Skate Park\">\n\
             <a href=\"https://github.com/BartoszSolkaBD/OpenDrone/issues/37\">the spec</a>\n\
             <a href=\"mailto:someone@example.com\">mail</a>\n\
             <script>let link = '<a href=\"not-a-page.html\">';</script>\n\
             <!-- <a href=\"also-not-a-page.html\"> -->\n",
        );
        book.write("css/general.css", "body {}");
        book.write("skate-park.png", "a picture");
        book.write(
            "book/map.html",
            "<h1 id=\"opendrone\">OpenDrone</h1><h2 id=\"topics\">Topics</h2>",
        );
        book.write(
            "research/index.html",
            "<a href=\"../index.html#opendrone\">back</a>",
        );
        book.write(
            "book/the-code.html",
            "<a href=\"../api/opendrone_maths/index.html#structs\">maths</a>",
        );
        book.write(
            "api/opendrone_maths/index.html",
            "<h2 id=\"structs\">Structs</h2><a href=\"gone.html\">",
        );
        book.write(
            "print.html",
            "<a href=\"gone.html\">the print page is mdBook's copy</a>",
        );
        book
    }

    fn write(&self, file: &str, text: &str) {
        let path = self.root.join(file);
        fs::create_dir_all(path.parent().expect("a folder")).expect("make the folder");
        fs::write(path, text).expect("write the page");
    }

    fn check(&self) -> Outcome {
        let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
            .arg("check-links")
            .arg(&self.root)
            .output()
            .expect("run xtask");
        Outcome {
            passed: output.status.success(),
            output: format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
        }
    }
}

struct Outcome {
    passed: bool,
    output: String,
}

impl Outcome {
    fn says(&self, text: &str) {
        assert!(
            self.output.contains(text),
            "expected {text:?} in:\n{}",
            self.output
        );
    }
}
