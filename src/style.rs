//! What each character of a block being edited is part of, worked out from the same
//! markdown parser that splits the document into blocks. The highlighter that draws it
//! knows nothing about markdown: it is handed one number per character and paints it.

use crate::parse;
use crate::text;
use pulldown_cmark::{Event, Parser, Tag};
use std::ops::Range;

#[cxx::bridge(namespace = "blogawrite")]
mod ffi {
    /// What a character is part of. One stretch of prose can be several of these at once
    /// — a word inside a link inside a bold run — so they are bits.
    #[repr(u16)]
    enum StyleBit {
        Bold = 1,
        Italic = 2,
        Code = 4,
        Strike = 8,
        Link = 16,
        /// A syntax marker with the cursor beside it: left legible, and muted.
        Marker = 32,
        /// A syntax marker away from the cursor: squeezed away to nothing.
        Hidden = 64,
        /// Not prose — code, a link, an address. The checker's marks keep off it.
        Unchecked = 16384,
    }

    extern "Rust" {
        /// One set of [`StyleBit`]s per UTF-16 unit of `text`, which is how a QString
        /// counts. `cursor` is where the cursor stands in those same units, or -1 for a
        /// block that does not hold it; `code` says this is a fenced block, where the
        /// only markup there is are the fences.
        fn style_mask(text: &str, cursor: i32, code: bool) -> Vec<u16>;
    }
}

use ffi::StyleBit;

const BOLD: u16 = StyleBit::Bold.repr;
const ITALIC: u16 = StyleBit::Italic.repr;
const CODE: u16 = StyleBit::Code.repr;
const STRIKE: u16 = StyleBit::Strike.repr;
const LINK: u16 = StyleBit::Link.repr;
const MARKER: u16 = StyleBit::Marker.repr;
const HIDDEN: u16 = StyleBit::Hidden.repr;
const UNCHECKED: u16 = StyleBit::Unchecked.repr;

/// What delimits a span, and so what of it is a marker rather than the thing itself.
#[derive(Clone, Copy)]
enum Delimiters {
    /// Nothing. A paragraph is made of its contents and no more.
    None,
    /// A marker in front and nothing behind — a bullet, a heading's hashes. It stays
    /// legible wherever the cursor is: it is how the line reads, not punctuation.
    Ahead,
    /// A marker either side: emphasis, a link. These shrink away unless the cursor is
    /// inside the span they belong to.
    Around,
}

struct Frame {
    range: Range<usize>,
    bits: u16,
    delimiters: Delimiters,
    /// Where this span's contents begin and end, as its children report themselves. What
    /// is left over at either edge is the marker.
    content: Option<Range<usize>>,
}

pub fn style_mask(text: &str, cursor: i32, code: bool) -> Vec<u16> {
    // Marked per byte, which is what the parser counts in, and counted out per UTF-16
    // unit at the end. Every range a parser hands back falls on a character boundary.
    let mut bytes = vec![0u16; text.len()];
    let cursor = (cursor >= 0).then(|| text::byte_offset(text, cursor));
    if code {
        mark_fences(text, cursor, &mut bytes);
    } else {
        mark_prose(text, cursor, &mut bytes);
    }

    let mut mask = Vec::with_capacity(text.len());
    for (offset, character) in text.char_indices() {
        for _ in 0..character.len_utf16() {
            mask.push(bytes[offset]);
        }
    }
    mask
}

fn mark(bytes: &mut [u16], range: Range<usize>, bits: u16) {
    let end = range.end.min(bytes.len());
    for byte in &mut bytes[range.start.min(end)..end] {
        *byte |= bits;
    }
}

/// How a marker belonging to `span` is drawn: legible while the cursor is inside the span,
/// squeezed away to nothing the moment it leaves.
fn marker_bits(cursor: Option<usize>, span: &Range<usize>) -> u16 {
    match cursor {
        Some(at) if span.contains(&at) || at == span.end => MARKER,
        _ => HIDDEN,
    }
}

fn mark_prose(text: &str, cursor: Option<usize>, bytes: &mut [u16]) {
    let mut stack: Vec<Frame> = Vec::new();

    for (event, range) in Parser::new_ext(text, parse::options()).into_offset_iter() {
        // A span closing is that span reported a second time, and is nobody's child.
        if matches!(event, Event::End(_)) {
            let Some(frame) = stack.pop() else { continue };
            mark(bytes, frame.range.clone(), inherited(&stack) | frame.bits);
            mark_delimiters(&frame, cursor, bytes);
            continue;
        }
        note_child(&mut stack, &range);

        match event {
            Event::Start(tag) => {
                let (bits, delimiters) = shape(&tag);
                stack.push(Frame { range, bits, delimiters, content: None });
            }
            // Inline code comes whole, contents and backticks together, so its markers
            // are the runs of backticks at either end.
            Event::Code(_) => {
                mark(bytes, range.clone(), inherited(&stack) | CODE | UNCHECKED);
                let source = text[range.clone()].as_bytes();
                let open = source.iter().take_while(|byte| **byte == b'`').count();
                let close = source.iter().rev().take_while(|byte| **byte == b'`').count();
                if open + close < source.len() {
                    let bits = marker_bits(cursor, &range);
                    mark(bytes, range.start..range.start + open, bits);
                    mark(bytes, range.end - close..range.end, bits);
                }
            }
            // A checkbox says what the line is; it is not punctuation to be typed over.
            Event::TaskListMarker(_) => mark(bytes, range, inherited(&stack) | MARKER),
            _ => mark(bytes, range, inherited(&stack)),
        }
    }
}

/// Take `range` into the contents of the span it sits inside, so that what is left of that
/// span at either edge is its markers.
fn note_child(stack: &mut Vec<Frame>, range: &Range<usize>) {
    let Some(frame) = stack.last_mut() else { return };
    match &mut frame.content {
        Some(content) => {
            content.start = content.start.min(range.start);
            content.end = content.end.max(range.end);
        }
        content => *content = Some(range.clone()),
    }
}

fn mark_delimiters(frame: &Frame, cursor: Option<usize>, bytes: &mut [u16]) {
    let Some(content) = &frame.content else { return };
    match frame.delimiters {
        Delimiters::None => {}
        Delimiters::Ahead => mark(bytes, frame.range.start..content.start, MARKER),
        Delimiters::Around => {
            let bits = marker_bits(cursor, &frame.range);
            mark(bytes, frame.range.start..content.start, bits);
            mark(bytes, content.end..frame.range.end, bits);
        }
    }
}

fn inherited(stack: &[Frame]) -> u16 {
    stack.iter().fold(0, |bits, frame| bits | frame.bits)
}

fn shape(tag: &Tag) -> (u16, Delimiters) {
    match tag {
        Tag::Strong => (BOLD, Delimiters::Around),
        Tag::Emphasis => (ITALIC, Delimiters::Around),
        Tag::Strikethrough => (STRIKE, Delimiters::Around),
        // A link is an address as much as it is words: nothing in it is spelled wrong.
        Tag::Link { .. } | Tag::Image { .. } => (LINK | UNCHECKED, Delimiters::Around),
        Tag::Heading { .. } | Tag::Item => (0, Delimiters::Ahead),
        _ => (0, Delimiters::None),
    }
}

/// Inside a fenced block the fences are the only markup there is, and they open up as soon
/// as the cursor reaches either end of the block.
fn mark_fences(text: &str, cursor: Option<usize>, bytes: &mut [u16]) {
    let lines = line_ranges(text);
    let (Some(first), Some(last)) = (lines.first(), lines.last()) else {
        return;
    };
    let holds = |line: &Range<usize>, at: usize| line.contains(&at) || at == line.end;
    let open = cursor.is_some_and(|at| holds(first, at) || holds(last, at));
    let bits = if open { MARKER } else { HIDDEN };

    for line in [first.clone(), last.clone()] {
        let fence = text[line.clone()].trim_start();
        if fence.starts_with("```") || fence.starts_with("~~~") {
            mark(bytes, line, bits);
        }
    }
}

/// Each line of `text`, without the newline that ends it — the spans a QTextDocument
/// divides itself into.
fn line_ranges(text: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = 0;
    for (offset, byte) in text.bytes().enumerate() {
        if byte == b'\n' {
            ranges.push(start..offset);
            start = offset + 1;
        }
    }
    ranges.push(start..text.len());
    ranges
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The mask as one letter per UTF-16 unit: what the highlighter would draw there.
    fn picture(text: &str, cursor: i32) -> String {
        drawing(&style_mask(text, cursor, false))
    }

    fn drawing(mask: &[u16]) -> String {
        mask.iter()
            .map(|bits| match bits {
                _ if bits & HIDDEN != 0 => 'h',
                _ if bits & MARKER != 0 => 'm',
                _ if bits & CODE != 0 => 'c',
                _ if bits & LINK != 0 => 'l',
                _ if bits & (BOLD | ITALIC) == BOLD | ITALIC => '3',
                _ if bits & BOLD != 0 => 'b',
                _ if bits & ITALIC != 0 => 'i',
                _ if bits & STRIKE != 0 => 's',
                _ => '.',
            })
            .collect()
    }

    #[test]
    fn shrinks_the_markers_away_and_brings_them_back_under_the_cursor() {
        assert_eq!(picture("**bold**", -1), "hhbbbbhh");
        assert_eq!(picture("**bold**", 3), "mmbbbbmm");
        assert_eq!(picture("a ~~gone~~ b", -1), "..hhsssshh..");
    }

    #[test]
    fn nests_emphasis_the_way_markdown_does() {
        assert_eq!(picture("**a *b* c**", -1), "hhbbh3hbbhh");
    }

    #[test]
    fn leaves_an_underscore_inside_a_word_alone() {
        assert_eq!(picture("snake_case_name", -1), ".".repeat(15));
    }

    #[test]
    fn marks_code_and_keeps_the_checker_off_it() {
        assert_eq!(picture("`x y`", -1), "hccch");
        assert_eq!(picture("`x y`", 2), "mcccm");
        let mask = style_mask("`x`", -1, false);
        assert!(mask.iter().all(|bits| bits & UNCHECKED != 0));
    }

    #[test]
    fn marks_a_link_whole_and_keeps_the_checker_off_it() {
        assert_eq!(picture("[a](b)", -1), "hlhhhh");
        assert_eq!(picture("[a](b)", 1), "mlmmmm");
        let mask = style_mask("[a](b)", -1, false);
        assert!(mask.iter().all(|bits| bits & UNCHECKED != 0));
    }

    /// A bullet is how the line reads, not punctuation: it stays legible either way.
    #[test]
    fn keeps_a_list_marker_in_view() {
        assert_eq!(picture("- one", -1), "mm...");
        assert_eq!(picture("1. one", -1), "mmm...");
        assert_eq!(picture("- one\n- two", -1), "mm....mm...");
    }

    #[test]
    fn counts_positions_the_way_qt_does() {
        // The emoji is two UTF-16 units, so the mask has an entry for each of them.
        assert_eq!(picture("🙂 **b**", -1), "...hhbhh");
        assert_eq!(picture("🙂 **b**", 4), "...mmbmm");
    }

    #[test]
    fn opens_a_fenced_block_only_at_its_ends() {
        let block = "```rs\ncode\n```";
        assert_eq!(drawing(&style_mask(block, -1, true)), "hhhhh......hhh");
        assert_eq!(drawing(&style_mask(block, 0, true)), "mmmmm......mmm");
        assert_eq!(drawing(&style_mask(block, 14, true)), "mmmmm......mmm");
        // The cursor down in the code itself leaves the fences shut.
        assert_eq!(drawing(&style_mask(block, 8, true)), "hhhhh......hhh");
    }
}
