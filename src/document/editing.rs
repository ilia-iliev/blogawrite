//! Moving the cursor about the document, the selection that follows it, and the rows the
//! view is shown: a block re-read into several, two blocks joined back into one.

use super::qobject::Document;
use crate::blocks::{self, Span};
use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use std::pin::Pin;
use std::sync::Arc;

use super::TEXT_ROLE;

impl Document {
    pub(super) fn activate(self: Pin<&mut Self>, target: i32) {
        self.activate_at(target, -1);
    }

    pub(super) fn move_to(self: Pin<&mut Self>, target: i32, direction: i32) {
        self.activate_at(target, if direction > 0 { 0 } else { -1 });
    }

    pub(super) fn activate_at(mut self: Pin<&mut Self>, target: i32, cursor: i32) {
        self.as_mut().set_selection_anchor(-1);
        let previous = *self.active_index();
        let mut target = target;
        if previous != target && previous >= 0 {
            let delta = self.as_mut().commit(previous);
            if previous < target {
                target += delta;
            }
        }
        // There is always a cursor somewhere: the ends of the document just hold it.
        let last = self.blocks.len() as i32 - 1;
        self.as_mut().set_pending_cursor(cursor);
        self.as_mut().set_active_index(target.clamp(0, last));
        self.as_mut().refresh_undo(cursor);
    }

    pub(super) fn select_to(mut self: Pin<&mut Self>, target: i32, anchor_position: i32) {
        let previous = *self.active_index();
        if target < 0 || target >= self.blocks.len() as i32 || target == previous {
            return;
        }
        if *self.selection_anchor() < 0 {
            self.as_mut().set_selection_anchor(previous);
            self.as_mut().set_selection_position(anchor_position);
        }
        // Nothing re-parses under a selection: the block being left keeps its shape, so
        // the rows the selection covers stay where they were while it grows.
        //
        // The cursor enters the block it moves into by the near edge, so that one press
        // of an arrow takes in one line rather than the whole block.
        let cursor = if target > previous { 0 } else { -1 };
        self.as_mut().set_pending_cursor(cursor);
        self.as_mut().set_active_index(target);
        self.as_mut().refresh_undo(cursor);
    }

    pub(super) fn select_all(mut self: Pin<&mut Self>) {
        // Pinned at the top of the document, with the cursor carried to the end of the
        // last block — the -1 the editor reads as "the far edge of the block".
        self.as_mut().set_selection_anchor(0);
        self.as_mut().set_selection_position(0);
        self.as_mut().set_pending_cursor(-1);
        let last = self.blocks.len() as i32 - 1;
        self.as_mut().set_active_index(last);
        self.as_mut().refresh_undo(-1);
    }

    pub(super) fn clear_selection(mut self: Pin<&mut Self>, cursor_position: i32) {
        self.as_mut().set_selection_anchor(-1);
        self.as_mut().refresh_undo(cursor_position);
    }

    pub(super) fn selection_text(&self, cursor_position: i32) -> QString {
        let Some(span) = self.selected_span(cursor_position) else {
            return QString::default();
        };
        QString::from(&blocks::selected_text(&self.blocks, &self.gaps, span))
    }

    pub(super) fn delete_selection(mut self: Pin<&mut Self>, cursor_position: i32, insert: &QString) {
        let Some(span) = self.selected_span(cursor_position) else {
            return;
        };
        let (kept, cursor) = blocks::spliced(&self.blocks, span, &insert.to_string());

        // The cursor and the text land before the rows go, so that the block that keeps
        // them is asked for them once, in one piece.
        self.as_mut().set_selection_anchor(-1);
        self.as_mut().set_pending_cursor(cursor);
        self.as_mut().rust_mut().blocks[span.first] = Arc::new(kept);
        self.as_mut().notify_changed(span.first as i32, false);
        if span.last > span.first {
            let trailing = self.gaps[span.last + 1].clone();
            self.as_mut().rust_mut().gaps[span.first + 1] = trailing;
            let count = (span.last - span.first) as i32;
            self.as_mut().remove_rows(span.first as i32 + 1, count);
        }
        self.as_mut().set_active_index(span.first as i32);
        self.as_mut().push_undo(cursor);
    }

    /// Where the selection runs, if one does. `cursor_position` is the cursor's own
    /// offset within its block, which only the editor knows.
    pub(super) fn selected_span(&self, cursor_position: i32) -> Option<Span> {
        let anchor = *self.selection_anchor();
        if anchor < 0 {
            return None;
        }
        blocks::span(
            &self.blocks,
            anchor,
            *self.selection_position(),
            *self.active_index(),
            cursor_position,
        )
    }

    /// Re-segment block `index` now that editing is done. Returns the change in row count.
    pub(super) fn commit(mut self: Pin<&mut Self>, index: i32) -> i32 {
        let Some(block) = self.blocks.get(index as usize) else {
            return 0;
        };

        // A block emptied out disappears, unless it is all that is left.
        if block.trim().is_empty() {
            if self.blocks.len() == 1 {
                return 0;
            }
            self.as_mut().remove_rows(index, 1);
            return -1;
        }

        let (replacement, separators) = blocks::replacement(block);
        if replacement.len() == 1 && replacement[0] == **block {
            // Typing invalidates only the raw role. Refresh the derived roles now that
            // this block is about to be rendered again.
            self.as_mut().notify_changed(index, false);
            return 0;
        }
        self.replace_block(index, replacement, separators)
    }

    /// Swap block `index` for the blocks it re-parsed into. Returns the change in row count.
    pub(super) fn replace_block(
        mut self: Pin<&mut Self>,
        index: i32,
        replacement: Vec<String>,
        separators: Vec<String>,
    ) -> i32 {
        let delta = replacement.len() as i32 - 1;
        let mut replacement = replacement.into_iter();
        let head = replacement.next().unwrap_or_default();

        self.as_mut().rust_mut().blocks[index as usize] = Arc::new(head);
        self.as_mut().notify_changed(index, false);

        let tail: Vec<String> = replacement.collect();
        if !tail.is_empty() {
            self.insert_rows(index + 1, tail, separators);
        }
        delta
    }

    pub(super) fn remove_rows(mut self: Pin<&mut Self>, first: i32, count: i32) {
        let parent = cxx_qt_lib::QModelIndex::default();
        self.as_mut()
            .begin_remove_rows(&parent, first, first + count - 1);
        let range = first as usize..(first + count) as usize;
        let mut rust = self.as_mut().rust_mut();
        rust.blocks.drain(range);
        // Keep the separator before the removed span and discard the rest of the source
        // occupied by it. Callers may replace that kept separator first.
        rust.gaps
            .drain(first as usize + 1..(first + count) as usize + 1);
        self.end_remove_rows();
    }

    pub(super) fn insert_rows(
        mut self: Pin<&mut Self>,
        first: i32,
        blocks: Vec<String>,
        separators: Vec<String>,
    ) {
        let parent = cxx_qt_lib::QModelIndex::default();
        self.as_mut()
            .begin_insert_rows(&parent, first, first + blocks.len() as i32 - 1);
        let mut rust = self.as_mut().rust_mut();
        for (offset, block) in blocks.into_iter().enumerate() {
            rust.blocks.insert(first as usize + offset, Arc::new(block));
        }
        for (offset, separator) in separators.into_iter().enumerate() {
            rust.gaps
                .insert(first as usize + offset, Arc::new(separator));
        }
        self.end_insert_rows();
    }

    pub(super) fn set_block_text(
        mut self: Pin<&mut Self>,
        index: i32,
        text: &QString,
        cursor_position: i32,
    ) {
        let text = text.to_string();
        if self.blocks.get(index as usize).is_none_or(|block| block.as_str() == text) {
            return;
        }
        self.as_mut().rust_mut().blocks[index as usize] = Arc::new(text);
        // The view may recycle this delegate mid-edit, so keep the model authoritative.
        // Only the raw role changed. Invalidating every role here made each keystroke parse
        // the block several times for data the active delegate does not use.
        self.as_mut().notify_changed(index, true);
        self.as_mut().push_typing(cursor_position);
    }

    pub(super) fn notify_changed(mut self: Pin<&mut Self>, index: i32, text_only: bool) {
        let parent = cxx_qt_lib::QModelIndex::default();
        let cell = self.as_mut().index(index, 0, &parent);
        let mut roles = cxx_qt_lib::QVector::<i32>::default();
        if text_only {
            roles.append(TEXT_ROLE);
        }
        self.data_changed(&cell, &cell, &roles);
    }

    pub(super) fn split_block(mut self: Pin<&mut Self>, index: i32, before: &QString, after: &QString) {
        if index < 0 || index as usize >= self.blocks.len() {
            return;
        }
        let (mut split, mut separators) = blocks::replacement(&before.to_string());
        let head = split.len() as i32;
        let (after_blocks, after_separators) = blocks::replacement(&after.to_string());
        separators.push("\n\n".to_string());
        separators.extend(after_separators);
        split.extend(after_blocks);

        self.as_mut().replace_block(index, split, separators);
        self.as_mut().set_pending_cursor(0);
        self.as_mut().set_active_index(index + head);
        self.as_mut().push_undo(0);
    }

    pub(super) fn merge_with_previous(mut self: Pin<&mut Self>, index: i32) {
        if index < 1 || index as usize >= self.blocks.len() {
            return;
        }
        let tail = self.blocks[index as usize].clone();
        let previous = index - 1;
        let cursor = self.blocks[previous as usize].encode_utf16().count() as i32;

        Arc::make_mut(&mut self.as_mut().rust_mut().blocks[previous as usize]).push_str(&tail);
        let trailing = self.gaps[index as usize + 1].clone();
        self.as_mut().rust_mut().gaps[index as usize] = trailing;
        self.as_mut().notify_changed(previous, false);
        self.as_mut().remove_rows(index, 1);
        self.as_mut().set_pending_cursor(cursor);
        self.as_mut().set_active_index(previous);
        self.as_mut().push_undo(cursor);
    }
}
