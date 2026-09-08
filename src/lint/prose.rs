//! What the checker makes of one block: harper's rules over the prose, the dictionary
//! over the words, and the parts of the markdown neither has any business in taken back
//! out again. Nothing here remembers anything — [`super`] does the remembering, and calls
//! this when it has nothing to hand.

use super::Lint;
use crate::parse;
use crate::spell;
use crate::text::{utf16_at, utf16_offsets};
use harper_core::linting::{LintGroup, LintKind, Linter, Suggestion};
use harper_core::{Document, TokenKind};
use pulldown_cmark::{Event, Parser, Tag};
use std::ops::Range;
use std::sync::Mutex;

/// Punctuation that only ever joins one thing to another. A word with one of these hard
/// against it is part of `snake_case`, a path or an address rather than a piece of prose.
/// The full stop that ends a sentence joins nothing to anything, and the word in front of
/// it is checked like any other.
const JOINS: [char; 5] = ['.', '/', ':', '@', '_'];

pub(super) fn run(group: &Mutex<LintGroup>, text: &str, markdown: bool) -> Vec<Lint> {
    let document = if markdown {
        Document::new_markdown_default_curated(text)
    } else {
        Document::new_plain_english_curated(text)
    };
    let offsets = utf16_offsets(text);
    let characters: Vec<char> = text.chars().collect();

    let mut found: Vec<Lint> = group
        .lock()
        .unwrap()
        .lint(&document)
        .into_iter()
        // Spelling is handled below against Harper's built-in dictionary plus the
        // writer's own words; the group's spelling rules would mark the same text twice.
        .filter(|lint| !matches!(lint.lint_kind, LintKind::Spelling))
        .filter_map(|lint| carry(lint, &offsets, &characters))
        .collect();
    found.extend(misspellings(&document, &offsets, &characters));
    if markdown {
        let left_alone = left_alone(text);
        found.retain(|lint| !left_alone.iter().any(|part| overlaps(lint, part)));
    }
    found.sort_by_key(|lint| (lint.at, lint.len));
    found
}

/// The parts of a block the checker has no business in, in the UTF-16 units a lint counts
/// in: code, links and tables. None of it is prose — it is a name, an address, a column of
/// figures — and a writer who has to spell it that way cannot take the advice anyway.
fn left_alone(text: &str) -> Vec<Range<u32>> {
    Parser::new_ext(text, parse::options())
        .into_offset_iter()
        .filter(|(event, _)| {
            matches!(
                event,
                Event::Code(_)
                    | Event::Start(Tag::CodeBlock(_) | Tag::Link { .. } | Tag::Table(_))
            )
        })
        .map(|(_, bytes)| utf16_at(text, bytes.start)..utf16_at(text, bytes.end))
        .collect()
}

/// Whether a lint has any of itself inside `part`.
fn overlaps(lint: &Lint, part: &Range<u32>) -> bool {
    lint.at < part.end && part.start < lint.at + lint.len
}

/// The words of a block the dictionary does not know. Which of the text is prose, harper
/// has already worked out: an address is a token of its own and not a word, so the
/// question is never asked about it. What is left over, [`left_alone`] takes out.
fn misspellings(document: &Document, offsets: &[u32], characters: &[char]) -> Vec<Lint> {
    document
        .get_tokens()
        .iter()
        .filter(|token| matches!(token.kind, TokenKind::Word(_)))
        .map(|token| token.span.start..token.span.end)
        .filter(|span| checkable(span.clone(), characters))
        .filter_map(|span| {
            let word: String = characters[span.clone()].iter().collect();
            if spell::known(&word) {
                return None;
            }
            let at = *offsets.get(span.start)?;
            let end = *offsets.get(span.end)?;
            Some(Lint {
                at,
                len: end - at,
                message: format!("`{word}` is not in the dictionary."),
                replacements: Vec::new(),
                word,
                pending: true,
            })
        })
        .collect()
}

/// Whether a word harper found is one to ask the dictionary about. A single letter is
/// never worth flagging, and anything with a digit in it is a name for something rather
/// than a word: `h1`, `3rd`, `utf8`. What sits either side of it counts too — see [`JOINS`].
fn checkable(span: Range<usize>, characters: &[char]) -> bool {
    let word = &characters[span.clone()];
    if word.len() < 2 || word.iter().any(|c| c.is_numeric() || JOINS.contains(c)) {
        return false;
    }
    let before = span.start.checked_sub(1).and_then(|i| characters.get(i));
    if before.is_some_and(|c| JOINS.contains(c)) {
        return false;
    }
    let after = characters.get(span.end);
    let beyond = characters.get(span.end + 1);
    !(after.is_some_and(|c| JOINS.contains(c)) && beyond.is_some_and(|c| c.is_alphanumeric()))
}

/// A harper lint in the terms the editor works in: UTF-16 offsets, and for each
/// suggestion the one piece of text that should stand where the lint is, whichever shape
/// the suggestion took. Taking the words out is a piece of text like any other — an
/// empty one.
fn carry(lint: harper_core::linting::Lint, offsets: &[u32], characters: &[char]) -> Option<Lint> {
    let at = *offsets.get(lint.span.start)?;
    let end = *offsets.get(lint.span.end)?;
    let marked = characters.get(lint.span.start..lint.span.end)?;
    let replacements = lint
        .suggestions
        .iter()
        .map(|suggestion| match suggestion {
            Suggestion::ReplaceWith(with) => with.iter().collect(),
            Suggestion::InsertAfter(after) => marked.iter().chain(after.iter()).collect(),
            Suggestion::Remove => String::new(),
        })
        .collect();
    Some(Lint {
        at,
        len: end - at,
        message: lint.message,
        replacements,
        word: String::new(),
        pending: false,
    })
}
