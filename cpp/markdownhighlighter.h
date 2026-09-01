#pragma once

#include <QtCore/QList>
#include <QtCore/QString>
#include <QtGui/QColor>
#include <QtGui/QSyntaxHighlighter>
#include <QtQuick/QQuickTextDocument>

// Styles raw markdown so that it reads as the rendered thing while it is being edited:
// every syntax marker shrinks away to nothing unless the cursor is inside the span it marks.
class MarkdownHighlighter : public QSyntaxHighlighter
{
    Q_OBJECT

    // The TextArea's document. Named `target` because QSyntaxHighlighter::document() is taken.
    Q_PROPERTY(QQuickTextDocument *target READ target WRITE setTarget NOTIFY targetChanged)
    // Where the cursor is, or -1 when the editor does not have it. Markers under it stay visible.
    Q_PROPERTY(int cursorPosition READ cursorPosition WRITE setCursorPosition NOTIFY cursorPositionChanged)
    // A fenced code block: everything is code, and only the fences are markers.
    Q_PROPERTY(bool code READ code WRITE setCode NOTIFY codeChanged)
    Q_PROPERTY(QColor accent READ accent WRITE setAccent NOTIFY accentChanged)
    Q_PROPERTY(QColor muted READ muted WRITE setMuted NOTIFY mutedChanged)
    Q_PROPERTY(QColor codeBackground READ codeBackground WRITE setCodeBackground NOTIFY codeBackgroundChanged)
    Q_PROPERTY(QString monoFamily READ monoFamily WRITE setMonoFamily NOTIFY monoFamilyChanged)
    Q_PROPERTY(int codeSize READ codeSize WRITE setCodeSize NOTIFY codeSizeChanged)
    // Proportional line spacing, which TextArea itself has no property for.
    Q_PROPERTY(qreal lineHeight READ lineHeight WRITE setLineHeight NOTIFY lineHeightChanged)

public:
    explicit MarkdownHighlighter(QObject *parent = nullptr);

    QQuickTextDocument *target() const { return m_target; }
    void setTarget(QQuickTextDocument *target);

    int cursorPosition() const { return m_cursorPosition; }
    void setCursorPosition(int position);

    bool code() const { return m_code; }
    void setCode(bool code);

    QColor accent() const { return m_accent; }
    void setAccent(const QColor &color);

    QColor muted() const { return m_muted; }
    void setMuted(const QColor &color);

    QColor codeBackground() const { return m_codeBackground; }
    void setCodeBackground(const QColor &color);

    QString monoFamily() const { return m_monoFamily; }
    void setMonoFamily(const QString &family);

    int codeSize() const { return m_codeSize; }
    void setCodeSize(int size);

    qreal lineHeight() const { return m_lineHeight; }
    void setLineHeight(qreal height);

Q_SIGNALS:
    void targetChanged();
    void cursorPositionChanged();
    void codeChanged();
    void accentChanged();
    void mutedChanged();
    void codeBackgroundChanged();
    void monoFamilyChanged();
    void codeSizeChanged();
    void lineHeightChanged();

protected:
    void highlightBlock(const QString &text) override;

private:
    void restyle();
    void restyleCursorLines(int from, int to);
    QTextCharFormat formatFor(quint16 bits) const;
    void applyLineHeight();
    void markCode(const QString &text, QList<quint16> &mask) const;
    void markProse(const QString &text, QList<quint16> &mask, int cursor) const;

    QQuickTextDocument *m_target = nullptr;
    int m_cursorPosition = -1;
    bool m_code = false;
    bool m_spacing = false;
    QColor m_accent = QColor(QStringLiteral("#2F6F4E"));
    QColor m_muted = QColor(QStringLiteral("#8A8378"));
    QColor m_codeBackground = QColor(QStringLiteral("#F0EEE9"));
    QString m_monoFamily = QStringLiteral("monospace");
    int m_codeSize = 12;
    qreal m_lineHeight = 1.0;
};

extern "C" void blogawrite_register_types();
