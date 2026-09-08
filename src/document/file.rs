//! Opening the document the editor was started on, and writing it back.

use super::qobject::Document;
use crate::blocks;
use crate::parse;
use crate::cursors;
use cxx_qt::CxxQtType;
use cxx_qt_lib::{QString, QUrl};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

impl Document {
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
    pub(super) fn open_path(mut self: Pin<&mut Self>, path: &QString) -> bool {
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
    pub(super) fn restore_position(self: Pin<&mut Self>) {
        let path = self.file_path().to_string();
        let last = self.blocks.len() as i32 - 1;
        let remembered = cursors::recall(Path::new(&path)).unwrap_or(last);
        self.set_active_index(remembered.clamp(0, last));
    }

    pub(super) fn remember_position(&self) {
        cursors::remember(Path::new(&self.file_path().to_string()), *self.active_index());
    }

    pub(super) fn save(mut self: Pin<&mut Self>) -> bool {
        let path = PathBuf::from(self.file_path().to_string());
        let source = blocks::source(&self.blocks, &self.gaps);
        if let Err(error) = crate::files::write_atomic(&path, source.as_bytes()) {
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

    pub(super) fn apply_path(mut self: Pin<&mut Self>, path: PathBuf) {
        let directory = path.parent().unwrap_or(&path).to_path_buf();
        self.as_mut()
            .set_file_path(QString::from(&path.to_string_lossy().to_string()));
        self.set_base_url(QUrl::from_local_file(&QString::from(
            // Trailing separator so relative image paths resolve inside the directory.
            &format!("{}/", directory.to_string_lossy()),
        )));
    }
}
