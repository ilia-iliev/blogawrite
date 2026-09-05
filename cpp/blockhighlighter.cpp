#include "blockhighlighter.h"

#include "lint.h"

#include "blogawrite/src/style.cxx.h"

#include <QtGui/QTextBlock>
#include <QtGui/QTextCursor>
#include <QtGui/QTextDocument>

BlockHighlighter::BlockHighlighter(QObject *parent)
    : QSyntaxHighlighter(parent)
{
    // The checker is still loading when the first blocks are drawn, so they go up
    // unmarked and are asked, a moment later, to look again — and again whenever a word
    // joins the writer's dictionary. It says only that something has changed somewhere,
    // and most of the time it was some other block: redraw this one only if what would
    // be said about it has actually moved.
    connect(CheckerWatch::instance(), &CheckerWatch::changed, this, [this] {
        if (updateLints()) {
            restyle();
        }
    });
}

void BlockHighlighter::contentChanged()
{
    m_lintsDirty = true;
}

void BlockHighlighter::setTarget(QQuickTextDocument *target)
{
    if (m_target == target) {
        return;
    }
    m_target = target;
    m_lintSpans.clear();
    contentChanged();
    QTextDocument *doc = target ? target->textDocument() : nullptr;
    if (doc) {
        // The default margin would inset the text from where its rendered self sat.
        doc->setDocumentMargin(0);
    }
    setDocument(doc);
    if (doc) {
        // Queued: the spacing is applied by editing the document, which a document must
        // never be asked to do while it is still delivering a change of its own.
        connect(doc,
                &QTextDocument::contentsChanged,
                this,
                &BlockHighlighter::applyLineHeight,
                Qt::QueuedConnection);
        // Styling a document counts as a change of it; only text coming or going does.
        connect(doc, &QTextDocument::contentsChange, this,
                [this](int, int removed, int added) {
                    if (removed || added) {
                        contentChanged();
                    }
                });
        applyLineHeight();
    }
    Q_EMIT targetChanged();
}

void BlockHighlighter::setLineHeight(qreal height)
{
    if (qFuzzyCompare(m_lineHeight, height)) {
        return;
    }
    m_lineHeight = height;
    applyLineHeight();
    Q_EMIT lineHeightChanged();
}

void BlockHighlighter::setMonoFamily(const QString &family)
{
    if (m_monoFamily == family) {
        return;
    }
    m_monoFamily = family;
    restyle();
    Q_EMIT monoFamilyChanged();
}

void BlockHighlighter::setCodeSize(int size)
{
    if (m_codeSize == size) {
        return;
    }
    m_codeSize = size;
    restyle();
    Q_EMIT codeSizeChanged();
}

void BlockHighlighter::setLint(const QColor &color)
{
    if (m_lint == color) {
        return;
    }
    m_lint = color;
    restyle();
    Q_EMIT lintChanged();
}

QTextCharFormat BlockHighlighter::codeFormat() const
{
    QTextCharFormat format;
    format.setFontFamilies({m_monoFamily});
    format.setProperty(QTextFormat::FontPixelSize, m_codeSize);
    return format;
}

bool BlockHighlighter::updateLints()
{
    QTextDocument *doc = document();
    if (!doc) {
        return false;
    }
    QList<LintSpan> found = lintSpans(doc->toPlainText(), checksMarkdown());
    if (!m_lintsDirty && found == m_lintSpans) {
        return false;
    }
    m_lintSpans = std::move(found);
    m_lintsDirty = false;
    return true;
}

void BlockHighlighter::markLints(QList<quint16> &mask)
{
    if (m_lintsDirty) {
        updateLints();
    }
    const int base = currentBlock().position();
    for (const LintSpan &lint : m_lintSpans) {
        const int from = qBound(0, lint.at - base, int(mask.size()));
        const int to = qBound(from, lint.at + lint.length - base, int(mask.size()));
        for (int i = from; i < to; ++i) {
            if (!(mask[i] & quint16(blogawrite::StyleBit::Unchecked))) {
                mask[i] |= Marked;
            }
        }
    }
}

/// A wash behind the words rather than the usual squiggle: Qt Quick turns a text
/// decoration into a filled rectangle, so it draws neither a wave nor an underline in any
/// colour but the text's own. A background is the one mark it will paint, and it is the
/// one that composes — a misspelled link keeps its accent and its underline.
void BlockHighlighter::applyMask(const QList<quint16> &mask)
{
    for (int start = 0; start < mask.size();) {
        int end = start + 1;
        while (end < mask.size() && mask[end] == mask[start]) {
            ++end;
        }
        if (mask[start] != 0) {
            QTextCharFormat format = formatFor(mask[start]);
            if (mask[start] & Marked) {
                format.setBackground(m_lint);
            }
            setFormat(start, end - start, format);
        }
        start = end;
    }
}

void BlockHighlighter::restyle()
{
    rehighlight();
    applyLineHeight();
}

/// A TextEdit has no line spacing of its own, so it is set on the document's blocks.
void BlockHighlighter::applyLineHeight()
{
    QTextDocument *doc = document();
    if (m_spacing || !doc || m_lineHeight <= 0) {
        return;
    }
    m_spacing = true;
    for (QTextBlock block = doc->begin(); block.isValid(); block = block.next()) {
        const bool collapsed = block.userState() == Collapsed;
        const int type = collapsed ? QTextBlockFormat::FixedHeight : QTextBlockFormat::ProportionalHeight;
        const qreal height = collapsed ? 1 : m_lineHeight * 100;

        const QTextBlockFormat current = block.blockFormat();
        if (current.lineHeightType() == type && qFuzzyCompare(current.lineHeight(), height)) {
            continue;
        }
        QTextBlockFormat format = current;
        format.setLineHeight(height, type);
        QTextCursor cursor(block);
        // Joined so that undo takes the spacing back with the edit that provoked it.
        cursor.joinPreviousEditBlock();
        cursor.setBlockFormat(format);
        cursor.endEditBlock();
    }
    m_spacing = false;
}
