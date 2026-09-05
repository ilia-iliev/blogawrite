#include "renderedhighlighter.h"

#include "blogawrite/src/style.cxx.h"

#include <QtCore/QList>
#include <QtGui/QTextBlock>
#include <QtGui/QTextCursor>
#include <QtGui/QTextFragment>

namespace {

/// A line of a fenced or indented code block. Qt's markdown reader records the fence and
/// the language it names on the block itself.
bool isCodeLine(const QTextBlock &line)
{
    const QTextBlockFormat format = line.blockFormat();
    return format.hasProperty(QTextFormat::BlockCodeFence)
        || format.hasProperty(QTextFormat::BlockCodeLanguage);
}

/// A line of a rendered table. Qt's markdown reader lays one out as a real text table,
/// so a cell is a block inside a table frame.
bool isTableLine(const QTextBlock &line)
{
    return QTextCursor(line).currentTable() != nullptr;
}

// The bits are worked out in Rust for the block being edited and read off Qt's own
// rendering here; both highlighters draw them the same way, so both name them the same.
constexpr quint16 Code = quint16(blogawrite::StyleBit::Code);
constexpr quint16 Unchecked = quint16(blogawrite::StyleBit::Unchecked);

} // namespace

RenderedHighlighter::RenderedHighlighter(QObject *parent)
    : BlockHighlighter(parent)
{
}

void RenderedHighlighter::highlightBlock(const QString &text)
{
    if (text.isEmpty()) {
        return;
    }

    // Qt's markdown reader draws code in the system's fixed font at the system's own
    // size, which is not the one the editor draws code at. Put the theme's back, so a
    // block reads the same rendered as it does under the cursor.
    if (isCodeLine(currentBlock())) {
        setFormat(0, text.size(), codeFormat());
        return;
    }

    QList<quint16> mask(text.size(), 0);
    // A cell of a table is not prose, and neither is code or a link. The words in them
    // are a name, an address, a column of figures; the checker is not to mark them.
    const quint16 tabular = isTableLine(currentBlock()) ? Unchecked : quint16(0);
    const int base = currentBlock().position();
    for (QTextBlock::iterator part = currentBlock().begin(); !part.atEnd(); ++part) {
        const QTextFragment fragment = part.fragment();
        if (!fragment.isValid()) {
            continue;
        }
        const QTextCharFormat format = fragment.charFormat();
        quint16 bits = tabular;
        if (format.fontFixedPitch()) {
            bits |= Code | Unchecked;
        }
        if (format.isAnchor()) {
            bits |= Unchecked;
        }
        if (!bits) {
            continue;
        }
        const int from = qBound(0, fragment.position() - base, int(text.size()));
        const int to = qBound(from, from + fragment.length(), int(text.size()));
        for (int i = from; i < to; ++i) {
            mask[i] |= bits;
        }
    }
    markLints(mask);

    applyMask(mask);
}

QTextCharFormat RenderedHighlighter::formatFor(quint16 bits) const
{
    QTextCharFormat format;
    if (bits & Code) {
        format.merge(codeFormat());
    }
    return format;
}
