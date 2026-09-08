//! What the writer can take back. A run of typing is one thing to undo rather than one
//! thing per letter, so the newest state is added to while the typing goes on and closed
//! the moment anything else happens.

use super::qobject::Document;
use super::{UndoState, UNDO_LIMIT};
use cxx_qt::CxxQtType;
use std::pin::Pin;

impl Document {
    fn snapshot(&self, cursor_position: i32) -> UndoState {
        UndoState {
            blocks: self.blocks.clone(),
            gaps: self.gaps.clone(),
            active_index: *self.active_index(),
            selection_anchor: *self.selection_anchor(),
            selection_position: *self.selection_position(),
            cursor_position,
            revision: self.revision,
        }
    }

    /// An edit that stands on its own — a block split, a merge, a selection deleted.
    /// The whole of it is one thing to undo.
    pub(super) fn push_undo(mut self: Pin<&mut Self>, cursor_position: i32) {
        self.as_mut().rust_mut().typing = false;
        self.record_undo(cursor_position);
    }

    /// A keystroke. A run of them is one thing to undo rather than one per letter: the
    /// writer means a word, not the letters of it. The run stays open until the typing
    /// stops — the editor says when — or until anything that is not typing happens.
    pub(super) fn push_typing(mut self: Pin<&mut Self>, cursor_position: i32) {
        let open = self.typing;
        self.as_mut().rust_mut().typing = true;
        if open {
            self.record_cursor(cursor_position);
            return;
        }
        self.record_undo(cursor_position);
    }

    /// What the checker had to say about the block a keystroke ago is about text that is
    /// no longer there, so it is taken down; the editor asks again once the typing stops.
    pub(super) fn record_undo(mut self: Pin<&mut Self>, cursor_position: i32) {
        self.as_mut().rust_mut().cursor_position = cursor_position;
        self.as_mut().rust_mut().revision += 1;
        self.as_mut().set_dirty(true);
        let state = self.as_mut().snapshot(cursor_position);
        let mut rust = self.as_mut().rust_mut();
        rust.undo.push_back(state);
        if rust.undo.len() > UNDO_LIMIT {
            rust.undo.pop_front();
        }
        rust.settled = false;
        self.update_lint(cursor_position);
    }

    /// Something other than typing happened where the cursor is: the run it was in is
    /// over, and the newest state is brought up to date rather than added to.
    pub(super) fn refresh_undo(mut self: Pin<&mut Self>, cursor_position: i32) {
        self.as_mut().rust_mut().typing = false;
        self.record_cursor(cursor_position);
    }

    /// Bring the newest state up to date without ending a run of typing. A keystroke
    /// moves the cursor, and the cursor moving is not something else happening.
    pub(super) fn record_cursor(mut self: Pin<&mut Self>, cursor_position: i32) {
        self.as_mut().rust_mut().cursor_position = cursor_position;
        let state = self.snapshot(cursor_position);
        let undo = &mut self.as_mut().rust_mut().undo;
        if let Some(last) = undo.back_mut() {
            *last = state;
        } else {
            undo.push_back(state);
        }
        self.update_lint(cursor_position);
    }

    pub(super) fn set_cursor_position(mut self: Pin<&mut Self>, index: i32, position: i32) {
        if index == *self.active_index() {
            self.as_mut().record_cursor(position);
        }
    }

    pub(super) fn undo(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().typing = false;
        let state = {
            let undo = &mut self.as_mut().rust_mut().undo;
            if undo.len() < 2 {
                return;
            }
            undo.pop_back();
            undo.back().expect("undo history has an initial snapshot").clone()
        };
        // Delegates are made while the model reset is processed, so put their initial
        // cursor state in place first. Setting it afterwards leaves a new editor at zero.
        self.as_mut().set_selection_anchor(state.selection_anchor);
        self.as_mut().set_selection_position(state.selection_position);
        self.as_mut().set_pending_cursor(state.cursor_position);
        self.as_mut().set_active_index(state.active_index);
        self.as_mut().begin_reset_model();
        {
            let mut rust = self.as_mut().rust_mut();
            rust.blocks = state.blocks;
            rust.gaps = state.gaps;
            rust.revision = state.revision;
            rust.cursor_position = state.cursor_position;
        }
        self.as_mut().end_reset_model();
        let dirty = state.revision != self.saved_revision;
        self.as_mut().set_dirty(dirty);
        self.update_lint(state.cursor_position);
    }
}
