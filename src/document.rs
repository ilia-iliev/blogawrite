mod editing;
mod file;
mod lint;
mod search;
mod undo;

use crate::parse;
use cxx_qt_lib::{QByteArray, QHash, QHashPair_i32_QByteArray, QString, QUrl, QVariant};
use std::collections::VecDeque;
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
        /// Whether the checker is being listened to at all. It is the whole editor's
        /// switch and not this document's — the blocks read it through the highlighters —
        /// and this is that state, for anything in the window that wants to show it.
        #[qproperty(bool, checking)]
        /// Whether the document is being read rather than written. Every block is
        /// rendered, there is no editor and so no cursor, and the checker is quiet:
        /// reading a post through is not the moment to be told what is wrong with it.
        #[qproperty(bool, reading)]
        /// Whether the search bar is open. While it is, the keyboard is its own and the
        /// foot of the window is given over to it.
        #[qproperty(bool, search_active)]
        /// How many occurrences the word looked for has, and which of them is on show.
        /// No occurrences leaves nothing chosen, which is -1.
        #[qproperty(i32, search_count)]
        #[qproperty(i32, search_choice)]
        /// Where the occurrence on show starts in its block, counted the way Qt counts.
        /// Its far end is `pending_cursor`, the same as for a cursor placed any other way.
        #[qproperty(i32, search_at)]
        /// Whether the writer asked for another occurrence of a word that has only the
        /// one. Nothing moves, so the foot of the window says why.
        #[qproperty(bool, search_alone)]
        /// Bumped whenever an occurrence is put on show. Moving between two occurrences
        /// of the same block leaves the cursor in the block it was already in, and there
        /// is no new editor made to select them: this is what tells the standing one.
        #[qproperty(i32, search_serial)]
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
        /// Turn the checker off, or on again. Off, it marks nothing and offers nothing;
        /// what it already found is kept, so turning it back on says so straight away.
        #[qinvokable]
        fn toggle_checking(self: Pin<&mut Document>);
        /// Go into reading mode, or come back out of it.
        #[qinvokable]
        fn toggle_reading(self: Pin<&mut Document>);
        /// Show the next suggestion the checker offered for what the cursor is standing
        /// in, or the one before it. They wrap around; only one is ever shown.
        #[qinvokable]
        fn cycle_lint(self: Pin<&mut Document>, direction: i32);

        /// Open the search bar, giving it the keyboard.
        #[qinvokable]
        fn open_search(self: Pin<&mut Document>);
        /// Close it again, leaving the cursor on the occurrence it walked to.
        #[qinvokable]
        fn close_search(self: Pin<&mut Document>);
        /// Look for `needle` afresh and select the first occurrence of it.
        #[qinvokable]
        fn search_for(self: Pin<&mut Document>, needle: &QString);
        /// Select the occurrence `direction` away from the one on show, wrapping round
        /// the document at either end.
        #[qinvokable]
        fn cycle_search(self: Pin<&mut Document>, direction: i32);

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
    checking: bool,
    reading: bool,
    search_active: bool,
    /// Every occurrence of the word looked for, and which of them is under the cursor.
    /// The two below are that, as the foot of the window needs to read it.
    search: crate::search::Search,
    search_count: i32,
    search_choice: i32,
    search_at: i32,
    search_alone: bool,
    search_serial: i32,
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
            checking: crate::lint::checking(),
            reading: false,
            search_active: false,
            search: crate::search::Search::default(),
            search_count: 0,
            search_choice: -1,
            search_at: -1,
            search_alone: false,
            search_serial: 0,
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
