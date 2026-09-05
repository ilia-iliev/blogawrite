#pragma once

#include "blockhighlighter.h"

#include <QtCore/QList>
#include <QtCore/QString>
#include <QtGui/QColor>

// Draws raw markdown so that it reads as the rendered thing while it is being edited:
// every syntax marker shrinks away to nothing unless the cursor is inside the span it
// marks. What each character is part of comes from Rust — the same markdown parser that
// splits the document into blocks — so this knows no markdown of its own, only paint.
class MarkdownHighlighter : public BlockHighlighter
{
    Q_OBJECT

    // Where the cursor is, or -1 when the editor does not have it. Markers under it stay visible.
    Q_PROPERTY(int cursorPosition READ cursorPosition WRITE setCursorPosition NOTIFY cursorPositionChanged)
    // A fenced code block: everything is code, and only the fences are markers.
    Q_PROPERTY(bool code READ code WRITE setCode NOTIFY codeChanged)
    // Whether the typing has stopped. A word half-written is not a word spelled wrong,
    // so nothing is marked in a block that is still being typed into.
    Q_PROPERTY(bool settled READ settled WRITE setSettled NOTIFY settledChanged)
    Q_PROPERTY(QColor accent READ accent WRITE setAccent NOTIFY accentChanged)
    Q_PROPERTY(QColor muted READ muted WRITE setMuted NOTIFY mutedChanged)
    Q_PROPERTY(QColor codeBackground READ codeBackground WRITE setCodeBackground NOTIFY codeBackgroundChanged)

public:
    explicit MarkdownHighlighter(QObject *parent = nullptr);

    int cursorPosition() const { return m_cursorPosition; }
    void setCursorPosition(int position);

    bool code() const { return m_code; }
    void setCode(bool code);

    bool settled() const { return m_settled; }
    void setSettled(bool settled);

    QColor accent() const { return m_accent; }
    void setAccent(const QColor &color);

    QColor muted() const { return m_muted; }
    void setMuted(const QColor &color);

    QColor codeBackground() const { return m_codeBackground; }
    void setCodeBackground(const QColor &color);

Q_SIGNALS:
    void cursorPositionChanged();
    void codeChanged();
    void settledChanged();
    void accentChanged();
    void mutedChanged();
    void codeBackgroundChanged();

protected:
    void highlightBlock(const QString &text) override;
    QTextCharFormat formatFor(quint16 bits) const override;
    void contentChanged() override;
    bool checksMarkdown() const override { return true; }

private:
    /// What every character of the block is part of, worked out once and kept: a
    /// QSyntaxHighlighter is called a line at a time, and markdown is not a line at a time.
    const QList<quint16> &mask();
    void restyleChangedLines();

    QList<quint16> m_mask;
    bool m_maskDirty = true;
    int m_cursorPosition = -1;
    bool m_code = false;
    bool m_settled = true;
    // The Theme singleton's, as above.
    QColor m_accent;
    QColor m_muted;
    QColor m_codeBackground;
};

extern "C" void blogawrite_register_types();
