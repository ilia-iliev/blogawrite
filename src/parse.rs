use pulldown_cmark::{Event, Options, Parser};
use std::ops::Range;

/// Top-level Markdown blocks and the source between them. `gaps` has one more item than
/// `blocks`: before the first block, between each pair, and after the last. Joining them
/// recreates the input byte for byte.
#[derive(Debug, PartialEq, Eq)]
pub struct Segments {
    pub blocks: Vec<String>,
    pub gaps: Vec<String>,
}

pub fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_FOOTNOTES
}

/// Split Markdown while retaining every byte outside its semantic blocks. Pulldown-cmark
/// deliberately emits no event for some source, notably reference definitions. Any such
/// non-whitespace gap becomes a block of its own rather than disappearing on save.
pub fn segments(source: &str) -> Segments {
    let mut ranges = semantic_ranges(source);
    for range in &mut ranges {
        range.end = range.start + source[range.clone()].trim_end().len();
    }
    ranges.retain(|range| !range.is_empty());
    include_unparsed(source, &mut ranges);
    ranges.sort_by_key(|range| range.start);

    if ranges.is_empty() {
        return Segments {
            blocks: vec![source.to_string()],
            gaps: vec![String::new(), String::new()],
        };
    }

    let blocks = ranges
        .iter()
        .map(|range| source[range.clone()].to_string())
        .collect();
    let mut gaps = Vec::with_capacity(ranges.len() + 1);
    gaps.push(source[..ranges[0].start].to_string());
    for pair in ranges.windows(2) {
        gaps.push(source[pair[0].end..pair[1].start].to_string());
    }
    gaps.push(source[ranges.last().expect("there is a range").end..].to_string());
    Segments { blocks, gaps }
}

fn semantic_ranges(source: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;

    for (event, range) in Parser::new_ext(source, options()).into_offset_iter() {
        match event {
            Event::Start(_) => {
                if depth == 0 {
                    start = range.start;
                }
                depth += 1;
            }
            Event::End(_) => {
                depth -= 1;
                if depth == 0 {
                    ranges.push(start..range.end);
                }
            }
            _ if depth == 0 => ranges.push(range),
            _ => {}
        }
    }
    ranges
}

fn include_unparsed(source: &str, ranges: &mut Vec<Range<usize>>) {
    ranges.sort_by_key(|range| range.start);
    let mut uncovered = Vec::new();
    let mut end = 0;
    for range in ranges.iter() {
        add_non_whitespace(source, end..range.start, &mut uncovered);
        end = end.max(range.end);
    }
    add_non_whitespace(source, end..source.len(), &mut uncovered);
    ranges.extend(uncovered);
}

fn add_non_whitespace(source: &str, range: Range<usize>, ranges: &mut Vec<Range<usize>>) {
    let part = &source[range.clone()];
    let Some(first) = part.find(|character: char| !character.is_whitespace()) else {
        return;
    };
    let last = part
        .rfind(|character: char| !character.is_whitespace())
        .expect("a first non-whitespace character has a last");
    let last = last + part[last..].chars().next().expect("last points at a character").len_utf8();
    ranges.push(range.start + first..range.start + last);
}

/// What kind of block this is, as the QML view names them.
pub fn kind(block: &str) -> &'static str {
    use pulldown_cmark::Tag;

    if lone_image(block).is_some() {
        return "image";
    }
    // The first event names the block: everything after it is inside that block.
    match Parser::new_ext(block, options()).next() {
        Some(Event::Start(Tag::Heading { .. })) => "heading",
        Some(Event::Start(Tag::CodeBlock(_))) => "code",
        Some(Event::Start(Tag::BlockQuote(_))) => "quote",
        Some(Event::Start(Tag::Table(_))) => "table",
        Some(Event::Start(Tag::List(_))) => "list",
        Some(Event::Rule) => "rule",
        _ => "paragraph",
    }
}

/// The image path of a block that is a lone `![alt](path)` paragraph, if any.
pub fn lone_image(block: &str) -> Option<String> {
    use pulldown_cmark::{Tag, TagEnd};

    let mut path = None;
    let mut in_image = false;

    for event in Parser::new_ext(block, options()) {
        match event {
            Event::Start(Tag::Paragraph) | Event::End(TagEnd::Paragraph) => {}
            Event::Start(Tag::Image { dest_url, .. }) => {
                if path.is_some() {
                    return None;
                }
                path = Some(dest_url.to_string());
                in_image = true;
            }
            Event::End(TagEnd::Image) => in_image = false,
            // Alt text lives inside the image; anything outside it disqualifies the block.
            _ if in_image => {}
            Event::Text(text) if text.trim().is_empty() => {}
            _ => return None,
        }
    }

    path
}

/// The markdown to render for a block. Qt folds the newlines a writer typed into
/// spaces, so in the blocks where a newline is a line break, make them hard ones.
pub fn rendered(block: &str) -> String {
    if !matches!(kind(block), "paragraph" | "quote" | "list") {
        return block.to_string();
    }

    let mut out = String::with_capacity(block.len());
    let mut lines = block.lines().peekable();
    while let Some(line) = lines.next() {
        out.push_str(line);
        let Some(next) = lines.peek() else { break };
        // Two trailing spaces are markdown's hard break. A line that already breaks, or
        // that ends its paragraph anyway, needs none.
        if !(line.trim().is_empty() || next.trim().is_empty()
            || line.ends_with("  ") || line.ends_with('\\')) {
            out.push_str("  ");
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The blocks of `source`, which is what the model is built out of. An empty
    /// document is one empty block, so that there is always somewhere to type.
    fn segment(source: &str) -> Vec<String> {
        if source.trim().is_empty() {
            return vec![String::new()];
        }
        segments(source).blocks
    }

    /// The blocks put back together with one blank line between them: what the document
    /// would look like had it been written out afresh rather than kept as it was found.
    fn roundtrip(source: &str) -> String {
        segment(source).join("\n\n")
    }

    /// The document put back together byte for byte, which is what saving does.
    fn exact_roundtrip(source: &str) -> String {
        let Segments { blocks, gaps } = segments(source);
        let mut text = gaps[0].clone();
        for (index, block) in blocks.iter().enumerate() {
            text.push_str(block);
            text.push_str(&gaps[index + 1]);
        }
        text
    }

    #[test]
    fn splits_headings_and_paragraphs() {
        let blocks = segment("# Title\n\nA paragraph.\n\n## Sub\n\nAnother one.\n");
        assert_eq!(blocks, ["# Title", "A paragraph.", "## Sub", "Another one."]);
    }

    #[test]
    fn keeps_a_list_whole() {
        let source = "Intro\n\n- one\n- two\n  - nested\n- three\n\nOutro";
        let blocks = segment(source);
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[1], "- one\n- two\n  - nested\n- three");
        assert_eq!(roundtrip(source), source);
    }

    #[test]
    fn keeps_a_fenced_code_block_whole() {
        let source = "```rust\nfn main() {\n\n    println!(\"hi\");\n}\n```";
        assert_eq!(segment(source), [source]);
        assert_eq!(roundtrip(source), source);
    }

    #[test]
    fn keeps_a_blockquote_whole() {
        let source = "> quoted\n> lines\n\nafter";
        assert_eq!(segment(source), ["> quoted\n> lines", "after"]);
    }

    #[test]
    fn preserves_setext_headings() {
        let source = "Title\n=====\n\nbody";
        assert_eq!(segment(source), ["Title\n=====", "body"]);
    }

    #[test]
    fn handles_rules_and_tables() {
        let source = "a\n\n---\n\n| x | y |\n| - | - |\n| 1 | 2 |\n\nb";
        let blocks = segment(source);
        assert_eq!(blocks.len(), 4);
        assert_eq!(blocks[1], "---");
        assert_eq!(roundtrip(source), source);
    }

    #[test]
    fn semantic_join_normalizes_blank_lines() {
        assert_eq!(roundtrip("a\n\n\n\nb"), "a\n\nb");
    }

    #[test]
    fn exact_segments_preserve_all_whitespace() {
        for source in ["a\n\n\n\nb", "  a  \n\n b\n", "", "   \n\n "] {
            assert_eq!(exact_roundtrip(source), source);
        }
    }

    #[test]
    fn exact_segments_keep_html_and_reference_definitions() {
        for source in [
            "<section>\nraw html\n</section>\n",
            "[home]: https://example.com\n\nGo [home].\n",
        ] {
            assert_eq!(exact_roundtrip(source), source);
            assert!(segments(source).blocks.iter().any(|block| !block.is_empty()));
        }
    }

    #[test]
    fn empty_source_is_one_empty_block() {
        assert_eq!(segment(""), [""]);
        assert_eq!(segment("   \n\n "), [""]);
    }

    #[test]
    fn names_block_kinds() {
        assert_eq!(kind("# Title"), "heading");
        assert_eq!(kind("Title\n====="), "heading");
        assert_eq!(kind("Just words."), "paragraph");
        assert_eq!(kind("- one\n- two"), "list");
        assert_eq!(kind("1. one"), "list");
        assert_eq!(kind("```rust\nfn main() {}\n```"), "code");
        assert_eq!(kind("> quoted"), "quote");
        assert_eq!(kind("| a | b |\n| - | - |\n| 1 | 2 |"), "table");
        assert_eq!(kind("---"), "rule");
        assert_eq!(kind("![alt](pic.png)"), "image");
        assert_eq!(kind(""), "paragraph");
    }

    #[test]
    fn detects_lone_images() {
        assert_eq!(lone_image("![alt](pic.png)"), Some("pic.png".into()));
        assert_eq!(lone_image("text ![alt](pic.png) more"), None);
        assert_eq!(lone_image("# heading"), None);
        assert_eq!(lone_image("![a](1.png) ![b](2.png)"), None);
    }

    #[test]
    fn hardens_the_line_breaks_a_writer_typed() {
        assert_eq!(rendered("TITLE: t\nAUTHOR: a"), "TITLE: t  \nAUTHOR: a");
        assert_eq!(rendered("> one\n> two"), "> one  \n> two");
        assert_eq!(rendered("- one\n- two"), "- one  \n- two");
        assert_eq!(rendered("alone"), "alone");
    }

    #[test]
    fn leaves_breaks_that_are_already_there() {
        assert_eq!(rendered("one  \ntwo\\\nthree"), "one  \ntwo\\\nthree");
        assert_eq!(rendered("- one\n\n- two"), "- one\n\n- two");
    }

    #[test]
    fn leaves_blocks_whose_newlines_are_structure() {
        for block in ["```\ncode\n  here\n```", "| a |\n| - |\n| 1 |", "Title\n====="] {
            assert_eq!(rendered(block), block);
        }
    }
}
