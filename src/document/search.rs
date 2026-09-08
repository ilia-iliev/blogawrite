//! Finding a word in the document and walking the places it turns up. The occurrence on
//! show is left selected by the cursor in the block it stands in.

use super::qobject::Document;
use crate::search;
use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use std::pin::Pin;

impl Document {
    /// The block being edited is re-read before the search bar opens: where a word turns
    /// up is worked out over the blocks as they will be once it is rendered again, so
    /// that walking to an occurrence never finds the document has moved underneath it.
    pub(super) fn open_search(mut self: Pin<&mut Self>) {
        self.as_mut().recommit_active();
        // A selection running across blocks is let go: what the search finds is the
        // only thing under the cursor from here on.
        self.as_mut().set_selection_anchor(-1);
        self.as_mut().set_search_alone(false);
        self.set_search_active(true);
    }

    /// The occurrence walked to is left selected: it is usually the very thing the
    /// writer opened the search to type over.
    pub(super) fn close_search(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().search.forget();
        self.as_mut().set_search_count(0);
        self.as_mut().set_search_choice(-1);
        self.as_mut().set_search_at(-1);
        self.as_mut().set_search_alone(false);
        self.set_search_active(false);
    }

    pub(super) fn search_for(mut self: Pin<&mut Self>, needle: &QString) {
        let needle = needle.to_string();
        let blocks = self.blocks.clone();
        let found = self.as_mut().rust_mut().search.look_for(&blocks, &needle);
        self.as_mut().set_search_alone(false);
        self.show_occurrence(found);
    }

    /// A word that turns up once has nowhere to walk to. Nothing moves, and the flag is
    /// what the foot of the window says so with.
    pub(super) fn cycle_search(mut self: Pin<&mut Self>, direction: i32) {
        match self.as_mut().rust_mut().search.walk(direction) {
            Some(found) => self.show_occurrence(Some(found)),
            None => {
                let alone = self.search.count() == 1;
                self.set_search_alone(alone);
            }
        }
    }

    /// Put `found` under the cursor, selected from its start to its end. The block's own
    /// editor draws that selection, rather than the document's cross-block one: a word
    /// found is a word to be typed over, and a selection of the editor's own is the one
    /// the keys already know how to replace.
    pub(super) fn show_occurrence(mut self: Pin<&mut Self>, found: Option<search::Occurrence>) {
        let count = self.search.count();
        let choice = self.search.choice();
        self.as_mut().set_search_count(count);
        self.as_mut().set_search_choice(choice);
        let Some(found) = found else {
            self.set_search_at(-1);
            return;
        };
        self.as_mut().set_search_at(found.at);
        self.as_mut().set_pending_cursor(found.end);
        self.as_mut().set_active_index(found.block);
        self.as_mut().refresh_undo(found.end);
        // Last of all, once everything it needs is in place: an editor already standing
        // in this block reads this to know to look again.
        let serial = *self.search_serial() + 1;
        self.set_search_serial(serial);
    }
}
