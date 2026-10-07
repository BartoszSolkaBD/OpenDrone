//! Writing text that came from a pull request into the Review Report.
//!
//! Everything a pull request controls (file names, Scenario text, lines of
//! code, library names and the licences crates.io gives for them) goes
//! through one of these two, so it can only ever show as text. It can't start
//! a line, so it can't make a heading, a list or a fake Verdict. It can't open
//! HTML, so it can't hide the rest of the Report or forge its hidden marker.
//! And it can't mention anyone.

/// Text to show as it is, inside a sentence or a table cell.
pub fn plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\n' | '\r' | '\t' => out.push(' '),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            // A zero-width space after `@`, so it never mentions anyone.
            '@' => out.push_str("@\u{200B}"),
            '\\' | '`' | '*' | '_' | '[' | ']' | '|' | '#' | '~' | '!' => {
                out.push('\\');
                out.push(c);
            }
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// Text to show as code, such as a file name or a line of Rust: in backticks,
/// with no backtick, line break or table bar inside to end it early.
pub fn code(text: &str) -> String {
    let inner: String = text
        .chars()
        .map(|c| match c {
            '`' => '\'',
            '|' => '¦',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    format!("`{}`", inner.trim())
}
