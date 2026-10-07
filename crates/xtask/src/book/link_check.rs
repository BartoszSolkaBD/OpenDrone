//! The link check on the drawn book: every link from a page of the book must
//! lead to a file that's there, and to a heading that's there when it names
//! one. Links to other websites aren't followed, so the check never depends
//! on the network.
//!
//! The rustdoc pages under `api/` are checked as link targets, not as pages:
//! rustdoc checks its own links (`cargo xtask book` runs it with warnings as
//! errors). The print page (`print.html`) and the "page not found" page
//! (`404.html`) are mdBook's own copies of the other pages and are skipped.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use super::paths;

/// What the link check found.
pub struct Checked {
    pub pages: usize,
    pub links: usize,
    pub problems: Vec<String>,
}

pub fn check(book: &Path) -> Checked {
    let mut files = Vec::new();
    collect_files(book, "", &mut files);
    let all: BTreeSet<&str> = files.iter().map(String::as_str).collect();
    let mut ids: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    let mut checked = Checked {
        pages: 0,
        links: 0,
        problems: Vec::new(),
    };
    for &page in &all {
        if !page.ends_with(".html") || !is_checked_page(page) {
            continue;
        }
        let Ok(text) = fs::read_to_string(book.join(page)) else {
            checked.problems.push(format!("`{page}` can't be read."));
            continue;
        };
        checked.pages += 1;
        for link in links(&text) {
            checked.links += 1;
            if let Some(problem) = check_link(book, page, &link, &all, &mut ids) {
                checked.problems.push(problem);
            }
        }
    }
    checked
}

fn is_checked_page(page: &str) -> bool {
    page != "print.html"
        && page != "404.html"
        && (!page.starts_with("api/") || page == "api/index.html")
}

/// The problem with one link on `page`, if it has one.
fn check_link(
    book: &Path,
    page: &str,
    link: &str,
    all: &BTreeSet<&str>,
    ids: &mut BTreeMap<String, BTreeSet<String>>,
) -> Option<String> {
    if paths::is_outside_link(link) && !link.starts_with('#') {
        return None;
    }
    let (path, suffix) = paths::split_suffix(link);
    let target = if path.is_empty() {
        page.to_owned()
    } else if path.starts_with('/') {
        return Some(format!(
            "`{page}` links to `{link}`, which starts at the web server's root. Links in the book \
             must be relative, because the book is published inside a folder."
        ));
    } else {
        let Some(joined) = paths::join(paths::folder(page), &paths::percent_decoded(path)) else {
            return Some(format!(
                "`{page}` links to `{link}`, which is outside the book."
            ));
        };
        if all.contains(joined.as_str()) {
            joined
        } else if all.contains(format!("{joined}/index.html").as_str()) {
            format!("{joined}/index.html")
        } else {
            return Some(format!(
                "`{page}` links to `{link}`, but the book has no `{joined}`."
            ));
        }
    };

    let fragment = suffix
        .split_once('#')
        .map(|(_, fragment)| paths::percent_decoded(fragment))
        .filter(|fragment| !fragment.is_empty())?;
    if !target.ends_with(".html") {
        return None;
    }
    let found = ids.entry(target.clone()).or_insert_with(|| {
        fs::read_to_string(book.join(&target))
            .map(|text| html_ids(&text))
            .unwrap_or_default()
    });
    (!found.contains(&fragment)).then(|| {
        format!("`{page}` links to `{link}`, but `{target}` has no heading or anchor `{fragment}`.")
    })
}

/// Every `href` and `src` in a page, outside comments, scripts and styles,
/// with HTML's `&amp;`-style escapes undone.
fn links(html: &str) -> Vec<String> {
    let mut found = Vec::new();
    for tag in tags(html) {
        for name in [" href=", " src="] {
            if let Some(value) = attribute(tag, name) {
                found.push(value);
            }
        }
    }
    found
}

/// Every `id` in a page: the targets of `#…` links.
fn html_ids(html: &str) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for tag in tags(html) {
        for name in [" id=", " name="] {
            if let Some(value) = attribute(tag, name) {
                ids.insert(value);
            }
        }
    }
    ids
}

/// Every tag in a page, such as `<a href="…">`, outside comments, scripts and
/// styles, whose text isn't HTML.
fn tags(html: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut rest = html;
    while let Some(at) = rest.find('<') {
        rest = &rest[at..];
        if rest.starts_with("<!--") {
            rest = rest.find("-->").map_or("", |end| &rest[end..]);
            continue;
        }
        let end = rest.find('>').map_or(rest.len(), |end| end + 1);
        let tag = &rest[..end];
        found.push(tag);
        rest = &rest[end..];
        for (opening, closing) in [("<script", "</script>"), ("<style", "</style>")] {
            if tag.starts_with(opening) {
                rest = rest.find(closing).map_or("", |end| &rest[end..]);
            }
        }
    }
    found
}

/// The value of an attribute such as ` href=` inside a tag.
fn attribute(tag: &str, name: &str) -> Option<String> {
    let at = tag.find(name)? + name.len();
    let quote = tag[at..]
        .chars()
        .next()
        .filter(|c| *c == '"' || *c == '\'')?;
    let value = &tag[at + 1..];
    let value = &value[..value.find(quote)?];
    Some(unescaped(value))
}

fn unescaped(text: &str) -> String {
    text.replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// Every file under `folder`, from the book's root, with `/` between folders.
fn collect_files(book: &Path, folder: &str, files: &mut Vec<String>) {
    for entry in fs::read_dir(book.join(folder))
        .into_iter()
        .flatten()
        .flatten()
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = if folder.is_empty() {
            name
        } else {
            format!("{folder}/{name}")
        };
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            collect_files(book, &path, files);
        } else {
            files.push(path);
        }
    }
}
