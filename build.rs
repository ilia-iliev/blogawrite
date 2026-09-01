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
}
