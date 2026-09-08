use cxx_qt_build::{CxxQtBuilder, QmlModule};
use qt_build_utils::QtBuild;
use std::path::PathBuf;

fn main() {
    CxxQtBuilder::new()
        .qt_module("Quick")
        // Hand-written QObjects: QSyntaxHighlighter subclasses, which cxx-qt cannot
        // express. One styles the raw markdown of the block being edited, the other marks
        // up the rendered blocks; both share a base for the document, the line spacing
        // and the checker's marks.
        .qobject_header("cpp/blockhighlighter.h")
        .qobject_header("cpp/lint.h")
        .qobject_header("cpp/markdownhighlighter.h")
        .qobject_header("cpp/renderedhighlighter.h")
        .cc_builder(|cc| {
            cc.include("cpp");
            // Qt's headers are not ours, but cxx-qt names them to the compiler with -I,
            // which makes every warning inside them ours to read. GCC 16 added one that
            // Qt 6 trips on every QChar it defines, and it drowned the build. Naming the
            // same directories with -isystem says what they are: GCC keeps quiet about
            // what it finds there, and about anything those headers go on to include.
            // A directory given both ways counts as a system one whichever flag comes
            // first, so the -I cxx-qt adds after this closure runs does not undo it.
            for path in qt_header_dirs() {
                cc.flag(format!("-isystem{}", path.display()));
            }
            // Left over is one warning inside a file cxx generates: it fills in a Vec
            // through a function it declares as taking a const pointer, so GCC reads the
            // empty vector as being used before it is written. cxx-gen is already at its
            // latest and the file is rewritten on every build, so there is nowhere to put
            // a pragma -- it has to come off the whole compiler invocation. Our own
            // sources each turn it back on, so only generated code goes unchecked.
            cc.flag("-Wno-maybe-uninitialized");
            for source in ["blockhighlighter", "fonts", "lint", "markdownhighlighter", "renderedhighlighter"] {
                println!("cargo::rerun-if-changed=cpp/{source}.cpp");
                cc.file(format!("cpp/{source}.cpp"));
            }
        })
        // A plain cxx bridge, not a QObject: the checker is asked questions by the
        // highlighters, which are C++, and it answers with strings.
        .file("src/lint.rs")
        // The same, for the styling of the block being edited: the highlighter is handed
        // one number per character and paints it, and knows no markdown of its own.
        .file("src/style.rs")
        .qml_module(QmlModule {
            uri: "com.blogawrite",
            rust_files: &["src/document.rs", "src/theme.rs"],
            qml_files: &[
                "qml/Main.qml",
                "qml/Block.qml",
                "qml/RenderedBlock.qml",
                "qml/ActiveBlock.qml",
                "qml/ImageBlock.qml",
                "qml/FootBar.qml",
                "qml/SearchBar.qml",
                "qml/ClosePrompt.qml",
            ],
            ..Default::default()
        })
        .build();

    // markdownhighlighter.cpp calls QQuickTextDocument::textDocument(), which lives in
    // Qt6Quick. The linker sees Qt6Quick before the static archive that generated file
    // ends up in, and --as-needed drops a library nothing has asked for yet — so by the
    // time the archive asks, it is gone. Name it once more, after everything else.
    println!("cargo::rustc-link-arg=-lQt6Quick");
    // The same, for the QML the module compiles ahead of time: it calls into Qt6Qml, and
    // the test binary is linked without ever having been told it needs it.
    println!("cargo::rustc-link-arg=-lQt6Qml");
}

/// The directories Qt keeps its headers in. Every include in this crate reaches Qt as
/// <QtModule/Header>, so the root the modules sit under is the one that has to be named;
/// the rest come along, because a header pulled in by a system header is one too.
fn qt_header_dirs() -> Vec<PathBuf> {
    QtBuild::new(Vec::new())
        .expect("Could not find Qt installation")
        .include_paths()
}
