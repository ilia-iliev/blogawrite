use crate::blocks::{self, Span};
use crate::lint;
use crate::parse;
use crate::state;
use cxx_qt::CxxQtType;
use cxx_qt_lib::{QByteArray, QHash, QHashPair_i32_QByteArray, QString, QUrl, QVariant};
use std::collections::VecDeque;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

const TEXT_ROLE: i32 = 0x0100; // Qt::UserRole
const KIND_ROLE: i32 = 0x0101;
const IMAGE_PATH_ROLE: i32 = 0x0102;
const RENDERED_ROLE: i32 = 0x0103;
const UNDO_LIMIT: usize = 512;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qurl.h");
        type QUrl = cxx_qt_lib::QUrl;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
        include!("cxx-qt-lib/qmodelindex.h");
        type QModelIndex = cxx_qt_lib::QModelIndex;
        include!("cxx-qt-lib/qvector.h");
        type QVector_i32 = cxx_qt_lib::QVector<i32>;
        include!("cxx-qt-lib/qhash.h");
        type QHash_i32_QByteArray = cxx_qt_lib::QHash<cxx_qt_lib::QHashPair_i32_QByteArray>;
    }

    extern "C++" {
        include!(<QtCore/QAbstractListModel>);
        type QAbstractListModel;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[base = QAbstractListModel]
        #[qproperty(QString, file_path)]
        #[qproperty(QUrl, base_url)]
        #[qproperty(bool, dirty)]
        #[qproperty(i32, active_index)]
        #[qproperty(i32, selection_anchor)]
        #[qproperty(i32, selection_position)]
        #[qproperty(i32, pending_cursor)]
        #[qproperty(QString, error_message)]
        #[qproperty(QString, lint_message)]
        #[qproperty(QString, lint_replacement)]
        /// The misspelled word under the cursor, which is what the writer would be taking
        /// into their dictionary. Empty where the checker objected to something a
        /// dictionary has no opinion about.
        #[qproperty(QString, lint_word)]
        #[qproperty(i32, lint_at)]
        #[qproperty(i32, lint_length)]
        #[qproperty(i32, lint_choice)]
        #[qproperty(i32, lint_options)]
        type Document = super::DocumentRust;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[cxx_override]
        fn row_count(self: &Document, parent: &QModelIndex) -> i32;
        #[cxx_override]
        fn data(self: &Document, index: &QModelIndex, role: i32) -> QVariant;
        #[cxx_override]
        fn role_names(self: &Document) -> QHash_i32_QByteArray;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        /// Commit the block being edited and move the cursor to `target`.
        #[qinvokable]
        fn activate(self: Pin<&mut Document>, target: i32);
        /// Move into the neighbouring block from `direction`, keeping to its near edge.
        #[qinvokable]
        fn move_to(self: Pin<&mut Document>, target: i32, direction: i32);
        /// Carry a selection into block `target`, starting one at `anchor_position` in the
        /// block the cursor is leaving if there is none yet.
        #[qinvokable]
        fn select_to(self: Pin<&mut Document>, target: i32, anchor_position: i32);
        /// Take the whole document into a selection, leaving the cursor at its foot.
        #[qinvokable]
        fn select_all(self: Pin<&mut Document>);
        /// Let a selection go, leaving the cursor where it stands.
        #[qinvokable]
        fn clear_selection(self: Pin<&mut Document>, cursor_position: i32);
        /// The markdown between the anchor and the cursor, as it would be written to disk.
        #[qinvokable]
        fn selection_text(self: &Document, cursor_position: i32) -> QString;
        /// Replace the selection with `insert`, joining what is left of the blocks at its
        /// two ends into one, and leave the cursor at the seam.
        #[qinvokable]
        fn delete_selection(self: Pin<&mut Document>, cursor_position: i32, insert: &QString);
        /// Store the raw text of a block as the user types it.
        #[qinvokable]
        fn set_block_text(
            self: Pin<&mut Document>,
            index: i32,
            text: &QString,
            cursor_position: i32,
        );
        /// Split a block the user broke in two, and activate the second half.
        #[qinvokable]
        fn split_block(self: Pin<&mut Document>, index: i32, before: &QString, after: &QString);
        /// Merge a block into its predecessor, keeping the cursor at the seam.
        #[qinvokable]
        fn merge_with_previous(self: Pin<&mut Document>, index: i32);
        /// Remember the cursor's current document position without creating an undo entry.
        #[qinvokable]
        fn set_cursor_position(self: Pin<&mut Document>, index: i32, position: i32);
        /// Undo the last document change, regardless of which block made it.
        #[qinvokable]
        fn undo(self: Pin<&mut Document>);
        /// Look again at where the cursor is standing. The checker comes up a moment
        /// after the window does, and what it makes of the block the document opened in
        /// would otherwise wait for the cursor to move.
        #[qinvokable]
        fn refresh_lint(self: Pin<&mut Document>);
        /// Whether the typing has stopped. Nothing is said about a block while it is
        /// being typed into — a word half-written is not a word spelled wrong — and the
        /// checker has its say once the writer pauses.
        #[qinvokable]
        fn settle(self: Pin<&mut Document>, settled: bool);
        /// Show the next suggestion the checker offered for what the cursor is standing
        /// in, or the one before it. They wrap around; only one is ever shown.
        #[qinvokable]
        fn cycle_lint(self: Pin<&mut Document>, direction: i32);

        /// Note where the cursor is so the next session can pick it up.
        #[qinvokable]
        fn remember_position(self: &Document);
        #[qinvokable]
        fn open_path(self: Pin<&mut Document>, path: &QString) -> bool;
        #[qinvokable]
        fn save(self: Pin<&mut Document>) -> bool;
    }

    #[auto_cxx_name]
    unsafe extern "RustQt" {
        #[inherit]
        fn begin_insert_rows(self: Pin<&mut Document>, parent: &QModelIndex, first: i32, last: i32);
        #[inherit]
        fn end_insert_rows(self: Pin<&mut Document>);
        #[inherit]
        fn begin_remove_rows(self: Pin<&mut Document>, parent: &QModelIndex, first: i32, last: i32);
        #[inherit]
        fn end_remove_rows(self: Pin<&mut Document>);
        #[inherit]
        fn index(self: &Document, row: i32, column: i32, parent: &QModelIndex) -> QModelIndex;
        #[inherit]
        #[qsignal]
        fn data_changed(
            self: Pin<&mut Document>,
            top_left: &QModelIndex,
            bottom_right: &QModelIndex,
            roles: &QVector_i32,
        );
        #[inherit]
        fn begin_reset_model(self: Pin<&mut Document>);
        #[inherit]
        fn end_reset_model(self: Pin<&mut Document>);
    }
}

use qobject::Document;

#[derive(Clone)]
struct UndoState {
    // Blocks and gaps are shared with the live document. An undo point is made on every
    // edit; cloning every allocation on every keystroke quickly dwarfs the document.
    blocks: Vec<Arc<String>>,
    gaps: Vec<Arc<String>>,
    active_index: i32,
    selection_anchor: i32,
    selection_position: i32,
    cursor_position: i32,
    revision: u64,
}

pub struct DocumentRust {
    blocks: Vec<Arc<String>>,
    /// Source around the blocks: before, between, and after. Keeping it separately lets
    /// the view stay block-oriented without normalizing the file on save.
    gaps: Vec<Arc<String>>,
    undo: VecDeque<UndoState>,
    revision: u64,
    saved_revision: u64,
    file_path: QString,
    base_url: QUrl,
    dirty: bool,
    active_index: i32,
    selection_anchor: i32,
    selection_position: i32,
    pending_cursor: i32,
    error_message: QString,
    /// The block the message below was worked out for.
    lint_block: i32,
    /// Whether the typing has stopped: see [`Document::settle`].
    settled: bool,
    /// Whether the newest undo state is a run of typing that is still being added to.
    typing: bool,
    /// How far into its block the cursor is, counted the way Qt counts. The block it is
    /// in is `active_index`; only the editor knows this half, and it reports it.
    cursor_position: i32,
    lint_message: QString,
    /// Every suggestion the checker offered, of which `lint_replacement` is the one on show.
    lint_replacements: Vec<String>,
    lint_replacement: QString,
    lint_word: QString,
    lint_at: i32,
    lint_length: i32,
    lint_choice: i32,
    lint_options: i32,
}

impl Default for DocumentRust {
    fn default() -> Self {
        Self {
            blocks: vec![Arc::new(String::new())],
            gaps: vec![Arc::new(String::new()), Arc::new(String::new())],
            undo: VecDeque::new(),
            revision: 0,
            saved_revision: 0,
            file_path: QString::default(),
            base_url: QUrl::default(),
            dirty: false,
            active_index: 0,
            selection_anchor: -1,
            selection_position: 0,
            pending_cursor: -1,
            error_message: QString::default(),
            lint_block: -1,
            settled: true,
            typing: false,
            cursor_position: -1,
            lint_message: QString::default(),
            lint_replacements: Vec::new(),
            lint_replacement: QString::default(),
            lint_word: QString::default(),
            lint_at: -1,
            lint_length: 0,
            lint_choice: 0,
            lint_options: 0,
        }
    }
}

impl Document {
    fn row_count(&self, _parent: &cxx_qt_lib::QModelIndex) -> i32 {
        self.blocks.len() as i32
    }

    fn data(&self, index: &cxx_qt_lib::QModelIndex, role: i32) -> QVariant {
        let Some(block) = self.blocks.get(index.row() as usize) else {
            return QVariant::default();
        };
        match role {
            TEXT_ROLE => QVariant::from(&QString::from(block.as_str())),
            RENDERED_ROLE => QVariant::from(&QString::from(&parse::rendered(block))),
            KIND_ROLE => QVariant::from(&QString::from(parse::kind(block))),
            IMAGE_PATH_ROLE => {
                QVariant::from(&QString::from(&parse::lone_image(block).unwrap_or_default()))
            }
            _ => QVariant::default(),
        }
    }

    fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        let mut roles = QHash::<QHashPair_i32_QByteArray>::default();
        roles.insert(TEXT_ROLE, QByteArray::from("text"));
        roles.insert(KIND_ROLE, QByteArray::from("kind"));
        roles.insert(IMAGE_PATH_ROLE, QByteArray::from("imagePath"));
        roles.insert(RENDERED_ROLE, QByteArray::from("rendered"));
        roles
    }
}

impl Document {
    fn activate(self: Pin<&mut Self>, target: i32) {
        self.activate_at(target, -1);
    }

    fn move_to(self: Pin<&mut Self>, target: i32, direction: i32) {
        self.activate_at(target, if direction > 0 { 0 } else { -1 });
    }

    fn activate_at(mut self: Pin<&mut Self>, target: i32, cursor: i32) {
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

    fn select_to(mut self: Pin<&mut Self>, target: i32, anchor_position: i32) {
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

    fn select_all(mut self: Pin<&mut Self>) {
        // Pinned at the top of the document, with the cursor carried to the end of the
        // last block — the -1 the editor reads as "the far edge of the block".
        self.as_mut().set_selection_anchor(0);
        self.as_mut().set_selection_position(0);
        self.as_mut().set_pending_cursor(-1);
        let last = self.blocks.len() as i32 - 1;
        self.as_mut().set_active_index(last);
        self.as_mut().refresh_undo(-1);
    }

    fn clear_selection(mut self: Pin<&mut Self>, cursor_position: i32) {
        self.as_mut().set_selection_anchor(-1);
        self.as_mut().refresh_undo(cursor_position);
    }

    fn selection_text(&self, cursor_position: i32) -> QString {
        let Some(span) = self.selected_span(cursor_position) else {
            return QString::default();
        };
        QString::from(&blocks::selected_text(&self.blocks, &self.gaps, span))
    }

    fn delete_selection(mut self: Pin<&mut Self>, cursor_position: i32, insert: &QString) {
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
    fn selected_span(&self, cursor_position: i32) -> Option<Span> {
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
    fn commit(mut self: Pin<&mut Self>, index: i32) -> i32 {
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
    fn replace_block(
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

    fn remove_rows(mut self: Pin<&mut Self>, first: i32, count: i32) {
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

    fn insert_rows(
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

    fn set_block_text(
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
    fn push_undo(mut self: Pin<&mut Self>, cursor_position: i32) {
        self.as_mut().rust_mut().typing = false;
        self.record_undo(cursor_position);
    }

    /// A keystroke. A run of them is one thing to undo rather than one per letter: the
    /// writer means a word, not the letters of it. The run stays open until the typing
    /// stops — the editor says when — or until anything that is not typing happens.
    fn push_typing(mut self: Pin<&mut Self>, cursor_position: i32) {
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
    fn record_undo(mut self: Pin<&mut Self>, cursor_position: i32) {
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
    fn refresh_undo(mut self: Pin<&mut Self>, cursor_position: i32) {
        self.as_mut().rust_mut().typing = false;
        self.record_cursor(cursor_position);
    }

    /// Bring the newest state up to date without ending a run of typing. A keystroke
    /// moves the cursor, and the cursor moving is not something else happening.
    fn record_cursor(mut self: Pin<&mut Self>, cursor_position: i32) {
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

    fn set_cursor_position(mut self: Pin<&mut Self>, index: i32, position: i32) {
        if index == *self.active_index() {
            self.as_mut().record_cursor(position);
        }
    }

    fn undo(mut self: Pin<&mut Self>) {
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

    fn settle(mut self: Pin<&mut Self>, settled: bool) {
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

    fn refresh_lint(self: Pin<&mut Self>) {
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
    fn update_lint(mut self: Pin<&mut Self>, cursor_position: i32) {
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
    fn show_suggestion(mut self: Pin<&mut Self>, choice: i32) {
        let shown = self
            .lint_replacements
            .get(choice as usize)
            .cloned()
            .unwrap_or_default();
        self.as_mut().set_lint_choice(choice);
        self.set_lint_replacement(QString::from(&shown));
    }

    fn cycle_lint(self: Pin<&mut Self>, direction: i32) {
        let count = self.lint_replacements.len() as i32;
        if count < 2 {
            return;
        }
        let choice = wrapped(*self.lint_choice(), direction, count);
        self.show_suggestion(choice);
    }

    fn notify_changed(mut self: Pin<&mut Self>, index: i32, text_only: bool) {
        let parent = cxx_qt_lib::QModelIndex::default();
        let cell = self.as_mut().index(index, 0, &parent);
        let mut roles = cxx_qt_lib::QVector::<i32>::default();
        if text_only {
            roles.append(TEXT_ROLE);
        }
        self.data_changed(&cell, &cell, &roles);
    }

    fn split_block(mut self: Pin<&mut Self>, index: i32, before: &QString, after: &QString) {
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

    fn merge_with_previous(mut self: Pin<&mut Self>, index: i32) {
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

impl Document {
    fn text(&self) -> String {
        blocks::source(&self.blocks, &self.gaps)
    }

    fn reset_blocks(mut self: Pin<&mut Self>, segments: parse::Segments) {
        self.as_mut().begin_reset_model();
        {
            let mut rust = self.as_mut().rust_mut();
            rust.blocks = segments.blocks.into_iter().map(Arc::new).collect();
            rust.gaps = segments.gaps.into_iter().map(Arc::new).collect();
            rust.undo.clear();
            rust.typing = false;
            rust.revision = 0;
            rust.saved_revision = 0;
        }
        self.as_mut().end_reset_model();
        self.as_mut().set_pending_cursor(-1);
        self.set_active_index(-1);
    }

    /// Open the path given on the command line, resolved against the working directory.
    /// A path that does not exist yet starts an empty document that `save` will create.
    fn open_path(mut self: Pin<&mut Self>, path: &QString) -> bool {
        let path = PathBuf::from(path.to_string());
        let path = if path.is_absolute() {
            path
        } else {
            std::env::current_dir().unwrap_or_default().join(path)
        };

        let blocks = match fs::read_to_string(&path) {
            Ok(source) => parse::segments(&source),
            Err(error) if error.kind() == ErrorKind::NotFound => parse::segments(""),
            Err(error) => {
                let message = format!("Could not open {}: {error}", path.display());
                eprintln!("blogawrite: {message}");
                self.as_mut().set_error_message(QString::from(&message));
                return false;
            }
        };

        self.as_mut().reset_blocks(blocks);
        self.as_mut().apply_path(path);
        self.as_mut().set_error_message(QString::default());
        self.as_mut().set_dirty(false);
        self.as_mut().restore_position();
        self.as_mut().refresh_undo(-1);
        true
    }

    /// Pick up where the last session left off in this file, or at its end.
    fn restore_position(self: Pin<&mut Self>) {
        let path = self.file_path().to_string();
        let last = self.blocks.len() as i32 - 1;
        let remembered = state::recall(Path::new(&path)).unwrap_or(last);
        self.set_active_index(remembered.clamp(0, last));
    }

    fn remember_position(&self) {
        state::remember(Path::new(&self.file_path().to_string()), *self.active_index());
    }

    fn save(mut self: Pin<&mut Self>) -> bool {
        let path = PathBuf::from(self.file_path().to_string());
        if let Err(error) = crate::storage::write_atomic(&path, self.text().as_bytes()) {
            let message = format!("Could not save {}: {error}", path.display());
            eprintln!("blogawrite: {message}");
            self.as_mut().set_error_message(QString::from(&message));
            return false;
        }
        self.as_mut().set_error_message(QString::default());
        self.as_mut().remember_position();
        self.as_mut().rust_mut().saved_revision = self.revision;
        self.set_dirty(false);
        true
    }

    fn apply_path(mut self: Pin<&mut Self>, path: PathBuf) {
        let directory = path.parent().unwrap_or(&path).to_path_buf();
        self.as_mut()
            .set_file_path(QString::from(&path.to_string_lossy().to_string()));
        self.set_base_url(QUrl::from_local_file(&QString::from(
            // Trailing separator so relative image paths resolve inside the directory.
            &format!("{}/", directory.to_string_lossy()),
        )));
    }
}

/// The suggestion `direction` away from the one on show. They wrap around: the checker
/// offers a handful and the writer walks them until one of them is right.
fn wrapped(choice: i32, direction: i32, count: i32) -> i32 {
    (choice + direction).rem_euclid(count)
}

#[cfg(test)]
mod tests {
    use super::wrapped;

    #[test]
    fn walks_the_suggestions_both_ways() {
        assert_eq!(wrapped(0, 1, 4), 1);
        assert_eq!(wrapped(2, -1, 4), 1);
    }

    #[test]
    fn comes_back_round_at_either_end() {
        assert_eq!(wrapped(3, 1, 4), 0);
        assert_eq!(wrapped(0, -1, 4), 3);
    }
}
