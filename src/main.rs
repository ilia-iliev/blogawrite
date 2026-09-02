pub mod document;
pub mod parse;
pub mod state;
pub mod theme;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QQuickStyle, QString, QUrl};

// Registers `MarkdownHighlighter` into the QML module; defined in cpp/.
unsafe extern "C" {
    fn blogawrite_register_types();
}

/// Qt Quick's OpenGL path spends the best part of two hundred milliseconds bringing the
/// graphics driver up before it can show anything, and this window is text in a column
/// narrower than most of it: the software renderer draws a frame of it in single-digit
/// milliseconds and starts three times sooner. Only a default — an explicit choice wins.
fn prefer_software_renderer() {
    if std::env::var_os("QT_QUICK_BACKEND").is_some() || std::env::var_os("QSG_RHI_BACKEND").is_some()
    {
        return;
    }
    // SAFETY: called before Qt starts, with no other thread running.
    unsafe { std::env::set_var("QT_QUICK_BACKEND", "software") };
}

/// Qt's Wayland plugin brings up EGL — and behind it the whole of Mesa, 140MB of
/// libLLVM and 50MB of libgallium — to settle one question: whether it could draw a
/// title bar itself, if it had to. It never has to. i3 and sway draw their own, the
/// answer is thrown away, and the window is identical either way — the compositor is
/// still asked for a server-side decoration and still gives one. Saying up front that
/// we will not be drawing our own saves the fifty milliseconds of loading Mesa to be
/// told so. Only a default — an explicit choice wins, which is how a compositor that
/// decorates nothing itself gets its title bar back.
fn leave_decorations_to_the_compositor() {
    if std::env::var_os("QT_WAYLAND_DISABLE_WINDOWDECORATION").is_some() {
        return;
    }
    // SAFETY: called before Qt starts, with no other thread running.
    unsafe { std::env::set_var("QT_WAYLAND_DISABLE_WINDOWDECORATION", "1") };
}

fn main() {
    // A document is the whole point: there is no way to pick one from inside the app.
    if std::env::args().nth(1).is_none() {
        eprintln!("usage: blogawrite <file.md>");
        std::process::exit(2);
    }
    prefer_software_renderer();
    leave_decorations_to_the_compositor();
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
