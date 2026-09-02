use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new()
        .qt_module("Quick")
        // Hand-written QObject: a QSyntaxHighlighter subclass, which cxx-qt cannot express.
        .qobject_header("cpp/markdownhighlighter.h")
        .cc_builder(|cc| {
            println!("cargo::rerun-if-changed=cpp/markdownhighlighter.cpp");
            cc.include("cpp");
            cc.file("cpp/markdownhighlighter.cpp");
        })
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
}
