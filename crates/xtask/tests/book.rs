//! Readable checks for `cargo xtask book-preprocessor`, the step mdBook runs on
//! every page of the docs site (#15 §11): the docs' links work in the book,
//! files from outside `docs/` are shown once, and the pages made from the repo
//! (the Scenario catalogue, the Feel Test logs and the list of crates) are
//! filled in.
//!
//! Each check makes a scratch repo, hands the preprocessor a book made of
//! some of its pages the way mdBook does, and reads the pages it gives back,
//! or the problems it reports.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use std::io::Write as _;

#[test]
fn a_link_from_a_deep_dive_to_the_map_lands_on_the_page_that_shows_it() {
    let repo = Repo::new("link-to-the-map");
    repo.write(
        "CONTEXT.md",
        "# OpenDrone\n\n## Topics\n\nFlying is in [flying.md](docs/context/flying.md).\n",
    );
    repo.write(
        "docs/book/map.md",
        "<!-- The book shows CONTEXT.md here. -->\n{{#show ../../CONTEXT.md}}\n",
    );
    repo.write(
        "docs/context/flying.md",
        "# Flying\n\nBack to the [map](../../CONTEXT.md), or its [topics](../../CONTEXT.md#topics).\n",
    );
    let book = repo
        .preprocess(&["book/map.md", "context/flying.md"])
        .expect("the book");

    book.page("context/flying.md")
        .says("Back to the [map](../book/map.md), or its [topics](../book/map.md#topics).");
    book.page("book/map.md").says("# OpenDrone");
    book.page("book/map.md")
        .says("Flying is in [flying.md](../context/flying.md).");
}

#[test]
fn a_link_to_a_file_outside_the_book_goes_to_the_file_on_github() {
    let repo = Repo::new("link-outside-the-book");
    repo.write("deny.toml", "# the licence policy\n");
    repo.write(
        "crates/maths/Cargo.toml",
        "[package]\nname = \"opendrone-maths\"\n",
    );
    repo.write("assets-src/skate-park.png", "a picture");
    repo.write(
        "docs/data-policy.md",
        "The policy is [`deny.toml`](../deny.toml), the code is in [crates](../crates/), \
         and here's a picture: ![the Skate Park](../assets-src/skate-park.png)\n",
    );
    let book = repo.preprocess(&["data-policy.md"]).expect("the book");

    let page = book.page("data-policy.md");
    page.says("[`deny.toml`](https://github.com/example/opendrone/blob/main/deny.toml)");
    page.says("[crates](https://github.com/example/opendrone/tree/main/crates)");
    page.says("![the Skate Park](https://github.com/example/opendrone/raw/main/assets-src/skate-park.png)");
}

#[test]
fn a_link_to_a_folders_readme_lands_on_its_index_page() {
    let repo = Repo::new("link-to-a-readme");
    repo.write("docs/research/README.md", "# Research\n");
    repo.write("docs/research/quad-settings/README.md", "# Quad settings\n");
    repo.write(
        "docs/pilot-guide.md",
        "See [the research](research/README.md) and [the settings](research/quad-settings/).\n",
    );
    let book = repo
        .preprocess(&[
            "pilot-guide.md",
            "research/README.md",
            "research/quad-settings/README.md",
        ])
        .expect("the book");

    book.page("pilot-guide.md").says(
        "See [the research](research/index.md) and [the settings](research/quad-settings/index.md).",
    );
}

#[test]
fn a_link_to_another_file_in_the_docs_folder_stays_in_the_book() {
    let repo = Repo::new("link-to-a-text-file");
    repo.write(
        "docs/research/quad-settings/cetus-x.diff-all.txt",
        "set roll_rc_rate = 120\n",
    );
    repo.write(
        "docs/research/README.md",
        "The Cetus X's [`diff all`](quad-settings/cetus-x.diff-all.txt).\n",
    );
    let book = repo.preprocess(&["research/README.md"]).expect("the book");
    book.page("research/README.md")
        .says("The Cetus X's [`diff all`](quad-settings/cetus-x.diff-all.txt).");
}

#[test]
fn a_link_to_a_file_that_doesnt_exist_stops_the_book_and_names_it() {
    let repo = Repo::new("link-to-nothing");
    repo.write("docs/units.md", "See [the old list](old-units.md).\n");
    let problems = repo.preprocess(&["units.md"]).expect_err("a broken link");
    assert!(
        problems.contains(
            "`docs/units.md` links to `old-units.md`, but there is no `docs/old-units.md`."
        ),
        "{problems}"
    );
}

#[test]
fn a_link_to_a_page_the_book_leaves_out_stops_the_book() {
    let repo = Repo::new("link-to-a-page-left-out");
    repo.write("docs/units.md", "See [the notes](notes.md).\n");
    repo.write("docs/notes.md", "# Notes\n");
    let problems = repo.preprocess(&["units.md"]).expect_err("a page left out");
    assert!(
        problems.contains(
            "`docs/units.md` links to `docs/notes.md`, which isn't in the book. Add it to `docs/SUMMARY.md`."
        ),
        "{problems}"
    );
}

#[test]
fn a_page_under_docs_missing_from_the_table_of_contents_stops_the_book() {
    let repo = Repo::new("page-left-out");
    repo.write("docs/units.md", "# The unit list\n");
    repo.write(
        "docs/verification/reading-a-scenario.md",
        "# Reading a Scenario\n",
    );
    let problems = repo.preprocess(&["units.md"]).expect_err("a page left out");
    assert!(
        problems.contains(
            "`docs/verification/reading-a-scenario.md` isn't in the book. Add it to \
             `docs/SUMMARY.md`, in the part it belongs to, so the book shows every page."
        ),
        "{problems}"
    );
}

#[test]
fn a_link_inside_code_is_left_alone() {
    let repo = Repo::new("link-in-code");
    let text =
        "Write a link like `[map](missing.md)`.\n\n```markdown\n[map](also-missing.md)\n```\n";
    repo.write("docs/units.md", text);
    let book = repo.preprocess(&["units.md"]).expect("the book");
    book.page("units.md").says(text);
}

#[test]
fn links_to_websites_and_headings_on_the_same_page_are_left_alone() {
    let repo = Repo::new("links-left-alone");
    let text = "See [#37](https://github.com/BartoszSolkaBD/OpenDrone/issues/37), \
                [below](#rules) and [mail](mailto:someone@example.com).\n\n## Rules\n";
    repo.write("docs/units.md", text);
    let book = repo.preprocess(&["units.md"]).expect("the book");
    book.page("units.md").says(text);
}

#[test]
fn each_quads_feel_test_log_is_shown_from_its_pack_folder() {
    let repo = Repo::new("feel-test-logs");
    repo.write(
        "packs/opendrone/pack.toml",
        "format = 1\nid = \"opendrone\"\n",
    );
    repo.write(
        "packs/opendrone/quads/whoop-65/quad.toml",
        "format = 1\nname = \"Whoop 65\"\n",
    );
    repo.write(
        "packs/opendrone/quads/whoop-65/feel-tests.md",
        "# Feel Test log\n\n| Date | Number | Old → new | Why |\n|---|---|---|---|\n\
         | 2026-11-02 | `bounce` | 0.3 → 0.25 | bounced off walls too far |\n\n\
         The numbers are in [the Quad definition](quad.toml).\n",
    );
    repo.write(
        "docs/book/feel-test-logs.md",
        "# Feel Test logs\n\n{{#feel-test-logs ../../packs}}\n",
    );
    let book = repo
        .preprocess(&["book/feel-test-logs.md"])
        .expect("the book");

    let page = book.page("book/feel-test-logs.md");
    page.says("## Whoop 65");
    page.says("Quad `opendrone/whoop-65`");
    page.says("### Feel Test log");
    page.says("| 2026-11-02 | `bounce` | 0.3 → 0.25 | bounced off walls too far |");
    page.says(
        "[the Quad definition](https://github.com/example/opendrone/blob/main/packs/opendrone/quads/whoop-65/quad.toml)",
    );
}

#[test]
fn with_no_feel_test_logs_the_page_says_so() {
    let repo = Repo::new("no-feel-test-logs");
    repo.write(
        "docs/book/feel-test-logs.md",
        "# Feel Test logs\n\n{{#feel-test-logs ../../packs}}\n",
    );
    let book = repo
        .preprocess(&["book/feel-test-logs.md"])
        .expect("the book");
    book.page("book/feel-test-logs.md")
        .says("No Quad has a Feel Test log yet.");
}

#[test]
fn the_code_page_lists_every_crate_with_a_link_to_its_rustdoc() {
    let repo = Repo::new("crate-list");
    repo.write("crates/xtask/walls.toml", "core = [\"opendrone-maths\"]\n");
    repo.write(
        "crates/maths/Cargo.toml",
        "[package]\nname = \"opendrone-maths\"\ndescription = \"Shared 64-bit number types.\"\n",
    );
    repo.write(
        "crates/sound/Cargo.toml",
        "[package]\nname = \"opendrone-sound\"\ndescription = \"Makes the Quad's sound.\"\n",
    );
    repo.write(
        "crates/opendrone/Cargo.toml",
        "[package]\nname = \"opendrone\"\ndescription = \"The game.\"\n",
    );
    repo.write(
        "docs/book/the-code.md",
        "# The code\n\n{{#crate-list ../../crates}}\n",
    );
    let book = repo.preprocess(&["book/the-code.md"]).expect("the book");

    let page = book.page("book/the-code.md");
    page.says("| [`opendrone-maths`](../api/opendrone_maths/index.html) | Core | Shared 64-bit number types. |");
    page.says("| [`opendrone-sound`](../api/opendrone_sound/index.html) | Edge | Makes the Quad's sound. |");
    page.says("| [`opendrone`](../api/opendrone/index.html) | The game | The game. |");
}

#[test]
fn the_scenario_catalogue_page_links_each_scenario_to_its_file_on_github() {
    let repo = Repo::new("catalogue-page");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/scenarios/physics/free-fall.toml");
    repo.write(
        "scenarios/physics/free-fall.toml",
        &fs::read_to_string(fixture).expect("read the fixture Scenario"),
    );
    repo.write(
        "docs/book/catalogue.md",
        "# The Scenario catalogue\n\n{{#scenario-catalogue ../../scenarios}}\n",
    );
    let book = repo.preprocess(&["book/catalogue.md"]).expect("the book");

    let page = book.page("book/catalogue.md");
    page.says("### Free fall is exactly g");
    page.says(
        "[`scenarios/physics/free-fall.toml`](https://github.com/example/opendrone/blob/main/scenarios/physics/free-fall.toml)",
    );
}

/// A scratch repo for one check.
struct Repo {
    root: PathBuf,
}

impl Repo {
    fn new(name: &str) -> Repo {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("book")
            .join(name);
        if root.exists() {
            fs::remove_dir_all(&root).expect("clear the scratch repo");
        }
        fs::create_dir_all(root.join("docs")).expect("make the scratch repo");
        Repo { root }
    }

    fn write(&self, file: &str, text: &str) {
        let path = self.root.join(file);
        fs::create_dir_all(path.parent().expect("a folder")).expect("make the folder");
        fs::write(path, text).expect("write the file");
    }

    /// Runs the preprocessor as mdBook does, on a book whose chapters are these
    /// pages under `docs/`, in order. Gives back each page's new text by its
    /// file, or the problems it reports.
    fn preprocess(&self, pages: &[&str]) -> Result<Book, String> {
        let chapters: Vec<serde_json::Value> = pages
            .iter()
            .map(|page| {
                let content =
                    fs::read_to_string(self.root.join("docs").join(page)).expect("read a page");
                // mdBook's own `index` step renames README.md pages first.
                let path = match page.strip_suffix("README.md") {
                    Some(folder) => format!("{folder}index.md"),
                    None => (*page).to_owned(),
                };
                serde_json::json!({ "Chapter": {
                    "name": page,
                    "content": content,
                    "number": null,
                    "sub_items": [],
                    "path": path,
                    "source_path": page,
                    "parent_names": [],
                }})
            })
            .collect();
        let input = serde_json::json!([
            {
                "root": self.root,
                "config": {
                    "book": { "src": "docs" },
                    "output": { "html": { "git-repository-url": "https://github.com/example/opendrone" } },
                },
                "renderer": "html",
                "mdbook_version": "0.5.4",
            },
            { "items": chapters },
        ]);

        let mut child = Command::new(env!("CARGO_BIN_EXE_xtask"))
            .arg("book-preprocessor")
            .current_dir(&self.root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("run xtask");
        child
            .stdin
            .take()
            .expect("its input")
            .write_all(input.to_string().as_bytes())
            .expect("send the book");
        let output = child.wait_with_output().expect("wait for xtask");
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).into_owned());
        }
        let book: serde_json::Value = serde_json::from_slice(&output.stdout).expect("a book back");
        let mut contents = BTreeMap::new();
        for item in book["items"].as_array().expect("its chapters") {
            let chapter = &item["Chapter"];
            contents.insert(
                chapter["source_path"].as_str().expect("a file").to_owned(),
                chapter["content"].as_str().expect("its text").to_owned(),
            );
        }
        Ok(Book(contents))
    }
}

#[derive(Debug)]
struct Book(BTreeMap<String, String>);

impl Book {
    fn page(&self, file: &str) -> Page<'_> {
        Page(
            self.0
                .get(file)
                .unwrap_or_else(|| panic!("no page {file} in {:?}", self.0.keys())),
        )
    }
}

struct Page<'a>(&'a str);

impl Page<'_> {
    fn says(&self, text: &str) {
        assert!(self.0.contains(text), "expected {text:?} in:\n{}", self.0);
    }
}
