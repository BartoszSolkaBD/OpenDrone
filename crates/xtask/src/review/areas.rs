//! Areas, read from `.github/CODEOWNERS` (#15 §9): each block of patterns
//! there starts with a comment naming its Area, such as `# Physics` or
//! `# Repo rules: CI, root Cargo files, …`. CODEOWNERS, PR labels and the
//! Review Report all use the same Areas.

/// The Area a file belongs to when only the catch-all `*` matches it.
pub const EVERYTHING_ELSE: &str = "Everything else";

/// The Area whose changes the Reviewer decides on (#15 §5).
pub const REPO_RULES: &str = "Repo rules";

/// The Areas and their patterns, in CODEOWNERS order.
#[derive(Clone, Debug, Default)]
pub struct Areas {
    patterns: Vec<(Pattern, String)>,
}

impl Areas {
    /// Reads CODEOWNERS. A block's Area is its first comment line, up to a
    /// colon or an opening bracket; a pattern with no comment above it in its
    /// block belongs to no Area.
    pub fn parse(codeowners: &str) -> Areas {
        let mut patterns = Vec::new();
        let mut area: Option<String> = None;
        let mut block_has_comment = false;
        for line in codeowners.lines() {
            let line = line.trim();
            if line.is_empty() {
                area = None;
                block_has_comment = false;
            } else if let Some(comment) = line.strip_prefix('#') {
                if !block_has_comment {
                    block_has_comment = true;
                    let name = comment.split([':', '(']).next().unwrap_or_default().trim();
                    area = (!name.is_empty()).then(|| name.to_string());
                }
            } else if let (Some(pattern), Some(area)) = (line.split_whitespace().next(), &area) {
                patterns.push((Pattern::new(pattern), area.clone()));
            }
        }
        Areas { patterns }
    }

    /// Every Area named in CODEOWNERS, in order, without repeats.
    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for (_, area) in &self.patterns {
            if !names.contains(area) {
                names.push(area.clone());
            }
        }
        names
    }

    /// The Area of a file: the last matching pattern wins, as in CODEOWNERS.
    pub fn of(&self, path: &str) -> Option<&str> {
        self.patterns
            .iter()
            .rev()
            .find(|(pattern, _)| pattern.matches(path))
            .map(|(_, area)| area.as_str())
    }
}

/// One CODEOWNERS pattern, with gitignore's rules for `/` and `*`.
#[derive(Clone, Debug)]
struct Pattern {
    segments: Vec<String>,
    /// Starts with `/`, or has a `/` in the middle: matches from the root.
    anchored: bool,
    /// Ends with `/`: matches a folder and everything in it.
    folder_only: bool,
}

impl Pattern {
    fn new(text: &str) -> Pattern {
        let folder_only = text.ends_with('/');
        let trimmed = text.trim_end_matches('/');
        let anchored = trimmed.starts_with('/') || trimmed.trim_start_matches('/').contains('/');
        let segments = trimmed
            .trim_start_matches('/')
            .split('/')
            .filter(|segment| !segment.is_empty())
            .map(str::to_string)
            .collect();
        Pattern {
            segments,
            anchored,
            folder_only,
        }
    }

    fn matches(&self, path: &str) -> bool {
        let parts: Vec<&str> = path.split('/').collect();
        let starts: Vec<usize> = if self.anchored {
            vec![0]
        } else {
            (0..parts.len()).collect()
        };
        starts
            .into_iter()
            .any(|start| self.matches_from(&parts[start..]))
    }

    /// Whether the pattern matches the path's first parts: the whole file, or
    /// a folder it sits in.
    fn matches_from(&self, parts: &[&str]) -> bool {
        let n = self.segments.len();
        if parts.len() < n {
            return false;
        }
        if self.folder_only && parts.len() == n {
            // A pattern ending in `/` names a folder, never a file.
            return false;
        }
        self.segments
            .iter()
            .zip(parts)
            .all(|(segment, part)| glob(segment, part))
    }
}

/// `*` matches any run of characters within one folder or file name.
fn glob(pattern: &str, text: &str) -> bool {
    let Some((first, rest)) = pattern.split_once('*') else {
        return pattern == text;
    };
    let Some(mut remaining) = text.strip_prefix(first) else {
        return false;
    };
    let pieces: Vec<&str> = rest.split('*').collect();
    for (i, piece) in pieces.iter().enumerate() {
        if i == pieces.len() - 1 {
            return remaining.ends_with(piece);
        }
        match remaining.find(piece) {
            Some(at) => remaining = &remaining[at + piece.len()..],
            None => return false,
        }
    }
    true
}
