use crate::parse;
use crate::state;
use cxx_qt::CxxQtType;
use cxx_qt_lib::{QByteArray, QHash, QHashPair_i32_QByteArray, QString, QUrl, QVariant};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::pin::Pin;

const TEXT_ROLE: i32 = 0x0100; // Qt::UserRole
const KIND_ROLE: i32 = 0x0101;
const IMAGE_PATH_ROLE: i32 = 0x0102;

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
        #[qproperty(i32, pending_cursor)]
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
        /// Store the raw text of a block as the user types it.
        #[qinvokable]
        fn set_block_text(self: Pin<&mut Document>, index: i32, text: &QString);
        /// Split a block the user broke in two, and activate the second half.
        #[qinvokable]
        fn split_block(self: Pin<&mut Document>, index: i32, before: &QString, after: &QString);
        /// Merge a block into its predecessor, keeping the cursor at the seam.
        #[qinvokable]
        fn merge_with_previous(self: Pin<&mut Document>, index: i32);

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

pub struct DocumentRust {
    blocks: Vec<String>,
    file_path: QString,
    base_url: QUrl,
    dirty: bool,
    active_index: i32,
    pending_cursor: i32,
}

impl Default for DocumentRust {
    fn default() -> Self {
        Self {
            blocks: vec![String::new()],
            file_path: QString::default(),
            base_url: QUrl::default(),
            dirty: false,
            active_index: 0,
            pending_cursor: -1,
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
            TEXT_ROLE => QVariant::from(&QString::from(block)),
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
        roles
    }
}

impl Document {
    fn activate(mut self: Pin<&mut Self>, target: i32) {
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
        self.as_mut().set_pending_cursor(-1);
        self.set_active_index(target.clamp(0, last));
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

        let replacement = parse::segment(block);
        if replacement.len() == 1 && replacement[0] == *block {
            return 0;
        }
        self.replace_block(index, replacement)
    }

    /// Swap block `index` for the blocks it re-parsed into. Returns the change in row count.
    fn replace_block(mut self: Pin<&mut Self>, index: i32, replacement: Vec<String>) -> i32 {
        let delta = replacement.len() as i32 - 1;
        let mut replacement = replacement.into_iter();
        let head = replacement.next().unwrap_or_default();

        self.as_mut().rust_mut().blocks[index as usize] = head;
        self.as_mut().notify_changed(index);

        let tail: Vec<String> = replacement.collect();
        if !tail.is_empty() {
            self.insert_rows(index + 1, tail);
        }
        delta
    }

    fn remove_rows(mut self: Pin<&mut Self>, first: i32, count: i32) {
        let parent = cxx_qt_lib::QModelIndex::default();
        self.as_mut()
            .begin_remove_rows(&parent, first, first + count - 1);
        let range = first as usize..(first + count) as usize;
        self.as_mut().rust_mut().blocks.drain(range);
        self.end_remove_rows();
    }

    fn insert_rows(mut self: Pin<&mut Self>, first: i32, blocks: Vec<String>) {
        let parent = cxx_qt_lib::QModelIndex::default();
        self.as_mut()
            .begin_insert_rows(&parent, first, first + blocks.len() as i32 - 1);
        let mut rust = self.as_mut().rust_mut();
        for (offset, block) in blocks.into_iter().enumerate() {
            rust.blocks.insert(first as usize + offset, block);
        }
        self.end_insert_rows();
    }

    fn set_block_text(mut self: Pin<&mut Self>, index: i32, text: &QString) {
        let text = text.to_string();
        let mut rust = self.as_mut().rust_mut();
        match rust.blocks.get_mut(index as usize) {
            Some(block) if *block == text => return,
            Some(block) => *block = text,
            None => return,
        }
        // The view may recycle this delegate mid-edit, so keep the model authoritative.
        self.as_mut().notify_changed(index);
        self.set_dirty(true);
    }

    fn notify_changed(mut self: Pin<&mut Self>, index: i32) {
        let parent = cxx_qt_lib::QModelIndex::default();
        let cell = self.as_mut().index(index, 0, &parent);
        self.data_changed(&cell, &cell, &cxx_qt_lib::QVector::<i32>::default());
    }

    fn split_block(mut self: Pin<&mut Self>, index: i32, before: &QString, after: &QString) {
        if index < 0 || index as usize >= self.blocks.len() {
            return;
        }
        let mut blocks = parse::segment(&before.to_string());
        let head = blocks.len() as i32;
        blocks.extend(parse::segment(&after.to_string()));

        self.as_mut().replace_block(index, blocks);
        self.as_mut().set_dirty(true);
        self.as_mut().set_pending_cursor(0);
        self.set_active_index(index + head);
    }

    fn merge_with_previous(mut self: Pin<&mut Self>, index: i32) {
        if index < 1 || index as usize >= self.blocks.len() {
            return;
        }
        let tail = self.blocks[index as usize].clone();
        let previous = index - 1;
        let cursor = self.blocks[previous as usize].encode_utf16().count() as i32;

        self.as_mut().rust_mut().blocks[previous as usize].push_str(&tail);
        self.as_mut().notify_changed(previous);
        self.as_mut().remove_rows(index, 1);
        self.as_mut().set_dirty(true);
        self.as_mut().set_pending_cursor(cursor);
        self.set_active_index(previous);
    }
}

impl Document {
    fn text(&self) -> String {
        format!("{}\n", parse::join(&self.blocks))
    }

    fn reset_blocks(mut self: Pin<&mut Self>, blocks: Vec<String>) {
        self.as_mut().begin_reset_model();
        self.as_mut().rust_mut().blocks = blocks;
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
            Ok(source) => parse::segment(&source),
            Err(error) if error.kind() == ErrorKind::NotFound => vec![String::new()],
            Err(error) => {
                eprintln!("blogawrite: {}: {error}", path.display());
                return false;
            }
        };

        self.as_mut().reset_blocks(blocks);
        self.as_mut().apply_path(path);
        self.as_mut().set_dirty(false);
        self.restore_position();
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
        if let Err(error) = fs::write(&path, self.text()) {
            eprintln!("blogawrite: {}: {error}", path.display());
            return false;
        }
        self.as_mut().remember_position();
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
