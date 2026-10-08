//! Telling Rust code from the words around it, for the Red Flags that look
//! at code: new `unsafe` code and house-rule exceptions.

/// A Rust file with every comment and the inside of every string and char
/// literal blanked out, so only code is left. Strings (`"…"`, `b"…"`), raw
/// strings (`r"…"`, `r#"…"#`, `br#"…"#`), char literals (`'"'`, `'\''`,
/// `'\u{22}'`), line comments and nested block comments are all understood, so
/// none of them can hide code from the checks or pass for code. The text keeps
/// its lines, so line N of the result is line N of the file.
pub fn code_only(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    // Blanks a character: a line break stays, so lines still line up.
    let blank = |out: &mut String, c: char| out.push(if c == '\n' { '\n' } else { ' ' });
    let is_name = |c: char| c.is_alphanumeric() || c == '_';
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        let after_name = i > 0 && is_name(chars[i - 1]);
        if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                blank(&mut out, chars[i]);
                i += 1;
            }
        } else if c == '/' && next == Some('*') {
            let mut depth = 0;
            while i < chars.len() {
                if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                    depth += 1;
                    blank(&mut out, chars[i]);
                    blank(&mut out, '*');
                    i += 2;
                } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    blank(&mut out, '*');
                    blank(&mut out, '/');
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    blank(&mut out, chars[i]);
                    i += 1;
                }
            }
        } else if let Some((hashes, quote_at)) = raw_string_start(&chars, i).filter(|_| !after_name)
        {
            // r"…", r#"…"#, br"…": no escapes, ends at a quote and the same
            // number of #s.
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
        } else if c == '"' || (c == 'b' && next == Some('"') && !after_name) {
            if c == 'b' {
                out.push('b');
                i += 1;
            }
            out.push('"');
            i += 1;
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
        } else if c == '\'' || (c == 'b' && next == Some('\'') && !after_name) {
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
                    i = end + 1;
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

/// If a raw string starts at `i` (`r"`, `r#"`, `br"`, …): its number of `#`s
/// and where its opening quote is.
fn raw_string_start(chars: &[char], i: usize) -> Option<(usize, usize)> {
    let mut j = i;
    if chars.get(j) == Some(&'b') {
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
