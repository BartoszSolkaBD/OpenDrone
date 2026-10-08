//! Writing text that came from a pull request into the Review Report.
//!
//! Everything a pull request controls (file names, Scenario text, lines of
//! code, library names and the licences crates.io gives for them) goes
//! through one of these two, so it can only ever show as text. It can't start
//! a line, so it can't make a heading, a list or a fake Verdict. It can't open
//! HTML, so it can't hide the rest of the Report or forge its hidden marker.
//! It can't mention anyone. And an invisible formatting character, such as
//! one that turns text right to left, shows as its code instead, so a file
//! name can't pass for another.

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
            c if is_format(c) => out.push_str(&format!("\\\\u{{{:04X}}}", c as u32)),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// Text to show as code, such as a file name or a line of Rust: in backticks,
/// with no backtick, line break or table bar inside to end it early.
pub fn code(text: &str) -> String {
    let mut inner = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '`' => inner.push('\''),
            '|' => inner.push('¦'),
            c if is_format(c) => inner.push_str(&format!("\\u{{{:04X}}}", c as u32)),
            c if c.is_control() => inner.push(' '),
            c => inner.push(c),
        }
    }
    format!("`{}`", inner.trim())
}

/// Unicode's invisible formatting characters (its `Cf` category), such as
/// U+202E, which shows the text after it right to left.
pub fn is_format(c: char) -> bool {
    matches!(
        c as u32,
        0x00AD
            | 0x0600..=0x0605
            | 0x061C
            | 0x06DD
            | 0x070F
            | 0x0890..=0x0891
            | 0x08E2
            | 0x180E
            | 0x200B..=0x200F
            | 0x202A..=0x202E
            | 0x2060..=0x2064
            | 0x2066..=0x206F
            | 0xFEFF
            | 0xFFF9..=0xFFFB
            | 0x110BD
            | 0x110CD
            | 0x13430..=0x1343F
            | 0x1BCA0..=0x1BCA3
            | 0x1D173..=0x1D17A
            | 0xE0001
            | 0xE0020..=0xE007F
    )
}
