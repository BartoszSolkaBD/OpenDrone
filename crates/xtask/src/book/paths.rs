//! Paths inside the repo and inside the book, always written with `/`, so
//! they read the same on every OS.

/// Joins `path` onto the folder `base` and removes `.` and `..`. Both are
/// relative to the same root; `None` means the result would leave it.
pub fn join(base: &str, path: &str) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    for part in base.split('/').chain(path.split('/')) {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            part => parts.push(part),
        }
    }
    Some(parts.join("/"))
}

/// The folder a file is in: `docs/context` for `docs/context/flying.md`, and
/// the empty string for a file at the root.
pub fn folder(file: &str) -> &str {
    file.rsplit_once('/').map_or("", |(folder, _)| folder)
}

/// How to write a link from the page `from` to the file `to`, both relative to
/// the same root: `../context/flying.md` from `book/map.md`.
pub fn relative(from: &str, to: &str) -> String {
    let from_folder: Vec<&str> = folder(from).split('/').filter(|p| !p.is_empty()).collect();
    let to_parts: Vec<&str> = to.split('/').filter(|p| !p.is_empty()).collect();
    let shared = from_folder
        .iter()
        .zip(&to_parts)
        .take_while(|(a, b)| a == b)
        .count();
    let mut link = "../".repeat(from_folder.len() - shared);
    link.push_str(&to_parts[shared..].join("/"));
    link
}

/// Whether a link goes somewhere other than a file of this repo or book: a
/// web address, an email address, or a heading on the same page.
pub fn is_outside_link(destination: &str) -> bool {
    if destination.is_empty() || destination.starts_with('#') || destination.starts_with("//") {
        return true;
    }
    // A scheme such as `https:` or `mailto:`: a letter, then letters, digits,
    // `+`, `-` or `.`, then a colon.
    let scheme_end = destination
        .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')))
        .unwrap_or(destination.len());
    scheme_end > 0
        && destination.starts_with(|c: char| c.is_ascii_alphabetic())
        && destination[scheme_end..].starts_with(':')
}

/// Splits a link into its path and what follows it (`?…` or `#…`).
pub fn split_suffix(destination: &str) -> (&str, &str) {
    let at = destination.find(['?', '#']).unwrap_or(destination.len());
    destination.split_at(at)
}

/// Undoes `%20`-style escapes in a link's path.
pub fn percent_decoded(path: &str) -> String {
    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%'
            && let Some(hex) = path.get(at + 1..at + 3)
            && let Ok(byte) = u8::from_str_radix(hex, 16)
        {
            decoded.push(byte);
            at += 3;
        } else {
            decoded.push(bytes[at]);
            at += 1;
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}
