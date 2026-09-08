//! What the checker makes of where the cursor is standing, for the foot of the window,
//! and the two switches that quieten it: the writer's own, and reading the post through.

use super::qobject::Document;
use crate::lint;
use crate::search;
use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use std::pin::Pin;

impl Document {
    pub(super) fn settle(mut self: Pin<&mut Self>, settled: bool) {
        if self.settled == settled {
            return;
        }
        {
            let mut rust = self.as_mut().rust_mut();
            rust.settled = settled;
            // The pause that lets the checker speak is also what ends a run of typing.
            rust.typing = rust.typing && !settled;
        }
        self.refresh_lint();
    }

    pub(super) fn refresh_lint(self: Pin<&mut Self>) {
        let cursor = self.cursor_position;
        self.update_lint(cursor);
    }

    /// What the checker makes of the cursor's surroundings, for the foot of the window:
    /// a wash says something is wrong there, not what.
    ///
    /// A negative position is the editor being told to put the cursor at the far edge of
    /// a block rather than the editor saying where it ended up, and activating the block
    /// the cursor is already in sends one of those after the real position. So it is
    /// taken as no news: within a block the message stands until the editor reports again,
    /// and only a move to another block clears it.
    pub(super) fn update_lint(mut self: Pin<&mut Self>, cursor_position: i32) {
        let index = *self.active_index();
        if cursor_position < 0 && index == self.lint_block {
            return;
        }
        let found = self
            .blocks
            .get(index as usize)
            .filter(|_| cursor_position >= 0 && self.settled)
            .and_then(|block| lint::at(block, cursor_position));
        let (message, at, length, replacements, word) = match found {
            Some(lint) => (
                lint.message,
                lint.at as i32,
                lint.len as i32,
                lint.replacements,
                lint.word,
            ),
            None => (String::new(), -1, 0, Vec::new(), String::new()),
        };
        self.as_mut().rust_mut().lint_block = index;
        self.as_mut().set_lint_message(QString::from(&message));
        self.as_mut().set_lint_word(QString::from(&word));
        // A lint with nothing to suggest has nothing to accept either, and says so by
        // having no span to put anything in.
        let at = if replacements.is_empty() { -1 } else { at };
        self.as_mut().set_lint_at(at);
        self.as_mut().set_lint_length(length);
        self.as_mut().set_lint_options(replacements.len() as i32);
        self.as_mut().rust_mut().lint_replacements = replacements;
        self.show_suggestion(0);
    }

    /// Put suggestion `choice` on show. Out of range — which is every choice when there
    /// are none — leaves nothing to show, and the foot of the window says only what is wrong.
    pub(super) fn show_suggestion(mut self: Pin<&mut Self>, choice: i32) {
        let shown = self
            .lint_replacements
            .get(choice as usize)
            .cloned()
            .unwrap_or_default();
        self.as_mut().set_lint_choice(choice);
        self.set_lint_replacement(QString::from(&shown));
    }

    /// Re-read the block under the cursor now, rather than when the cursor next leaves
    /// it: what stands in its place is then what has just been typed rather than what was
    /// last parsed. Re-segmenting can leave more rows than it found, so the cursor is put
    /// back inside whatever is there afterwards.
    ///
    /// Both the ways out of writing want this — reading the post through, and opening the
    /// search — so that neither finds the document moving underneath it.
    pub(super) fn recommit_active(mut self: Pin<&mut Self>) {
        let index = *self.active_index();
        if index < 0 {
            return;
        }
        self.as_mut().commit(index);
        let last = self.blocks.len() as i32 - 1;
        self.set_active_index(index.clamp(0, last));
    }

    /// The writer's own switch. Reading mode overrules it while it lasts, so this is
    /// kept rather than read back off the checker: it is what the checker comes back to.
    pub(super) fn toggle_checking(mut self: Pin<&mut Self>) {
        let on = !*self.checking();
        self.as_mut().set_checking(on);
        lint::set_checking(on);
        // The blocks hear about it from the checker itself; the foot of the window here.
        self.refresh_lint();
    }

    pub(super) fn toggle_reading(mut self: Pin<&mut Self>) {
        let reading = !*self.reading();
        if reading {
            self.as_mut().recommit_active();
        } else {
            // The editor that opens on the way out is handed the cursor back where it
            // was standing. Left to itself it would put it at the end of its block, and
            // reading a post through is no reason for the cursor to have moved.
            let cursor = self.cursor_position;
            self.as_mut().set_pending_cursor(cursor);
        }
        // A selection running across blocks has no editors left to draw it with.
        self.as_mut().set_selection_anchor(-1);
        self.as_mut().set_reading(reading);
        lint::set_checking(!reading && *self.checking());
        self.refresh_lint();
    }

    pub(super) fn cycle_lint(self: Pin<&mut Self>, direction: i32) {
        let count = self.lint_replacements.len() as i32;
        if count < 2 {
            return;
        }
        let choice = search::wrapped(*self.lint_choice(), direction, count);
        self.show_suggestion(choice);
    }
}
