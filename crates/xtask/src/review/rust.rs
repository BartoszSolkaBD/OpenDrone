//! Telling Rust code from the words around it, for the Red Flags that look
//! at code: new `unsafe` code and house-rule exceptions.
//!
//! It reads a file the way Rust 1.99 reads one in the 2024 edition, which
//! every crate here uses. Reading a literal differently from Rust could hide
//! code inside what looks like a string, so every kind of literal Rust accepts
//! is understood, with its prefix and its suffix.

/// A Rust file with every comment and the inside of every string and char
/// literal blanked out, so only code is left. It understands:
///
/// - strings, with escapes: `"…"`, byte strings `b"…"` and C strings `c"…"`;
/// - raw strings, with no escapes, which end at a quote followed by as many
///   `#`s as they start with: `r"…"`, `r#"…"#`, `br#"…"#` and `cr#"…"#`;
/// - char literals: `'"'`, `'\''`, `'\u{22}'` and `b'"'`, while a lifetime
///   such as `'a` is code;
/// - a literal's suffix, such as `u8` in `1u8`, or `r` in `"…"r` (allowed
///   inside a macro's input): a name, never the start of another literal;
/// - line comments, nested block comments, and a first line that Rust skips
///   because it starts with `#!` (a shebang).
///
/// So none of them can hide code from the checks or pass for code. The text
/// keeps its lines, so line N of the result is line N of the file.
pub fn code_only(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = shebang(&chars, &mut out);
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        // A literal's prefix (`b`, `c`, `r`, `br` or `cr`) counts only at the
        // start of a word: in `xr"…"` or `1r"…"`, `r` belongs to a name.
        let starts_word = i == 0 || !is_name(chars[i - 1]);
        if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                blank(&mut out, chars[i]);
                i += 1;
            }
        } else if c == '/' && next == Some('*') {
            i = block_comment(&chars, i, &mut out);
        } else if let Some((hashes, quote_at)) = raw_string_start(&chars, i).filter(|_| starts_word)
        {
            // No escapes: it ends at a quote followed by the same number of #s.
            for &c in &chars[i..=quote_at] {
                out.push(c);
            }
            i = quote_at + 1;
            while i < chars.len() {
                let closes =
                    chars[i] == '"' && (0..hashes).all(|k| chars.get(i + 1 + k) == Some(&'#'));
                if closes {
                    out.push('"');
                    for _ in 0..hashes {
                        out.push('#');
                    }
                    i += 1 + hashes;
                    break;
                }
                blank(&mut out, chars[i]);
                i += 1;
            }
            i = suffix(&chars, i, &mut out);
        } else if let Some(quote_at) =
            string_start(&chars, i).filter(|&quote_at| quote_at == i || starts_word)
        {
            for &c in &chars[i..=quote_at] {
                out.push(c);
            }
            i = quote_at + 1;
            while i < chars.len() {
                match chars[i] {
                    '\\' => {
                        blank(&mut out, '\\');
                        if let Some(&escaped) = chars.get(i + 1) {
                            blank(&mut out, escaped);
                        }
                        i += 2;
                    }
                    '"' => {
                        out.push('"');
                        i += 1;
                        break;
                    }
                    other => {
                        blank(&mut out, other);
                        i += 1;
                    }
                }
            }
            i = suffix(&chars, i, &mut out);
        } else if c == '\'' || (c == 'b' && next == Some('\'') && starts_word) {
            let start = if c == 'b' { i + 1 } else { i };
            match char_literal_end(&chars, start) {
                Some(end) => {
                    if c == 'b' {
                        out.push('b');
                    }
                    out.push('\'');
                    for &inner in &chars[start + 1..end] {
                        blank(&mut out, inner);
                    }
                    out.push('\'');
                    i = suffix(&chars, end + 1, &mut out);
                }
                None => {
                    // A lifetime, such as 'a: code.
                    out.push(c);
                    i += 1;
                }
            }
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

/// Whether a character can be part of a name.
fn is_name(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Whether Rust reads a character as a space between tokens. That is fewer
/// than Unicode's spaces: a no-break space isn't one, while the invisible
/// left-to-right mark is.
fn is_rust_whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n'
            | '\u{000B}'
            | '\u{000C}'
            | '\r'
            | ' '
            | '\u{0085}'
            | '\u{200E}'
            | '\u{200F}'
            | '\u{2028}'
            | '\u{2029}'
    )
}

/// Blanks a character: a line break stays, so lines still line up.
fn blank(out: &mut String, c: char) {
    out.push(if c == '\n' { '\n' } else { ' ' });
}

/// Rust skips a file's first line when it starts with `#!` and isn't an inner
/// attribute such as `#![allow(…)]`: that is, when the first thing after the
/// `#!`, past spaces and plain comments, isn't a `[`. Blanks such a line, and
/// returns where the rest of the file starts.
fn shebang(chars: &[char], out: &mut String) -> usize {
    // Rust drops a byte order mark before it looks.
    let start = usize::from(chars.first() == Some(&'\u{feff}'));
    if chars.get(start) != Some(&'#') || chars.get(start + 1) != Some(&'!') {
        return 0;
    }
    let mut j = start + 2;
    loop {
        let fourth = chars.get(j + 3).copied();
        match (chars.get(j), chars.get(j + 1), chars.get(j + 2)) {
            (Some(&c), _, _) if is_rust_whitespace(c) => j += 1,
            // A plain line comment: not `//!`, and not `///` unless it is
            // `////`.
            (Some('/'), Some('/'), third)
                if match third {
                    Some('!') => false,
                    Some('/') => fourth == Some('/'),
                    _ => true,
                } =>
            {
                while j < chars.len() && chars[j] != '\n' {
                    j += 1;
                }
            }
            // A plain block comment: not `/*!`, and not `/**` unless it is
            // `/**/` or `/***`.
            (Some('/'), Some('*'), third)
                if match third {
                    Some('!') => false,
                    Some('*') => matches!(fourth, Some('*' | '/')),
                    _ => true,
                } =>
            {
                j = block_comment(chars, j, &mut String::new());
            }
            (next, _, _) => {
                if next == Some(&'[') {
                    return 0;
                }
                break;
            }
        }
    }
    let mut i = 0;
    while i < chars.len() && chars[i] != '\n' {
        if i < start {
            out.push(chars[i]);
        } else {
            blank(out, chars[i]);
        }
        i += 1;
    }
    i
}

/// Blanks the block comment that starts at `i`, nested ones inside it
/// included, and returns where it ends.
fn block_comment(chars: &[char], mut i: usize, out: &mut String) -> usize {
    let mut depth = 0;
    while i < chars.len() {
        if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
            depth += 1;
            blank(out, '/');
            blank(out, '*');
            i += 2;
        } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
            depth -= 1;
            blank(out, '*');
            blank(out, '/');
            i += 2;
            if depth == 0 {
                break;
            }
        } else {
            blank(out, chars[i]);
            i += 1;
        }
    }
    i
}

/// If a raw string starts at `i` (`r"`, `r#"`, `br"`, `cr#"`, …): its number
/// of `#`s and where its opening quote is.
fn raw_string_start(chars: &[char], i: usize) -> Option<(usize, usize)> {
    let mut j = i;
    if matches!(chars.get(j), Some('b' | 'c')) {
        j += 1;
    }
    if chars.get(j) != Some(&'r') {
        return None;
    }
    j += 1;
    let mut hashes = 0;
    while chars.get(j) == Some(&'#') {
        hashes += 1;
        j += 1;
    }
    (chars.get(j) == Some(&'"')).then_some((hashes, j))
}

/// If a string with escapes starts at `i` (`"`, `b"` or `c"`): where its
/// opening quote is.
fn string_start(chars: &[char], i: usize) -> Option<usize> {
    match chars.get(i)? {
        '"' => Some(i),
        'b' | 'c' if chars.get(i + 1) == Some(&'"') => Some(i + 1),
        _ => None,
    }
}

/// A literal's suffix, the name right after it, as in `1u8` or `"…"r`: Rust
/// reads it as part of the literal, so it never starts another one. Copies it
/// as code, and returns where it ends.
fn suffix(chars: &[char], mut i: usize, out: &mut String) -> usize {
    while let Some(&c) = chars.get(i).filter(|c| is_name(**c)) {
        out.push(c);
        i += 1;
    }
    i
}

/// If a char literal starts at the quote at `i`, where its closing quote is.
/// `'a'`, `'"'`, `'\''`, `'\n'` and `'\u{22}'` are char literals; `'a` in
/// `&'a str` is a lifetime.
fn char_literal_end(chars: &[char], i: usize) -> Option<usize> {
    match chars.get(i + 1)? {
        '\\' => {
            if chars.get(i + 2) == Some(&'u') {
                let close = (i + 3..chars.len().min(i + 14)).find(|&k| chars[k] == '}')?;
                (chars.get(close + 1) == Some(&'\'')).then_some(close + 1)
            } else if chars.get(i + 2) == Some(&'x') {
                (chars.get(i + 5) == Some(&'\'')).then_some(i + 5)
            } else {
                (chars.get(i + 3) == Some(&'\'')).then_some(i + 3)
            }
        }
        '\n' => None,
        _ => (chars.get(i + 2) == Some(&'\'')).then_some(i + 2),
    }
}
