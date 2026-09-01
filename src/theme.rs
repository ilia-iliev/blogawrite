use cxx_qt_lib::QString;

/// Light theme constants, exposed to QML as the `Theme` singleton.
#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        #[qproperty(QString, background)]
        #[qproperty(QString, surface)]
        #[qproperty(QString, text)]
        #[qproperty(QString, muted)]
        #[qproperty(QString, accent)]
        #[qproperty(QString, code_background)]
        #[qproperty(QString, active_background)]
        #[qproperty(QString, border)]
        #[qproperty(QString, mono_family)]
        #[qproperty(QString, body_family)]
        #[qproperty(i32, content_width)]
        #[qproperty(i32, body_size)]
        #[qproperty(i32, code_size)]
        #[qproperty(f64, line_height)]
        #[qproperty(i32, block_spacing)]
        type Theme = super::ThemeRust;
    }
}

pub struct ThemeRust {
    background: QString,
    surface: QString,
    text: QString,
    muted: QString,
    accent: QString,
    code_background: QString,
    active_background: QString,
    border: QString,
    mono_family: QString,
    body_family: QString,
    content_width: i32,
    body_size: i32,
    code_size: i32,
    line_height: f64,
    block_spacing: i32,
}

impl Default for ThemeRust {
    fn default() -> Self {
        Self {
            background: QString::from("#FAFAF7"),
            surface: QString::from("#FFFFFE"),
            text: QString::from("#2D2A26"),
            muted: QString::from("#8A8378"),
            accent: QString::from("#2F6F4E"),
            code_background: QString::from("#F0EEE9"),
            active_background: QString::from("#F3F1EA"),
            border: QString::from("#E4E0D6"),
            mono_family: QString::from("monospace"),
            body_family: QString::from("sans-serif"),
            content_width: 720,
            body_size: 16,
            // Qt renders markdown code at about three quarters of the body size.
            code_size: 12,
            line_height: 1.5,
            block_spacing: 14,
        }
    }
}
