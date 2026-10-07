//! Finding and changing the links in a Markdown page, for the book.
//!
//! Links are found with a Markdown parser, so a link-like piece of text inside
//! a code block or a code span is left alone. Only the destination of each
//! link changes; everything else in the page stays byte for byte the same.

use std::ops::Range;

use pulldown_cmark::{Event, LinkType, Options, Parser, Tag};

/// The Markdown features mdBook reads, so pages are parsed the way it parses
/// them.
fn options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_HEADING_ATTRIBUTES
}

/// Gives every link in `markdown` to `change`, with whether it shows an image,
/// and puts back the new destination it returns. `None` keeps a link as it is.
///
/// A reference link (`[text][name]` with `[name]: destination` elsewhere) is
/// changed once, where its destination is written.
pub fn change_links(
    markdown: &str,
    mut change: impl FnMut(&str, bool) -> Option<String>,
) -> String {
    let mut edits: Vec<(Range<usize>, String)> = Vec::new();
    let parser = Parser::new_ext(markdown, options()).into_offset_iter();
    for (_, definition) in parser.reference_definitions().iter() {
        let written = &markdown[definition.span.clone()];
        let Some(colon) = written.find("]:") else {
            continue;
        };
        if let Some(range) = destination_after(
            markdown,
            definition.span.start + colon + 2,
            &definition.dest,
        ) && let Some(new) = change(&definition.dest, false)
        {
            edits.push((range, new));
        }
    }
    for (event, span) in parser {
        let (destination, is_image) = match &event {
            Event::Start(Tag::Link {
                link_type: LinkType::Inline,
                dest_url,
                ..
            }) => (dest_url, false),
            Event::Start(Tag::Image {
                link_type: LinkType::Inline,
                dest_url,
                ..
            }) => (dest_url, true),
            _ => continue,
        };
        if let Some(range) = inline_destination(markdown, span, destination)
            && let Some(new) = change(destination, is_image)
        {
            edits.push((range, new));
        }
    }

    edits.sort_by_key(|(range, _)| range.start);
    let mut changed = String::with_capacity(markdown.len());
    let mut copied_to = 0;
    for (range, new) in edits {
        if range.start < copied_to {
            continue;
        }
        changed.push_str(&markdown[copied_to..range.start]);
        changed.push_str(&new);
        copied_to = range.end;
    }
    changed.push_str(&markdown[copied_to..]);
    changed
}

/// Where the destination of an inline link `[text](destination "title")` is
/// written, inside the link's span.
fn inline_destination(
    markdown: &str,
    span: Range<usize>,
    destination: &str,
) -> Option<Range<usize>> {
    let link = &markdown[span.clone()];
    let mut before = link.len();
    while let Some(at) = link[..before].rfind("](") {
        if let Some(range) = destination_after(markdown, span.start + at + 2, destination) {
            return Some(range);
        }
        before = at;
    }
    None
}

/// The destination written at `from`, after any spaces and an opening `<`, if
/// it is written there exactly as `destination`. A destination written with
/// escapes reads differently from its text and is left alone.
fn destination_after(markdown: &str, from: usize, destination: &str) -> Option<Range<usize>> {
    let rest = &markdown[from..];
    let mut start = from + (rest.len() - rest.trim_start().len());
    if markdown[start..].starts_with('<') {
        start += 1;
    }
    markdown[start..]
        .starts_with(destination)
        .then(|| start..start + destination.len())
}

/// Moves every heading down `levels` levels, so a whole page can sit inside a
/// section of another page. Headings stop at level 6.
pub fn demote_headings(markdown: &str, levels: usize) -> String {
    let mut fence: Option<String> = None;
    let mut demoted = String::with_capacity(markdown.len() + 64);
    for line in markdown.split_inclusive('\n') {
        let trimmed = line.trim_start();
        let marker: String = trimmed
            .chars()
            .take_while(|&c| c == '`' || c == '~')
            .collect();
        if marker.len() >= 3
            && marker
                .chars()
                .all(|c| c == marker.chars().next().unwrap_or('`'))
        {
            match &fence {
                None => fence = Some(marker),
                Some(open) if marker.starts_with(open.as_str()) => fence = None,
                Some(_) => {}
            }
            demoted.push_str(line);
            continue;
        }
        let hashes = trimmed.chars().take_while(|&c| c == '#').count();
        let is_heading = fence.is_none()
            && line.len() - trimmed.len() <= 3
            && (1..=6).contains(&hashes)
            && trimmed[hashes..].starts_with([' ', '\t', '\n', '\r']);
        if is_heading {
            let level = (hashes + levels).min(6);
            demoted.push_str(&"#".repeat(level));
            demoted.push_str(&trimmed[hashes..]);
        } else {
            demoted.push_str(line);
        }
    }
    demoted
}
