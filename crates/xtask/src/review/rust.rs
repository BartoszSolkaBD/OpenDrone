//! Telling Rust code from the words around it, for the Red Flags that look
//! at code: new `unsafe` code and house-rule exceptions.
//!
//! It reads a file with rustc's own lexer, `ra-ap-rustc_lexer` 0.174.0, whose
//! source is byte for byte the lexer of Rust 1.99.0, the pinned Rust. It goes
//! the way rustc 1.99 goes for a source file, so it finds exactly the comments
//! and literals rustc finds:
//!
//! 1. drop a byte order mark, and read Windows line ends as plain ones;
//! 2. skip the first line if `rustc_lexer::strip_shebang` calls it a shebang;
//! 3. lex, with frontmatter allowed at the start;
//! 4. make the changes rustc's parser makes, by edition, to where a literal
//!    ends: before 2021, `c"…"` and `cr"…"` are a name and then the rest;
//!    before 2024, `#"…"` is `#` and then a string.
//!
//! The edition is the one the file's crate uses, read from its `Cargo.toml`
//! ([`edition_of`]). What this can't see: a file a crate pulls in with
//! `include!` or `#[path]` from another crate, which rustc reads with the
//! other crate's edition, and an edition set on one target (`[lib]`) alone.

use ra_ap_rustc_lexer::{
    Cursor, FrontmatterAllowed, LiteralKind, TokenKind, strip_shebang, tokenize,
};

/// A Rust edition, as far as reading a file goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Edition {
    E2015,
    E2018,
    E2021,
    E2024,
}

impl Edition {
    /// The edition a manifest names. Rust 1.99 knows no edition after 2024,
    /// so an unknown name doesn't compile; it reads as 2024.
    pub fn from_name(name: &str) -> Edition {
        match name {
            "2015" => Edition::E2015,
            "2018" => Edition::E2018,
            "2021" => Edition::E2021,
            _ => Edition::E2024,
        }
    }
}

/// The edition of the crate that holds `path` (from the repository's root,
/// with `/` between folders), as cargo works it out: the nearest
/// `Cargo.toml` above it with a `[package]` gives its `edition`, takes the
/// workspace's with `edition.workspace = true`, or is 2015 without one. A file
/// in no package reads with the workspace's edition, else 2024. `read` gives a
/// file's text on one side of the pull request.
pub fn edition_of(path: &str, read: impl Fn(&str) -> Option<String>) -> Edition {
    let manifest = |folder: &str| -> Option<toml::Table> {
        let file = if folder.is_empty() {
            "Cargo.toml".to_string()
        } else {
            format!("{folder}/Cargo.toml")
        };
        read(&file)?.parse().ok()
    };
    let workspace = || {
        manifest("")
            .as_ref()
            .and_then(|root| {
                root.get("workspace")?
                    .get("package")?
                    .get("edition")?
                    .as_str()
            })
            .map_or(Edition::E2024, Edition::from_name)
    };
    let mut folder = path;
    while let Some((parent, _)) = folder.rsplit_once('/') {
        folder = parent;
        if let Some(package) = manifest(folder).as_ref().and_then(|m| m.get("package")) {
            return package_edition(package, workspace);
        }
    }
    match manifest("").as_ref().and_then(|m| m.get("package")) {
        Some(package) => package_edition(package, workspace),
        None => workspace(),
    }
}

/// A `[package]` table's edition.
fn package_edition(package: &toml::Value, workspace: impl Fn() -> Edition) -> Edition {
    match package.get("edition") {
        Some(toml::Value::String(name)) => Edition::from_name(name),
        Some(toml::Value::Table(inherited)) if inherited.get("workspace").is_some() => workspace(),
        Some(_) => Edition::E2024,
        None => Edition::E2015,
    }
}

/// A Rust file with every comment, every string and char literal, a skipped
/// first line and any frontmatter blanked out, so only code is left: the
/// names, numbers, punctuation and literal suffixes rustc reads. The text
/// keeps its lines, so line N of the result is line N of the file.
pub fn code_only(text: &str, edition: Edition) -> String {
    let text = text
        .strip_prefix('\u{feff}')
        .unwrap_or(text)
        .replace("\r\n", "\n");
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    if let Some(shebang) = strip_shebang(&text) {
        blank(&mut out, &text[..shebang]);
        at = shebang;
    }
    let mut frontmatter = FrontmatterAllowed::Yes;
    'lexing: while at < text.len() {
        for token in tokenize(&text[at..], frontmatter) {
            frontmatter = FrontmatterAllowed::No;
            let piece = &text[at..at + token.len as usize];
            if let Some(again) = relex(token.kind, &text[at..], edition) {
                match again {
                    Again::AfterCode(len) => out.push_str(&text[at..at + len]),
                    Again::AfterWords(len) => blank(&mut out, &text[at..at + len]),
                }
                at += again.len();
                continue 'lexing;
            }
            match token.kind {
                TokenKind::LineComment { .. }
                | TokenKind::BlockComment { .. }
                | TokenKind::Frontmatter { .. } => blank(&mut out, piece),
                // A string, byte string, C string, raw string or char: its
                // suffix, a name, is code; the rest is words.
                TokenKind::Literal { kind, suffix_start }
                    if !matches!(kind, LiteralKind::Int { .. } | LiteralKind::Float { .. }) =>
                {
                    let suffix = suffix_start as usize;
                    blank(&mut out, &piece[..suffix]);
                    out.push_str(&piece[suffix..]);
                }
                // Everything else is code.
                _ => out.push_str(piece),
            }
            at += piece.len();
        }
        break;
    }
    out
}

/// Where rustc's parser lexes again, because of the edition, and what comes
/// before that place.
#[derive(Clone, Copy, Debug)]
enum Again {
    /// So many bytes of code.
    AfterCode(usize),
    /// So many bytes of a literal: words.
    AfterWords(usize),
}

impl Again {
    fn len(self) -> usize {
        match self {
            Again::AfterCode(len) | Again::AfterWords(len) => len,
        }
    }
}

/// What rustc 1.99's parser does after the lexer gives it `kind` at the start
/// of `rest`, when it doesn't take the token as it is (in
/// `rustc_parse::lexer`):
///
/// - before 2021, a C string's `c` or `cr` is a name, and the rest is lexed
///   again;
/// - a guarded string's `#"` or `##` (the lexer's token takes the second
///   character too): before 2024 the `#` alone is code, and the rest is
///   lexed again; in 2024 a whole `#"…"#` is one literal (which 2024
///   refuses), and a `##` without a string stays as it is.
fn relex(kind: TokenKind, rest: &str, edition: Edition) -> Option<Again> {
    match kind {
        TokenKind::Literal {
            kind: LiteralKind::CStr { .. },
            ..
        } if edition < Edition::E2021 => Some(Again::AfterCode(1)),
        TokenKind::Literal {
            kind: LiteralKind::RawCStr { .. },
            ..
        } if edition < Edition::E2021 => Some(Again::AfterCode(2)),
        TokenKind::GuardedStrPrefix if edition < Edition::E2024 => Some(Again::AfterCode(1)),
        TokenKind::GuardedStrPrefix => Cursor::new(rest, FrontmatterAllowed::No)
            .guarded_double_quoted_string()
            .map(|guarded| Again::AfterWords(guarded.token_len as usize)),
        _ => None,
    }
}

/// Blanks text: each line break stays, so lines still line up.
fn blank(out: &mut String, text: &str) {
    out.extend(text.chars().map(|c| if c == '\n' { '\n' } else { ' ' }));
}
