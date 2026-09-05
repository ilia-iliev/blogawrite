#pragma once

#include "lint.h"

#include <QtCore/QString>
#include <QtGui/QColor>
#include <QtGui/QSyntaxHighlighter>
#include <QtCore/QList>
#include <QtGui/QTextCharFormat>
#include <QtQuick/QQuickTextDocument>

// What the two highlighters have in common: the document they work on, the line spacing
// a TextEdit has no property of its own for — it has to be set on the document's blocks
// instead — and the two things they both draw, which are code and what the checker
// objected to.
class BlockHighlighter : public QSyntaxHighlighter
{
    Q_OBJECT

    // The TextEdit's document. Named `target` because QSyntaxHighlighter::document() is taken.
    Q_PROPERTY(QQuickTextDocument *target READ target WRITE setTarget NOTIFY targetChanged)
    // Proportional line spacing, which TextEdit itself has no property for.
    Q_PROPERTY(qreal lineHeight READ lineHeight WRITE setLineHeight NOTIFY lineHeightChanged)
    // How code is drawn, in the block being edited and in the rendered ones alike.
    Q_PROPERTY(QString monoFamily READ monoFamily WRITE setMonoFamily NOTIFY monoFamilyChanged)
    Q_PROPERTY(int codeSize READ codeSize WRITE setCodeSize NOTIFY codeSizeChanged)
    // The wash behind anything the checker took exception to. One colour: a misspelled
    // word and a clumsy turn of phrase are the same offer, and are accepted the same way.
    Q_PROPERTY(QColor lint READ lint WRITE setLint NOTIFY lintChanged)

public:
    explicit BlockHighlighter(QObject *parent = nullptr);

    QQuickTextDocument *target() const { return m_target; }
    void setTarget(QQuickTextDocument *target);

    qreal lineHeight() const { return m_lineHeight; }
    void setLineHeight(qreal height);

    QString monoFamily() const { return m_monoFamily; }
    void setMonoFamily(const QString &family);

    int codeSize() const { return m_codeSize; }
    void setCodeSize(int size);

    QColor lint() const { return m_lint; }
    void setLint(const QColor &color);

Q_SIGNALS:
    void targetChanged();
    void lineHeightChanged();
    void monoFamilyChanged();
    void codeSizeChanged();
    void lintChanged();

protected:
    /// A line that is to take up no height at all — a code fence shrunk away to nothing.
    static constexpr int Collapsed = 1;

    /// The checker's own mark. Everything else a character can be part of is a
    /// `blogawrite::StyleBit`, worked out in Rust and shared by both highlighters.
    static constexpr quint16 Marked = 1 << 15;

    /// Mark what the checker objects to in the line being highlighted. It works over a
    /// whole block — a sentence runs across the lines a writer happened to type — so it
    /// is asked about the document, and its answer is cut down to this line here.
    /// Whatever the caller has already marked `StyleBit::Unchecked` is left as it is.
    void markLints(QList<quint16> &mask);

    /// Whether this highlighter's document holds raw markdown or prose Qt has already
    /// rendered. The checker reads the two differently.
    virtual bool checksMarkdown() const = 0;

    /// The document underneath has been swapped or typed into: whatever was worked out
    /// about its text is about text that is no longer there.
    virtual void contentChanged();

    /// Paint the runs of `mask`, asking the subclass what its own bits look like and
    /// adding the shared marks on top.
    void applyMask(const QList<quint16> &mask);

    /// What this highlighter's own bits look like. The shared ones are not its business.
    virtual QTextCharFormat formatFor(quint16 bits) const = 0;

    /// Re-style, then take in the lines that just opened up or collapsed.
    void restyle();
    void applyLineHeight();

    /// Code, wherever it is drawn: the theme's font at the theme's size.
    QTextCharFormat codeFormat() const;

private:
    QQuickTextDocument *m_target = nullptr;
    bool m_spacing = false;
    qreal m_lineHeight = 1.0;
    QString m_monoFamily = QStringLiteral("monospace");
    int m_codeSize = 12;
    // Every colour is the Theme singleton's, bound from QML before the first frame.
    // Nothing here has a colour of its own to fall back on: src/theme.rs is the one
    // place a palette is written down.
    QColor m_lint;
    // QSyntaxHighlighter calls highlightBlock once per line. The checker works on the
    // whole document, so keep its answer instead of crossing the FFI and checking the
    // same block again for every line.
    bool m_lintsDirty = true;
    QList<LintSpan> m_lintSpans;

    /// Ask the checker about this document again, and say whether its answer moved.
    bool updateLints();
};
