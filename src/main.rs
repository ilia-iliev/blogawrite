pub mod document;
pub mod parse;
pub mod state;
pub mod theme;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QQuickStyle, QString, QUrl};

// Registers `MarkdownHighlighter` into the QML module; defined in cpp/.
unsafe extern "C" {
    fn blogawrite_register_types();
}

fn main() {
    // A document is the whole point: there is no way to pick one from inside the app.
    if std::env::args().nth(1).is_none() {
        eprintln!("usage: blogawrite <file.md>");
        std::process::exit(2);
    }
    QQuickStyle::set_style(&QString::from("Basic"));

    let mut app = QGuiApplication::new();
    unsafe { blogawrite_register_types() };
    let mut engine = QQmlApplicationEngine::new();
    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from("qrc:/qt/qml/com/blogawrite/qml/Main.qml"));
    }
    if let Some(app) = app.as_mut() {
        app.exec();
    }
}
