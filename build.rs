use cxx_qt_build::{CxxQtBuilder, QmlModule};

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
            for source in ["blockhighlighter", "lint", "markdownhighlighter", "renderedhighlighter"] {
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
                "qml/RenderedBlock.qml",
                "qml/ActiveBlock.qml",
                "qml/ImageBlock.qml",
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
